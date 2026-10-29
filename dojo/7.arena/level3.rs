// Level 3: a blank page. Write the arena from memory.
//
// Start it with: python tools/dojo.py start arena 3
//
// The harness needs exactly this, and checks what each does. It owns the
// bytes, 16-aligned, and lends them to the arena.
//
//   pub struct Arena<'a>
//   Arena::new(bytes: &'a mut [u8]) -> Arena<'a>
//   push(&mut self, size: usize, align: usize) -> Option<usize>   an offset that is a multiple of
//                                                                align; None, and no change, when full
//   bytes(&self, offset: usize, len: usize) -> &[u8]
//   bytes_mut(&mut self, offset: usize, len: usize) -> &mut [u8]
//   used(&self) -> usize                                          padding included
//   capacity(&self) -> usize
//   mark(&self) -> usize
//   rewind(&mut self, mark: usize)                                let go of what came after mark
//   clear(&mut self)

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
