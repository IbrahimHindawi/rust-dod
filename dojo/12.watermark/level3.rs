// Level 3: a blank page. Write the query from memory.
//
// Start it with: python tools/dojo.py start watermark 3
//
// The harness needs exactly this, and checks what each does. A table is its
// signature, a u64 with a bit per component, and the world's tables are a
// slice of them that only ever grows at the end.
//
//   pub struct Query
//   Query::new(all: u64, none: u64) -> Query        has looked at nothing yet
//   update(&mut self, signatures: &[u64])           finds the matching tables among
//                                                   the ones it hasn't looked at
//   tables(&self) -> &[u32]                         the matching tables, in increasing order
//   checked(&self) -> usize                         how many tables it has been updated over
//   scanned(&self) -> usize                         how many update() has looked at, in all;
//                                                   each table once
//
// A table matches when its signature has every bit of `all` and no bit of
// `none`.

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
