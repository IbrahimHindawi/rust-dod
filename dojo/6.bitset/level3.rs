// Level 3: a blank page. Write the bitset functions from memory.
//
// Start it with: python tools/dojo.py start bitset 3
//
// The harness needs exactly this, and checks what each does. A set is a u64,
// bit `id` set for each id in it, ids 0 to 63.
//
//   pub fn bit(id: u32) -> u64                                the set of id alone
//   pub fn with(set: u64, id: u32) -> u64
//   pub fn without(set: u64, id: u32) -> u64
//   pub fn has(set: u64, id: u32) -> bool
//   pub fn contains_all(set: u64, wanted: u64) -> bool        every id of wanted is in set
//   pub fn intersects(a: u64, b: u64) -> bool                 an id in both
//   pub fn matches(set: u64, all: u64, none: u64) -> bool     all of all, none of none
//   pub fn count(set: u64) -> u32
//   pub fn ids(set: u64) -> Vec<u32>                          smallest first

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
