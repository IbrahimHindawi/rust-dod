// Level 2: every function. The fields and the signatures are given; write the
// bodies. Each function says what it must do; the harness checks how.
//
// Start it with: python tools/dojo.py start slot_map 2

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

#[path = "../dojo.rs"]
mod dojo;
// The harness: main hands it the program, and it checks the code below.
mod harness;

use dojo::Handle;

use std::num::NonZeroU32;

// One slot per index: the life it is in now (or next, if it is free), and the
// row its value is in while it is alive.
#[derive(Clone, Copy)]
struct Slot {
    generation: NonZeroU32,
    row: u32,
}

pub struct SlotMap<T> {
    slots: Vec<Slot>,
    free: Vec<u32>,
    // Packed: values[row] belongs to handles[row].
    values: Vec<T>,
    handles: Vec<Handle>,
}

impl<T> SlotMap<T> {
    pub fn new() -> SlotMap<T> {
        todo!("an empty map")
    }

    // The value goes in a new row at the end; its handle reuses a freed index
    // before making a new one.
    pub fn insert(&mut self, value: T) -> Handle {
        todo!("a new row, and a handle no one has had before")
    }

    // A live handle's value comes back, and no handle to it is alive again.
    // The rows stay packed. A dead handle gives None and changes nothing.
    pub fn remove(&mut self, h: Handle) -> Option<T> {
        todo!("free the index, swap-remove the row")
    }

    pub fn contains(&self, h: Handle) -> bool {
        todo!("is h alive?")
    }

    pub fn get(&self, h: Handle) -> Option<&T> {
        todo!("handle, slot, row, value")
    }

    pub fn get_mut(&mut self, h: Handle) -> Option<&mut T> {
        todo!("the same, to change")
    }

    pub fn len(&self) -> usize {
        todo!("how many values")
    }

    // The values, packed, in row order.
    pub fn values(&self) -> &[T] {
        todo!("the packed values")
    }

    // handles()[row] is the handle that owns values()[row].
    pub fn handles(&self) -> &[Handle] {
        todo!("the owner of each row")
    }
}

fn main() {
    harness::run();
}
