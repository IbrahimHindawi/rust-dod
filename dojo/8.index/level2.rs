// Level 2: every function. The fields and the signatures are given; write the
// bodies. Each function says what it must do; the harness checks how.
//
// Start it with: python tools/dojo.py start index 2

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

#[path = "../dojo.rs"]
mod dojo;
// The harness: main hands it the program, and it checks the code below.
mod harness;

// A node links to its neighbours by their index in `nodes`. A freed node
// stays in the Vec, not alive, until a new value takes it.
#[derive(Clone, Copy)]
struct Node {
    value: u64,
    prev: Option<u32>,
    next: Option<u32>,
    alive: bool,
}

pub struct List {
    nodes: Vec<Node>,
    // Freed nodes, used again before the Vec grows.
    free: Vec<u32>,
    head: Option<u32>,
    tail: Option<u32>,
    len: usize,
}

impl List {
    pub fn new() -> List {
        todo!("an empty list")
    }

    // A node for `value`, linked to nothing yet: a freed one if there is one,
    // or a new one at the end of the Vec, which may move every node.
    fn take(&mut self, value: u64) -> u32 {
        todo!("a freed node, or a new one")
    }

    // A new node at the end; its index stays its own until it's removed.
    pub fn push_back(&mut self, value: u64) -> u32 {
        todo!("take a node, link it after the tail")
    }

    pub fn push_front(&mut self, value: u64) -> u32 {
        todo!("take a node, link it before the head")
    }

    // A new node between `node` and the one after it; None if `node` isn't in
    // the list.
    pub fn insert_after(&mut self, node: u32, value: u64) -> Option<u32> {
        todo!("four links to set, and maybe the tail")
    }

    // A live node's value comes back, its neighbours link past it, and its
    // index can be handed out again. A node not in the list gives None and
    // changes nothing.
    pub fn remove(&mut self, node: u32) -> Option<u64> {
        todo!("unlink it, free it")
    }

    pub fn get(&self, node: u32) -> Option<u64> {
        todo!("its value, if it's alive")
    }

    pub fn get_mut(&mut self, node: u32) -> Option<&mut u64> {
        todo!("the same, to change")
    }

    // The ends of the list, and the links between: the list in order is
    // head(), next(head()), and so on, until None. A node not in the list
    // has no links.
    pub fn head(&self) -> Option<u32> {
        todo!("the first node")
    }

    pub fn tail(&self) -> Option<u32> {
        todo!("the last node")
    }

    pub fn next(&self, node: u32) -> Option<u32> {
        todo!("the node after this one")
    }

    pub fn prev(&self, node: u32) -> Option<u32> {
        todo!("the node before this one")
    }

    pub fn len(&self) -> usize {
        todo!("how many nodes are in the list")
    }
}

fn main() {
    harness::run();
}
