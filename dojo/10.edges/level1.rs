// Level 1: one function. Everything here works except with(). Write its body.
//
// Start it with: python tools/dojo.py start edges 1

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

#[path = "../dojo.rs"]
mod dojo;
// The harness: main hands it the program, and it checks the code below.
mod harness;

// Component `id`'s bit in a signature.
fn bit(id: u32) -> u64 {
    1 << id
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
        let mut g = Graph { nodes: Vec::new(), lookups: 0, hits: 0 };
        g.make(0);
        g
    }

    fn make(&mut self, signature: u64) -> u32 {
        self.nodes.push(Node { signature, add: [None; 64], remove: [None; 64] });
        self.nodes.len() as u32 - 1
    }

    // The slow path: the node with this signature, made if there is none yet.
    // A search through every node; 2.4 keeps a hash map instead, and dojo 9
    // builds one.
    fn find_or_make(&mut self, signature: u64) -> u32 {
        self.lookups += 1;
        match self.nodes.iter().position(|n| n.signature == signature) {
            Some(i) => i as u32,
            None => self.make(signature),
        }
    }

    pub fn signature(&self, node: u32) -> u64 {
        self.nodes[node as usize].signature
    }

    // The node with component `id` added. A known edge is followed. An unknown
    // one is looked up, once, and written on both nodes: the add edge here and
    // the remove edge back.
    pub fn with(&mut self, node: u32, id: u32) -> u32 {
        todo!("level 1: follow the add edge if it is known; otherwise look the node up, and write the edge and the edge back")
    }

    // The node with component `id` removed: with's mirror.
    pub fn without(&mut self, node: u32, id: u32) -> u32 {
        let signature = self.signature(node);
        if signature & bit(id) == 0 {
            return node;
        }
        if let Some(to) = self.nodes[node as usize].remove[id as usize] {
            self.hits += 1;
            return to;
        }
        let to = self.find_or_make(signature & !bit(id));
        self.nodes[node as usize].remove[id as usize] = Some(to);
        self.nodes[to as usize].add[id as usize] = Some(node);
        to
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    // How many moves took the slow path.
    pub fn lookups(&self) -> u32 {
        self.lookups
    }

    // How many moves followed an edge.
    pub fn hits(&self) -> u32 {
        self.hits
    }
}

fn main() {
    harness::run();
}
