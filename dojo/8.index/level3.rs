// Level 3: a blank page. Write the list from memory.
//
// Start it with: python tools/dojo.py start index 3
//
// The harness needs exactly this, and checks what each does. A node is named
// by its index in the list's Vec, a u32.
//
//   pub struct List
//   List::new() -> List
//   push_back(&mut self, value: u64) -> u32                      the new node; a freed one first
//   push_front(&mut self, value: u64) -> u32
//   insert_after(&mut self, node: u32, value: u64) -> Option<u32>  None if node isn't in the list
//   remove(&mut self, node: u32) -> Option<u64>                  its index can be handed out again
//   get(&self, node: u32) -> Option<u64>
//   get_mut(&mut self, node: u32) -> Option<&mut u64>
//   head(&self) -> Option<u32>
//   tail(&self) -> Option<u32>
//   next(&self, node: u32) -> Option<u32>                        None after the tail, or for a free node
//   prev(&self, node: u32) -> Option<u32>                        None before the head, or for a free node
//   len(&self) -> usize

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
