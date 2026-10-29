// Level 2: every function. The fields and the signatures are given; write the
// bodies. Each function says what it must do; the harness checks how.
//
// Start it with: python tools/dojo.py start interning 2

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

#[path = "../dojo.rs"]
mod dojo;
// The harness: main hands it the program, and it checks the code below.
mod harness;

// splitmix64's finisher: every bit of the key moves every bit of the hash, so
// keys that differ only in their high bits still land far apart.
fn hash(key: u64) -> u64 {
    let mut x = key;
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58476D1CE4E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D049BB133111EB);
    x ^ (x >> 31)
}

pub struct Interner {
    // The table: a power of two long, never more than 3/4 full. 0 is an empty
    // slot, and anything else is an id + 1.
    slots: Vec<u32>,
    // keys[id] is the key interned as id.
    keys: Vec<u64>,
}

impl Interner {
    pub fn new() -> Interner {
        todo!("an empty table of 8 slots")
    }

    // Where `key` is in the table, or the empty slot where it would go.
    fn slot_of(&self, key: u64) -> usize {
        todo!("start at its hash, step to the next slot, wrapping")
    }

    // The key's id, or None if it was never interned.
    pub fn find(&self, key: u64) -> Option<u32> {
        todo!("look, don't make")
    }

    // The key's id: the one it was given before, or, for a new key, the next
    // one. The table grows before it would be more than 3/4 full.
    pub fn intern(&mut self, key: u64) -> u32 {
        todo!("found, or made")
    }

    // Twice the slots, and every id put back where its key's hash says.
    fn grow(&mut self) {
        todo!("a bigger table, and every id in it again")
    }

    pub fn key(&self, id: u32) -> u64 {
        todo!("the key interned as id")
    }

    pub fn len(&self) -> usize {
        todo!("how many keys")
    }
}

fn main() {
    harness::run();
}
