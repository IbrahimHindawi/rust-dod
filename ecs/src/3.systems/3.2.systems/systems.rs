// 3.2 Systems
//
// 3.1's movement loop had the shape of every system: bring a query up to
// date, then for each of its tables take the columns as slices and loop over
// the rows. This chapter names that shape and splits it in two:
//
//   the kernel   a plain loop over plain slices, like lesson 1's move_soa. It
//                knows nothing about tables, entities or the world.
//   the system   a query, and a small function kayan calls on each table the
//                query matches. It takes the table's columns and calls the
//                kernel.
//
// A frame runs the systems in order, each over all of its tables. The order is
// the schedule, and it is written down once, as data.

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

// A reflect record: a type's name, size and alignment, and its fields. One
// field's record says where its bytes are in the struct, and what they are: a
// struct with a record of its own, or a builtin, which tools read by its
// type's name. `hidden` marks bookkeeping a tool need not show.
struct Info {
    name: &'static str,
    layout: Layout,
    fields: &'static [Field],
}

struct Field {
    name: &'static str,
    offset: usize,
    size: usize,
    ty: &'static str,
    info: Option<&'static Info>,
    hidden: bool,
}

// Every type a component is made of has a record, or None for a builtin.
trait Reflect {
    const RECORD: Option<&'static Info>;
}

impl Reflect for f32 {
    const RECORD: Option<&'static Info> = None;
}
impl Reflect for u32 {
    const RECORD: Option<&'static Info> = None;
}
impl Reflect for i32 {
    const RECORD: Option<&'static Info> = None;
}

macro_rules! is_hidden {
    () => {
        false
    };
    (hidden) => {
        true
    };
}

