// The harness for dojo 5, a command buffer. It is fixed. Your work is kata.rs,
// beside it: kata.rs brings this file in as a module and its main calls run(),
// so the two are one program. The reference, solution.rs, and each level file
// do the same, so each of them builds on its own, and this file never does.
// Build and run yours with
//
//   python tools/dojo.py commands
//
// The buffer changes a world, and the world is here, in the harness: a plain
// generational store of numbers, so the kata is only the buffer. First a
// short scenario: a set, a despawn, a spawn that reuses the despawned index,
// a set through the spawn's pending number, and two commands that find their
// target already dead. Then a fuzz run, frame after frame: random commands
// recorded against live handles, dead handles and this frame's own spawns,
// then applied. The model runs the same commands straight away, in order, on
// a copy of the world from before the frame, and the two worlds must match.

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

use crate::dojo::{Dead, Handle, Rng, Run};
use crate::{Commands, Pending};
use std::fmt;
use std::num::NonZeroU32;

// The world the commands change: for each index, its generation and its value
// if it is alive, and a free list. Exactly the free list of dojo 1.
#[derive(Clone, PartialEq, Debug)]
pub struct World {
    slots: Vec<(NonZeroU32, Option<u32>)>,
    free: Vec<u32>,
}

impl World {
    pub fn new() -> World {
        World { slots: Vec::new(), free: Vec::new() }
    }

    pub fn spawn(&mut self, value: u32) -> Handle {
        let index = match self.free.pop() {
            Some(index) => index,
            None => {
                self.slots.push((NonZeroU32::MIN, None));
                self.slots.len() as u32 - 1
            }
        };
        let slot = &mut self.slots[index as usize];
        slot.1 = Some(value);
        Handle { index, generation: slot.0 }
    }

    pub fn despawn(&mut self, h: Handle) -> Result<(), Dead> {
        self.get(h).ok_or(Dead)?;
        let slot = &mut self.slots[h.index as usize];
        *slot = (slot.0.checked_add(1).expect("4 billion lives"), None);
        self.free.push(h.index);
        Ok(())
    }

    pub fn set(&mut self, h: Handle, value: u32) -> Result<(), Dead> {
        self.get(h).ok_or(Dead)?;
        self.slots[h.index as usize].1 = Some(value);
        Ok(())
    }

    pub fn get(&self, h: Handle) -> Option<u32> {
        match self.slots.get(h.index as usize) {
            Some(&(generation, value)) if generation == h.generation => value,
            _ => None,
        }
    }

    // Every live handle, in index order.
    pub fn handles(&self) -> Vec<Handle> {
        let alive = self.slots.iter().enumerate().filter(|(_, s)| s.1.is_some());
        alive.map(|(i, s)| Handle { index: i as u32, generation: s.0 }).collect()
    }
}

impl fmt::Display for World {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let live: Vec<String> = self.handles().iter().map(|&h| format!("{h}={}", self.get(h).unwrap_or(0))).collect();
        write!(f, "{}", live.join(" "))
    }
}

// What apply did: commands that changed something, and commands whose target
// was already dead.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Applied {
    pub applied: u32,
    pub skipped: u32,
}

impl fmt::Display for Applied {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "applied {}, skipped {}", self.applied, self.skipped)
    }
}

fn scenario(run: &Run) {
    println!("scenario");
    let mut w = World::new();
    let (a, b, c) = (w.spawn(1), w.spawn(2), w.spawn(3));
    run.same("a world of three", &w, "0:1=1 1:1=2 2:1=3");
    let mut cmds = Commands::new();
    cmds.set(a, 10);
    cmds.despawn(b);
    let p = cmds.spawn(40);
    cmds.set(p, 41);
    cmds.despawn(b);
    cmds.set(b, 20);
    run.same("record: set a 10, despawn b, spawn 40, set it 41, despawn b, set b 20", cmds.len(), "6");
    run.same("the world before apply, unchanged", &w, "0:1=1 1:1=2 2:1=3");
    run.same("apply", cmds.apply(&mut w), "applied 4, skipped 2");
    run.same("the spawn reused b's index, in its next life", &w, "0:1=10 1:2=41 2:1=3");
    run.same("b after apply", format!("{:?}", w.get(b)), "None");
    run.same("commands left in the buffer", cmds.len(), "0");
    run.same("apply the empty buffer", cmds.apply(&mut w), "applied 0, skipped 0");
    cmds.despawn(c);
    let q = cmds.spawn(50);
    cmds.despawn(q);
    cmds.set(q, 51);
    run.same("the buffer again: despawn c, spawn 50, despawn it, set it 51", cmds.apply(&mut w), "applied 3, skipped 1");
    run.same("the world", &w, "0:1=10 1:2=41");
}

