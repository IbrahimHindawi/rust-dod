// The reference: a set of ids, 0 to 63, as the bits of a u64. Each function
// is one or two bit operations; the block is knowing which.

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

#[path = "../dojo.rs"]
mod dojo;
// The harness: main hands it the program, and it checks the code below.
mod harness;

// The set holding only `id`. The 1 is a u64, so ids 32 to 63 have a bit too.
pub fn bit(id: u32) -> u64 {
    1u64 << id
}

pub fn with(set: u64, id: u32) -> u64 {
    set | bit(id)
}

pub fn without(set: u64, id: u32) -> u64 {
    set & !bit(id)
}

pub fn has(set: u64, id: u32) -> bool {
    set & bit(id) != 0
}

// Every id of `wanted` is in `set`: masking `set` with `wanted` leaves all of
// `wanted`. An empty `wanted` is in every set.
pub fn contains_all(set: u64, wanted: u64) -> bool {
    set & wanted == wanted
}

pub fn intersects(a: u64, b: u64) -> bool {
    a & b != 0
}

// A query: all of `all`, and none of `none`.
pub fn matches(set: u64, all: u64, none: u64) -> bool {
    contains_all(set, all) && !intersects(set, none)
}

pub fn count(set: u64) -> u32 {
    set.count_ones()
}

// The ids in the set, smallest first.
pub fn ids(set: u64) -> Vec<u32> {
    (0..64).filter(|&id| has(set, id)).collect()
}

fn main() {
    harness::run();
}
