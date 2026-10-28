// Level 3: a blank page. Write the column from memory.
//
// Start it with: python tools/dojo.py start column 3
//
// The harness needs exactly this, and checks what each does:
//
//   use crate::Pod;                   unsafe trait Pod: Copy + 'static, plain data
//
//   pub struct Column
//   Column::new(size: usize, align: usize) -> Column
//                                     panics unless align is a power of two, 1 to 16
//   len(&self) -> usize
//   capacity(&self) -> usize          rows that fit before it grows
//   push_zeroed(&mut self) -> usize   a zeroed row at the end, its number; a full
//                                     column grows by at least half
//   swap_remove(&mut self, row: usize)
//   row(&self, row: usize) -> &[u8]   panics past the rows in use
//   row_mut(&mut self, row: usize) -> &mut [u8]
//   slice<T: Pod>(&self) -> &[T]      panics unless T has the column's size and
//   slice_mut<T: Pod>(&mut self) -> &mut [T]           alignment
//
// Rows of zero bytes, tags, are in the scenario too. The harness denies unsafe
// code except in functions marked #[allow(unsafe_code)].