// A command's target as the model sees it: a handle, or the k-th spawn of
// this frame.
#[derive(Clone, Copy)]
enum Who {
    Entity(Handle),
    Spawn(usize),
}

#[derive(Clone, Copy)]
enum Op {
    Spawn(u32),
    Set(Who, u32),
    Despawn(Who),
}

impl fmt::Display for Who {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Who::Entity(h) => write!(f, "{h}"),
            Who::Spawn(k) => write!(f, "spawn {k} of this frame"),
        }
    }
}

impl fmt::Display for Op {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Op::Spawn(v) => write!(f, "spawn({v})"),
            Op::Set(who, v) => write!(f, "set({who}, {v})"),
            Op::Despawn(who) => write!(f, "despawn({who})"),
        }
    }
}

// The same commands, applied straight away, in order: what apply must do.
fn model(before: &World, ops: &[Op]) -> (World, Applied) {
    let mut w = before.clone();
    let mut spawned = Vec::new();
    let mut done = Applied { applied: 0, skipped: 0 };
    let handle = |who: Who, spawned: &Vec<Handle>| match who {
        Who::Entity(h) => h,
        Who::Spawn(k) => spawned[k],
    };
    for &op in ops {
        let result = match op {
            Op::Spawn(v) => {
                spawned.push(w.spawn(v));
                Ok(())
            }
            Op::Set(who, v) => w.set(handle(who, &spawned), v),
            Op::Despawn(who) => w.despawn(handle(who, &spawned)),
        };
        match result {
            Ok(()) => done.applied += 1,
            Err(Dead) => done.skipped += 1,
        }
    }
    (w, done)
}

// Where two worlds first differ, in words.
fn difference(got: &World, want: &World) -> String {
    for i in 0..got.slots.len().max(want.slots.len()) {
        let show = |w: &World| match w.slots.get(i) {
            Some(&(g, Some(v))) => format!("{i}:{g} = {v}"),
            Some(&(g, None)) => format!("index {i} free, next generation {g}"),
            None => format!("no index {i}"),
        };
        if got.slots.get(i) != want.slots.get(i) {
            return format!("the world has {}, the model has {}", show(got), show(want));
        }
    }
    format!("the free lists differ: {:?} and {:?}", got.free, want.free)
}

fn fuzz(run: &Run) {
    run.fuzz();
    let mut rng = Rng::new(run.seed);
    let mut w = World::new();
    let mut cmds = Commands::new();
    let mut seen: Vec<Handle> = (0..8).map(|v| w.spawn(v)).collect();
    let mut op = 0;
    while op < run.ops {
        let live = w.handles();
        let before = w.clone();
        let mut ops: Vec<Op> = Vec::new();
        let mut pendings: Vec<Pending> = Vec::new();
        for _ in 0..1 + rng.below(12) {
            op += 1;
            let value = rng.below(1000) as u32;
            let roll = rng.below(100);
            let target = rng.below(100);
            let who = if target < 55 && !live.is_empty() {
                Who::Entity(rng.pick(&live))
            } else if target < 80 || pendings.is_empty() {
                Who::Entity(rng.pick(&seen))
            } else {
                Who::Spawn(rng.below(pendings.len() as u64) as usize)
            };
            let next = if roll < 30 {
                Op::Spawn(value)
            } else if roll < 65 {
                Op::Set(who, value)
            } else {
                Op::Despawn(who)
            };
            run.before(op, format_args!("record {next}"));
            match next {
                Op::Spawn(v) => pendings.push(cmds.spawn(v)),
                Op::Set(Who::Entity(h), v) => cmds.set(h, v),
                Op::Set(Who::Spawn(k), v) => cmds.set(pendings[k], v),
                Op::Despawn(Who::Entity(h)) => cmds.despawn(h),
                Op::Despawn(Who::Spawn(k)) => cmds.despawn(pendings[k]),
            }
            ops.push(next);
            run.check(op, cmds.len() == ops.len(), || format!("len() is {} after recording {} commands", cmds.len(), ops.len()));
        }
        run.check(op, w == before, || "recording commands changed the world".to_string());
        op += 1;
        run.before(op, format_args!("apply() of {} commands", ops.len()));
        let got = cmds.apply(&mut w);
        let (want_world, want) = model(&before, &ops);
        run.check(op, got == want, || format!("apply() returned {got}, the model says {want}"));
        run.check(op, w == want_world, || format!("after apply(), {}", difference(&w, &want_world)));
        run.check(op, cmds.len() == 0, || format!("len() is {} after apply(); the buffer should be empty", cmds.len()));
        seen.extend(w.handles().into_iter().filter(|h| before.get(*h).is_none()));
    }
    run.pass();
}

// The kata's main calls this.
pub fn run() {
    let run = Run::new("commands", "5. a command buffer");
    scenario(&run);
    fuzz(&run);
}
