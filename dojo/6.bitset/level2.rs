// Level 2: every function. The signatures are given; write the bodies. Each
// function says what it must do; the harness checks how.
//
// Start it with: python tools/dojo.py start bitset 2

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

#[path = "../dojo.rs"]
mod dojo;
// The harness: main hands it the program, and it checks the code below.
mod harness;

// The set holding only `id`, for ids 0 to 63.
pub fn bit(id: u32) -> u64 {
    todo!("one bit")
}

pub fn with(set: u64, id: u32) -> u64 {
    todo!("the set, and id")
}

pub fn without(set: u64, id: u32) -> u64 {
    todo!("the set, but not id")
}

pub fn has(set: u64, id: u32) -> bool {
    todo!("is id in the set?")
}

// Every id of `wanted` is in `set`. An empty `wanted` is in every set.
pub fn contains_all(set: u64, wanted: u64) -> bool {
    todo!("all of wanted, not any of it")
}

pub fn intersects(a: u64, b: u64) -> bool {
    todo!("an id in both?")
}

// A query: all of `all`, and none of `none`.
pub fn matches(set: u64, all: u64, none: u64) -> bool {
    todo!("all of all, none of none")
}

pub fn count(set: u64) -> u32 {
    todo!("how many ids")
}

// The ids in the set, smallest first.
pub fn ids(set: u64) -> Vec<u32> {
    todo!("each id, in order")
}

fn main() {
    harness::run();
}
