// Level 1: one function. Everything here works except remove(). Write its body.
//
// Start it with: python tools/dojo.py start index 1

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
        List { nodes: Vec::new(), free: Vec::new(), head: None, tail: None, len: 0 }
    }

    // A node for `value`, linked to nothing yet: a freed one if there is one,
    // or a new one at the end of the Vec, which may move every node.
    fn take(&mut self, value: u64) -> u32 {
        let node = Node { value, prev: None, next: None, alive: true };
        self.len += 1;
        if let Some(index) = self.free.pop() {
            self.nodes[index as usize] = node;
            return index;
        }
        self.nodes.push(node);
        self.nodes.len() as u32 - 1
    }

    pub fn push_back(&mut self, value: u64) -> u32 {
        let index = self.take(value);
        self.nodes[index as usize].prev = self.tail;
        match self.tail {
            Some(tail) => self.nodes[tail as usize].next = Some(index),
            None => self.head = Some(index),
        }
        self.tail = Some(index);
        index
    }

    pub fn push_front(&mut self, value: u64) -> u32 {
        let index = self.take(value);
        self.nodes[index as usize].next = self.head;
        match self.head {
            Some(head) => self.nodes[head as usize].prev = Some(index),
            None => self.tail = Some(index),
        }
        self.head = Some(index);
        index
    }

    // A new node between `node` and the one after it; None if `node` isn't in
    // the list.
    pub fn insert_after(&mut self, node: u32, value: u64) -> Option<u32> {
        self.get(node)?;
        let next = self.nodes[node as usize].next;
        let index = self.take(value);
        self.nodes[index as usize].prev = Some(node);
        self.nodes[index as usize].next = next;
        self.nodes[node as usize].next = Some(index);
        match next {
            Some(next) => self.nodes[next as usize].prev = Some(index),
            None => self.tail = Some(index),
        }
        Some(index)
    }

    // Its neighbours link to each other, past it, and its index goes on the
    // free list.
    pub fn remove(&mut self, node: u32) -> Option<u64> {
        todo!("level 1: link its neighbours past it, and put it on the free list")
    }

    pub fn get(&self, node: u32) -> Option<u64> {
        self.nodes.get(node as usize).filter(|n| n.alive).map(|n| n.value)
    }

    pub fn get_mut(&mut self, node: u32) -> Option<&mut u64> {
        self.nodes.get_mut(node as usize).filter(|n| n.alive).map(|n| &mut n.value)
    }

    // The ends of the list, and the links between: the list in order is
    // head(), next(head()), and so on, until None.
    pub fn head(&self) -> Option<u32> {
        self.head
    }

    pub fn tail(&self) -> Option<u32> {
        self.tail
    }

    pub fn next(&self, node: u32) -> Option<u32> {
        self.nodes.get(node as usize).filter(|n| n.alive).and_then(|n| n.next)
    }

    pub fn prev(&self, node: u32) -> Option<u32> {
        self.nodes.get(node as usize).filter(|n| n.alive).and_then(|n| n.prev)
    }

    pub fn len(&self) -> usize {
        self.len
    }
}

fn main() {
    harness::run();
}
