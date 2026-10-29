// Level 2: every function. The fields and the signatures are given; write the
// bodies. Each function says what it must do; the harness checks how.
//
// Start it with: python tools/dojo.py start edges 2

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

#[path = "../dojo.rs"]
mod dojo;
// The harness: main hands it the program, and it checks the code below.
mod harness;

// Component `id`'s bit in a signature.
fn bit(id: u32) -> u64 {
    todo!("one bit, in 64")
}

struct Node {
    signature: u64,
    // add[id] is the node with component id added, once a move has looked it
    // up; remove[id] is the node with it removed.
    add: [Option<u32>; 64],
    remove: [Option<u32>; 64],
}

pub struct Graph {
    nodes: Vec<Node>,
    lookups: u32,
    hits: u32,
}

impl Graph {
    // Node 0 is the empty signature, where every entity starts.
    pub fn new() -> Graph {
        todo!("a graph with one node, the empty signature")
    }

    fn make(&mut self, signature: u64) -> u32 {
        todo!("a new node, with no edges known")
    }

    // The slow path: the node with this signature, made if there is none yet.
    fn find_or_make(&mut self, signature: u64) -> u32 {
        todo!("a lookup: search the nodes")
    }

    pub fn signature(&self, node: u32) -> u64 {
        todo!("the node's signature")
    }

    // The node with component `id` added. A node that has it already is its
    // own answer, and nothing is counted.
    pub fn with(&mut self, node: u32, id: u32) -> u32 {
        todo!("follow the add edge, or look it up once and write the edge both ways")
    }

    // The node with component `id` removed. A node without it is its own
    // answer.
    pub fn without(&mut self, node: u32, id: u32) -> u32 {
        todo!("with's mirror")
    }

    pub fn len(&self) -> usize {
        todo!("how many nodes")
    }

    // How many moves took the slow path.
    pub fn lookups(&self) -> u32 {
        todo!()
    }

    // How many moves followed an edge.
    pub fn hits(&self) -> u32 {
        todo!()
    }
}

fn main() {
    harness::run();
}
