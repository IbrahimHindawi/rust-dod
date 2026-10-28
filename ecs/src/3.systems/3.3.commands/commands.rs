// 3.3 Commands
//
// 3.2's systems change values, never structure, so the bullets whose time ran
// out are still in the world. Despawning them from inside the loop that walks
// their table is the obvious fix, and it is wrong: a despawn swap-removes from
// the very table the loop is walking, so the last row moves into a row the
// loop has already passed, and is never looked at. Adding or removing a
// component is worse: the entity moves to another table the query may visit
// later, and a new table can move every table, the one being walked included.
//
// Rust catches some of this. A loop that iterates the table cannot also
// despawn, because the loop borrows the table and despawn needs the whole
// world: that does not compile. A loop over row numbers does compile, and has
// the bug. And a system cannot despawn at all: Iter hands it tables and the
// command buffer, never the world.
//
// So a system records the change, a command, and the world applies every
// command after the frame's last system:
//
//   during the frame   values change; tables, rows and borrows do not
//   after the frame    spawns, despawns, adds and removes, in order
//
// The second half of this file is a command buffer: the record, the methods
// systems call to fill it, and the flush that empties it.

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

impl Error {
    fn name(self) -> &'static str {
        match self {
            Error::Dead => "dead",
            Error::Missing => "missing",
        }
    }
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

