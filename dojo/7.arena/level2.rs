// Level 2: every function. The fields and the signatures are given; write the
// bodies. Each function says what it must do; the harness checks how.
//
// Start it with: python tools/dojo.py start arena 2

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

#[path = "../dojo.rs"]
mod dojo;
// The harness: main hands it the program, and it checks the code below.
mod harness;

pub struct Arena<'a> {
    bytes: &'a mut [u8],
    // Bytes before `used` are handed out; bytes after it are free.
    used: usize,
}

impl<'a> Arena<'a> {
    // An arena over the caller's bytes, which start 16-aligned.
    pub fn new(bytes: &'a mut [u8]) -> Arena<'a> {
        todo!("nothing used yet")
    }

    // `size` bytes at the first multiple of `align`, a power of two, at or
    // after `used`. None, and nothing changes, if they would end past the
    // last byte.
    pub fn push(&mut self, size: usize, align: usize) -> Option<usize> {
        todo!("an aligned offset, if it fits")
    }

    pub fn bytes(&self, offset: usize, len: usize) -> &[u8] {
        todo!("len bytes from offset, to read")
    }

    pub fn bytes_mut(&mut self, offset: usize, len: usize) -> &mut [u8] {
        todo!("the same, to write")
    }

    pub fn used(&self) -> usize {
        todo!("how many bytes are handed out, padding included")
    }

    pub fn capacity(&self) -> usize {
        todo!("how many bytes there are")
    }

    // Where the arena is now, to come back to.
    pub fn mark(&self) -> usize {
        todo!("something rewind can use")
    }

    // Let go of everything pushed since `mark`.
    pub fn rewind(&mut self, mark: usize) {
        todo!("back to the mark")
    }

    pub fn clear(&mut self) {
        todo!("let go of everything")
    }
}

fn main() {
    harness::run();
}
