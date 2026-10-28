// Level 3: a blank page. Write the sparse set from memory.
//
// Start it with: python tools/dojo.py start swap_remove 3
//
// The harness needs exactly this, and checks what each does:
//
//   pub struct SparseSet<T>
//   SparseSet::new() -> SparseSet<T>
//   insert(&mut self, id: u32, value: T) -> Option<T>   replaces, returning the old value
//   remove(&mut self, id: u32) -> Option<T>             keeps the rows packed
//   get(&self, id: u32) -> Option<&T>
//   get_mut(&mut self, id: u32) -> Option<&mut T>
//   len(&self) -> usize
//   values(&self) -> &[T]                               packed, in row order
//   ids(&self) -> &[u32]                                ids()[row] owns values()[row]

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

#[path = "../dojo.rs"]
mod dojo;
// The harness: main hands it the program, and it checks the code below.
mod harness;

// Your code here.

fn main() {
    harness::run();
}
