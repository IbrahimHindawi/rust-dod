// Dojo 7, the playground: an arena.
//
// An arena is one buffer and one number: how much of the buffer is used. A
// push rounds that number up to the alignment asked for, hands out the
// offset, and moves the number past the new bytes. Nothing is freed on its
// own. A mark remembers the number, a rewind goes back to it, and a clear goes
// back to 0, and everything pushed since is gone at once, for the cost of one
// store. That is all the memory a frame of a game needs.
//
// This program pushes things of different sizes and alignments into a 64-byte
// buffer, rewinds, fills it to the last byte, and clears it. After each step it
// draws the buffer, 16 bytes to a group: a letter for each byte of a push, -
// for padding, . for free. Each push is filled with its letter, and verify()
// checks the arena's invariant and that every push still holds its letter;
// break inside its `if`s to stop on the first thing wrong.
//
// The switch: set ALIGN_OFFSETS to false and run it again. A push then hands
// out the used count as it is, without rounding it up, and the u32, the u64
// and the 16-byte vector land at addresses they can't be read from as what
// they are. It's a static, read each time it's used, so a debugger can flip
// it mid-run too: stop on `push`, set it to false in the watch window, and
// continue.

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

use std::sync::atomic::{AtomicBool, Ordering::Relaxed};

static ALIGN_OFFSETS: AtomicBool = AtomicBool::new(true);

// 64 bytes, starting on a 16-byte boundary, so an offset that is a multiple
// of 16, or of anything less, is an address that is one too.
#[repr(C, align(16))]
struct Buffer([u8; 64]);

struct Arena {
    buffer: Buffer,
    // Bytes before `used` are handed out; bytes after it are free.
    used: usize,
}

impl Arena {
    // `size` bytes at a multiple of `align`, a power of two, or None if they
    // don't fit before the end.
    fn push(&mut self, size: usize, align: usize) -> Option<usize> {
        let offset = if ALIGN_OFFSETS.load(Relaxed) { (self.used + align - 1) & !(align - 1) } else { self.used };
        if offset + size > self.buffer.0.len() {
            return None;
        }
        self.used = offset + size;
        Some(offset)
    }

    fn mark(&self) -> usize {
        self.used
    }

    fn rewind(&mut self, mark: usize) {
        self.used = mark;
    }

    fn clear(&mut self) {
        self.used = 0;
    }
}

// What the program has pushed and not yet let go of, to draw and to check:
// up to eight pushes, each a letter, an offset, a size and an alignment.
#[derive(Clone, Copy)]
struct Push {
    letter: u8,
    offset: usize,
    size: usize,
    align: usize,
}

struct Pushes {
    live: [Push; 8],
    count: usize,
}

fn push(arena: &mut Arena, pushes: &mut Pushes, letter: u8, size: usize, align: usize) {
    let before = arena.used;
    match arena.push(size, align) {
        Some(offset) => {
            arena.buffer.0[offset..offset + size].fill(letter);
            pushes.live[pushes.count] = Push { letter, offset, size, align };
            pushes.count += 1;
            let padding = offset - before;
            let note = match padding {
                0 if arena.used == 64 => ": the last byte".to_string(),
                0 => String::new(),
                1 => ": 1 byte of padding first".to_string(),
                n => format!(": {n} bytes of padding first"),
            };
            println!("    offset {offset}, used {} of 64{note}", arena.used);
        }
        None => {
            let unit = if size == 1 { "byte" } else { "bytes" };
            println!("    no room for {size} {unit} after {before} of 64");
        }
    }
}

// Rewinding or clearing lets go of every push from after the mark.
fn let_go(arena: &Arena, pushes: &mut Pushes) {
    while pushes.count > 0 && pushes.live[pushes.count - 1].offset >= arena.used {
        pushes.count -= 1;
    }
}

fn draw(arena: &Arena, pushes: &Pushes) {
    let mut cells = [b'.'; 64];
    cells[..arena.used.min(64)].fill(b'-');
    for p in &pushes.live[..pushes.count] {
        cells[p.offset..p.offset + p.size].fill(p.letter);
    }
    let groups: Vec<&str> = cells.chunks(16).map(|g| std::str::from_utf8(g).unwrap_or("?")).collect();
    println!("    {}", groups.join("|"));
}

