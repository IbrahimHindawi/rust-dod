// The harness for dojo 1, the free list and generations. It is fixed: your
// work is kata.rs, beside it. Run both with
//
//   python tools/dojo.py free_list
//
// First a short scenario, the same steps as the playground, each printed as
// it passes. Then a fuzz run: thousands of random allocs and frees from a
// seed, each checked against a model that keeps every handle ever handed out.
// The model doesn't care how kata.rs works inside, only that what it does is
// right, so any correct design passes, including the level 4 variations.

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

#[path = "../dojo.rs"]
mod dojo;
mod kata;

use dojo::{Rng, Run};
pub use dojo::{Dead, Handle};
use kata::Handles;
use std::collections::HashSet;

fn status(r: Result<(), Dead>) -> &'static str {
    if r.is_ok() { "ok" } else { "dead" }
}

fn scenario(run: &Run) {
    println!("scenario");
    let mut h = Handles::new();
    let (a, b, c) = (h.alloc(), h.alloc(), h.alloc());
    run.same("alloc a, b, c", format!("{a} {b} {c}"), "0:1 1:1 2:1");
    run.same("free b", status(h.free(b)), "ok");
    run.same("b is alive", h.alive(b), "false");
    run.same("free b again", status(h.free(b)), "dead");
    let d = h.alloc();
    run.same("alloc d, b's index in its next life", d, "1:2");
    run.same("b is alive", h.alive(b), "false");
    run.same("d is alive", h.alive(d), "true");
    run.same("live, slots", format!("{} {}", h.live(), h.slots()), "3 3");
    let e = h.alloc();
    run.same("alloc e, nothing free: a new index", e, "3:1");
    run.same("live, slots", format!("{} {}", h.live(), h.slots()), "4 4");
}

// What a correct allocator must do, kept the simple way: every handle ever
// handed out, which are alive, and which indices are free.
struct Model {
    issued: HashSet<Handle>,
    alive: Vec<Handle>,
    dead: Vec<Handle>,
    free: HashSet<u32>,
    slots: usize,
}

fn fuzz(run: &Run) {
    run.fuzz();
    let mut rng = Rng::new(run.seed);
    let mut h = Handles::new();
    let mut m = Model { issued: HashSet::new(), alive: Vec::new(), dead: Vec::new(), free: HashSet::new(), slots: 0 };
    for op in 1..=run.ops {
        let roll = rng.below(100);
        if roll < 50 || m.alive.is_empty() {
            run.before(op, "alloc()");
            let got = h.alloc();
            run.check(op, !m.issued.contains(&got), || format!("alloc returned {got}, which was handed out before"));
            if m.free.is_empty() {
                run.check(op, got.index as usize == m.slots, || {
                    format!("alloc returned {got} with nothing free; the next new index is {}", m.slots)
                });
                m.slots += 1;
            } else {
                run.check(op, m.free.remove(&got.index), || {
                    format!("alloc returned {got}, but index {} is not free, and {} freed indices wait", got.index, m.free.len())
                });
            }
            run.check(op, h.alive(got), || format!("alloc returned {got}, and alive({got}) is false"));
            m.issued.insert(got);
            m.alive.push(got);
        } else if roll < 90 {
            let at = rng.below(m.alive.len() as u64) as usize;
            let victim = m.alive.swap_remove(at);
            run.before(op, format_args!("free({victim})"));
            let got = h.free(victim);
            run.check(op, got.is_ok(), || format!("free({victim}) returned Err(Dead), but {victim} is alive"));
            run.check(op, !h.alive(victim), || format!("after free({victim}), alive({victim}) is still true"));
            m.free.insert(victim.index);
            m.dead.push(victim);
        } else if !m.dead.is_empty() {
            let stale = rng.pick(&m.dead);
            run.before(op, format_args!("free({stale}), a dead handle"));
            let live = h.live();
            run.check(op, h.free(stale) == Err(Dead), || format!("free({stale}) of a dead handle should be Err(Dead)"));
            run.check(op, h.live() == live, || format!("free({stale}) of a dead handle changed live() from {live} to {}", h.live()));
        }
        run.check(op, h.live() == m.alive.len(), || format!("live() is {}, the model says {}", h.live(), m.alive.len()));
        run.check(op, h.slots() == m.slots, || format!("slots() is {}, the model says {}", h.slots(), m.slots));
        if op % 256 == 0 {
            for &x in &m.alive {
                run.check(op, h.alive(x), || format!("{x} should be alive"));
            }
            for &x in &m.dead {
                run.check(op, !h.alive(x), || format!("{x} should be dead"));
            }
        }
    }
    run.pass();
}

fn main() {
    let run = Run::new("free_list", "1. free list and generations");
    scenario(&run);
    fuzz(&run);
}
