// Level 2: every function. The fields and the signatures are given; write the
// bodies. Each function says what it must do; the harness checks how.
//
// Start it with: python tools/dojo.py start free_list 2

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

#[path = "../dojo.rs"]
mod dojo;
// The harness: main hands it the program, and it checks the code below.
mod harness;

use dojo::{Dead, Handle};

use std::num::NonZeroU32;

pub struct Handles {
    // The life each index is in now, or will be in next if it is free.
    generations: Vec<NonZeroU32>,
    // Freed indices, waiting to be reused.
    free: Vec<u32>,
}

impl Handles {
    pub fn new() -> Handles {
        todo!("an empty allocator")
    }

    // A handle no one has had before. A freed index is reused before a new one
    // is made.
    pub fn alloc(&mut self) -> Handle {
        todo!("a handle no one has had before")
    }

    // After this, no handle to h's index is alive until alloc reuses the index.
    // A dead handle returns Err(Dead) and changes nothing.
    pub fn free(&mut self, h: Handle) -> Result<(), Dead> {
        todo!("free h")
    }

    pub fn alive(&self, h: Handle) -> bool {
        todo!("is h the life its index is in now?")
    }

    // How many handles are alive.
    pub fn live(&self) -> usize {
        todo!("live handles")
    }

    // How many indices have been handed out, alive or free.
    pub fn slots(&self) -> usize {
        todo!("indices so far")
    }
}

fn main() {
    harness::run();
}
