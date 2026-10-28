// 2.4 Many tables: archetypes
//
// 2.3's world had one table, so every entity had the same components. Real
// games mix: a turret has a position and a turret component, a drone a
// position, velocity and health, a bullet a position, velocity, damage and a
// lifetime. Each combination gets a table of its own, and every row of a table
// has every component in its signature, so there are no holes and no flags.
// A table for one combination is called an archetype.
//
//   table 0 {Position Turret}                   0:1
//   table 1 {Position Velocity Health}          1:1 2:1 3:1
//   table 2 {Position Velocity Damage Lifetime} 4:1 5:1
//
// Nobody declares the archetypes. The first time something spawns with a new
// combination, the world makes its table, and remembers it by signature so the
// next spawn finds it. A slot now says which table an entity is in as well as
// which row.

// Components are declared here and not all read, and each chapter carries
// kayan code that not every program calls. Dead-code warnings would be noise.
#![allow(dead_code)]
// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

use std::alloc::Layout;
use std::collections::HashMap;
use std::fmt;
use std::num::NonZeroU32;

// ---------------------------------------------------------------------------
// Components (2.1, 2.2)
// ---------------------------------------------------------------------------

/// Plain old data: what lets a column hold a component as bytes, zero a row,
/// copy a row with a byte copy, and hand the bytes back as the type.
///
/// # Safety
///
/// The type is Copy, has no padding bytes, and every bit pattern is a valid
/// value of it. That is a promise the compiler cannot check by itself, so the
/// trait is unsafe; pod! makes the promise only after checking it at compile
/// time.
#[allow(unsafe_code)]
unsafe trait Pod: Copy + 'static {}

#[allow(unsafe_code)]
unsafe impl Pod for f32 {}
#[allow(unsafe_code)]
unsafe impl Pod for u32 {}
#[allow(unsafe_code)]
unsafe impl Pod for i32 {}

macro_rules! pod {
    ($name:ident { $($field:ident: $ty:ty),* $(,)? }) => {
        #[derive(Clone, Copy, Default, Debug)]
        #[repr(C)]
        struct $name {
            $($field: $ty),*
        }

        #[allow(unsafe_code)]
        unsafe impl Pod for $name {}

        const _: () = {
            const fn field_is_pod<T: Pod>() {}
            $(field_is_pod::<$ty>();)*
            assert!(size_of::<$name>() == 0 $(+ size_of::<$ty>())*, "a component has padding");
        };
    };
}

// A reflect record: what the world knows about a component it cannot name.
struct Info {
    name: &'static str,
    layout: Layout,
}

// A component is plain data, with an id and a reflect record.
trait Component: Pod {
    const ID: u32;
    const INFO: &'static Info;
}

#[derive(Clone, Copy)]
enum ComponentId {
    Position,
    Velocity,
    Health,
    Damage,
    Lifetime,
    Turret,
}

macro_rules! component {
    ($name:ident { $($field:ident: $ty:ty),* $(,)? }) => {
        pod!($name { $($field: $ty),* });

        impl Component for $name {
            const ID: u32 = ComponentId::$name as u32;
            const INFO: &'static Info = &Info { name: stringify!($name), layout: Layout::new::<$name>() };
        }
    };
}

pod!(Vec2 { x: f32, y: f32 });
component!(Position { at: Vec2 });
component!(Velocity { per_second: Vec2 });
component!(Health { points: f32, max: f32 });
component!(Damage { amount: f32 });
component!(Lifetime { seconds: f32 });
component!(Turret { cooldown: f32, timer: f32 });

// ---------------------------------------------------------------------------
// Entities (Part 1)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
struct Entity {
    index: u32,
    generation: NonZeroU32,
}

impl fmt::Display for Entity {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:{}", self.index, self.generation)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Error {
    Dead,
}

const ONE: NonZeroU32 = NonZeroU32::MIN;

// Where an entity lives: which table, and which row of it.
#[derive(Clone, Copy)]
struct Slot {
    generation: NonZeroU32,
    table: u32,
    row: u32,
}

// ---------------------------------------------------------------------------
// Columns (2.2)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Default)]
#[repr(C, align(16))]
struct Block([u8; 16]);

struct Column {
    blocks: Vec<Block>,
    size: usize,
    id: u32,
    info: &'static Info,
}

impl Column {
    fn new(id: u32, info: &'static Info) -> Column {
        assert!(info.layout.align() <= 16, "{} needs more alignment than a column has", info.name);
        Column { blocks: Vec::new(), size: info.layout.size(), id, info }
    }

