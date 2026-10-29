// Level 3: a blank page. Write the walk from memory.
//
// Start it with: python tools/dojo.py start reflect 3
//
// The harness needs exactly this, and checks what each does:
//
//   Given, imported below from the harness, as 2.7 writes them:
//   Info { name, layout, fields: &'static [Field] }
//   Field { name, offset, size, ty: &'static str, info: Option<&'static Info>, hidden: bool }
//   `offset` is from the start of the field's own struct; `info` is the field
//   type's record, None for a builtin (f32, u8); `ty` is its name.
//
//   paths(info: &Info) -> Vec<String>                         "at.x": every leaf a tool shows, in order
//   diff(info: &Info, a: &[u8], b: &[u8]) -> Vec<String>      the paths whose bytes differ
//   get_f32(info: &Info, bytes: &[u8], path: &str) -> Option<f32>
//   set_f32(info: &Info, bytes: &mut [u8], path: &str, value: f32) -> bool
//
// A hidden field is left out of paths and diff, with everything inside it,
// but a path still reaches it. get_f32 and set_f32 refuse a path that names a
// struct, a u8, or nothing.

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

#[path = "../dojo.rs"]
mod dojo;
// The harness: main hands it the program, and it checks the code below.
mod harness;

use harness::{Field, Info};

// Your code here.

fn main() {
    harness::run();
}
