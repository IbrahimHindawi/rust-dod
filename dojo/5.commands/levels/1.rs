// Level 1: one function. Everything here works except apply(). Write its body:
// run every command in the order it was recorded, count what happened, and
// leave the buffer empty for the next frame.
//
// Start it with: python tools/dojo.py start commands 1

use crate::{Applied, Dead, Handle, World};

// A spawn that has been recorded and not applied. It has no handle yet, so it
// is named by its place among this buffer's spawns: 0 for the first.
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
        Target::Entity(h)
    }
}

impl From<Pending> for Target {
    fn from(p: Pending) -> Target {
        Target::Pending(p)
    }
}

#[derive(Clone, Copy)]
enum Command {
    Spawn(u32),
    Set(Target, u32),
    Despawn(Target),
}

// The frame's commands. Cleared after apply and kept, so once the game has run
// a few frames, recording allocates nothing: a frame arena made of Vecs.
pub struct Commands {
    list: Vec<Command>,
    spawns: usize,
    // While the buffer is applied: the handle each spawn made, in order.
    spawned: Vec<Handle>,
}

impl Commands {
    pub fn new() -> Commands {
        Commands { list: Vec::new(), spawns: 0, spawned: Vec::new() }
    }

    pub fn spawn(&mut self, value: u32) -> Pending {
        self.list.push(Command::Spawn(value));
        self.spawns += 1;
        Pending(self.spawns - 1)
    }

    pub fn set(&mut self, target: impl Into<Target>, value: u32) {
        self.list.push(Command::Set(target.into(), value));
    }

    pub fn despawn(&mut self, target: impl Into<Target>) {
        self.list.push(Command::Despawn(target.into()));
    }

    // Commands recorded and not yet applied.
    pub fn len(&self) -> usize {
        self.list.len()
    }

    // Every command, in the order it was recorded. A spawn always happens; a
    // set or despawn whose target is dead by then, perhaps despawned earlier in
    // this same buffer, changes nothing and is counted as skipped.
    pub fn apply(&mut self, w: &mut World) -> Applied {
        todo!("level 1: every command, in order; then an empty buffer")
    }
}

// The handle a target names: its own, or the one its spawn made, which was
// applied before any command that names it.
fn resolve(spawned: &[Handle], target: Target) -> Handle {
    match target {
        Target::Entity(h) => h,
        Target::Pending(Pending(i)) => spawned[i],
    }
}
