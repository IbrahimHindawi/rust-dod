// kayan: archetype tables, entity handles, and moving entities between tables.
//
// This is the world lesson 4 builds, kept as a module so lessons 5 and 6 can
// use it as it is. Read it with lesson 4: the lesson explains the why, this
// file is the how.
//
// A program brings it in with two lines at the top, and still builds with
// plain rustc:
//
//   #[path = "../../includes/kayan.rs"]
//   mod kayan;
//
// What the program provides:
//
//   * an enum `ComponentId` with one member per component;
//   * its components, declared with component!(Name { field: Type, ... }),
//     which checks each is plain data, gives it the id of the ComponentId
//     member with the same name, and writes its reflect record;
//   * a w.register::<T>() for each component.
//
// Up to 64 components, because a signature is a u64.
//
// Not here, on purpose: threads, change tracking, chunked storage, relations.
// Each is worth a lesson of its own and none is needed to see the idea.

// Not every program calls every function here.
#![allow(dead_code)]
// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

use std::alloc::Layout;
use std::collections::HashMap;
use std::fmt;
use std::num::NonZeroU32;

// ---------------------------------------------------------------------------
// Plain data, and what a tool can know about it
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
pub unsafe trait Pod: Copy + 'static {}

#[allow(unsafe_code)]
unsafe impl Pod for f32 {}
#[allow(unsafe_code)]
unsafe impl Pod for u32 {}
#[allow(unsafe_code)]
unsafe impl Pod for i32 {}
#[allow(unsafe_code)]
unsafe impl<const N: usize> Pod for [u8; N] {}

// A reflect record: a type's name, size and alignment, and what its bytes are.
// The compiler knows all of it; the macros below write it down.
pub struct Info {
    pub name: &'static str,
    pub layout: Layout,
    pub kind: Kind,
}

pub enum Kind {
    F32,
    U32,
    I32,
    // A [u8; N]: text, padded with zeros.
    Text,
    // An i32 that is one of these named values (pod_enum!).
    Enum(&'static [(&'static str, i32)]),
    // A struct, field by field (pod!).
    Struct(&'static [Field]),
}

// One field: where its bytes start in the struct, and the record of its type.
// `hidden` marks bookkeeping a tool need not show.
pub struct Field {
    pub name: &'static str,
    pub offset: usize,
    pub info: &'static Info,
    pub hidden: bool,
}

// Every type a component is made of has a record.
pub trait Reflect {
    const INFO: &'static Info;
}

impl Reflect for f32 {
    const INFO: &'static Info = &Info { name: "f32", layout: Layout::new::<f32>(), kind: Kind::F32 };
}

impl Reflect for u32 {
    const INFO: &'static Info = &Info { name: "u32", layout: Layout::new::<u32>(), kind: Kind::U32 };
}

impl Reflect for i32 {
    const INFO: &'static Info = &Info { name: "i32", layout: Layout::new::<i32>(), kind: Kind::I32 };
}

impl<const N: usize> Reflect for [u8; N] {
    const INFO: &'static Info = &Info { name: "text", layout: Layout::new::<[u8; N]>(), kind: Kind::Text };
}

macro_rules! is_hidden {
    () => {
        false
    };
    (hidden) => {
        true
    };
}
pub(crate) use is_hidden;

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

        const _: () = {
            use $crate::kayan::{Field, Info, Kind, Pod, Reflect};

            #[allow(unsafe_code)]
            unsafe impl Pod for $name {}

            impl Reflect for $name {
                const INFO: &'static Info = &Info {
                    name: stringify!($name),
                    layout: std::alloc::Layout::new::<$name>(),
                    kind: Kind::Struct(&[$(Field {
                        name: stringify!($field),
                        offset: std::mem::offset_of!($name, $field),
                        info: <$ty as Reflect>::INFO,
                        hidden: $crate::kayan::is_hidden!($($attr)?),
                    }),*]),
                };
            }

            const fn field_is_pod<T: Pod>() {}
            $(field_is_pod::<$ty>();)*
            assert!(size_of::<$name>() == 0 $(+ size_of::<$ty>())*, "a component has padding");
        };
    };
}
pub(crate) use pod;

