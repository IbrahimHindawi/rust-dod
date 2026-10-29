// Level 1: one function. Everything here works except intern(). Write its body.
//
// Start it with: python tools/dojo.py start interning 1

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
        Interner { slots: vec![0; 8], keys: Vec::new() }
    }

    // Where `key` is in the table, or the empty slot where it would go: start
    // at its hash, and step to the next slot, wrapping, until one is either.
    fn slot_of(&self, key: u64) -> usize {
        let mask = self.slots.len() - 1;
        let mut at = hash(key) as usize & mask;
        while self.slots[at] != 0 && self.keys[self.slots[at] as usize - 1] != key {
            at = (at + 1) & mask;
        }
        at
    }

    pub fn find(&self, key: u64) -> Option<u32> {
        self.slots[self.slot_of(key)].checked_sub(1)
    }

    // Found, or made: a new key gets the next id, and the table grows before
    // it would be more than 3/4 full.
    pub fn intern(&mut self, key: u64) -> u32 {
        todo!("level 1: its id if it's in the table, or the next id, in its empty slot")
    }

    // Twice the slots, and every id put back where its key's hash says, which
    // in a bigger table is somewhere else.
    fn grow(&mut self) {
        self.slots = vec![0; self.slots.len() * 2];
        for id in 0..self.keys.len() {
            let at = self.slot_of(self.keys[id]);
            self.slots[at] = id as u32 + 1;
        }
    }

    pub fn key(&self, id: u32) -> u64 {
        self.keys[id as usize]
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }
}

fn main() {
    harness::run();
}
