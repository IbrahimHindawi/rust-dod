// The harness for dojo 7, the arena. It is fixed. Your work is kata.rs,
// beside it: kata.rs brings this file in as a module and its main calls
// run(), so the two are one program. The reference, solution.rs, and each
// level file do the same, so each of them builds on its own, and this file
// never does. Build and run yours with
//
//   python tools/dojo.py arena
//
// First a short scenario, the playground's steps, each printed as it passes.
// Then a fuzz run: thousands of random pushes, marks, rewinds and clears from
// a seed, against a model that keeps the used count and every push not yet
// let go of. Each push is filled with a pattern through bytes_mut, and read
// back through bytes, so a push that lands on another shows.

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

use crate::Arena;
use crate::dojo::{Rng, Run};

// The buffers the arena borrows, starting on a 16-byte boundary.
#[repr(C, align(16))]
struct Small([u8; 64]);

#[repr(C, align(16))]
struct Buffer([u8; 256]);

fn show(offset: Option<usize>) -> String {
    offset.map_or("None".to_string(), |o| o.to_string())
}

fn scenario(run: &Run) {
    println!("scenario");
    let mut small = Small([0; 64]);
    let mut a = Arena::new(&mut small.0);
    run.same("capacity", a.capacity(), "64");
    run.same("push 1 byte, align 1", show(a.push(1, 1)), "0");
    run.same("push 4 bytes, align 4: 3 bytes of padding first", show(a.push(4, 4)), "4");
    run.same("push 8 bytes, align 8", show(a.push(8, 8)), "8");
    run.same("push 16 bytes, align 16", show(a.push(16, 16)), "16");
    run.same("used", a.used(), "32");
    let mark = a.mark();
    run.same("mark, then push 3 bytes, align 1", show(a.push(3, 1)), "32");
    run.same("push 8 bytes, align 8: 5 bytes of padding first", show(a.push(8, 8)), "40");
    run.same("used", a.used(), "48");
    a.rewind(mark);
    run.same("rewind to the mark: used", a.used(), "32");
    run.same("push 40 bytes, align 8: past the end", show(a.push(40, 8)), "None");
    run.same("used, after a push that didn't fit", a.used(), "32");
    run.same("push 32 bytes, align 16: to the last byte", show(a.push(32, 16)), "32");
    run.same("push 1 byte: full", show(a.push(1, 1)), "None");
    a.clear();
    run.same("clear: used", a.used(), "0");
    run.same("push 8 bytes, align 8", show(a.push(8, 8)), "0");
    a.bytes_mut(0, 8).copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
    let read: Vec<String> = a.bytes(0, 8).iter().map(|b| b.to_string()).collect();
    run.same("write 1 to 8 with bytes_mut, read them with bytes", read.join(" "), "1 2 3 4 5 6 7 8");
}

// A push the model holds: where, how big, and the pattern written into it,
// byte k being tag + k.
struct Live {
    offset: usize,
    size: usize,
    tag: u8,
    op: u64,
}

// A mark: the arena's, and the model's used count and number of pushes then.
struct Mark {
    mark: usize,
    used: usize,
    live: usize,
}

fn check_pattern(run: &Run, op: u64, a: &Arena, p: &Live) {
    let got = a.bytes(p.offset, p.size);
    run.check(op, got.len() == p.size, || format!("bytes({}, {}) gave {} bytes", p.offset, p.size, got.len()));
    for (k, &b) in got.iter().enumerate() {
        let want = p.tag.wrapping_add(k as u8);
        run.check(op, b == want, || {
            format!(
                "the {} bytes at {}, pushed at operation {}, have changed: byte {k} is {b}, it should be {want}",
                p.size, p.offset, p.op
            )
        });
    }
}

fn fuzz(run: &Run) {
    run.fuzz();
    let mut rng = Rng::new(run.seed);
    let mut buffer = Buffer([0; 256]);
    let mut a = Arena::new(&mut buffer.0);
    let cap = 256;
    run.check(0, a.capacity() == cap, || format!("capacity() is {}, the buffer has {cap} bytes", a.capacity()));
    let mut used: usize = 0;
    let mut live: Vec<Live> = Vec::new();
    let mut marks: Vec<Mark> = Vec::new();
    for op in 1..=run.ops {
        let roll = rng.below(100);
        if roll < 58 {
            let align: usize = 1 << rng.below(5);
            let rounded = used.div_ceil(align) * align;
            // Mostly small pushes; some that end exactly at the last byte, and
            // some one byte past it.
            let size = if roll < 50 {
                1 + rng.below(24) as usize
            } else if roll < 55 {
                (cap.saturating_sub(rounded)).max(1)
            } else {
                cap.saturating_sub(rounded) + 1
            };
            let fits = rounded + size <= cap;
            run.before(op, format_args!("push({size}, {align}) with {used} of {cap} used"));
            let got = a.push(size, align);
            match got {
                Some(offset) => {
                    run.check(op, fits, || {
                        format!(
                            "push({size}, {align}) with {used} used gave offset {offset}, but there is no room: \
                             it would end at {}, past the {cap} bytes there are",
                            rounded + size
                        )
                    });
                    run.check(op, offset % align == 0, || {
                        format!("push({size}, {align}) gave offset {offset}, which isn't a multiple of {align}")
                    });
                    run.check(op, offset == rounded, || {
                        format!(
                            "push({size}, {align}) with {used} used gave offset {offset}; the first multiple of \
                             {align} at or after {used} is {rounded}"
                        )
                    });
                    let tag = op as u8;
                    let place = a.bytes_mut(offset, size);
                    run.check(op, place.len() == size, || format!("bytes_mut({offset}, {size}) gave {} bytes", place.len()));
                    for (k, b) in place.iter_mut().enumerate() {
                        *b = tag.wrapping_add(k as u8);
                    }
                    live.push(Live { offset, size, tag, op });
                    used = rounded + size;
                }
                None => run.check(op, !fits, || {
                    format!("push({size}, {align}) with {used} of {cap} used gave None, but it fits at {rounded} and ends at {}", rounded + size)
                }),
            }
            run.check(op, a.used() == used, || format!("after push({size}, {align}), used() is {}, it should be {used}", a.used()));
        } else if roll < 66 || (roll < 74 && marks.is_empty()) {
            run.before(op, format_args!("mark, with {used} used"));
            marks.push(Mark { mark: a.mark(), used, live: live.len() });
        } else if roll < 74 {
            let at = rng.below(marks.len() as u64) as usize;
            marks.truncate(at + 1);
            let m = &marks[at];
            run.before(op, format_args!("rewind to a mark from {} used, with {used} used", m.used));
            a.rewind(m.mark);
            used = m.used;
            live.truncate(m.live);
            run.check(op, a.used() == used, || format!("after the rewind, used() is {}, it should be {used}", a.used()));
        } else if roll < 77 {
            run.before(op, format_args!("clear, with {used} used"));
            a.clear();
            used = 0;
            live.clear();
            marks.clear();
            run.check(op, a.used() == 0, || format!("after clear, used() is {}", a.used()));
        } else if !live.is_empty() {
            let p = &live[rng.below(live.len() as u64) as usize];
            run.before(op, format_args!("read the {} bytes at {}", p.size, p.offset));
            check_pattern(run, op, &a, p);
        }
        if op % 64 == 0 {
            for p in &live {
                check_pattern(run, op, &a, p);
            }
        }
    }
    for p in &live {
        check_pattern(run, run.ops, &a, p);
    }
    run.pass();
}

// The kata's main calls this.
pub fn run() {
    let run = Run::new("arena", "7. an arena");
    scenario(&run);
    fuzz(&run);
}
