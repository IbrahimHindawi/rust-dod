// The reference: a bump arena over a buffer it borrows. One number, `used`,
// is all it keeps: a push rounds it up and moves it on, a rewind or a clear
// moves it back.

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
    // An arena over the caller's bytes. Offsets are aligned from the start of
    // them, so the start should be aligned to the most a push will ask for;
    // the harness's is 16-aligned.
    pub fn new(bytes: &'a mut [u8]) -> Arena<'a> {
        Arena { bytes, used: 0 }
    }

    // `size` bytes at the first multiple of `align`, a power of two, at or
    // after `used`. None, and nothing changes, if they would end past the
    // last byte.
    pub fn push(&mut self, size: usize, align: usize) -> Option<usize> {
        let offset = (self.used + align - 1) & !(align - 1);
        if offset + size > self.bytes.len() {
            return None;
        }
        self.used = offset + size;
        Some(offset)
    }

    pub fn bytes(&self, offset: usize, len: usize) -> &[u8] {
        &self.bytes[offset..offset + len]
    }

    pub fn bytes_mut(&mut self, offset: usize, len: usize) -> &mut [u8] {
        &mut self.bytes[offset..offset + len]
    }

    pub fn used(&self) -> usize {
        self.used
    }

    pub fn capacity(&self) -> usize {
        self.bytes.len()
    }

    // Where the arena is now, to come back to.
    pub fn mark(&self) -> usize {
        self.used
    }

    // Let go of everything pushed since `mark`.
    pub fn rewind(&mut self, mark: usize) {
        self.used = mark;
    }

    pub fn clear(&mut self) {
        self.used = 0;
    }
}

fn main() {
    harness::run();
}