// Declares a plain-data struct and its reflect record. Two checks run at
// compile time: every field is Pod, and the struct is exactly as big as its
// fields, so it has no padding. A field written `#[hidden] timer: f32` is
// recorded as hidden.
macro_rules! pod {
    ($name:ident { $($(#[$attr:ident])? $field:ident: $ty:ty),* $(,)? }) => {
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

        impl Reflect for $name {
            const RECORD: Option<&'static Info> = Some(&Info {
                name: stringify!($name),
                layout: Layout::new::<$name>(),
                fields: &[$(Field {
                    name: stringify!($field),
                    offset: std::mem::offset_of!($name, $field),
                    size: size_of::<$ty>(),
                    ty: stringify!($ty),
                    info: <$ty as Reflect>::RECORD,
                    hidden: is_hidden!($($attr)?),
                }),*],
            });
        }
    };
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
    ($name:ident { $($(#[$attr:ident])? $field:ident: $ty:ty),* $(,)? }) => {
        pod!($name { $($(#[$attr])? $field: $ty),* });

        impl Component for $name {
            const ID: u32 = ComponentId::$name as u32;
            const INFO: &'static Info = <$name as Reflect>::RECORD.unwrap();
        }
    };
}

pod!(Vec2 { x: f32, y: f32 });
component!(Position { at: Vec2 });
component!(Velocity { per_second: Vec2 });
component!(Health { points: f32, max: f32 });
component!(Damage { amount: f32 });
component!(Lifetime { seconds: f32 });
// A field tagged hidden is bookkeeping the inspector does not show.
component!(Turret { cooldown: f32, #[hidden] timer: f32 });
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
    // Two of the table's columns at once, as slices. Both live in `columns`,
    // so the borrow checker must be shown they are two different columns:
    // asking for the same component twice is a compile error, and
    // get_disjoint_mut checks the two indices at run time as well. Take `&`
    // of either one to only read it.
    fn pair_mut<A: Component, B: Component>(&mut self) -> (&mut [A], &mut [B]) {
        const { assert!(A::ID != B::ID, "pair_mut needs two different components") };
        let count = self.count();
        let [a, b] = self.columns.get_disjoint_mut([A::ID as usize, B::ID as usize]).expect("two different columns");
        let a = a.as_mut().unwrap_or_else(|| panic!("the table has no {} column", A::INFO.name));
        let b = b.as_mut().unwrap_or_else(|| panic!("the table has no {} column", B::INFO.name));
        (a.slice_mut::<A>(count), b.slice_mut::<B>(count))
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
            return to;
        }
        let to = self.table_for(self.tables[from as usize].signature | bit(id));
        self.tables[from as usize].add_edges[id as usize] = Some(to);
        self.tables[to as usize].remove_edges[id as usize] = Some(from);
        to
    }

    // The same for losing component `id`.
    fn table_without(&mut self, from: u32, id: u32) -> u32 {
        if let Some(to) = self.tables[from as usize].remove_edges[id as usize] {
            return to;
        }
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

    // e's component `id` as its bytes: None if e is dead or has no such
    // component. The typed layer is for game code, which knows its types; this
    // is for tools, which work from reflect records instead.
    fn get_bytes(&self, e: Entity, id: u32) -> Option<&[u8]> {
        if !self.alive(e) || id >= self.component_count {
            return None;
        }
        let row = self.row(e);
        self.table_of(e).columns[id as usize].as_ref().map(|c| c.row(row))
    }

    fn inspect(&self, e: Entity) {
        if !self.alive(e) {
            println!("{e} (dead)");
            return;
        }
        let signature = self.signature(e);
        println!("{e} {}", self.names(signature));
        for id in (0..self.component_count).filter(|&id| signature & bit(id) != 0) {
            let info = self.info(id);
            println!("    {}", info.name);
            print_fields(info, self.get_bytes(e, id).expect("e has it"), 2);
        }
    }

    // Every table: rows in use out of room, and per column its row size and
    // its bytes in use out of bytes allocated. The memory a table holds and
    // does not use is the cost of doubling on growth.
    fn print_layout(&self) {
        for (ti, t) in self.tables.iter().enumerate() {
            println!("table {ti} {}: {} rows, room for {}", self.names(t.signature), t.count(), t.capacity);
            for c in t.columns.iter().flatten() {
                println!("    {:<9} {:2} bytes a row, {:4} of {:4} bytes in use",
                    c.info.name, c.size, c.size * t.count(), c.blocks.len() * 16);
            }
        }
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

    fn print_table(&self, ti: usize) {
        let t = &self.tables[ti];
        print!("    table {ti} {}:", self.names(t.signature));
        for e in &t.entities {
            print!(" {e}");
        }
        println!();
    }

    fn print_tables(&self) {
        for ti in 0..self.tables.len() {
            self.print_table(ti);
        }
    }

    fn print_query(&self, name: &str, q: &mut Query) {
        let entities = q.count(&self.tables);
        println!("{name}: all {}, none {}; tables: {}, entities: {entities}",
            self.names(q.all), self.names(q.none), q.tables.len());
        for &ti in &q.tables {
            self.print_table(ti as usize);
        }
    }
}

// ---------------------------------------------------------------------------
// The inspector: reflect records, no component types
// ---------------------------------------------------------------------------

// One value of a builtin type, read from its bytes by its type's name.
// from_ne_bytes turns bytes into a number, so this needs no unsafe.
fn print_builtin(ty: &str, at: &[u8]) {
    match ty {
        "f32" => print!("{:.2}", f32::from_ne_bytes(at.try_into().expect("4 bytes"))),
        "u32" => print!("{}", u32::from_ne_bytes(at.try_into().expect("4 bytes"))),
        "i32" => print!("{}", i32::from_ne_bytes(at.try_into().expect("4 bytes"))),
        _ => print!("<{ty}>"),
    }
}

// Every field of a struct whose bytes are `base`, nested structs indented
// under their field. A field's bytes are cut from `base` by its offset and
// size, both from the reflect record.
fn print_fields(info: &Info, base: &[u8], depth: usize) {
    for f in info.fields.iter().filter(|f| !f.hidden) {
        let at = &base[f.offset..f.offset + f.size];
        print!("{:indent$}{}:", "", f.name, indent = depth * 4);
        match f.info {
            Some(inner) => {
                println!();
                print_fields(inner, at, depth + 1);
            }
            None => {
                print!(" ");
                print_builtin(f.ty, at);
                println!();
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Queries
// ---------------------------------------------------------------------------

// The tables a system runs over: every table whose signature has all of `all`
// and none of `none`, kept by index. Tables are only ever added, at the end of
// the world's `tables`, so the query remembers how many it has checked and,
// brought up to date, looks at the new ones only.
struct Query {
    all: u64,
    none: u64,
    tables: Vec<u32>,
    checked: usize,
}

fn matches(signature: u64, all: u64, none: u64) -> bool {
    (signature & all) == all && (signature & none) == 0
}

impl Query {
    fn new(all: u64, none: u64) -> Query {
        Query { all, none, tables: Vec::new(), checked: 0 }
    }

    // Check the tables made since the query last looked. With none new, this
    // is one comparison, so call it before every walk.
    fn update(&mut self, tables: &[Table]) {
        for (ti, t) in tables.iter().enumerate().skip(self.checked) {
            if matches(t.signature, self.all, self.none) {
                self.tables.push(ti as u32);
            }
        }
        self.checked = tables.len();
    }

    // How many entities the query matches: the rows of its tables.
    fn count(&mut self, tables: &[Table]) -> usize {
        self.update(tables);
        self.tables.iter().map(|&ti| tables[ti as usize].count()).sum()
    }
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

// What a system gets for each table it runs on: every table, which one to
// work on, and the frame's time step. One struct rather than three parameters,
// so that adding to it later changes no system's signature. Its fields are
// separate borrows, so a system can take `&mut it.tables[it.table]` and still
// read `it.dt`.
struct Iter<'a> {
    tables: &'a mut [Table],
    table: usize,
    dt: f32,
}

type SystemFn = fn(&mut Iter<'_>);

// A system: a name, the query that finds its tables, and the function run on
// each of those tables that has rows. `calls` and `rows` count what it has run
// over.
struct System {
    name: &'static str,
    query: Query,
    run: SystemFn,
    calls: usize,
    rows: usize,
}

impl System {
    fn new(name: &'static str, all: u64, none: u64, run: SystemFn) -> System {
        System { name, query: Query::new(all, none), run, calls: 0, rows: 0 }
    }
}

// One frame: every system in order, each over every table it matches that has
// rows. A system sees everything the systems before it wrote this frame.
fn progress(w: &mut World, systems: &mut [System], dt: f32) {
    for s in systems.iter_mut() {
        s.query.update(&w.tables);
        for &ti in &s.query.tables {
            let rows = w.tables[ti as usize].count();
            if rows > 0 {
                s.calls += 1;
                s.rows += rows;
                (s.run)(&mut Iter { tables: &mut w.tables, table: ti as usize, dt });
            }
        }
    }
}

fn print_schedule(w: &World, systems: &mut [System]) {
    for s in systems.iter_mut() {
        w.print_query(s.name, &mut s.query);
    }
}

// ---------------------------------------------------------------------------
// The program
// ---------------------------------------------------------------------------

// Kernels: plain loops over plain slices, with components for elements. A
// `&mut` slice is the promise rin spells `noalias`: nothing else reaches it,
// so the loop can be vectorised without checking for overlap.
fn integrate(ps: &mut [Position], vs: &[Velocity], dt: f32) {
    for (p, v) in ps.iter_mut().zip(vs) {
        p.at.x += v.per_second.x * dt;
        p.at.y += v.per_second.y * dt;
    }
}

fn burn(hs: &mut [Health], fires: &[Burning], dt: f32) {
    for (h, f) in hs.iter_mut().zip(fires) {
        h.points -= f.per_second * dt;
    }
}

fn age(ls: &mut [Lifetime], dt: f32) {
    for l in ls {
        l.seconds -= dt;
    }
}

// Systems: the table's columns, handed to a kernel.
fn move_system(it: &mut Iter) {
    let (ps, vs) = it.tables[it.table].pair_mut::<Position, Velocity>();
    integrate(ps, vs, it.dt);
}

fn burn_system(it: &mut Iter) {
    let (hs, fires) = it.tables[it.table].pair_mut::<Health, Burning>();
    burn(hs, fires, it.dt);
}

fn age_system(it: &mut Iter) {
    age(it.tables[it.table].slice_mut::<Lifetime>(), it.dt);
}

fn spawn_turret(w: &mut World, x: f32, y: f32) -> Entity {
    let e = w.spawn(mask_of::<Position>() | mask_of::<Turret>());
    w.set(e, Position { at: Vec2 { x, y } }).expect("just spawned");
    w.set(e, Turret { cooldown: 0.5, timer: 0.0 }).expect("just spawned");
    e
}

fn spawn_drone(w: &mut World, x: f32, y: f32) -> Entity {
    let e = w.spawn(mask_of::<Position>() | mask_of::<Velocity>() | mask_of::<Health>());
    w.set(e, Position { at: Vec2 { x, y } }).expect("just spawned");
    w.set(e, Velocity { per_second: Vec2 { x: -1.0, y: 0.0 } }).expect("just spawned");
    w.set(e, Health { points: 100.0, max: 100.0 }).expect("just spawned");
    e
}

fn spawn_bullet(w: &mut World, x: f32, y: f32, seconds: f32) -> Entity {
    let e = w.spawn(mask_of::<Position>() | mask_of::<Velocity>() | mask_of::<Damage>() | mask_of::<Lifetime>());
    w.set(e, Position { at: Vec2 { x, y } }).expect("just spawned");
    w.set(e, Velocity { per_second: Vec2 { x: 8.0, y: 0.0 } }).expect("just spawned");
    w.set(e, Damage { amount: 25.0 }).expect("just spawned");
    w.set(e, Lifetime { seconds }).expect("just spawned");
    e
}

// Every entity, with the components this program cares about.
fn print_state(w: &World) {
    for t in &w.tables {
        for &e in &t.entities {
            let p = w.get::<Position>(e).expect("everything here has a position");
            print!("    {e} at ({:5.2}, {:5.2})", p.at.x, p.at.y);
            if let Some(h) = w.get::<Health>(e) {
                print!("  health {:.0}", h.points);
            }
            if w.has::<Burning>(e) {
                print!(", burning");
            }
            if let Some(l) = w.get::<Lifetime>(e) {
                print!("  lifetime {:5.2}", l.seconds);
            }
            println!();
        }
    }
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

    spawn_turret(&mut w, 0.0, 0.0);
    spawn_drone(&mut w, 20.0, 4.0);
    let hot = spawn_drone(&mut w, 24.0, 0.0);
    spawn_drone(&mut w, 28.0, -4.0);
    w.add(hot, Burning { per_second: 8.0 }).expect("alive");
    spawn_bullet(&mut w, 1.0, 0.0, 0.5);
    spawn_bullet(&mut w, 1.0, 4.0, 1.0);
    spawn_bullet(&mut w, 1.0, -4.0, 1.5);

    // The schedule. The order is the data flow: a system that reads what
    // another writes runs after it.
    let mut systems = [
        System::new("burn", mask_of::<Health>() | mask_of::<Burning>(), 0, burn_system),
        System::new("age", mask_of::<Lifetime>(), 0, age_system),
        System::new("move", mask_of::<Position>() | mask_of::<Velocity>(), 0, move_system),
    ];
    println!("the schedule, run in this order every frame:");
    print_schedule(&w, &mut systems);
    println!();
    println!("at the start:");
    print_state(&w);

    let dt = 0.25;
    for frame in 1..=8 {
        progress(&mut w, &mut systems, dt);
        if frame % 4 == 0 {
            println!();
            println!("after frame {frame}, {:.2} seconds:", frame as f32 * dt);
            print_state(&w);
        }
    }

    println!();
    println!("8 frames, one call per table per frame:");
    for s in &systems {
        println!("    {:<4} called {:2} times, over {:2} rows", s.name, s.calls, s.rows);
    }

    // A system may change values, never structure, so nothing has removed the
    // bullets whose time is up.
    let ages = &systems[1].query;
    let expired = ages.tables.iter()
        .flat_map(|&ti| w.tables[ti as usize].slice::<Lifetime>())
        .filter(|l| l.seconds <= 0.0)
        .count();
    println!("{expired} bullets are out of time and still in the world");
    println!("verify: {} problems", w.verify());
}
