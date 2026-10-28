// Dojo 4, the playground: a type-erased column.
//
// A column is one component's rows, kept as bytes. It doesn't know the type
// it holds, only how many bytes make a row and how they must be aligned, and
// that is enough to grow, to zero a row, and to move the last row into a hole.
// The type comes back only at the edge: a typed view turns the bytes into a
// &[T], for a T of the column's size and alignment, in one unsafe line.
//
// Two things make it work:
//
//   * the storage starts on a 16-byte boundary, so every row of any type
//     aligned to 16 or less lands where it may live;
//   * a full column grows by a factor, not by a row, so the copies that
//     growing costs add up to about one per push.
//
// This program fills a column, draws each row's bytes next to the value they
// are, grows it, swap-removes from it, and asks it for the wrong type. Break
// on `grow` in a debugger and watch the rows move to the new allocation.
//
// The switch: set DOUBLE to false, so a full column grows by one row, and run
// it again. Look at the last line: the same 1000 pushes, and how many rows
// were copied to make room for them.

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

const DOUBLE: bool = true;

/// Plain old data: a type a column can hold as bytes and hand back.
///
/// # Safety
///
/// The type is Copy, has no padding bytes, and every bit pattern is a valid
/// value of it.
#[allow(unsafe_code)]
unsafe trait Pod: Copy + 'static {}

#[derive(Clone, Copy)]
#[repr(C)]
struct Vec2 {
    x: f32,
    y: f32,
}

#[allow(unsafe_code)]
unsafe impl Pod for Vec2 {}
#[allow(unsafe_code)]
unsafe impl Pod for u32 {}
#[allow(unsafe_code)]
unsafe impl Pod for [u8; 8] {}

#[derive(Clone, Copy, Default)]
#[repr(C, align(16))]
struct Block([u8; 16]);

struct Column {
    blocks: Vec<Block>,
    size: usize,
    align: usize,
    len: usize,
    capacity: usize,
    // What growing has cost so far.
    grows: usize,
    copied: usize,
}

impl Column {
    fn new(size: usize, align: usize) -> Column {
        Column { blocks: Vec::new(), size, align, len: 0, capacity: 0, grows: 0, copied: 0 }
    }

    #[allow(unsafe_code)]
    fn bytes(&self) -> &[u8] {
        // SAFETY: a Block is 16 initialised bytes with no padding.
        unsafe { std::slice::from_raw_parts(self.blocks.as_ptr().cast::<u8>(), self.blocks.len() * 16) }
    }

    #[allow(unsafe_code)]
    fn bytes_mut(&mut self) -> &mut [u8] {
        // SAFETY: as in bytes(); `&mut self` makes the view the only one.
        unsafe { std::slice::from_raw_parts_mut(self.blocks.as_mut_ptr().cast::<u8>(), self.blocks.len() * 16) }
    }

    // A new allocation, the rows in use copied into it, and the old one freed
    // when `blocks` is replaced. A Vec would often do this inside `resize`;
    // here it is spelled out, so the move is always a move.
    fn grow(&mut self) {
        let capacity = match (DOUBLE, self.capacity) {
            (_, 0) => 4,
            (true, c) => c * 2,
            (false, c) => c + 1,
        };
        let mut bigger = vec![Block::default(); (capacity * self.size).div_ceil(16)];
        let used = (self.len * self.size).div_ceil(16);
        bigger[..used].copy_from_slice(&self.blocks[..used]);
        self.blocks = bigger;
        self.grows += 1;
        self.copied += self.len;
        self.capacity = capacity;
    }

    fn push_zeroed(&mut self) -> usize {
        if self.len == self.capacity {
            self.grow();
        }
        let row = self.len;
        self.len += 1;
        self.row_mut(row).fill(0);
        row
    }

    fn swap_remove(&mut self, row: usize) {
        let (size, last) = (self.size, self.len - 1);
        self.bytes_mut().copy_within(last * size..(last + 1) * size, row * size);
        self.len = last;
    }

    fn row(&self, row: usize) -> &[u8] {
        &self.bytes()[row * self.size..(row + 1) * self.size]
    }

