// Level 3: a blank page. Write the command buffer from memory.
//
// Start it with: python tools/dojo.py start commands 3
//
// The harness needs exactly this, and checks what each does:
//
//   Given, imported below: Dead and Handle from dojo.rs, and from the harness
//       World: spawn(value) -> Handle, despawn(h) -> Result<(), Dead>,
//              set(h, value) -> Result<(), Dead>, get(h) -> Option<u32>
//       Applied { applied: u32, skipped: u32 }
//
//   #[derive(Clone, Copy)] pub struct Pending          a spawn not applied yet
//   pub enum Target                                    a Handle or a Pending,
//       with From<Handle> and From<Pending>            either converts into one
//   pub struct Commands
//   Commands::new() -> Commands
//   spawn(&mut self, value: u32) -> Pending
//   set(&mut self, target: impl Into<Target>, value: u32)
//   despawn(&mut self, target: impl Into<Target>)
//   len(&self) -> usize                                recorded, not yet applied
//   apply(&mut self, w: &mut World) -> Applied         every command in order; a
//                                                      dead target is skipped;
//                                                      the buffer ends up empty

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

#[path = "../dojo.rs"]
mod dojo;
// The harness: main hands it the program, and it checks the code below.
mod harness;

use dojo::{Dead, Handle};
use harness::{Applied, World};

// Your code here.

fn main() {
    harness::run();
}