// A plain-data enum: an i32 with named values. A Rust enum cannot be stored
// as bytes, because only its members are valid values and a column holds
// whatever bytes it holds. So the field is the number, and the record keeps
// the names, for tools. Only lesson 5 uses it.
#[allow(unused_macros)]
macro_rules! pod_enum {
    ($name:ident { $($member:ident = $value:literal),* $(,)? }) => {
        #[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
        #[repr(transparent)]
        struct $name(i32);

        // A program need not name every member.
        #[allow(dead_code)]
        impl $name {
            $(const $member: $name = $name($value);)*
        }

        const _: () = {
            use $crate::kayan::{Info, Kind, Pod, Reflect};

            #[allow(unsafe_code)]
            unsafe impl Pod for $name {}

            impl Reflect for $name {
                const INFO: &'static Info = &Info {
                    name: stringify!($name),
                    layout: std::alloc::Layout::new::<$name>(),
                    kind: Kind::Enum(&[$((stringify!($member), $value)),*]),
                };
            }
        };
    };
}
#[allow(unused_imports)]
pub(crate) use pod_enum;

// A component: plain data with a record, and an id.
pub trait Component: Pod + Reflect {
    const ID: u32;
}

// pod!, plus the id: the ComponentId member with the same name as the struct.
macro_rules! component {
    ($name:ident { $($(#[$attr:ident])? $field:ident: $ty:ty),* $(,)? }) => {
        $crate::kayan::pod!($name { $($(#[$attr])? $field: $ty),* });

        impl $crate::kayan::Component for $name {
            const ID: u32 = ComponentId::$name as u32;
        }
    };
}
pub(crate) use component;

// One bit per component. A signature is a u64, so a world has at most 64.
pub fn bit(id: u32) -> u64 {
    1 << id
}

pub fn mask_of<T: Component>() -> u64 {
    bit(T::ID)
}

// A plain-data value as its bytes, for storing it somewhere that only knows
// bytes, like lesson 6's command buffer.
#[allow(unsafe_code)]
pub fn bytes_of<T: Pod>(value: &T) -> &[u8] {
    // SAFETY: T is Pod, so it has no padding and all its bytes are
    // initialised; the slice borrows `value` and cannot outlive it.
    unsafe { std::slice::from_raw_parts((value as *const T).cast::<u8>(), size_of::<T>()) }
}

// ---------------------------------------------------------------------------
// Entities
// ---------------------------------------------------------------------------

// A handle. `index` picks a slot in the world's slot list and `generation`
// says which life of that slot the handle was made for. Despawning bumps the
// slot's generation, so every handle to the dead entity stops matching, even
// after the index is reused. Generations start at 1, so "no entity" is None,
// and an Option<Entity> is no bigger than an Entity.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Entity {
    pub index: u32,
    pub generation: NonZeroU32,
}

impl fmt::Display for Entity {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:{}", self.index, self.generation)
    }
}

const ONE: NonZeroU32 = NonZeroU32::MIN;

// Where an entity lives now: which table, which row. Rows move (swap-remove,
// lesson 2), so this is updated whenever one does. Handles never move. A free
// slot already holds its next generation, which no handle has yet, so the
// generation check alone says whether a handle is alive.
#[derive(Clone, Copy)]
pub struct Slot {
    pub generation: NonZeroU32,
    pub table: u32,
    pub row: u32,
}

// Why a structural change changed nothing: the handle no longer names a live
// entity. It is the only way one can fail.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Dead;

// "ok" or "dead", for printing what a change returned.
pub fn status(r: Result<(), Dead>) -> &'static str {
    match r {
        Ok(()) => "ok",
        Err(Dead) => "dead",
    }
}

// ---------------------------------------------------------------------------
// Columns: one component's rows, as bytes
// ---------------------------------------------------------------------------

// The storage of a column: 16-byte blocks, so it is aligned for any component
// a column accepts.
#[derive(Clone, Copy, Default)]
#[repr(C, align(16))]
struct Block([u8; 16]);

pub struct Column {
    blocks: Vec<Block>,
    pub info: &'static Info,
    id: u32,
}

impl Column {
    fn new(id: u32, info: &'static Info) -> Column {
        assert!(info.layout.align() <= 16, "{} needs more alignment than a column has", info.name);
        Column { blocks: Vec::new(), info, id }
    }

    pub fn size(&self) -> usize {
        self.info.layout.size()
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
        let size = self.size();
        &self.bytes()[row * size..(row + 1) * size]
    }

    fn row_mut(&mut self, row: usize) -> &mut [u8] {
        let size = self.size();
        &mut self.bytes_mut()[row * size..(row + 1) * size]
    }

    // Room for `capacity` rows. The rows move to the new allocation and the
    // old one is freed.
    fn grow(&mut self, capacity: usize) {
        self.blocks.resize((capacity * self.size()).div_ceil(16), Block::default());
    }

    fn copy_row(&mut self, to: usize, from: usize) {
        let size = self.size();
        self.bytes_mut().copy_within(from * size..(from + 1) * size, to * size);
    }

    fn zero_row(&mut self, row: usize) {
        self.row_mut(row).fill(0);
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
// Tables
// ---------------------------------------------------------------------------

// One archetype. `entities[row]` is the handle of the entity in that row,
// which is how a swap-remove finds the slot to fix. `columns[id]` is Some when
// the signature has bit `id`. The row count is the length of `entities`.
pub struct Table {
    pub signature: u64,
    pub capacity: usize,
    pub entities: Vec<Entity>,
    pub columns: [Option<Column>; 64],
}

// The tables a system wants: every component in `all`, none in `none`.
#[derive(Clone, Copy, Default)]
pub struct Query {
    pub all: u64,
    pub none: u64,
}

impl Table {
    // A table for a signature: one empty column per component in it.
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

    pub fn count(&self) -> usize {
        self.entities.len()
    }

    pub fn has(&self, id: u32) -> bool {
        self.signature & bit(id) != 0
    }

    pub fn matches(&self, q: Query) -> bool {
        (self.signature & q.all) == q.all && (self.signature & q.none) == 0
    }

    // Room for 16 rows at first, then double, in `entities` and every column.
    fn grow(&mut self) {
        self.capacity = if self.capacity == 0 { 16 } else { self.capacity * 2 };
        self.entities.reserve_exact(self.capacity - self.entities.len());
        for c in self.columns.iter_mut().flatten() {
            c.grow(self.capacity);
        }
    }

    // A new row with zeroed components, owned by `e`.
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

    // Swap-remove: the last row moves into `row`, and the slot of the entity
    // that moved is told its new row. This is the line lesson 2 could not
    // write. The slots are the world's, so the world passes them in.
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

    // T's column, cut to the rows in use. Asking for a column the table does
    // not have is a bug in the caller, so it stops the program.
    pub fn column<T: Component>(&self) -> &[T] {
        self.columns[T::ID as usize]
            .as_ref()
            .unwrap_or_else(|| panic!("table {} has no {} column", self.signature, T::INFO.name))
            .slice::<T>(self.count())
    }

    pub fn column_mut<T: Component>(&mut self) -> &mut [T] {
        let count = self.count();
        self.columns[T::ID as usize]
            .as_mut()
            .unwrap_or_else(|| panic!("table {} has no {} column", self.signature, T::INFO.name))
            .slice_mut::<T>(count)
    }

    // Two columns at once, the first to write. They must be two different
    // components, which is checked when the program is built.
    pub fn pair_mut<A: Component, B: Component>(&mut self) -> (&mut [A], &[B]) {
        const { assert!(A::ID != B::ID, "pair_mut needs two different components") };
        let (count, signature) = (self.count(), self.signature);
        let [a, b] = self.columns.get_disjoint_mut([A::ID as usize, B::ID as usize]).expect("two different columns");
        let a = a.as_mut().unwrap_or_else(|| panic!("table {signature} has no {} column", A::INFO.name));
        let b = b.as_ref().unwrap_or_else(|| panic!("table {signature} has no {} column", B::INFO.name));
        (a.slice_mut::<A>(count), b.slice::<B>(count))
    }
}

// ---------------------------------------------------------------------------
// The world
// ---------------------------------------------------------------------------

// The registry, every table, and the entity index. Tables are referred to by
// index: making a table can grow `tables` and move all of them, and an index
// survives that.
pub struct World {
    pub infos: [Option<&'static Info>; 64],
    pub component_count: u32,
    pub tables: Vec<Table>,
    pub by_signature: HashMap<u64, u32>,
    pub slots: Vec<Slot>,
    pub free: Vec<u32>,
    pub live: usize,
}

impl World {
    pub fn new() -> World {
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

    pub fn register<T: Component>(&mut self) {
        assert!(T::ID < 64, "component id {} is past the 64 a signature can hold", T::ID);
        self.infos[T::ID as usize] = Some(T::INFO);
        self.component_count = self.component_count.max(T::ID + 1);
    }

    pub fn info(&self, id: u32) -> &'static Info {
        self.infos[id as usize].expect("every component in a signature is registered")
    }

    // The table for a signature, made the first time anything needs it.
    pub fn table_for(&mut self, signature: u64) -> u32 {
        if let Some(&index) = self.by_signature.get(&signature) {
            return index;
        }
        let index = self.tables.len() as u32;
        self.tables.push(Table::new(&self.infos, signature));
        self.by_signature.insert(signature, index);
        index
    }

    pub fn alive(&self, e: Entity) -> bool {
        self.slots.get(e.index as usize).is_some_and(|s| s.generation == e.generation)
    }

    // A new entity with the components in `signature`, all zeroed. A freed
    // index is reused before a new one is made, with the generation its
    // despawn left behind.
    pub fn spawn(&mut self, signature: u64) -> Entity {
        let index = match self.free.pop() {
            Some(index) => index,
            None => {
                self.slots.push(Slot { generation: ONE, table: 0, row: 0 });
                self.slots.len() as u32 - 1
            }
        };
        let e = Entity { index, generation: self.slots[index as usize].generation };
        let table = self.table_for(signature);
        let row = self.tables[table as usize].add_row(e);
        self.slots[index as usize] = Slot { generation: e.generation, table, row: row as u32 };
        self.live += 1;
        e
    }

    // A dead handle changes nothing, so despawning twice is harmless.
    pub fn despawn(&mut self, e: Entity) -> Result<(), Dead> {
        if !self.alive(e) {
            return Err(Dead);
        }
        let Slot { table, row, .. } = self.slots[e.index as usize];
        self.tables[table as usize].remove_row(row as usize, &mut self.slots);
        let slot = &mut self.slots[e.index as usize];
        slot.generation = slot.generation.checked_add(1).expect("a slot has lived 4 billion lives");
        self.free.push(e.index);
        self.live -= 1;
        Ok(())
    }

    // The components a live entity has: its table's signature. 0 for a dead
    // one.
    pub fn signature(&self, e: Entity) -> u64 {
        if !self.alive(e) {
            return 0;
        }
        self.tables[self.slots[e.index as usize].table as usize].signature
    }

    // Move `e` to the table for `signature`. Components both tables have are
    // copied; components only the new table has start zeroed; components only
    // the old table has are dropped.
    pub fn move_to(&mut self, e: Entity, signature: u64) -> Result<(), Dead> {
        if !self.alive(e) {
            return Err(Dead);
        }
        let Slot { table: from, row, .. } = self.slots[e.index as usize];
        if self.tables[from as usize].signature == signature {
            return Ok(());
        }
        // table_for can make a table and move them all, so no table is
        // borrowed until it returns.
        let to = self.table_for(signature);
        let [old, new] = self.tables.get_disjoint_mut([from as usize, to as usize]).expect("two different tables");
        let new_row = new.add_row(e);
        for (o, n) in old.columns.iter().zip(new.columns.iter_mut()) {
            if let (Some(o), Some(n)) = (o, n) {
                n.row_mut(new_row).copy_from_slice(o.row(row as usize));
            }
        }
        old.remove_row(row as usize, &mut self.slots);
        self.slots[e.index as usize] = Slot { generation: e.generation, table: to, row: new_row as u32 };
        Ok(())
    }

    // e's T, or None if e is dead or has no T.
    pub fn get<T: Component>(&self, e: Entity) -> Option<&T> {
        if !self.alive(e) {
            return None;
        }
        let Slot { table, row, .. } = self.slots[e.index as usize];
        let t = &self.tables[table as usize];
        t.has(T::ID).then(|| &t.column::<T>()[row as usize])
    }

    pub fn get_mut<T: Component>(&mut self, e: Entity) -> Option<&mut T> {
        if !self.alive(e) {
            return None;
        }
        let Slot { table, row, .. } = self.slots[e.index as usize];
        let t = &mut self.tables[table as usize];
        t.has(T::ID).then(|| &mut t.column_mut::<T>()[row as usize])
    }

    // e's component `id` as its bytes, or None if e is dead or has no such
    // component. For tools that work from reflect records rather than from
    // types, like lesson 5's inspector.
    pub fn get_bytes(&self, e: Entity, id: u32) -> Option<&[u8]> {
        if !self.alive(e) || id >= self.component_count {
            return None;
        }
        let Slot { table, row, .. } = self.slots[e.index as usize];
        self.tables[table as usize].columns[id as usize].as_ref().map(|c| c.row(row as usize))
    }

    pub fn get_bytes_mut(&mut self, e: Entity, id: u32) -> Option<&mut [u8]> {
        if !self.alive(e) || id >= self.component_count {
            return None;
        }
        let Slot { table, row, .. } = self.slots[e.index as usize];
        self.tables[table as usize].columns[id as usize].as_mut().map(|c| c.row_mut(row as usize))
    }

    // Give e a T, moving it to a new table if it did not have one.
    pub fn add<T: Component>(&mut self, e: Entity, value: T) -> Result<(), Dead> {
        self.move_to(e, self.signature(e) | mask_of::<T>())?;
        *self.get_mut::<T>(e).expect("its table has a T now") = value;
        Ok(())
    }

    // Take e's T away, moving it to the table without it.
    pub fn remove<T: Component>(&mut self, e: Entity) -> Result<(), Dead> {
        self.move_to(e, self.signature(e) & !mask_of::<T>())
    }

    // Every row of every table must belong to a live entity whose slot points
    // back at that table and row, and the rows must add up to the live count.
    // Returns how many of those checks failed, printing each.
    pub fn verify(&self) -> usize {
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
            println!("verify: {rows} rows in tables, {} entities alive", self.live);
            problems += 1;
        }
        problems
    }

    // A signature as names, {Position Velocity}.
    pub fn names(&self, signature: u64) -> String {
        let names: Vec<&str> = (0..self.component_count)
            .filter(|&id| signature & bit(id) != 0)
            .map(|id| self.info(id).name)
            .collect();
        format!("{{{}}}", names.join(" "))
    }

    pub fn print_tables(&self) {
        for (ti, t) in self.tables.iter().enumerate() {
            print!("    table {ti} {}:", self.names(t.signature));
            for e in &t.entities {
                print!(" {e}");
            }
            println!();
        }
    }
}