    fn row_mut(&mut self, row: usize) -> &mut [u8] {
        let size = self.size;
        &mut self.bytes_mut()[row * size..(row + 1) * size]
    }

    // The rows as T's, or None for a T that can't be the column's type. The
    // kata panics instead; this program wants to keep going.
    #[allow(unsafe_code)]
    fn slice<T: Pod>(&self) -> Option<&[T]> {
        if size_of::<T>() != self.size || align_of::<T>() != self.align {
            return None;
        }
        let bytes = &self.bytes()[..self.len * self.size];
        // SAFETY: `len` rows of T's size, on a 16-byte boundary; T is Pod.
        Some(unsafe { std::slice::from_raw_parts(bytes.as_ptr().cast::<T>(), self.len) })
    }

    #[allow(unsafe_code)]
    fn slice_mut<T: Pod>(&mut self) -> Option<&mut [T]> {
        if size_of::<T>() != self.size || align_of::<T>() != self.align {
            return None;
        }
        let (size, len) = (self.size, self.len);
        let bytes = &mut self.bytes_mut()[..len * size];
        // SAFETY: as in slice(); T has no padding, so the bytes stay initialised.
        Some(unsafe { std::slice::from_raw_parts_mut(bytes.as_mut_ptr().cast::<T>(), len) })
    }

    fn draw(&self) {
        println!("    capacity {}, {} rows in use, {} blocks of storage", self.capacity, self.len, self.blocks.len());
        let view = self.slice::<Vec2>().expect("a Vec2 column");
        for (row, v) in view.iter().enumerate() {
            let bytes: Vec<String> = self.row(row).iter().map(|b| format!("{b:02x}")).collect();
            println!("    row {row}  {}   as a Vec2 ({}, {})", bytes.join(" "), v.x, v.y);
        }
    }
}

fn push(c: &mut Column, x: f32, y: f32) {
    let row = c.push_zeroed();
    c.slice_mut::<Vec2>().expect("a Vec2 column")[row] = Vec2 { x, y };
}

fn main() {
    let mut c = Column::new(size_of::<Vec2>(), align_of::<Vec2>());
    println!("a column of Vec2: {}-byte rows aligned to {}, kept in 16-byte blocks", c.size, c.align);

    println!("push 3 zeroed rows, and write each through the typed view");
    for i in 0..3 {
        push(&mut c, (2 * i + 1) as f32, (2 * i + 2) as f32);
    }
    c.draw();

    println!("push 2 more: the column is full at 4 rows, so it grows before the fifth");
    push(&mut c, 7.0, 8.0);
    let before = c.grows;
    push(&mut c, 9.0, 10.0);
    if c.grows > before {
        println!("    grew to {} rows: a new allocation, {} rows copied into it, the old one freed", c.capacity, c.len - 1);
        println!("    a reference into the old one would point at freed memory: Rust won't let you keep one");
    }
    c.draw();

    println!("swap_remove row 0: the last row's 8 bytes move into row 0");
    c.swap_remove(0);
    c.draw();

    println!("ask for the rows as other types");
    let refused = |ok: bool| if ok { "a slice" } else { "refused" };
    println!("    as u32: {}, a u32 is 4 bytes and these rows are 8", refused(c.slice::<u32>().is_some()));
    println!("    as [u8; 8]: {}, the size is right, but it is aligned to 1, not 4", refused(c.slice::<[u8; 8]>().is_some()));

    let mut big = Column::new(size_of::<u32>(), align_of::<u32>());
    for i in 0..1000 {
        let row = big.push_zeroed();
        big.slice_mut::<u32>().expect("a u32 column")[row] = i;
    }
    let how = if DOUBLE { "doubling" } else { "one row at a time" };
    println!("1000 pushes into a u32 column, growing by {how}");
    println!("    {} grows, {} rows copied to make room", big.grows, big.copied);
    let sum: u32 = big.slice::<u32>().expect("a u32 column").iter().sum();
    println!("    and the rows are all there: they add up to {sum}");
}
