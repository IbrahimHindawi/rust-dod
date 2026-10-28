// Level 2: every function. The types and the signatures are given; write the
// bodies. Each function says what it must do; the harness checks how.
//
// Start it with: python tools/dojo.py start commands 2

use crate::{Applied, Dead, Handle, World};

// A spawn that has been recorded and not applied: its place among this
// buffer's spawns, 0 for the first.
#[derive(Clone, Copy)]
pub struct Pending(usize);

// Who a command is for: something alive now, or something this buffer spawns.
#[derive(Clone, Copy)]
pub enum Target {
    Entity(Handle),
    Pending(Pending),
}

impl From<Handle> for Target {
    fn from(h: Handle) -> Target {
        todo!("a handle as a target")
    }
}

impl From<Pending> for Target {
    fn from(p: Pending) -> Target {
        todo!("a pending spawn as a target")
    }
}

#[derive(Clone, Copy)]
enum Command {
    Spawn(u32),
    Set(Target, u32),
    Despawn(Target),
}

pub struct Commands {
    list: Vec<Command>,
    spawns: usize,
    // While the buffer is applied: the handle each spawn made, in order.
    spawned: Vec<Handle>,
}

impl Commands {
    pub fn new() -> Commands {
        todo!("an empty buffer")
    }

    // Record a spawn; it happens in apply. Returns its pending number.
    pub fn spawn(&mut self, value: u32) -> Pending {
        todo!("record a spawn")
    }

    pub fn set(&mut self, target: impl Into<Target>, value: u32) {
        todo!("record a set")
    }

    pub fn despawn(&mut self, target: impl Into<Target>) {
        todo!("record a despawn")
    }

    // Commands recorded and not yet applied.
    pub fn len(&self) -> usize {
        todo!("commands waiting")
    }

    // Every command, in the order recorded. A spawn always happens; a set or
    // despawn of a dead target is skipped. Afterwards the buffer is empty.
    pub fn apply(&mut self, w: &mut World) -> Applied {
        todo!("apply the buffer")
    }
}

// The handle a target names: its own, or the one its spawn made.
fn resolve(spawned: &[Handle], target: Target) -> Handle {
    todo!("a target's handle")
}
