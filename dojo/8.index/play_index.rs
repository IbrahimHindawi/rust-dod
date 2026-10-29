// Dojo 8, the playground: index, not pointer.
//
// A linked list whose nodes live side by side in one array and link to each
// other by index: a node's next is "node 2", not an address. When the array is
// full it moves: the nodes are copied to a place twice the size, and the old
// place is let go. An index is a position in the array, so it finds its node
// after the move. A pointer is a place in memory, so it still points where
// the node was.
//
// Memory here is an array of 16 cells, small enough to see whole in a
// debugger's watch window. The list takes 2 cells, then 4, then 8: cells 0..2,
// then 2..6, then 6..14. Each move leaves the old copy where it was, the way
// freed memory keeps its bytes until something else is put there.
//
// The program keeps node b two ways: its index, and the cell it's in, which
// is what a pointer to it holds. Then the list moves, and the program writes
// to b. verify() runs after every step: it checks the links, and that every
// node holds what the program last wrote to it. Put a breakpoint on `broken`
// to stop where it finds something wrong.
//
// The switch: USE_INDEX. Break on the line that writes 21 into b, set it to
// false in the watch window, and continue. The write goes through the kept
// cell to the old copy, and verify reports it lost. Rust won't let a
// reference into the list live across the move (error E0499), so the program
// keeps the number a pointer would hold, and the borrow checker has nothing
// to say about numbers. rin-dod's playground keeps a real pointer.

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};

static USE_INDEX: AtomicBool = AtomicBool::new(true);

#[derive(Clone, Copy, Default)]
struct Node {
    value: u64,
    prev: Option<u32>,
    next: Option<u32>,
    alive: bool,
}

struct List {
    // All of memory. Node i is in cell base + i, and the list owns cells
    // base..base + capacity.
    memory: [Node; 16],
    base: usize,
    capacity: usize,
    // Nodes made so far are 0..made; the freed ones wait in free.
    made: usize,
    free: [u32; 8],
    free_count: usize,
    head: Option<u32>,
    tail: Option<u32>,
}

impl List {
    fn new() -> List {
        List {
            memory: [Node::default(); 16],
            base: 0,
            capacity: 2,
            made: 0,
            free: [0; 8],
            free_count: 0,
            head: None,
            tail: None,
        }
    }

    // Where node `index` is now.
    fn cell(&self, index: u32) -> usize {
        self.base + index as usize
    }

    fn node(&mut self, index: u32) -> &mut Node {
        &mut self.memory[self.base + index as usize]
    }

    // A node for `value`: a freed one, or a new one, moving the list to twice
    // the room first when it's full.
    fn take(&mut self, value: u64) -> u32 {
        let index = if self.free_count > 0 {
            self.free_count -= 1;
            self.free[self.free_count]
        } else {
            if self.made == self.capacity {
                let to = self.base + self.capacity;
                println!("    the list moves from cells {}..{} to cells {}..{}", self.base, to, to, to + 2 * self.capacity);
                for i in 0..self.made {
                    self.memory[to + i] = self.memory[self.base + i];
                }
                self.base = to;
                self.capacity *= 2;
            }
            self.made += 1;
            self.made as u32 - 1
        };
        *self.node(index) = Node { value, prev: None, next: None, alive: true };
        index
    }

    fn push_back(&mut self, value: u64) -> u32 {
        let index = self.take(value);
        self.node(index).prev = self.tail;
        match self.tail {
            Some(tail) => self.node(tail).next = Some(index),
            None => self.head = Some(index),
        }
        self.tail = Some(index);
        index
    }

    fn push_front(&mut self, value: u64) -> u32 {
        let index = self.take(value);
        self.node(index).next = self.head;
        match self.head {
            Some(head) => self.node(head).prev = Some(index),
            None => self.tail = Some(index),
        }
        self.head = Some(index);
        index
    }

    fn insert_after(&mut self, at: u32, value: u64) -> u32 {
        let index = self.take(value);
        let next = self.node(at).next;
        *self.node(index) = Node { value, prev: Some(at), next, alive: true };
        self.node(at).next = Some(index);
        match next {
            Some(next) => self.node(next).prev = Some(index),
            None => self.tail = Some(index),
        }
        index
    }

    fn remove(&mut self, index: u32) {
        let Node { prev, next, .. } = *self.node(index);
        match prev {
            Some(prev) => self.node(prev).next = next,
            None => self.head = next,
        }
        match next {
            Some(next) => self.node(next).prev = prev,
            None => self.tail = prev,
        }
        self.node(index).alive = false;
        self.free[self.free_count] = index;
        self.free_count += 1;
    }