    #[allow(unsafe_code)]
    fn bytes(&self) -> &[u8] {
        // SAFETY: a Block is 16 initialised bytes with no padding, so the Vec's
        // buffer is `len * 16` initialised bytes.
        unsafe { std::slice::from_raw_parts(self.blocks.as_ptr().cast::<u8>(), self.blocks.len() * 16) }
    }

    #[allow(unsafe_code)]
    fn bytes_mut(&mut self) -> &mut [u8] {
        // SAFETY: as in bytes(); `&mut self` makes the view the only one.
        unsafe { std::slice::from_raw_parts_mut(self.blocks.as_mut_ptr().cast::<u8>(), self.blocks.len() * 16) }
    }

    fn row(&self, row: usize) -> &[u8] {
        &self.bytes()[row * self.size..(row + 1) * self.size]
    }

    fn grow(&mut self, capacity: usize) {
        self.blocks.resize((capacity * self.size).div_ceil(16), Block::default());
    }

    fn copy_row(&mut self, to: usize, from: usize) {
        let size = self.size;
        self.bytes_mut().copy_within(from * size..(from + 1) * size, to * size);
    }

    fn zero_row(&mut self, row: usize) {
        let size = self.size;
        self.bytes_mut()[row * size..(row + 1) * size].fill(0);
    }

    #[allow(unsafe_code)]
    fn slice<T: Component>(&self, count: usize) -> &[T] {
        assert!(self.id == T::ID, "a {} column read as {}", self.info.name, T::INFO.name);
        let bytes = &self.bytes()[..count * size_of::<T>()];
        // SAFETY: T is the column's component, so the bytes are `count` T's;
        // the storage is aligned to 16 and T needs at most 16; T is Pod, so
        // any bytes are a valid T.
        unsafe { std::slice::from_raw_parts(bytes.as_ptr().cast::<T>(), count) }
    }

    #[allow(unsafe_code)]
    fn slice_mut<T: Component>(&mut self, count: usize) -> &mut [T] {
        assert!(self.id == T::ID, "a {} column read as {}", self.info.name, T::INFO.name);
        let bytes = &mut self.bytes_mut()[..count * size_of::<T>()];
        // SAFETY: as in slice(). T has no padding, so writing a T leaves every
        // byte initialised, and the byte views stay valid.
        unsafe { std::slice::from_raw_parts_mut(bytes.as_mut_ptr().cast::<T>(), count) }
    }
}

// ---------------------------------------------------------------------------
// The table
// ---------------------------------------------------------------------------

// One bit per component. A signature is a u64, so a world has at most 64.
fn bit(id: u32) -> u64 {
    1 << id
}

fn mask_of<T: Component>() -> u64 {
    bit(T::ID)
}

// `columns[id]` is Some when the signature has bit `id`. Indexing by id means
// finding a column is an index, never a search. The capacity is the table's,
// shared by its columns, and the row count is the length of `entities`, so
// neither can disagree with the columns.
struct Table {
    signature: u64,
    capacity: usize,
    entities: Vec<Entity>,
    columns: [Option<Column>; 64],
}

