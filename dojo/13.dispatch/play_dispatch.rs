// Dojo 13, the playground: a dispatch table.
//
// A frame is a list of systems run in order, and the list is data: a name and
// a pointer to a function for each, in an array the program changes as it
// runs. Adding, removing, reordering or switching off a system is an edit to
// the array, not to the code that calls them:
//
//   schedule  0 move   1 shoot   2 damage   3 die
//   a frame   for each enabled system, in order: (system.run)(&mut world)
//
// The order is a statement about data: a system that reads what another
// writes runs after it. damage writes health and die reads it, so damage
// goes before die, and a drone dies in the frame its health runs out. This is
// the course's schedule (3.2).
//
// This program builds a schedule of four systems for a drone flying at a
// turret, runs it frame by frame, and edits it between frames: shoot is
// switched off while the turret reloads, and removed at the end. After each
// step it prints the schedule or the world, and verify() checks that the
// drone is dead exactly when its health has run out; break inside its `if` to
// stop there. To see the dispatch in a debugger, break on `run` and step into
// each call.
//
// The switch: set IN_ORDER to false and run it again. damage is added at the
// end of the schedule, after die, instead of before it, and the drone dies a
// frame late. It's a static, read when damage is added, so a debugger can
// flip it too: stop on the first line of main, set it to false in the watch
// window, and continue.

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

use std::sync::atomic::{AtomicBool, Ordering::Relaxed};

static IN_ORDER: AtomicBool = AtomicBool::new(true);

const CAPACITY: usize = 8;

// A drone flying at a turret.
struct World {
    distance: i32,
    health: i32,
    alive: bool,
    // Hits this frame, waiting for damage.
    hits: i32,
}

// The systems. Each reads and writes a little of the world.
fn move_system(w: &mut World) {
    if w.alive {
        w.distance = (w.distance - 2).max(0);
    }
}

fn shoot_system(w: &mut World) {
    if w.alive && w.distance <= 4 {
        w.hits += 1;
    }
}

fn damage_system(w: &mut World) {
    if w.alive {
        w.health -= 40 * w.hits;
    }
    w.hits = 0;
}

fn die_system(w: &mut World) {
    if w.alive && w.health <= 0 {
        w.alive = false;
    }
}

// A system is a plain pointer to a function.
type SystemFn = fn(&mut World);

#[derive(Clone, Copy)]
struct System {
    name: &'static str,
    run: SystemFn,
    enabled: bool,
}

fn nothing(_: &mut World) {}

const EMPTY: System = System { name: "", run: nothing, enabled: false };

struct Schedule {
    systems: [System; CAPACITY],
    count: usize,
}

impl Schedule {
    fn find(&self, name: &str) -> Option<usize> {
        (0..self.count).find(|&i| self.systems[i].name == name)
    }

    fn add(&mut self, name: &'static str, run: SystemFn) {
        self.systems[self.count] = System { name, run, enabled: true };
        self.count += 1;
    }

    fn insert_before(&mut self, before: &str, name: &'static str, run: SystemFn) -> bool {
        let Some(at) = self.find(before) else {
            return false;
        };
        for i in (at..self.count).rev() {
            self.systems[i + 1] = self.systems[i];
        }
        self.systems[at] = System { name, run, enabled: true };
        self.count += 1;
        true
    }

    fn remove(&mut self, name: &str) -> bool {
        let Some(at) = self.find(name) else {
            return false;
        };
        for i in at..self.count - 1 {
            self.systems[i] = self.systems[i + 1];
        }
        self.count -= 1;
        self.systems[self.count] = EMPTY;
        true
    }

    fn set_enabled(&mut self, name: &str, on: bool) -> bool {
        let Some(at) = self.find(name) else {
            return false;
        };
        self.systems[at].enabled = on;
        true
    }

    // One frame: every enabled system, once, in order.
    fn run(&self, w: &mut World) {
        let mut ran = Vec::new();
        for s in &self.systems[..self.count] {
            if s.enabled {
                (s.run)(w);
                ran.push(s.name);
            }
        }
        println!("    ran       {}", ran.join(" "));
    }

    fn show(&self) {
        let names: Vec<String> = self.systems[..self.count]
            .iter()
            .map(|s| if s.enabled { s.name.to_string() } else { format!("({} off)", s.name) })
            .collect();
        println!("    schedule  {}", names.join(" "));
    }
}

fn show(w: &World) {
    println!(
        "    drone     {} from the turret, health {}, {}",
        w.distance,
        w.health,
        if w.alive { "alive" } else { "dead" }
    );
}

// The invariant, after every step: the drone is dead exactly when its health
// has run out.
fn verify(w: &World) {
    if w.alive == (w.health <= 0) {
        if w.alive {
            println!("    verify: the drone has health {} and is still alive", w.health);
        } else {
            println!("    verify: the drone is dead with health {}", w.health);
        }
    }
}

fn frame(s: &Schedule, w: &mut World, n: u32) {
    println!("frame {n}");
    s.run(w);
    show(w);
    verify(w);
}

fn main() {
    let mut s = Schedule { systems: [EMPTY; CAPACITY], count: 0 };
    let mut w = World { distance: 8, health: 100, alive: true, hits: 0 };
    println!("a drone 8 from a turret, with health 100: it flies 2 closer a frame,");
    println!("and the turret hits it for 40 a frame once it's within 4");
    show(&w);

    println!("add move, shoot and die");
    s.add("move", move_system);
    s.add("shoot", shoot_system);
    s.add("die", die_system);
    s.show();
    verify(&w);

    if IN_ORDER.load(Relaxed) {
        println!("insert damage before die: die reads the health that damage writes");
        s.insert_before("die", "damage", damage_system);
    } else {
        println!("add damage at the end, after die");
        s.add("damage", damage_system);
    }
    s.show();
    verify(&w);

    frame(&s, &mut w, 1);
    frame(&s, &mut w, 2);

    println!("switch shoot off: the turret is reloading");
    s.set_enabled("shoot", false);
    s.show();
    verify(&w);

    frame(&s, &mut w, 3);

    println!("switch shoot back on");
    s.set_enabled("shoot", true);
    s.show();
    verify(&w);

    frame(&s, &mut w, 4);
    frame(&s, &mut w, 5);
    frame(&s, &mut w, 6);

    println!("remove shoot: the turret is gone");
    s.remove("shoot");
    s.show();
    verify(&w);
}
