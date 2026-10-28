// Level 1: one function. Everything here works except free(). Write its body.
//
// Start it with: python tools/dojo.py start free_list 1

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
    // Freed indices, waiting to be reused. A stack: the last one freed is the
    // first one reused.
    free: Vec<u32>,
}

impl Handles {
    pub fn new() -> Handles {
        Handles { generations: Vec::new(), free: Vec::new() }
    }

    // A handle no one has had before: a freed index in its next life, or a
    // new index in its first.
    pub fn alloc(&mut self) -> Handle {
        let index = match self.free.pop() {
            Some(index) => index,
            None => {
                self.generations.push(NonZeroU32::MIN);
                self.generations.len() as u32 - 1
            }
        };
        Handle { index, generation: self.generations[index as usize] }
    }

    // The generation goes up now, not at the next alloc, so a free index
    // already holds a generation that no handle has.
    pub fn free(&mut self, h: Handle) -> Result<(), Dead> {
        todo!("level 1: free h, so no handle to it is alive again")
    }

    pub fn alive(&self, h: Handle) -> bool {
        self.generations.get(h.index as usize) == Some(&h.generation)
    }

    pub fn live(&self) -> usize {
        self.generations.len() - self.free.len()
    }

    pub fn slots(&self) -> usize {
        self.generations.len()
    }
}

fn main() {
    harness::run();
}
