// 2.6 Adding and removing components
//
// A drone catches fire: it gains a `Burning` component. The fire goes out: it
// loses it. In an archetype ECS a table holds exactly one combination, so a
// drone that gains a component no longer belongs in its table. It moves:
//
//   1. find the table for its signature plus (or minus) the component;
//   2. add a row there, and copy every component the two tables share;
//   3. swap-remove its old row, fixing whoever moved into the hole;
//   4. point its slot at the new table and row.
//
// Its handle does not change. Everything else about it is where it was, one
// table over.
//
// Step 1 is a hashmap lookup by signature, and a game toggling components
// does it constantly, always between the same few tables. So each table
// remembers its neighbours: "add Burning: table 1", "remove Burning: table 0".
// These edges make the tables a graph, the archetype graph, and after the
// first move between two tables every later one follows an edge instead of
// looking anything up.

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
    Burning,
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
component!(Burning { per_second: f32 });

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

// Why an operation did nothing. `Dead`: the handle no longer names a live
// entity. `Missing`: the entity is alive but has no such component.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Error {
    Dead,
    Missing,
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

    fn row_mut(&mut self, row: usize) -> &mut [u8] {
        let size = self.size;
        &mut self.bytes_mut()[row * size..(row + 1) * size]
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
//
// The edges: `add_edges[id]` is the table this one's entities move to when
// they gain component `id`, and `remove_edges[id]` when they lose it. None
// means "not known yet".
struct Table {
    signature: u64,
    capacity: usize,
    entities: Vec<Entity>,
    columns: [Option<Column>; 64],
    add_edges: [Option<u32>; 64],
    remove_edges: [Option<u32>; 64],
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
        Table { signature, capacity: 0, entities: Vec::new(), columns, add_edges: [None; 64], remove_edges: [None; 64] }
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
    // How table moves found their destination, for this chapter's output.
    lookups: u32,
    edge_hits: u32,
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
            lookups: 0,
            edge_hits: 0,
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

    // The components a live entity has: its table's signature. 0 for a dead
    // one.
    fn signature(&self, e: Entity) -> u64 {
        if !self.alive(e) {
            return 0;
        }
        self.table_of(e).signature
    }

    // e's T, or None if e is dead or has no T. Handle, slot, table, row, typed
    // view: the same path as 1.4's get_position, for any component.
    fn get<T: Component>(&self, e: Entity) -> Option<&T> {
        if !self.alive(e) {
            return None;
        }
        let t = self.table_of(e);
        if !t.has(T::ID) {
            return None;
        }
        Some(&t.slice::<T>()[self.row(e)])
    }

    fn get_mut<T: Component>(&mut self, e: Entity) -> Option<&mut T> {
        if !self.alive(e) {
            return None;
        }
        let row = self.row(e);
        let t = self.table_of_mut(e);
        if !t.has(T::ID) {
            return None;
        }
        Some(&mut t.slice_mut::<T>()[row])
    }

    fn has<T: Component>(&self, e: Entity) -> bool {
        self.signature(e) & mask_of::<T>() != 0
    }

    // Write e's T. It must already have one: giving an entity a component it
    // does not have changes its table, which is 2.6.
    fn set<T: Component>(&mut self, e: Entity, value: T) -> Result<(), Error> {
        if !self.alive(e) {
            return Err(Error::Dead);
        }
        *self.get_mut::<T>(e).ok_or(Error::Missing)? = value;
        Ok(())
    }

    // The table an entity of table `from` moves to when it gains component
    // `id`. The first time, it is found by signature, which can make the
    // table; the answer is written on both tables, as from's add edge and the
    // destination's remove edge, so neither direction is looked up again.
    fn table_with(&mut self, from: u32, id: u32) -> u32 {
        if let Some(to) = self.tables[from as usize].add_edges[id as usize] {
            self.edge_hits += 1;
            return to;
        }
        self.lookups += 1;
        let to = self.table_for(self.tables[from as usize].signature | bit(id));
        self.tables[from as usize].add_edges[id as usize] = Some(to);
        self.tables[to as usize].remove_edges[id as usize] = Some(from);
        to
    }

    // The same for losing component `id`.
    fn table_without(&mut self, from: u32, id: u32) -> u32 {
        if let Some(to) = self.tables[from as usize].remove_edges[id as usize] {
            self.edge_hits += 1;
            return to;
        }
        self.lookups += 1;
        let to = self.table_for(self.tables[from as usize].signature & !bit(id));
        self.tables[from as usize].remove_edges[id as usize] = Some(to);
        self.tables[to as usize].add_edges[id as usize] = Some(from);
        to
    }

    // Move a live entity to table `to`. Components both tables have are
    // copied, byte for byte; components only the new table has start zeroed;
    // components only the old table has are dropped with its old row.
    // get_disjoint_mut lends out two tables of the Vec at once, after
    // checking at run time that they are two different tables.
    fn move_to(&mut self, e: Entity, to: u32) {
        let Slot { table: from, row, .. } = self.slots[e.index as usize];
        if from == to {
            return;
        }
        let [old, new] = self.tables.get_disjoint_mut([from as usize, to as usize]).expect("two different tables");
        let new_row = new.add_row(e);
        for (o, n) in old.columns.iter().zip(new.columns.iter_mut()) {
            if let (Some(o), Some(n)) = (o, n) {
                n.row_mut(new_row).copy_from_slice(o.row(row as usize));
            }
        }
        old.remove_row(row as usize, &mut self.slots);
        self.slots[e.index as usize] = Slot { generation: e.generation, table: to, row: new_row as u32 };
    }

    // Give e a T. If it already has one, this is set; if not, it moves to the
    // table with T and then sets it.
    fn add<T: Component>(&mut self, e: Entity, value: T) -> Result<(), Error> {
        if !self.alive(e) {
            return Err(Error::Dead);
        }
        if !self.has::<T>(e) {
            let from = self.slots[e.index as usize].table;
            let to = self.table_with(from, T::ID);
            self.move_to(e, to);
        }
        self.set(e, value)
    }

    // Take e's T away, moving it to the table without T.
    fn remove<T: Component>(&mut self, e: Entity) -> Result<(), Error> {
        if !self.alive(e) {
            return Err(Error::Dead);
        }
        if !self.has::<T>(e) {
            return Err(Error::Missing);
        }
        let from = self.slots[e.index as usize].table;
        let to = self.table_without(from, T::ID);
        self.move_to(e, to);
        Ok(())
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

// Game code builds things with functions like these: spawn with a signature,
// set each component. Each set is expected to succeed, because the entity was
// just spawned with exactly these components; expect says so, and stops the
// program if that is ever wrong.
fn spawn_drone(w: &mut World, x: f32, y: f32) -> Entity {
    let e = w.spawn(mask_of::<Position>() | mask_of::<Velocity>() | mask_of::<Health>());
    w.set(e, Position { at: Vec2 { x, y } }).expect("just spawned");
    w.set(e, Velocity { per_second: Vec2 { x: -1.0, y: 0.0 } }).expect("just spawned");
    w.set(e, Health { points: 100.0, max: 100.0 }).expect("just spawned");
    e
}

fn spawn_bullet(w: &mut World, x: f32, y: f32) -> Entity {
    let e = w.spawn(mask_of::<Position>() | mask_of::<Velocity>() | mask_of::<Damage>() | mask_of::<Lifetime>());
    w.set(e, Position { at: Vec2 { x, y } }).expect("just spawned");
    w.set(e, Velocity { per_second: Vec2 { x: 8.0, y: 0.0 } }).expect("just spawned");
    w.set(e, Damage { amount: 25.0 }).expect("just spawned");
    w.set(e, Lifetime { seconds: 2.0 }).expect("just spawned");
    e
}

fn print_drone(w: &World, e: Entity) {
    match (w.get::<Position>(e), w.get::<Health>(e)) {
        (Some(p), Some(h)) => println!("    {e} at ({:5.2}, {:5.2}), health {:.0}", p.at.x, p.at.y, h.points),
        _ => println!("    {e} is not a live drone"),
    }
}

// "ok", or why not: how the programs print a Result.
fn status(r: Result<(), Error>) -> &'static str {
    match r {
        Ok(()) => "ok",
        Err(Error::Dead) => "dead",
        Err(Error::Missing) => "missing",
    }
}

fn ignite(w: &mut World, e: Entity) {
    println!("    {e} catches fire: {}", status(w.add(e, Burning { per_second: 10.0 })));
}

fn put_out(w: &mut World, e: Entity) {
    println!("    {e} is put out: {}", status(w.remove::<Burning>(e)));
}

fn main() {
    let mut w = World::new();
    w.register::<Position>();
    w.register::<Velocity>();
    w.register::<Health>();
    w.register::<Damage>();
    w.register::<Lifetime>();
    w.register::<Turret>();
    w.register::<Burning>();

    let d: Vec<Entity> = (0..4).map(|i| spawn_drone(&mut w, 10.0 * (i + 1) as f32, 0.0)).collect();
    println!("4 drones:");
    w.print_tables();

    println!("frame 1");
    ignite(&mut w, d[0]);
    ignite(&mut w, d[2]);
    w.print_tables();

    println!("frame 2");
    put_out(&mut w, d[0]);
    ignite(&mut w, d[1]);
    ignite(&mut w, d[2]);
    w.print_tables();

    println!("frame 3");
    put_out(&mut w, d[1]);
    put_out(&mut w, d[2]);
    put_out(&mut w, d[3]);
    ignite(&mut w, d[3]);
    w.print_tables();

    let p = w.get::<Position>(d[2]).expect("alive");
    let h = w.get::<Health>(d[2]).expect("alive");
    println!("{} moved tables twice and kept its data: at ({:.2}, {:.2}), health {:.0}", d[2], p.at.x, p.at.y, h.points);
    println!("moves that looked up a table by signature: {}; that followed an edge: {}", w.lookups, w.edge_hits);
    println!("verify: {} problems", w.verify());
}