// A plain-data value as its bytes, for storing in a command.
#[allow(unsafe_code)]
fn bytes_of<T: Pod>(value: &T) -> &[u8] {
    // SAFETY: T is Pod, so it has no padding and all its bytes are
    // initialised; the slice borrows `value` and cannot outlive it.
    unsafe { std::slice::from_raw_parts((value as *const T).cast::<u8>(), size_of::<T>()) }
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
    // The frame's commands. `trace` prints each one as it is applied.
    commands: Commands,
    applied: usize,
    skipped: usize,
    trace: bool,
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
            commands: Commands::default(),
            applied: 0,
            skipped: 0,
            trace: false,
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

    // Give e component `id`, with `value` for its bytes: 2.6's add, by id
    // instead of by type, because a command holds its value as bytes.
    fn add_id(&mut self, e: Entity, id: u32, value: &[u8]) -> Result<(), Error> {
        if !self.alive(e) {
            return Err(Error::Dead);
        }
        let from = self.slots[e.index as usize].table;
        if !self.tables[from as usize].has(id) {
            let to = self.table_with(from, id);
            self.move_to(e, to);
        }
        let row = self.row(e);
        let column = self.table_of_mut(e).columns[id as usize].as_mut().expect("its table has it now");
        column.row_mut(row).copy_from_slice(value);
        Ok(())
    }

    fn remove_id(&mut self, e: Entity, id: u32) -> Result<(), Error> {
        if !self.alive(e) {
            return Err(Error::Dead);
        }
        let from = self.slots[e.index as usize].table;
        if !self.tables[from as usize].has(id) {
            return Err(Error::Missing);
        }
        let to = self.table_without(from, id);
        self.move_to(e, to);
        Ok(())
    }

    // Give e a T, or take its T away: the byte versions, with the type turned
    // into an id and the value into its bytes.
    fn add<T: Component>(&mut self, e: Entity, value: T) -> Result<(), Error> {
        self.add_id(e, T::ID, bytes_of(&value))
    }

    fn remove<T: Component>(&mut self, e: Entity) -> Result<(), Error> {
        self.remove_id(e, T::ID)
    }

    // Apply the frame's commands in the order they were recorded, then let them
    // all go at once. A command for an entity that died after it was recorded
    // (despawned by an earlier command, most likely) does nothing, and is
    // counted. The buffer is taken out of the world while it is applied, so
    // applying it can change the world freely, and put back after, emptied but
    // with its capacity.
    fn flush(&mut self) {
        let mut cmds = std::mem::take(&mut self.commands);
        for i in 0..cmds.list.len() {
            let result = match cmds.list[i] {
                Command::Spawn { signature, .. } => {
                    let e = self.spawn(signature);
                    cmds.list[i] = Command::Spawn { signature, entity: Some(e) };
                    Ok(())
                }
                Command::Despawn(e) => self.despawn(e),
                Command::Add { target, id, start, len } => {
                    let e = cmds.resolve(target);
                    self.add_id(e, id, &cmds.bytes[start..start + len])
                }
                Command::Remove { entity, id } => self.remove_id(entity, id),
            };
            match result {
                Ok(()) => self.applied += 1,
                Err(_) => self.skipped += 1,
            }
            if self.trace {
                self.print_command(&cmds, i, result);
            }
        }
        cmds.list.clear();
        cmds.bytes.clear();
        self.commands = cmds;
    }

    // One applied command. The components given to a new entity are part of
    // its spawn line, so they are not printed on their own.
    fn print_command(&self, cmds: &Commands, i: usize, result: Result<(), Error>) {
        let line = match cmds.list[i] {
            Command::Add { target: Target::Pending(_), .. } if result.is_ok() => return,
            Command::Spawn { signature, entity } => {
                format!("spawn {} -> {}", self.names(signature), entity.expect("applied"))
            }
            Command::Despawn(e) => format!("despawn {e}"),
            Command::Add { target, id, .. } => format!("add {}, {}", self.info(id).name, cmds.resolve(target)),
            Command::Remove { entity, id } => format!("remove {}, {entity}", self.info(id).name),
        };
        match result {
            Ok(()) => println!("    {line}"),
            Err(e) => println!("    {line}: skipped, {}", e.name()),
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
// Commands
// ---------------------------------------------------------------------------

// A spawn that has been recorded and not applied. There is no entity yet, so
// it is named by its place in the command list; give it components with
// `commands.add(pending, value)`.
#[derive(Clone, Copy)]
struct Pending(usize);

// Who an add is for: a live entity, or one spawned earlier in the same frame.
// Both convert into a Target, so `add` takes either.
#[derive(Clone, Copy)]
enum Target {
    Entity(Entity),
    Pending(usize),
}

impl From<Entity> for Target {
    fn from(e: Entity) -> Target {
        Target::Entity(e)
    }
}

impl From<Pending> for Target {
    fn from(p: Pending) -> Target {
        Target::Pending(p.0)
    }
}

// A structural change recorded during a frame and applied after it. An add's
// value is `len` bytes at `start` in the buffer's byte store. A spawn's entity
// is filled in when it is applied, which is how a pending add finds it.
#[derive(Clone, Copy)]
enum Command {
    Spawn { signature: u64, entity: Option<Entity> },
    Despawn(Entity),
    Add { target: Target, id: u32, start: usize, len: usize },
    Remove { entity: Entity, id: u32 },
}

// The frame's commands, and the bytes of their values. Both Vecs are cleared
// after every flush and keep their capacity, so once a game has run a few
// frames, recording a command allocates nothing: a frame arena, made of Vecs.
#[derive(Default)]
struct Commands {
    list: Vec<Command>,
    bytes: Vec<u8>,
}

impl Commands {
    fn spawn(&mut self, signature: u64) -> Pending {
        self.list.push(Command::Spawn { signature, entity: None });
        Pending(self.list.len() - 1)
    }

    fn despawn(&mut self, e: Entity) {
        self.list.push(Command::Despawn(e));
    }

    fn add<T: Component>(&mut self, target: impl Into<Target>, value: T) {
        let start = self.bytes.len();
        self.bytes.extend_from_slice(bytes_of(&value));
        self.list.push(Command::Add { target: target.into(), id: T::ID, start, len: size_of::<T>() });
    }

    fn remove<T: Component>(&mut self, e: Entity) {
        self.list.push(Command::Remove { entity: e, id: T::ID });
    }

    // The entity a command is for: the one it names, or the one its spawn made.
    fn resolve(&self, target: Target) -> Entity {
        match target {
            Target::Entity(e) => e,
            Target::Pending(i) => match self.list[i] {
                Command::Spawn { entity: Some(e), .. } => e,
                _ => unreachable!("a spawn is applied before the commands that name it"),
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

// What a system gets for each table it runs on: every table, which one to
// work on, the frame's command buffer, and its time step. Its fields are
// separate borrows, so a system can take `&mut it.tables[it.table]` and record
// into `it.commands` at the same time. There is no world here, so a system has
// no way to change structure except through a command.
struct Iter<'a> {
    tables: &'a mut [Table],
    table: usize,
    commands: &'a mut Commands,
    dt: f32,
}

// A system's function gets the Iter, and the context the game passes to
// progress: whatever its systems share, such as another query or a score.
type SystemFn<C> = fn(&mut Iter<'_>, &mut C);

// A system: a name, the query that finds its tables, and the function run on
// each of those tables that has rows. `calls` and `rows` count what it has run
// over.
struct System<C> {
    name: &'static str,
    query: Query,
    run: SystemFn<C>,
    calls: usize,
    rows: usize,
}

impl<C> System<C> {
    fn new(name: &'static str, all: u64, none: u64, run: SystemFn<C>) -> System<C> {
        System { name, query: Query::new(all, none), run, calls: 0, rows: 0 }
    }
}

// One frame: every system in order, each over every table it matches that has
// rows, then the commands they recorded. A system sees every value the systems
// before it wrote this frame, and no structural change until the next.
fn progress<C>(w: &mut World, systems: &mut [System<C>], ctx: &mut C, dt: f32) {
    for s in systems.iter_mut() {
        s.query.update(&w.tables);
        for &ti in &s.query.tables {
            let rows = w.tables[ti as usize].count();
            if rows > 0 {
                s.calls += 1;
                s.rows += rows;
                let mut it = Iter { tables: &mut w.tables, table: ti as usize, commands: &mut w.commands, dt };
                (s.run)(&mut it, ctx);
            }
        }
    }
    w.flush();
}

fn print_schedule<C>(w: &World, systems: &mut [System<C>]) {
    for s in systems.iter_mut() {
        w.print_query(s.name, &mut s.query);
    }
}

// ---------------------------------------------------------------------------
// The program
// ---------------------------------------------------------------------------

// Kernels: plain loops over plain slices, as in 3.2.
fn integrate(ps: &mut [Position], vs: &[Velocity], dt: f32) {
    for (p, v) in ps.iter_mut().zip(vs) {
        p.at.x += v.per_second.x * dt;
        p.at.y += v.per_second.y * dt;
    }
}

fn age(ls: &mut [Lifetime], dt: f32) {
    for l in ls {
        l.seconds -= dt;
    }
}

fn bullet_signature() -> u64 {
    mask_of::<Position>() | mask_of::<Velocity>() | mask_of::<Damage>() | mask_of::<Lifetime>()
}

fn spawn_bullet(w: &mut World, x: f32, seconds: f32) -> Entity {
    let e = w.spawn(bullet_signature());
    w.set(e, Position { at: Vec2 { x, y: 0.0 } }).expect("just spawned");
    w.set(e, Velocity { per_second: Vec2 { x: 8.0, y: 0.0 } }).expect("just spawned");
    w.set(e, Damage { amount: 25.0 }).expect("just spawned");
    w.set(e, Lifetime { seconds }).expect("just spawned");
    e
}

fn spawn_target(w: &mut World, x: f32, y: f32) -> Entity {
    let e = w.spawn(mask_of::<Position>() | mask_of::<Health>());
    w.set(e, Position { at: Vec2 { x, y } }).expect("just spawned");
    w.set(e, Health { points: 100.0, max: 100.0 }).expect("just spawned");
    e
}

// The wrong way: despawn as the loop goes. Row `row` is swap-removed, the last
// row moves into it, and the loop moves on to row + 1. Written with an
// iterator over the table, this would not compile: the iterator borrows the
// table, and despawn needs the whole world. Written over row numbers, it
// borrows nothing between despawns, so it compiles, and is wrong.
fn expire_now(w: &mut World, table: usize) {
    let mut row = 0;
    while row < w.tables[table].count() {
        if w.tables[table].slice::<Lifetime>()[row].seconds <= 0.0 {
            let e = w.tables[table].entities[row];
            w.despawn(e).expect("alive");
        }
        row += 1;
    }
}

// The right way: record the despawn. The table does not change until the
// frame is over, so every row is looked at once.
fn expire_system<C>(it: &mut Iter, _ctx: &mut C) {
    let t = &it.tables[it.table];
    for (&e, l) in t.entities.iter().zip(t.slice::<Lifetime>()) {
        if l.seconds <= 0.0 {
            it.commands.despawn(e);
        }
    }
}

fn age_system<C>(it: &mut Iter, _ctx: &mut C) {
    age(it.tables[it.table].slice_mut::<Lifetime>(), it.dt);
}

fn move_system<C>(it: &mut Iter, _ctx: &mut C) {
    let (ps, vs) = it.tables[it.table].pair_mut::<Position, Velocity>();
    integrate(ps, vs, it.dt);
}

// A turret whose timer is up spawns a bullet where it stands. The bullet does
// not exist until the frame ends, so it first moves next frame.
fn turret_system<C>(it: &mut Iter, _ctx: &mut C) {
    let (ps, ts) = it.tables[it.table].pair_mut::<Position, Turret>();
    for (p, t) in ps.iter().zip(ts.iter_mut()) {
        if t.timer <= 0.0 {
            let shot = it.commands.spawn(bullet_signature());
            it.commands.add(shot, *p);
            it.commands.add(shot, Velocity { per_second: Vec2 { x: 8.0, y: 0.0 } });
            it.commands.add(shot, Damage { amount: 25.0 });
            it.commands.add(shot, Lifetime { seconds: 2.0 });
            t.timer += t.cooldown;
        }
        t.timer -= it.dt;
    }
}

// A bullet within half a unit of a target hits it. The damage is a value, so
// it is written now; the bullet's despawn is structure, so it is recorded. The
// targets are another query, which this schedule passes as its context. The
// bullets' table and a target table are borrowed together with
// get_disjoint_mut; during a frame no table is made or moved, so the indices
// stay good.
fn hit_system(it: &mut Iter, targets: &mut Query) {
    targets.update(it.tables);
    for b in 0..it.tables[it.table].count() {
        for &ti in &targets.tables {
            let Ok([bullets, t]) = it.tables.get_disjoint_mut([it.table, ti as usize]) else {
                continue;
            };
            let p = bullets.slice::<Position>()[b];
            let d = bullets.slice::<Damage>()[b];
            let e = bullets.entities[b];
            let (tps, ths) = t.pair_mut::<Position, Health>();
            for (tp, th) in tps.iter().zip(ths.iter_mut()) {
                let dx = tp.at.x - p.at.x;
                let dy = tp.at.y - p.at.y;
                if dx * dx + dy * dy < 0.25 {
                    th.points -= d.amount;
                    it.commands.despawn(e);
                }
            }
        }
    }
}

fn make_world() -> World {
    let mut w = World::new();
    w.register::<Position>();
    w.register::<Velocity>();
    w.register::<Health>();
    w.register::<Damage>();
    w.register::<Lifetime>();
    w.register::<Turret>();
    w.register::<Burning>();
    w
}

// Four bullets, three of them out of time.
fn four_bullets(w: &mut World) {
    spawn_bullet(w, 1.0, 0.0);
    spawn_bullet(w, 2.0, 1.0);
    spawn_bullet(w, 3.0, 0.0);
    spawn_bullet(w, 4.0, 0.0);
}

fn print_lifetimes(w: &World) {
    let t = &w.tables[w.by_signature[&bullet_signature()] as usize];
    print!("    lifetimes:");
    for (e, l) in t.entities.iter().zip(t.slice::<Lifetime>()) {
        print!("  {e} {:.2}", l.seconds);
    }
    println!();
}

fn main() {
    println!("1. an expire loop that despawns as it goes");
    let mut a = make_world();
    four_bullets(&mut a);
    print_lifetimes(&a);
    let bullets = a.by_signature[&bullet_signature()] as usize;
    expire_now(&mut a, bullets);
    println!("after one pass:");
    print_lifetimes(&a);
    println!("verify: {} problems. The world is fine; the loop missed one.", a.verify());

    println!();
    println!("2. an expire system that records despawns");
    let mut b = make_world();
    b.trace = true;
    four_bullets(&mut b);
    print_lifetimes(&b);
    let mut right = [System::new("expire", mask_of::<Lifetime>(), 0, expire_system)];
    progress(&mut b, &mut right, &mut (), 0.25);
    println!("after one frame:");
    print_lifetimes(&b);

    println!();
    println!("3. a turret, two targets in its line of fire, one out of it");
    let mut c = make_world();
    c.trace = true;
    let gun = c.spawn(mask_of::<Position>() | mask_of::<Turret>());
    c.set(gun, Turret { cooldown: 0.5, timer: 0.0 }).expect("just spawned");
    let near = spawn_target(&mut c, 4.0, 0.0);
    let far = spawn_target(&mut c, 4.25, 0.0);
    let aside = spawn_target(&mut c, 4.0, 3.0);
    let mut targets = Query::new(mask_of::<Position>() | mask_of::<Health>(), 0);
    let lifetime_only = mask_of::<Lifetime>();
    let mut systems = [
        System::new("turret", mask_of::<Position>() | mask_of::<Turret>(), 0, turret_system),
        System::new("age", lifetime_only, 0, age_system),
        System::new("expire", lifetime_only, 0, expire_system),
        System::new("hit", mask_of::<Position>() | mask_of::<Damage>(), 0, hit_system),
        System::new("move", mask_of::<Position>() | mask_of::<Velocity>(), 0, move_system),
    ];
    for frame in 1..=6 {
        println!("frame {frame}");
        progress(&mut c, &mut systems, &mut targets, 0.25);
    }
    let health = |e| c.get::<Health>(e).expect("alive").points;
    println!("health: {near} {:.0}, {far} {:.0}, {aside} {:.0}", health(near), health(far), health(aside));
    println!("commands applied: {}, skipped: {}", c.applied, c.skipped);
    c.print_tables();
    println!("verify: {} problems", c.verify());
}
