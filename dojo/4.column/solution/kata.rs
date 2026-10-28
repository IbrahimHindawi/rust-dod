// The reference: one component's rows, kept as bytes, viewed as any plain-data
// type with the same size and alignment.

use crate::Pod;

// The storage is 16-byte blocks, so it starts on a 16-byte boundary and any
// row type with an alignment up to 16 lands where it may live.
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
    pub fn new(size: usize, align: usize) -> Column {
        assert!(align.is_power_of_two() && align <= 16, "a column holds alignments 1 to 16 that are powers of two, not {align}");
        Column { blocks: Vec::new(), size, align, len: 0, capacity: 0 }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn capacity(&self) -> usize {
        self.capacity
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

    // A new row at the end, zeroed. A full column doubles first: the rows are
    // copied to a bigger allocation, so anything that pointed into the old one
    // is stale, and the borrow checker won't let you keep such a thing.
    pub fn push_zeroed(&mut self) -> usize {
        if self.len == self.capacity {
            self.capacity = if self.capacity == 0 { 4 } else { self.capacity * 2 };
            self.blocks.resize((self.capacity * self.size).div_ceil(16), Block::default());
        }
        let row = self.len;
        self.len += 1;
        self.row_mut(row).fill(0);
        row
    }

    // The last row's bytes move into `row`, and the column is one row shorter.
    pub fn swap_remove(&mut self, row: usize) {
        assert!(row < self.len, "row {row} is past the {} rows in use", self.len);
        let (size, last) = (self.size, self.len - 1);
        self.bytes_mut().copy_within(last * size..(last + 1) * size, row * size);
        self.len = last;
    }

    pub fn row(&self, row: usize) -> &[u8] {
        let size = self.size;
        &self.bytes()[..self.len * size][row * size..(row + 1) * size]
    }

    pub fn row_mut(&mut self, row: usize) -> &mut [u8] {
        let (size, len) = (self.size, self.len);
        &mut self.bytes_mut()[..len * size][row * size..(row + 1) * size]
    }

    // The rows in use as T's. Only a T of the column's size and alignment: the
    // column can't know the type, but it can refuse every type that can't be
    // the one it was made for.
    #[allow(unsafe_code)]
    pub fn slice<T: Pod>(&self) -> &[T] {
        self.check::<T>();
        let bytes = &self.bytes()[..self.len * self.size];
        // SAFETY: the bytes are `len` rows of T's size, starting on a 16-byte
        // boundary, and T needs at most 16; T is Pod, so any bytes are a valid
        // T.
        unsafe { std::slice::from_raw_parts(bytes.as_ptr().cast::<T>(), self.len) }
    }

    #[allow(unsafe_code)]
    pub fn slice_mut<T: Pod>(&mut self) -> &mut [T] {
        self.check::<T>();
        let (size, len) = (self.size, self.len);
        let bytes = &mut self.bytes_mut()[..len * size];
        // SAFETY: as in slice(). T has no padding, so writing a T leaves every
        // byte initialised, and the byte views stay valid.
        unsafe { std::slice::from_raw_parts_mut(bytes.as_mut_ptr().cast::<T>(), len) }
    }

    fn check<T: Pod>(&self) {
        assert!(
            size_of::<T>() == self.size && align_of::<T>() == self.align,
            "a column of {}-byte rows aligned to {} read as a type of {} bytes aligned to {}",
            self.size, self.align, size_of::<T>(), align_of::<T>()
        );
    }
}
