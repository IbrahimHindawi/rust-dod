// Level 2: every function. The fields and the signatures are given; write the
// bodies. Each function says what it must do; the harness checks how.
//
// Start it with: python tools/dojo.py start column 2

use crate::Pod;

// The storage is 16-byte blocks, so it starts on a 16-byte boundary.
#[derive(Clone, Copy, Default)]
#[repr(C, align(16))]
struct Block([u8; 16]);

pub struct Column {
    blocks: Vec<Block>,
    size: usize,
    align: usize,
    len: usize,
    capacity: usize,
}

impl Column {
    // An empty column of rows `size` bytes long, aligned to `align`. Panics
    // unless align is a power of two from 1 to 16.
    pub fn new(size: usize, align: usize) -> Column {
        todo!("an empty column")
    }

    pub fn len(&self) -> usize {
        todo!("rows in use")
    }

    // How many rows fit before the column has to grow.
    pub fn capacity(&self) -> usize {
        todo!("rows that fit")
    }

    // Every byte of the blocks, as bytes: one unsafe line. The harness denies
    // unsafe code everywhere but in functions that allow it by name.
    #[allow(unsafe_code)]
    fn bytes(&self) -> &[u8] {
        todo!("the blocks as bytes")
    }

    #[allow(unsafe_code)]
    fn bytes_mut(&mut self) -> &mut [u8] {
        todo!("the blocks as bytes, to write")
    }

    // A new zeroed row at the end; returns its row number. A full column grows
    // first, by at least half.
    pub fn push_zeroed(&mut self) -> usize {
        todo!("a new zeroed row")
    }

    // The last row's bytes move into `row`, and the column is one row shorter.
    pub fn swap_remove(&mut self, row: usize) {
        todo!("swap-remove a row")
    }

    // One row's bytes. Panics past the rows in use.
    pub fn row(&self, row: usize) -> &[u8] {
        todo!("a row's bytes")
    }

    pub fn row_mut(&mut self, row: usize) -> &mut [u8] {
        todo!("a row's bytes, to write")
    }

    // The rows in use as T's. Panics unless T has the column's size and
    // alignment.
    #[allow(unsafe_code)]
    pub fn slice<T: Pod>(&self) -> &[T] {
        todo!("the rows as a &[T]")
    }

    #[allow(unsafe_code)]
    pub fn slice_mut<T: Pod>(&mut self) -> &mut [T] {
        todo!("the rows as a &mut [T]")
    }
}