impl Table {
    // A table for a signature: one empty column per component in it, made from
    // the registry. Every component in the signature must be registered.
    fn new(infos: &[Option<&'static Info>; 64], signature: u64) -> Table {
        let mut columns = [const { None }; 64];
        for id in 0..64 {
            if signature & bit(id) != 0 {
                let info = infos[id as usize]
                    .unwrap_or_else(|| panic!("signature {signature} uses component {id}, which was never registered"));
                columns[id as usize] = Some(Column::new(id, info));
            }
        }
        Table { signature, capacity: 0, entities: Vec::new(), columns }
    }

    fn count(&self) -> usize {
        self.entities.len()
    }

    fn has(&self, id: u32) -> bool {
        self.signature & bit(id) != 0
    }

    // Double the room in `entities` and in every column at once.
    fn grow(&mut self) {
        self.capacity = if self.capacity == 0 { 4 } else { self.capacity * 2 };
        self.entities.reserve_exact(self.capacity - self.entities.len());
        for c in self.columns.iter_mut().flatten() {
            c.grow(self.capacity);
        }
    }

    // A new row at the end, owned by `e`, with every component zeroed.
    fn add_row(&mut self, e: Entity) -> usize {
        if self.count() == self.capacity {
            self.grow();
        }
        let row = self.count();
        self.entities.push(e);
        for c in self.columns.iter_mut().flatten() {
            c.zero_row(row);
        }
        row
    }

    // 1.4's swap-remove, for every column: the last row is copied over `row`,
    // and the slot of the entity that owned the last row is told where it
    // went. The slots are the world's, so the world passes them in: the table
    // and the world's slots are two separate borrows.
    fn remove_row(&mut self, row: usize, slots: &mut [Slot]) {
        let last = self.count() - 1;
        if row != last {
            for c in self.columns.iter_mut().flatten() {
                c.copy_row(row, last);
            }
        }
        self.entities.swap_remove(row);
        if let Some(moved) = self.entities.get(row) {
            slots[moved.index as usize].row = row as u32;
        }
    }

    fn column<T: Component>(&self) -> &Column {
        self.columns[T::ID as usize]
            .as_ref()
            .unwrap_or_else(|| panic!("the table has no {} column", T::INFO.name))
    }

    // The typed view of one of the table's columns: T's rows, all of them in
    // use. Asking for a component the table does not have is a bug, so it
    // stops.
    fn slice<T: Component>(&self) -> &[T] {
        self.column::<T>().slice::<T>(self.count())
    }

    fn slice_mut<T: Component>(&mut self) -> &mut [T] {
        let count = self.count();
        self.columns[T::ID as usize]
            .as_mut()
            .unwrap_or_else(|| panic!("the table has no {} column", T::INFO.name))
            .slice_mut::<T>(count)
    }
}

// ---------------------------------------------------------------------------
// The world
// ---------------------------------------------------------------------------

// The world: the registry, the slots and free list, and every table, with a
// map from signature to table index so finding one is a lookup, not a search.
// Tables are referred to by index, never by reference: making a table can grow
// `tables` and move all of them, and an index survives that.
struct World {
    infos: [Option<&'static Info>; 64],
    component_count: u32,
    tables: Vec<Table>,
    by_signature: HashMap<u64, u32>,
    slots: Vec<Slot>,
    free: Vec<u32>,
    live: usize,
}

impl World {
    fn new() -> World {
        World {
            infos: [None; 64],
            component_count: 0,
            tables: Vec::new(),
            by_signature: HashMap::new(),
            slots: Vec::new(),
            free: Vec::new(),
            live: 0,
        }
    }

    fn register<T: Component>(&mut self) {
        assert!(T::ID < 64, "component id {} is past the 64 a signature can hold", T::ID);
        self.infos[T::ID as usize] = Some(T::INFO);
        self.component_count = self.component_count.max(T::ID + 1);
    }

    fn alive(&self, e: Entity) -> bool {
        self.slots.get(e.index as usize).is_some_and(|s| s.generation == e.generation)
    }

    // The table for a signature: found if it exists, made if it does not. This
    // is where archetypes come from.
    fn table_for(&mut self, signature: u64) -> u32 {
        if let Some(&index) = self.by_signature.get(&signature) {
            return index;
        }
        let index = self.tables.len() as u32;
        self.tables.push(Table::new(&self.infos, signature));
        self.by_signature.insert(signature, index);
        index
    }

    // A new entity with the components in `signature`, all zeroed, in the
    // table for that signature.
    fn spawn(&mut self, signature: u64) -> Entity {
        let index = match self.free.pop() {
            Some(index) => index,
            None => {
                self.slots.push(Slot { generation: ONE, table: 0, row: 0 });
                self.slots.len() as u32 - 1
            }
        };
        let e = Entity { index, generation: self.slots[index as usize].generation };
        // table_for can make a table, and pushing onto `tables` can move every
        // table. A reference to a table cannot live across that call (the
        // borrow checker refuses), so the index comes first and the table after.
        let table = self.table_for(signature);
        let row = self.tables[table as usize].add_row(e);
        self.slots[index as usize] = Slot { generation: e.generation, table, row: row as u32 };
        self.live += 1;
        e
    }

    fn despawn(&mut self, e: Entity) -> Result<(), Error> {
        if !self.alive(e) {
            return Err(Error::Dead);
        }
        let Slot { table, row, .. } = self.slots[e.index as usize];
        self.tables[table as usize].remove_row(row as usize, &mut self.slots);
        let slot = &mut self.slots[e.index as usize];
        slot.generation = slot.generation.checked_add(1).expect("a number has lived 4 billion lives");
        self.free.push(e.index);
        self.live -= 1;
        Ok(())
    }

