// The harness for dojo 10, memoized edges. It is fixed. Your work is kata.rs,
// beside it: kata.rs brings this file in as a module and its main calls run(),
// so the two are one program. The reference, solution.rs, and each level file
// do the same, so each of them builds on its own, and this file never does.
// Build and run yours with
//
//   python tools/dojo.py edges
//
// First a short scenario, the playground's moves, each printed as it passes.
// Then a fuzz run: thousands of random moves from a seed, from random nodes,
// with ids up to 63, against a model that knows each node's signature and
// which edges a graph that remembers both directions knows by now. After every
// move it checks the node the move gave, its signature, and the two counters:
// a move along a known edge is a hit, any other move that changes the
// signature is one lookup, and a move that changes nothing is neither.

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

use crate::Graph;
use crate::dojo::{Rng, Run};
use std::collections::{HashMap, HashSet};

fn bit(id: u32) -> u64 {
    1 << id
}

// A signature as its ids: {0 2 3}.
fn ids(signature: u64) -> String {
    let ids: Vec<String> = (0..64).filter(|&id| signature & bit(id) != 0).map(|id| id.to_string()).collect();
    format!("{{{}}}", ids.join(" "))
}

fn counts(g: &Graph) -> String {
    format!("{} {}", g.lookups(), g.hits())
}

fn scenario(run: &Run) {
    println!("scenario");
    let mut g = Graph::new();
    run.same("new: how many nodes", g.len(), "1");
    run.same("node 0's signature", ids(g.signature(0)), "{}");
    let a = g.with(0, 0);
    run.same("with(0, 0), position: a new node", a, "1");
    run.same("its signature", ids(g.signature(a)), "{0}");
    let b = g.with(a, 1);
    run.same("with(1, 1), velocity", b, "2");
    let c = g.with(b, 2);
    run.same("with(2, 2), health", c, "3");
    let d = g.with(c, 3);
    run.same("with(3, 3), burning", d, "4");
    run.same("its signature", ids(g.signature(d)), "{0 1 2 3}");
    run.same("lookups and hits", counts(&g), "4 0");
    run.same("without(4, 3): the fire goes out, by the edge back", g.without(d, 3), "3");
    run.same("with(3, 3): it catches fire again, by the edge", g.with(c, 3), "4");
    run.same("lookups and hits", counts(&g), "4 2");
    run.same("with(4, 3): it has 3 already, so it stays", g.with(d, 3), "4");
    run.same("without(0, 3): it has no 3 to lose", g.without(0, 3), "0");
    run.same("lookups and hits: nothing moved", counts(&g), "4 2");
    let e = g.without(c, 1);
    run.same("without(3, 1): it stops", e, "5");
    run.same("its signature", ids(g.signature(e)), "{0 2}");
    let f = g.with(e, 3);
    run.same("with(5, 3)", f, "6");
    run.same("with(6, 1): node 4 is known, the edge to it isn't", g.with(f, 1), "4");
    run.same("lookups and hits", counts(&g), "7 2");
    run.same("without(4, 1): the edge back", g.without(d, 1), "6");
    run.same("lookups and hits", counts(&g), "7 3");
    run.same("len", g.len(), "7");
}

// The ids the fuzz run uses: 31, 32 and 63 catch a bit made in 32 bits.
const IDS: [u32; 8] = [0, 1, 2, 3, 5, 31, 32, 63];

fn fuzz(run: &Run) {
    run.fuzz();
    let mut rng = Rng::new(run.seed);
    let mut g = Graph::new();
    // The model: each node's signature, in the order the graph made them;
    // each signature's node; the edges a graph that remembers both directions
    // knows by now, as (node, id, adding); and the two counters.
    let mut nodes: Vec<u64> = vec![0];
    let mut by_signature: HashMap<u64, u32> = HashMap::from([(0, 0)]);
    let mut known: HashSet<(u32, u32, bool)> = HashSet::new();
    let (mut lookups, mut hits) = (0u32, 0u32);
    for op in 1..=run.ops {
        let node = rng.below(nodes.len() as u64) as u32;
        let id = rng.pick(&IDS);
        let signature = nodes[node as usize];
        let has = signature & bit(id) != 0;
        // Mostly the move that changes the signature; now and then the one
        // that doesn't.
        let adding = if rng.below(5) == 0 { has } else { !has };
        let want = if adding { signature | bit(id) } else { signature & !bit(id) };
        let call = format!("{}({node}, {id})", if adding { "with" } else { "without" });
        run.before(op, &call);
        let got = if adding { g.with(node, id) } else { g.without(node, id) };
        run.check(op, (got as usize) < g.len(), || format!("{call} gave node {got}, but there are {} nodes", g.len()));
        run.check(op, g.signature(got) == want, || {
            format!("{call} gave node {got}, which is {}; from {} it should be {}", ids(g.signature(got)), ids(signature), ids(want))
        });
        match by_signature.get(&want) {
            Some(&m) => run.check(op, got == m, || format!("{call} gave node {got}, but node {m} is {} already", ids(want))),
            None => {
                run.check(op, got as usize == nodes.len(), || {
                    format!("{call} made node {got}; the next new node is {}", nodes.len())
                });
                nodes.push(want);
                by_signature.insert(want, got);
            }
        }
        if want != signature {
            if known.contains(&(node, id, adding)) {
                hits += 1;
            } else {
                lookups += 1;
                known.insert((node, id, adding));
                known.insert((got, id, !adding));
            }
        }
        run.check(op, g.lookups() == lookups && g.hits() == hits, || {
            format!(
                "after {call}, lookups and hits are {} and {}; the model says {lookups} and {hits} (a known edge is a hit, an unknown one a lookup)",
                g.lookups(),
                g.hits()
            )
        });
        run.check(op, g.len() == nodes.len(), || format!("len() is {}, the model has {}", g.len(), nodes.len()));
    }
    run.pass();
}

// The kata's main calls this.
pub fn run() {
    let run = Run::new("edges", "10. memoized edges");
    scenario(&run);
    fuzz(&run);
}
