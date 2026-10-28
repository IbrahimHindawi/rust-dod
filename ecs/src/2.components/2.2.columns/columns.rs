// 2.2 Columns
//
// 1.4 kept its components in typed arrays, `positions: Vec<Vec2>`. A world
// that works for any component cannot: it does not know the types. It knows,
// from the registry, each component's size and alignment, and that is enough.
// A column is an array of a component kept as bytes: row `r` is the `size`
// bytes at `r * size`.
//
// Everything a table needs to do with its data is a byte operation on columns:
// grow, copy a row over another (swap-remove), zero a new row. None of it
// depends on the type. The type comes back at the edge, where game code reads
// the column as `&[T]`, and that typed view checks it is the right component.
//
// This is the one place in the course that needs `unsafe`: turning bytes into
// a `&[T]` is a promise the compiler cannot check. Two things make the promise
// true. Components are plain data (`Pod`, checked at compile time when one is
// declared), so any bytes are a valid value; and the column's storage is
// aligned for any component. Everything else is safe code on byte slices.
//
// This chapter builds the column and exercises it on its own, with the count
// and capacity kept beside it by hand. 2.3 puts columns in a table.

// Components are declared here and measured, never read, and from here on each
// chapter carries kayan code that not every program calls. Dead-code warnings
// would be noise.
#![allow(dead_code)]
// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

use std::alloc::Layout;

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

// Declares a plain-data struct. Two checks run at compile time: every field is
// Pod, and the struct is exactly as big as its fields, so it has no padding.
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
component!(Health { points: f32, max: f32 });

// ---------------------------------------------------------------------------
// The column: bytes, and how many of them make one row
// ---------------------------------------------------------------------------

// The column's storage is a Vec of 16-byte blocks aligned to 16. No component
// may need more than 16 (checked when the column is made), so with rows packed
// from the start of the storage every row of every component is aligned.
#[derive(Clone, Copy, Default)]
#[repr(C, align(16))]
struct Block([u8; 16]);

// A column does not know its count. A table keeps that once for all its
// columns, since every column of a table has the same number of rows.
struct Column {
    blocks: Vec<Block>,
    size: usize,
    id: u32,
    info: &'static Info,
}

impl Column {
    // An empty column for one component. Its size comes from the component's
    // reflect record.
    fn new(id: u32, info: &'static Info) -> Column {
        assert!(info.layout.align() <= 16, "{} needs more alignment than a column has", info.name);
        Column { blocks: Vec::new(), size: info.layout.size(), id, info }
    }

    // The whole storage as bytes. Rows live in the first `count * size`.
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

    // Row `row`'s bytes.
    fn row(&self, row: usize) -> &[u8] {
        &self.bytes()[row * self.size..(row + 1) * self.size]
    }

    // Room for `capacity` rows, new bytes zeroed. The Vec copies the old rows
    // into its new block and frees the old one, so a growing column moves:
    // anything that pointed into it would be left behind.
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

    // The typed view: the column's first `count` rows as `&[T]`. Reading a
    // column as the wrong component would reinterpret its bytes, so the id is
    // checked: the one mistake the byte level cannot catch is caught here, at
    // the edge.
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
// Using it
// ---------------------------------------------------------------------------

fn print_rows(positions: &Column, healths: &Column, count: usize) {
    let ps = positions.slice::<Position>(count);
    let hs = healths.slice::<Health>(count);
    for (row, (p, h)) in ps.iter().zip(hs).enumerate() {
        println!("    row {row}  position ({:5.2}, {:5.2})  health {:3.0} / {:3.0}", p.at.x, p.at.y, h.points, h.max);
    }
}

fn main() {
    // Two columns, made from nothing but reflect records.
    let mut positions = Column::new(Position::ID, Position::INFO);
    let mut healths = Column::new(Health::ID, Health::INFO);
    println!("a position column has {}-byte rows, a health column {}-byte rows", positions.size, healths.size);

    // Count and capacity, kept by hand here. 2.3 moves them into a table.
    let mut count = 0;
    let mut capacity = 0;
    for i in 0..5 {
        if count == capacity {
            capacity = if capacity == 0 { 2 } else { capacity * 2 };
            positions.grow(capacity);
            healths.grow(capacity);
            println!("    grew to room for {capacity} rows");
        }
        let row = count;
        count += 1;
        positions.zero_row(row);
        healths.zero_row(row);
        // Back to types to write the values.
        positions.slice_mut::<Position>(count)[row] = Position { at: Vec2 { x: i as f32, y: 1.0 } };
        healths.slice_mut::<Health>(count)[row] = Health { points: 50.0 + 10.0 * i as f32, max: 100.0 };
    }
    println!("five rows, written through the typed view:");
    print_rows(&positions, &healths, count);
    print!("row 1's health as the column stores it, bytes:");
    for b in healths.row(1) {
        print!(" {b:02x}");
    }
    println!();
    println!("    that is 60.0 and 100.0 as little-endian f32s; the column never knew");

    // Swap-remove row 1 from both columns: the same byte copy for both, sized
    // by each column, so a row stays a row across columns.
    let last = count - 1;
    positions.copy_row(1, last);
    healths.copy_row(1, last);
    count -= 1;
    println!("swap-removed row 1: row {last} moved into it, in both columns");
    print_rows(&positions, &healths, count);
}