// The invariant: every push is aligned, inside the used bytes, clear of the
// one after it, and still holds its letter.
fn verify(arena: &Arena, pushes: &Pushes) {
    if arena.used > 64 {
        println!("    verify: used is {}, past the end of the buffer", arena.used);
    }
    let start = arena.buffer.0.as_ptr() as usize;
    for (i, p) in pushes.live[..pushes.count].iter().enumerate() {
        let name = p.letter as char;
        if !(start + p.offset).is_multiple_of(p.align) {
            println!("    verify: {name} is at offset {}, not a multiple of {}: its address isn't aligned", p.offset, p.align);
        }
        if p.offset + p.size > arena.used {
            println!("    verify: {name} ends at {}, past the used count, {}", p.offset + p.size, arena.used);
        }
        if let Some(next) = pushes.live[..pushes.count].get(i + 1)
            && p.offset + p.size > next.offset
        {
            println!("    verify: {name} runs into {}", next.letter as char);
        }
        if arena.buffer.0[p.offset..p.offset + p.size].iter().any(|&b| b != p.letter) {
            println!("    verify: {name}'s bytes have been written over");
        }
    }
}

fn main() {
    let mut arena = Arena { buffer: Buffer([0; 64]), used: 0 };
    let mut pushes = Pushes { live: [Push { letter: 0, offset: 0, size: 0, align: 1 }; 8], count: 0 };
    println!("a 64-byte buffer, 16 bytes to a group: a letter per byte pushed, - for padding, . for free");

    println!("push a: a u8 flag, 1 byte, align 1");
    push(&mut arena, &mut pushes, b'a', 1, 1);
    draw(&arena, &pushes);
    verify(&arena, &pushes);

    println!("push b: a u32 count, 4 bytes, align 4");
    push(&mut arena, &mut pushes, b'b', 4, 4);
    draw(&arena, &pushes);
    verify(&arena, &pushes);

    println!("push c: a u64 id, 8 bytes, align 8");
    push(&mut arena, &mut pushes, b'c', 8, 8);
    draw(&arena, &pushes);
    verify(&arena, &pushes);

    println!("push d: four f32s for SIMD, 16 bytes, align 16");
    push(&mut arena, &mut pushes, b'd', 16, 16);
    draw(&arena, &pushes);
    verify(&arena, &pushes);

    let mark = arena.mark();
    println!("mark: remember used, {mark}");

    println!("push e: a name, 3 bytes, align 1");
    push(&mut arena, &mut pushes, b'e', 3, 1);
    draw(&arena, &pushes);
    verify(&arena, &pushes);

    println!("push f: a u64 time, 8 bytes, align 8");
    push(&mut arena, &mut pushes, b'f', 8, 8);
    draw(&arena, &pushes);
    verify(&arena, &pushes);

    println!("rewind to the mark: e and f are gone at once");
    arena.rewind(mark);
    let_go(&arena, &mut pushes);
    draw(&arena, &pushes);
    verify(&arena, &pushes);

    println!("push g: a path, 40 bytes, align 8");
    push(&mut arena, &mut pushes, b'g', 40, 8);
    draw(&arena, &pushes);
    verify(&arena, &pushes);

    println!("push h: a 2x4 matrix of f32s, 32 bytes, align 16");
    push(&mut arena, &mut pushes, b'h', 32, 16);
    draw(&arena, &pushes);
    verify(&arena, &pushes);

    println!("push i: one more byte");
    push(&mut arena, &mut pushes, b'i', 1, 1);
    draw(&arena, &pushes);
    verify(&arena, &pushes);

    println!("clear: the frame is over, and everything goes at once");
    arena.clear();
    let_go(&arena, &mut pushes);
    draw(&arena, &pushes);
    verify(&arena, &pushes);

    println!("push j: the next frame's first u64, over a's old bytes");
    push(&mut arena, &mut pushes, b'j', 8, 8);
    draw(&arena, &pushes);
    verify(&arena, &pushes);
}
