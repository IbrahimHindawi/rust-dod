// Level 3: a blank page. Write the allocator from memory.
//
// Start it with: python tools/dojo.py start free_list 3
//
// The harness needs exactly this, and checks what each does:
//
//   Given, imported below from dojo.rs: Dead, and
//   Handle { index: u32, generation: NonZeroU32 }
//
//   pub struct Handles
//   Handles::new() -> Handles
//   alloc(&mut self) -> Handle                   reuses a freed index first
//   free(&mut self, h: Handle) -> Result<(), Dead>
//   alive(&self, h: Handle) -> bool
//   live(&self) -> usize                         handles alive now
//   slots(&self) -> usize                        indices handed out, alive or free

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

#[path = "../dojo.rs"]
mod dojo;
// The harness: main hands it the program, and it checks the code below.
mod harness;

use dojo::{Dead, Handle};

// Your code here.

fn main() {
    harness::run();
}
