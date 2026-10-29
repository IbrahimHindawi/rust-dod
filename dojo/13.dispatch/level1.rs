// Level 1: one function. Everything here works except run(). Write its body.
//
// Start it with: python tools/dojo.py start dispatch 1

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

#[path = "../dojo.rs"]
mod dojo;
// The harness: main hands it the program, and it checks the code below.
mod harness;

use harness::Ctx;

// A system is a plain pointer to a function: no closure, no allocation.
type SystemFn = fn(&mut Ctx);

struct System {
    name: &'static str,
    run: SystemFn,
    enabled: bool,
    // How many times run() has called it.
    calls: u32,
}

pub struct Schedule {
    // In the order they run.
    systems: Vec<System>,
}

impl Schedule {
    pub fn new() -> Schedule {
        Schedule { systems: Vec::new() }
    }

    // Where the system called `name` is in the list, if it's there.
    fn find(&self, name: &str) -> Option<usize> {
        self.systems.iter().position(|s| s.name == name)
    }

    // At the end: it runs after every system already there.
    pub fn add(&mut self, name: &'static str, run: SystemFn) {
        self.systems.push(System { name, run, enabled: true, calls: 0 });
    }

    // Just before `before`, so it runs first. False, and nothing added, if
    // there is no `before`.
    pub fn insert_before(&mut self, before: &str, name: &'static str, run: SystemFn) -> bool {
        let Some(at) = self.find(before) else {
            return false;
        };
        self.systems.insert(at, System { name, run, enabled: true, calls: 0 });
        true
    }

    // The others keep their order.
    pub fn remove(&mut self, name: &str) -> bool {
        let Some(at) = self.find(name) else {
            return false;
        };
        self.systems.remove(at);
        true
    }

    // A disabled system keeps its place, and is skipped.
    pub fn set_enabled(&mut self, name: &str, on: bool) -> bool {
        let Some(at) = self.find(name) else {
            return false;
        };
        self.systems[at].enabled = on;
        true
    }

    // One frame: every enabled system, once, in order.
    pub fn run(&mut self, ctx: &mut Ctx) {
        todo!("level 1: call every enabled system, once, in order, and count it")
    }

    // Every system's name, enabled or not, in the order they run.
    pub fn order(&self) -> Vec<&'static str> {
        self.systems.iter().map(|s| s.name).collect()
    }

    // How many times run() has called the system; 0 if there's none by that
    // name.
    pub fn calls(&self, name: &str) -> u32 {
        self.find(name).map_or(0, |at| self.systems[at].calls)
    }
}

fn main() {
    harness::run();
}
