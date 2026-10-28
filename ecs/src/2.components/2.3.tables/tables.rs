// 2.3 A table
//
// Two halves meet. Part 1 built the entity index: handles, slots, packed rows,
// swap-remove. 2.1 and 2.2 built the component side: ids, a registry, columns
// of bytes. A table is the packed rows of 1.4 with their typed Vecs replaced
// by columns, one per component in the table's signature:
//
//   table {Position Velocity Health}
//     entities  [ 0:1 | 3:1 | 2:1 | ... ]     who owns each row
//     columns   Position [ ... ]  Velocity [ ... ]  Health [ ... ]
//     capacity                                shared by every column
//
// Row r is entity entities[r], and its components are row r of every column.
// Adding a row grows every column together, and swap-remove copies the last
// row over the hole in every column together, so a row stays one entity.
//
// This world still has a single table, so every entity has the same
// components. 2.4 gives each combination its own.

// Components are declared here and not all read, and each chapter carries
// kayan code that not every program calls. Dead-code warnings would be noise.
#![allow(dead_code)]
// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

use std::alloc::Layout;
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

#[derive(Clone, Copy)]
struct Slot {
    generation: NonZeroU32,
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

// The world: the registry (2.1), the slots and free list (Part 1), and for now
// a single table.
struct World {
    infos: [Option<&'static Info>; 64],
    component_count: u32,
    slots: Vec<Slot>,
    free: Vec<u32>,
    live: usize,
    table: Table,
}

impl World {
    fn new() -> World {
        let infos = [None; 64];
        World { infos, component_count: 0, slots: Vec::new(), free: Vec::new(), live: 0, table: Table::new(&infos, 0) }
    }

    fn register<T: Component>(&mut self) {
        assert!(T::ID < 64, "component id {} is past the 64 a signature can hold", T::ID);
        self.infos[T::ID as usize] = Some(T::INFO);
        self.component_count = self.component_count.max(T::ID + 1);
    }

    fn alive(&self, e: Entity) -> bool {
        self.slots.get(e.index as usize).is_some_and(|s| s.generation == e.generation)
    }

    // Part 1's spawn, with the row coming from the table.
    fn spawn(&mut self) -> Entity {
        let index = match self.free.pop() {
            Some(index) => index,
            None => {
                self.slots.push(Slot { generation: ONE, row: 0 });
                self.slots.len() as u32 - 1
            }
        };
        let e = Entity { index, generation: self.slots[index as usize].generation };
        self.slots[index as usize].row = self.table.add_row(e) as u32;
        self.live += 1;
        e
    }

    fn despawn(&mut self, e: Entity) -> Result<(), Error> {
        if !self.alive(e) {
            return Err(Error::Dead);
        }
        let row = self.slots[e.index as usize].row as usize;
        self.table.remove_row(row, &mut self.slots);
        let slot = &mut self.slots[e.index as usize];
        slot.generation = slot.generation.checked_add(1).expect("a number has lived 4 billion lives");
        self.free.push(e.index);
        self.live -= 1;
        Ok(())
    }

    // The row a live entity is in. Typed get and set arrive in 2.5; until
    // then, game code finds the row and indexes the table's columns.
    fn row(&self, e: Entity) -> usize {
        self.slots[e.index as usize].row as usize
    }

    // 1.4's invariant, unchanged: every row belongs to a live entity whose slot
    // points back at it, and the rows add up to the live count.
    fn verify(&self) -> usize {
        let mut problems = 0;
        for (row, &e) in self.table.entities.iter().enumerate() {
            if !self.alive(e) {
                println!("verify: row {row} holds dead entity {e}");
                problems += 1;
            } else if self.row(e) != row {
                println!("verify: {e} is in row {row}, its slot says row {}", self.row(e));
                problems += 1;
            }
        }
        if self.table.count() != self.live {
            println!("verify: {} rows, {} live entities", self.table.count(), self.live);
            problems += 1;
        }
        problems
    }
}

// ---------------------------------------------------------------------------
// The program
// ---------------------------------------------------------------------------

fn print_table(w: &World) {
    let t = &w.table;
    println!("    {} rows, room for {}", t.count(), t.capacity);
    for (row, ((e, p), h)) in t.entities.iter().zip(t.slice::<Position>()).zip(t.slice::<Health>()).enumerate() {
        println!("    row {row}  {e}  position ({:5.2}, {:5.2})  health {:3.0}", p.at.x, p.at.y, h.points);
    }
}

fn main() {
    let mut w = World::new();
    w.register::<Position>();
    w.register::<Velocity>();
    w.register::<Health>();

    // One table, so one kind of entity: every entity is a drone.
    w.table = Table::new(&w.infos, mask_of::<Position>() | mask_of::<Velocity>() | mask_of::<Health>());

    let mut drones = Vec::new();
    for i in 0..6 {
        let e = w.spawn();
        // Write the new row through the typed views of its columns.
        let row = w.row(e);
        w.table.slice_mut::<Position>()[row] = Position { at: Vec2 { x: 10.0 * i as f32, y: 0.0 } };
        w.table.slice_mut::<Velocity>()[row] = Velocity { per_second: Vec2 { x: 0.0, y: 1.0 } };
        w.table.slice_mut::<Health>()[row] = Health { points: 100.0, max: 100.0 };
        drones.push(e);
    }
    println!("spawned 6 drones into one table; it grew from 4 rows to 8");
    print_table(&w);

    println!("despawn {} and {}", drones[0], drones[3]);
    w.despawn(drones[0]).expect("alive");
    w.despawn(drones[3]).expect("alive");
    print_table(&w);

    let again = w.spawn();
    let row = w.row(again);
    w.table.slice_mut::<Health>()[row] = Health { points: 40.0, max: 100.0 };
    println!("spawn {again}: a reused number, the next row, zeroed except what we set");
    print_table(&w);

    println!("verify: {} problems", w.verify());
}
