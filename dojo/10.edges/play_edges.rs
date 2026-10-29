// Dojo 10, the playground: memoized edges.
//
// In an archetype ECS, an entity that gains a component moves to the table
// whose signature has one more bit. Finding that table by its signature is a
// search. But a game moves entities between the same few tables over and
// over, so each table remembers where each component leads: an add edge per
// component, and a remove edge back. The tables and their edges form a graph,
// the archetype graph of the course's 2.6.
//
// Here the nodes are bare signatures, eight at most, over four components. A
// drone gains and loses components, and each move prints the node it went to
// and whether it took a lookup or followed an edge. verify() runs after every
// move: every known edge must lead to the node with that one component added
// (or removed) and have its edge back, and the drone's node must have the
// components the drone has. Put a breakpoint inside its failure branch to stop
// where the graph goes wrong.
//
// The switch: set MEMO to false and run it again. Nothing is remembered: the
// drone goes to the same nodes, but every move is a lookup. It's a static,
// read on every move, so a debugger can flip it mid-run too: stop on the
// first `lose` in main, set it to false in the watch window, and continue.

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

use std::sync::atomic::{AtomicBool, Ordering::Relaxed};

static MEMO: AtomicBool = AtomicBool::new(true);

const NAMES: [&str; 4] = ["position", "velocity", "health", "burning"];
const POSITION: u32 = 0;
const VELOCITY: u32 = 1;
const HEALTH: u32 = 2;
const BURNING: u32 = 3;

fn bit(id: u32) -> u64 {
    1 << id
}

#[derive(Clone, Copy)]
struct Node {
    signature: u64,
    // add[id] is the node with component id added, once a move has found it;
    // remove[id] the node with it removed.
    add: [Option<u32>; 4],
    remove: [Option<u32>; 4],
}

struct Graph {
    nodes: [Node; 8],
    count: u32,
    lookups: u32,
    edges: u32,
}

impl Graph {
    // The slow path: search every node for the signature, and make one if
    // none has it.
    fn find_or_make(&mut self, signature: u64) -> u32 {
        self.lookups += 1;
        if let Some(i) = self.nodes[..self.count as usize].iter().position(|n| n.signature == signature) {
            return i as u32;
        }
        self.nodes[self.count as usize] = Node { signature, add: [None; 4], remove: [None; 4] };
        self.count += 1;
        self.count - 1
    }

    fn with(&mut self, from: u32, id: u32) -> u32 {
        let memo = MEMO.load(Relaxed);
        if memo && let Some(to) = self.nodes[from as usize].add[id as usize] {
            self.edges += 1;
            return to;
        }
        let to = self.find_or_make(self.nodes[from as usize].signature | bit(id));
        if memo {
            self.nodes[from as usize].add[id as usize] = Some(to);
            self.nodes[to as usize].remove[id as usize] = Some(from);
        }
        to
    }

    fn without(&mut self, from: u32, id: u32) -> u32 {
        let memo = MEMO.load(Relaxed);
        if memo && let Some(to) = self.nodes[from as usize].remove[id as usize] {
            self.edges += 1;
            return to;
        }
        let to = self.find_or_make(self.nodes[from as usize].signature & !bit(id));
        if memo {
            self.nodes[from as usize].remove[id as usize] = Some(to);
            self.nodes[to as usize].add[id as usize] = Some(from);
        }
        to
    }
}

fn names(signature: u64) -> String {
    let names: Vec<&str> = (0..4).filter(|&id| signature & bit(id) != 0).map(|id| NAMES[id as usize]).collect();
    format!("{{{}}}", names.join(" "))
}

struct Drone {
    node: u32,
    // What the drone has, kept apart from the graph: verify checks the two
    // agree.
    has: u64,
}

// Every edge leads to the node one component away, and comes back; and the
// drone's node is the one with its components.
fn verify(g: &Graph, d: &Drone) {
    for (n, node) in g.nodes[..g.count as usize].iter().enumerate() {
        for id in 0..4 {
            if let Some(to) = node.add[id as usize] {
                let there = &g.nodes[to as usize];
                if there.signature != node.signature | bit(id) || there.remove[id as usize] != Some(n as u32) {
                    println!("verify: node {n}'s +{} edge leads to node {to} {}", NAMES[id as usize], names(there.signature));
                }
            }
            if let Some(to) = node.remove[id as usize] {
                let there = &g.nodes[to as usize];
                if there.signature != node.signature & !bit(id) || there.add[id as usize] != Some(n as u32) {
                    println!("verify: node {n}'s -{} edge leads to node {to} {}", NAMES[id as usize], names(there.signature));
                }
            }
        }
    }
    if g.nodes[d.node as usize].signature != d.has {
        println!("verify: the drone has {}, but its node {} is {}", names(d.has), d.node, names(g.nodes[d.node as usize].signature));
    }
}

fn report(g: &Graph, d: &Drone, what: &str, from: u32, lookups: u32) {
    let how = if g.lookups > lookups { "a lookup" } else { "an edge" };
    println!("{what}: node {from} -> {} {}, by {how}", d.node, names(g.nodes[d.node as usize].signature));
    verify(g, d);
}

fn gain(g: &mut Graph, d: &mut Drone, id: u32, what: &str) {
    let (from, lookups) = (d.node, g.lookups);
    d.node = g.with(d.node, id);
    d.has |= bit(id);
    report(g, d, what, from, lookups);
}

fn lose(g: &mut Graph, d: &mut Drone, id: u32, what: &str) {
    let (from, lookups) = (d.node, g.lookups);
    d.node = g.without(d.node, id);
    d.has &= !bit(id);
    report(g, d, what, from, lookups);
}

fn draw(g: &Graph) {
    for (n, node) in g.nodes[..g.count as usize].iter().enumerate() {
        let mut edges = Vec::new();
        for (id, name) in NAMES.iter().enumerate() {
            if let Some(to) = node.add[id] {
                edges.push(format!("+{name} {to}"));
            }
        }
        for (id, name) in NAMES.iter().enumerate() {
            if let Some(to) = node.remove[id] {
                edges.push(format!("-{name} {to}"));
            }
        }
        let edges = if edges.is_empty() { "no edges known".to_string() } else { edges.join(", ") };
        println!("    node {n} {}: {edges}", names(node.signature));
    }
}

fn main() {
    let empty = Node { signature: 0, add: [None; 4], remove: [None; 4] };
    let mut g = Graph { nodes: [empty; 8], count: 1, lookups: 0, edges: 0 };
    let mut d = Drone { node: 0, has: 0 };
    println!("the drone starts at node 0 {{}}");
    gain(&mut g, &mut d, POSITION, "gains position");
    gain(&mut g, &mut d, VELOCITY, "gains velocity");
    gain(&mut g, &mut d, HEALTH, "gains health");
    gain(&mut g, &mut d, BURNING, "catches fire");
    lose(&mut g, &mut d, BURNING, "the fire goes out");
    gain(&mut g, &mut d, BURNING, "catches fire again");
    lose(&mut g, &mut d, BURNING, "the fire goes out");
    println!("the graph so far:");
    draw(&g);
    lose(&mut g, &mut d, VELOCITY, "stops");
    gain(&mut g, &mut d, BURNING, "catches fire");
    gain(&mut g, &mut d, VELOCITY, "moves again");
    lose(&mut g, &mut d, BURNING, "the fire goes out");
    println!("the graph:");
    draw(&g);
    println!("moves that looked up a node by signature: {}; that followed an edge: {}", g.lookups, g.edges);
}
