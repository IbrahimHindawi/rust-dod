// Dojo 5, the playground: a command buffer.
//
// Systems walk tables, and while they walk they decide to change which things
// exist: a bullet's time is up, a turret fires. Doing it on the spot breaks the
// walk. A despawn swap-removes, so the last row moves into the hole, behind the
// loop, and is never looked at. So a system records the change instead, a
// command, and the frame applies every command, in order, once no one is
// walking anything:
//
//   * a spawn has no handle until it is applied, so the buffer hands back a
//     pending number, and later commands can name the spawn by it;
//   * a command whose target died earlier in the same buffer is skipped, which
//     generations make one comparison;
//   * the buffer is cleared after it is applied, and keeps its memory for the
//     next frame: a frame arena.
//
// This program builds a small packed world, runs one frame of three systems,
// and prints every command, then the buffer being applied. Break on `apply`
// in a debugger and step through the list.
//
// The switch: set DEFER to false, so the systems change the world on the spot,
// and run it again. Look at which things are left with no life.

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

use std::fmt;

const DEFER: bool = true;

#[derive(Clone, Copy, PartialEq, Eq)]
struct Handle {
    index: u32,
    generation: u32,
}

impl fmt::Display for Handle {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:{}", self.index, self.generation)
    }
}

#[derive(Clone, Copy)]
struct Slot {
    generation: u32,
    row: usize,
}

// A packed world, like the course's tables: things in rows 0 to len - 1, each
// with a life in frames, and a slot per index that says which row it is in.
struct World {
    slots: Vec<Slot>,
    free: Vec<u32>,
    owners: Vec<Handle>,
    lives: Vec<u32>,
}

impl World {
    fn alive(&self, h: Handle) -> bool {
        self.slots.get(h.index as usize).is_some_and(|s| s.generation == h.generation)
    }

    fn spawn(&mut self, life: u32) -> Handle {
        let index = self.free.pop().unwrap_or_else(|| {
            self.slots.push(Slot { generation: 1, row: 0 });
            self.slots.len() as u32 - 1
        });
        let slot = &mut self.slots[index as usize];
        slot.row = self.owners.len();
        let h = Handle { index, generation: slot.generation };
        self.owners.push(h);
        self.lives.push(life);
        h
    }

    // Swap-remove h's row. Returns the thing that moved into it, if any.
    fn despawn(&mut self, h: Handle) -> Result<Option<Handle>, &'static str> {
        if !self.alive(h) {
            return Err("dead");
        }
        let row = self.slots[h.index as usize].row;
        self.owners.swap_remove(row);
        self.lives.swap_remove(row);
        self.slots[h.index as usize].generation += 1;
        self.free.push(h.index);
        let moved = self.owners.get(row).copied();
        if let Some(m) = moved {
            self.slots[m.index as usize].row = row;
        }
        Ok(moved)
    }

    fn set_life(&mut self, h: Handle, life: u32) -> Result<(), &'static str> {
        if !self.alive(h) {
            return Err("dead");
        }
        let row = self.slots[h.index as usize].row;
        self.lives[row] = life;
        Ok(())
    }

    fn draw(&self) {
        for (row, (h, life)) in self.owners.iter().zip(&self.lives).enumerate() {
            println!("    row {row}  {h}  life {life}");
        }
    }
}

#[derive(Clone, Copy)]
enum Target {
    Entity(Handle),
    Pending(usize),
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Target::Entity(h) => write!(f, "{h}"),
            Target::Pending(k) => write!(f, "pending {k}"),
        }
    }
}

#[derive(Clone, Copy)]
enum Command {
    Spawn(u32),
    SetLife(Target, u32),
    Despawn(Target),
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Command::Spawn(life) => write!(f, "spawn with life {life}"),
            Command::SetLife(t, life) => write!(f, "set {t} life {life}"),
            Command::Despawn(t) => write!(f, "despawn {t}"),
        }
    }
}

#[derive(Default)]
struct Commands {
    list: Vec<Command>,
    spawns: usize,
    spawned: Vec<Handle>,
}

