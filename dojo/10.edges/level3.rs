// Level 3: a blank page. Write the graph from memory.
//
// Start it with: python tools/dojo.py start edges 3
//
// The harness needs exactly this, and checks what each does:
//
//   pub struct Graph
//   Graph::new() -> Graph                      node 0 is the empty signature
//   signature(&self, node: u32) -> u64
//   with(&mut self, node: u32, id: u32) -> u32     the node with id added
//   without(&mut self, node: u32, id: u32) -> u32  the node with id removed
//   len(&self) -> usize                        how many nodes
//   lookups(&self) -> u32                      moves that searched by signature
//   hits(&self) -> u32                         moves that followed an edge
//
// A new node is numbered len() at the time. A move to a node one component
// away follows an edge if one is known (a hit); otherwise it searches (a
// lookup) and writes the edge and the edge back. A move that changes nothing
// counts as neither. Ids go up to 63.

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

#[path = "../dojo.rs"]
mod dojo;
// The harness: main hands it the program, and it checks the code below.
mod harness;

// Your code here.

fn main() {
    harness::run();
}
