// Level 2: every function. The fields and the signatures are given; write the
// bodies. Each function says what it must do; the harness checks how.
//
// Start it with: python tools/dojo.py start swap_remove 2

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

#[path = "../dojo.rs"]
mod dojo;
// The harness: main hands it the program, and it checks the code below.
mod harness;

pub struct SparseSet<T> {
    // Packed: values[row] belongs to ids[row]. No holes, in no order.
    values: Vec<T>,
    ids: Vec<u32>,
    // Sparse: rows[id] is the row id's value is in, or None.
    rows: Vec<Option<u32>>,
}

impl<T> SparseSet<T> {
    pub fn new() -> SparseSet<T> {
        todo!("an empty set")
    }

    // Give `id` a value. If it has one already, the new value replaces it and
    // the old one comes back; otherwise it goes in a new row at the end.
    pub fn insert(&mut self, id: u32, value: T) -> Option<T> {
        todo!("insert or replace")
    }

    // Take id's value out. The rows stay packed, and every id's row stays right.
    pub fn remove(&mut self, id: u32) -> Option<T> {
        todo!("swap-remove")
    }

    pub fn get(&self, id: u32) -> Option<&T> {
        todo!("id's value, through its row")
    }

    pub fn get_mut(&mut self, id: u32) -> Option<&mut T> {
        todo!("id's value, to change")
    }

    pub fn len(&self) -> usize {
        todo!("how many ids have a value")
    }

    // The values, packed, in row order.
    pub fn values(&self) -> &[T] {
        todo!("the packed values")
    }

    // ids()[row] is the id that owns values()[row].
    pub fn ids(&self) -> &[u32] {
        todo!("the owner of each row")
    }
}

fn main() {
    harness::run();
}