    fn draw(&self) {
        let nodes: Vec<String> = (0..self.made)
            .map(|i| {
                let n = self.memory[self.base + i];
                if n.alive {
                    format!("{i}: {}, prev {}, next {}", n.value, show(n.prev), show(n.next))
                } else {
                    format!("{i}: free")
                }
            })
            .collect();
        println!("    nodes    {}", nodes.join(" | "));
        let mut values = Vec::new();
        let mut at = self.head;
        while let Some(i) = at {
            if values.len() == self.made {
                break;
            }
            values.push(self.memory[self.base + i as usize].value.to_string());
            at = self.memory[self.base + i as usize].next;
        }
        println!("    list     {}", values.join(" "));
    }

    // Every place the list has lived, oldest first, and what's in each cell.
    fn draw_memory(&self) {
        let mut places = Vec::new();
        let (mut from, mut size) = (0, 2);
        while from <= self.base {
            let cells: Vec<String> = (from..from + size)
                .map(|c| if self.memory[c].alive { self.memory[c].value.to_string() } else { ".".to_string() })
                .collect();
            let what = if from == self.base { "the list" } else { "old copy" };
            places.push(format!("{from}..{} {what}: {}", from + size, cells.join(" ")));
            from += size;
            size *= 2;
        }
        println!("    memory   {}", places.join(" | "));
    }
}

// A link or a value, or - for none.
fn show(x: Option<impl fmt::Display>) -> String {
    x.map_or("-".to_string(), |x| x.to_string())
}

#[inline(never)]
fn broken(what: fmt::Arguments) {
    println!("    verify: {what}");
}

// The list's invariant, and the program's: walking from the head reaches
// every live node once, each node's prev is the one before it, the walk ends
// at the tail, and every node holds what the program last wrote to it.
fn verify(l: &List, wrote: &[Option<u64>; 8]) {
    let mut before = None;
    let mut at = l.head;
    let mut linked = 0;
    while let Some(i) = at {
        let n = l.memory[l.base + i as usize];
        if !n.alive {
            broken(format_args!("node {i} is linked, but it's free"));
        }
        if n.prev != before {
            broken(format_args!("node {i} comes after {}, but its prev is {}", show(before), show(n.prev)));
        }
        linked += 1;
        if linked > l.made {
            broken(format_args!("the links go round in a loop"));
            break;
        }
        before = at;
        at = n.next;
    }
    if l.tail != before {
        broken(format_args!("the list ends at {}, but the tail is {}", show(before), show(l.tail)));
    }
    for (i, &want) in wrote.iter().enumerate().take(l.made) {
        let n = l.memory[l.base + i];
        let holds = n.alive.then_some(n.value);
        if holds != want {
            broken(format_args!("node {i} holds {}, but the program last wrote {} to it", show(holds), show(want)));
        }
    }
    let alive = (0..l.made).filter(|&i| l.memory[l.base + i].alive).count();
    if linked != alive {
        broken(format_args!("{linked} nodes are linked, but {alive} are alive"));
    }
}

fn main() {
    let mut l = List::new();
    // What the program last wrote to each node, for verify.
    let mut wrote = [None; 8];
    println!("a list with room for 2 nodes, in cells 0..2 of memory");

    println!("push_back 10 as a");
    let a = l.push_back(10);
    wrote[a as usize] = Some(10);
    println!("    a is node {a}");
    l.draw();
    verify(&l, &wrote);

    println!("push_back 20 as b");
    let b = l.push_back(20);
    wrote[b as usize] = Some(20);
    println!("    b is node {b}");
    l.draw();
    verify(&l, &wrote);

    let kept = l.cell(b);
    println!("keep b's index, {b}, and where it is, cell {kept}, which is what a pointer holds: the same, until the list moves");

    println!("push_back 30 as c: the list is full");
    let c = l.push_back(30);
    wrote[c as usize] = Some(30);
    println!("    c is node {c}");
    l.draw();
    verify(&l, &wrote);

    if USE_INDEX.load(Ordering::Relaxed) {
        println!("write 21 into b through its index, {b}: cell {} now", l.cell(b));
        l.node(b).value = 21;
    } else {
        println!("write 21 into b through what was kept, cell {kept}: where b was before the move");
        l.memory[kept].value = 21;
    }
    wrote[b as usize] = Some(21);
    l.draw();
    l.draw_memory();
    verify(&l, &wrote);

    println!("push_front 5 as d");
    let d = l.push_front(5);
    wrote[d as usize] = Some(5);
    println!("    d is node {d}");
    l.draw();
    verify(&l, &wrote);

    println!("remove a: node {a} goes on the free list");
    l.remove(a);
    wrote[a as usize] = None;
    l.draw();
    verify(&l, &wrote);

    println!("insert 25 after b as e");
    let e = l.insert_after(b, 25);
    wrote[e as usize] = Some(25);
    println!("    e is node {e}: a's node, from the free list");
    l.draw();
    verify(&l, &wrote);

    println!("push_back 40 as f: the list is full again");
    let f = l.push_back(40);
    wrote[f as usize] = Some(40);
    println!("    f is node {f}");
    l.draw();
    l.draw_memory();
    verify(&l, &wrote);

    println!("read b through its index, {b}: {}", l.node(b).value);
}
