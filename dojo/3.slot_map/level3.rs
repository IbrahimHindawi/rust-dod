// Level 3: a blank page. Write the slot map from memory.
//
// Start it with: python tools/dojo.py start slot_map 3
//
// The harness needs exactly this, and checks what each does:
//
//   Given, imported below from dojo.rs:
//   Handle { index: u32, generation: NonZeroU32 }
//
//   pub struct SlotMap<T>
//   SlotMap::new() -> SlotMap<T>
//   insert(&mut self, value: T) -> Handle          reuses a freed index first
//   remove(&mut self, h: Handle) -> Option<T>      keeps the rows packed
//   contains(&self, h: Handle) -> bool
//   get(&self, h: Handle) -> Option<&T>
//   get_mut(&mut self, h: Handle) -> Option<&mut T>
//   len(&self) -> usize
//   values(&self) -> &[T]                          packed, in row order
//   handles(&self) -> &[Handle]                    handles()[row] owns values()[row]

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

#[path = "../dojo.rs"]
mod dojo;
// The harness: main hands it the program, and it checks the code below.
mod harness;

use dojo::Handle;

// Your code here.

fn main() {
    harness::run();
}
