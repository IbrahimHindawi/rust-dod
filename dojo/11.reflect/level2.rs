// Level 2: every function. The signatures are given; write the bodies. Each
// function says what it must do; the harness checks how.
//
// Start it with: python tools/dojo.py start reflect 2

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

#[path = "../dojo.rs"]
mod dojo;
// The harness: main hands it the program, and it checks the code below.
mod harness;

use harness::{Field, Info};

// Every field a tool shows, depth first in the record's order: its path, and
// where its bytes are in the outermost value. A struct field is walked into
// rather than listed; a hidden one is left out, with everything inside it.
fn leaves(info: &Info, prefix: &str, base: usize, out: &mut Vec<(String, usize, usize)>) {
    todo!("push (path, offset, size) for every shown field that isn't a struct")
}

// The paths of the fields a tool shows: "at.x", "at.y".
pub fn paths(info: &Info) -> Vec<String> {
    todo!("the leaves' paths")
}

// The paths, of those, whose bytes differ between two values of the type.
pub fn diff(info: &Info, a: &[u8], b: &[u8]) -> Vec<String> {
    todo!("the leaves whose bytes differ")
}

// The field a path names, hidden or not, and where its bytes start in the
// outermost value.
fn find(info: &Info, path: &str) -> Option<(usize, &'static Field)> {
    todo!("look each name up in the record of the field before it, adding offsets")
}

// The f32 a path names, or None if it names something else, or nothing.
pub fn get_f32(info: &Info, bytes: &[u8], path: &str) -> Option<f32> {
    todo!("find it, check it's an f32, read 4 bytes")
}

// Write the f32 a path names. False, and nothing written, if it names
// something else, or nothing.
pub fn set_f32(info: &Info, bytes: &mut [u8], path: &str, value: f32) -> bool {
    todo!("find it, check it's an f32, write 4 bytes")
}

fn main() {
    harness::run();
}
