// Level 3: a blank page. Write the interner from memory, its hash table too.
//
// Start it with: python tools/dojo.py start interning 3
//
// The harness needs exactly this, and checks what each does. Write the table
// yourself, without std's HashMap: a power-of-two Vec of slots, linear
// probing, grown before it's 3/4 full.
//
//   pub struct Interner
//   Interner::new() -> Interner
//   intern(&mut self, key: u64) -> u32         its id; a new key gets the next one, from 0
//   find(&self, key: u64) -> Option<u32>       None if it was never interned
//   key(&self, id: u32) -> u64                 the key interned as id
//   len(&self) -> usize                        how many keys

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