    fn table_of(&self, e: Entity) -> &Table {
        &self.tables[self.slots[e.index as usize].table as usize]
    }

    fn table_of_mut(&mut self, e: Entity) -> &mut Table {
        &mut self.tables[self.slots[e.index as usize].table as usize]
    }

    fn row(&self, e: Entity) -> usize {
        self.slots[e.index as usize].row as usize
    }

    // The invariant, over every table: each row belongs to a live entity whose
    // slot points back at that table and row, and the rows of all tables add
    // up to the live count.
    fn verify(&self) -> usize {
        let mut problems = 0;
        let mut rows = 0;
        for (ti, t) in self.tables.iter().enumerate() {
            for (row, &e) in t.entities.iter().enumerate() {
                if !self.alive(e) {
                    println!("verify: table {ti} row {row} holds dead entity {e}");
                    problems += 1;
                    continue;
                }
                let s = self.slots[e.index as usize];
                if s.table as usize != ti || s.row as usize != row {
                    println!("verify: {e} is in table {ti} row {row}, its slot says table {} row {}", s.table, s.row);
                    problems += 1;
                }
            }
            rows += t.count();
        }
        if rows != self.live {
            println!("verify: {rows} rows in tables, {} live entities", self.live);
            problems += 1;
        }
        problems
    }

    fn info(&self, id: u32) -> &'static Info {
        self.infos[id as usize].expect("every component in a signature is registered")
    }

    // A signature as names, {Position Velocity}.
    fn names(&self, signature: u64) -> String {
        let names: Vec<&str> = (0..self.component_count)
            .filter(|&id| signature & bit(id) != 0)
            .map(|id| self.info(id).name)
            .collect();
        format!("{{{}}}", names.join(" "))
    }

    fn print_tables(&self) {
        for (ti, t) in self.tables.iter().enumerate() {
            print!("    table {ti} {}:", self.names(t.signature));
            for e in &t.entities {
                print!(" {e}");
            }
            println!();
        }
    }
}

// ---------------------------------------------------------------------------
// The program
// ---------------------------------------------------------------------------

// Until 2.5's typed get, writing a component is: find the entity's table and
// row, take the typed view of the column, write the row.
fn place(w: &mut World, e: Entity, x: f32, y: f32) {
    let row = w.row(e);
    w.table_of_mut(e).slice_mut::<Position>()[row] = Position { at: Vec2 { x, y } };
}

fn main() {
    let mut w = World::new();
    w.register::<Position>();
    w.register::<Velocity>();
    w.register::<Health>();
    w.register::<Damage>();
    w.register::<Lifetime>();
    w.register::<Turret>();

    // What each kind of thing is, as a set of components.
    let gun = mask_of::<Position>() | mask_of::<Turret>();
    let drone = mask_of::<Position>() | mask_of::<Velocity>() | mask_of::<Health>();
    let bullet = mask_of::<Position>() | mask_of::<Velocity>() | mask_of::<Damage>() | mask_of::<Lifetime>();

    let t0 = w.spawn(gun);
    place(&mut w, t0, 0.0, 0.0);
    println!("spawned the turret: the world made table {} for it", w.tables.len() - 1);

    let mut drones = Vec::new();
    for i in 0..3 {
        let d = w.spawn(drone);
        place(&mut w, d, 10.0 * (i + 1) as f32, 5.0);
        drones.push(d);
    }
    println!("spawned 3 drones: the first made a table, the others found it");

    let b0 = w.spawn(bullet);
    let b1 = w.spawn(bullet);
    place(&mut w, b0, 0.0, 1.0);
    place(&mut w, b1, 1.0, 0.0);
    println!("spawned 2 bullets");
    println!("{} entities in {} tables:", w.live, w.tables.len());
    w.print_tables();

    println!("despawn drone {}: a swap-remove inside its own table", drones[0]);
    w.despawn(drones[0]).expect("alive");
    w.print_tables();

    // The positions of everything, table by table: the same column, in each
    // table that has it.
    println!("every position, table by table:");
    for (ti, t) in w.tables.iter().enumerate() {
        for (row, (e, p)) in t.entities.iter().zip(t.slice::<Position>()).enumerate() {
            println!("    table {ti} row {row}  {e} at ({:5.2}, {:5.2})", p.at.x, p.at.y);
        }
    }
    println!("verify: {} problems", w.verify());
}
