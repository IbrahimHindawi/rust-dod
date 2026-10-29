// Level 2: every function. The fields and the signatures are given; write the
// bodies. Each function says what it must do; the harness checks how.
//
// Start it with: python tools/dojo.py start dispatch 2

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
        todo!("an empty schedule")
    }

    // Where the system called `name` is in the list, if it's there.
    fn find(&self, name: &str) -> Option<usize> {
        todo!("the place of the system called name")
    }

    // At the end: it runs after every system already there.
    pub fn add(&mut self, name: &'static str, run: SystemFn) {
        todo!("a new system, enabled, at the end")
    }

    // Just before `before`, so it runs first. False, and nothing added, if
    // there is no `before`.
    pub fn insert_before(&mut self, before: &str, name: &'static str, run: SystemFn) -> bool {
        todo!("a new system, enabled, just before the one called before")
    }

    // The others keep their order.
    pub fn remove(&mut self, name: &str) -> bool {
        todo!("take it out of the list")
    }

    // A disabled system keeps its place, and is skipped.
    pub fn set_enabled(&mut self, name: &str, on: bool) -> bool {
        todo!("switch it on or off, in its place")
    }

    // One frame: every enabled system, once, in order.
    pub fn run(&mut self, ctx: &mut Ctx) {
        todo!("call every enabled system, once, in order, and count it")
    }

    // Every system's name, enabled or not, in the order they run.
    pub fn order(&self) -> Vec<&'static str> {
        todo!("the names, in order")
    }

    // How many times run() has called the system; 0 if there's none by that
    // name.
    pub fn calls(&self, name: &str) -> u32 {
        todo!("its count of calls")
    }
}

fn main() {
    harness::run();
}
