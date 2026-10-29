// Level 3: a blank page. Write the schedule from memory.
//
// Start it with: python tools/dojo.py start dispatch 3
//
// The harness needs exactly this, and checks what each does:
//
//   Given, imported below from the harness:
//       Ctx                                         what a system is given when it runs
//
//   pub struct Schedule
//   Schedule::new() -> Schedule
//   add(&mut self, name: &'static str, run: fn(&mut Ctx))
//                                                   at the end, enabled
//   insert_before(&mut self, before: &str, name: &'static str, run: fn(&mut Ctx)) -> bool
//                                                   just before `before`, enabled; false,
//                                                   and nothing added, if it isn't there
//   remove(&mut self, name: &str) -> bool           false if it isn't there
//   set_enabled(&mut self, name: &str, on: bool) -> bool
//                                                   a disabled system keeps its place
//   run(&mut self, ctx: &mut Ctx)                   every enabled system, once, in order
//   order(&self) -> Vec<&'static str>               every name, enabled or not, in order
//   calls(&self, name: &str) -> u32                 times run() has called it since it was
//                                                   added; 0 if it isn't there

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

#[path = "../dojo.rs"]
mod dojo;
// The harness: main hands it the program, and it checks the code below.
mod harness;

use harness::Ctx;

// Your code here.

fn main() {
    harness::run();
}