impl Commands {
    fn record(&mut self, c: Command) {
        println!("    record: {c}");
        self.list.push(c);
    }

    fn spawn(&mut self, life: u32) -> usize {
        self.record(Command::Spawn(life));
        self.spawns += 1;
        self.spawns - 1
    }

    fn resolve(&self, t: Target) -> Handle {
        match t {
            Target::Entity(h) => h,
            Target::Pending(k) => self.spawned[k],
        }
    }

    // Every command in order, then an empty buffer that keeps its memory.
    fn apply(&mut self, w: &mut World) {
        for i in 0..self.list.len() {
            let c = self.list[i];
            let what = match c {
                Command::Spawn(life) => {
                    let h = w.spawn(life);
                    self.spawned.push(h);
                    format!("pending {} is {h}", self.spawned.len() - 1)
                }
                Command::SetLife(t, life) => match w.set_life(self.resolve(t), life) {
                    Ok(()) => format!("ok, {} has life {life}", self.resolve(t)),
                    Err(why) => format!("skipped, {} is {why}", self.resolve(t)),
                },
                Command::Despawn(t) => despawned(w.despawn(self.resolve(t)), self.resolve(t)),
            };
            println!("    {c}: {what}");
        }
        self.list.clear();
        self.spawned.clear();
        self.spawns = 0;
    }
}

fn despawned(result: Result<Option<Handle>, &'static str>, h: Handle) -> String {
    match result {
        Ok(Some(moved)) => format!("ok, {moved} moves into {h}'s row"),
        Ok(None) => "ok, it was the last row".to_string(),
        Err(why) => format!("skipped, {h} is {why}"),
    }
}

fn main() {
    let mut w = World { slots: Vec::new(), free: Vec::new(), owners: Vec::new(), lives: Vec::new() };
    for life in [0, 3, 0, 0, 2] {
        w.spawn(life);
    }
    let mut cmds = Commands::default();
    println!("a world of 5 things, packed in rows; a life of 0 means its time is up");
    w.draw();

    if DEFER {
        println!("the expire system walks the rows, and records a despawn for each life of 0");
    } else {
        println!("the expire system walks the rows, and despawns each life of 0 on the spot");
    }
    let mut row = 0;
    while row < w.owners.len() {
        let h = w.owners[row];
        if w.lives[row] == 0 {
            if DEFER {
                cmds.record(Command::Despawn(Target::Entity(h)));
            } else {
                println!("    row {row}: despawn {h}: {}", despawned(w.despawn(h), h));
            }
        }
        row += 1;
    }

    println!("the spawner makes a replacement, and gives it a life through its pending number");
    if DEFER {
        let p = cmds.spawn(1);
        cmds.record(Command::SetLife(Target::Pending(p), 5));
    } else {
        let h = w.spawn(1);
        println!("    spawn: {h}");
        println!("    set {h} life 5: {}", if w.set_life(h, 5).is_ok() { "ok" } else { "dead" });
    }

    let doomed = Handle { index: 2, generation: 1 };
    println!("a second system despawns {doomed} too: it doesn't know the first one did");
    if DEFER {
        cmds.record(Command::Despawn(Target::Entity(doomed)));
        println!("the buffer holds {} commands, and the world has not changed:", cmds.list.len());
        w.draw();
        println!("apply the buffer, in the order it was recorded");
        cmds.apply(&mut w);
    } else {
        println!("    despawn {doomed}: {}", despawned(w.despawn(doomed), doomed));
    }

    println!("the world after the frame");
    w.draw();
    let left: Vec<String> = w.owners.iter().zip(&w.lives).filter(|(_, l)| **l == 0).map(|(h, _)| h.to_string()).collect();
    if left.is_empty() {
        println!("things left with no life: none");
    } else {
        println!("things left with no life: {}, moved behind the loop and never looked at", left.join(" "));
    }
    println!("commands left in the buffer: {}", cmds.list.len());
}
