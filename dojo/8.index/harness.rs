// The harness for dojo 8, index not pointer. It is fixed. Your work is
// kata.rs, beside it: kata.rs brings this file in as a module and its main
// calls run(), so the two are one program. The reference, solution.rs, and
// each level file do the same, so each of them builds on its own, and this
// file never does. Build and run yours with
//
//   python tools/dojo.py index
//
// First a short scenario, the playground's steps, each printed as it passes.
// Then a fuzz run: thousands of random pushes, inserts, removes, reads and
// writes from a seed, against a model that keeps the live nodes in list order
// and each node's value. After every operation the harness walks the list
// both ways, head to tail by next() and tail to head by prev(), so a link left
// wrong is caught by the operation that left it. The list grows to about a
// thousand nodes, so its Vec moves many times, and every node is checked by
// the index it was given.

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

use crate::dojo::{Rng, Run};
use crate::List;

// The values, head to tail, following the links; at most `limit` of them, in
// case a link loops back.
fn values(l: &List, limit: usize) -> String {
    let mut values = Vec::new();
    let mut at = l.head();
    while let Some(node) = at {
        if values.len() == limit {
            break;
        }
        values.push(show(l.get(node)));
        at = l.next(node);
    }
    values.join(" ")
}

fn show<T: ToString>(value: Option<T>) -> String {
    value.map_or("None".to_string(), |v| v.to_string())
}

fn scenario(run: &Run) {
    println!("scenario");
    let mut l = List::new();
    let (a, b, c) = (l.push_back(10), l.push_back(20), l.push_back(30));
    run.same("push_back 10, 20, 30 as a, b, c", format!("{a} {b} {c}"), "0 1 2");
    if let Some(v) = l.get_mut(b) {
        *v = 21;
    }
    run.same("write 21 into b, by its index", values(&l, 10), "10 21 30");
    let d = l.push_front(5);
    run.same("push_front 5 as d", d, "3");
    run.same("values", values(&l, 10), "5 10 21 30");
    run.same("remove a", show(l.remove(a)), "10");
    run.same("remove a again", show(l.remove(a)), "None");
    run.same("get a", show(l.get(a)), "None");
    let e = l.insert_after(b, 25);
    run.same("insert 25 after b as e: a's node, from the free list", show(e), "0");
    run.same("values", values(&l, 10), "5 21 25 30");
    for v in 100..200 {
        l.push_back(v);
    }
    run.same("push_back 100 more, so the nodes move; get b by its index", show(l.get(b)), "21");
    run.same("remove c", show(l.remove(c)), "30");
    run.same("insert after c, now removed", show(l.insert_after(c, 1)), "None");
    run.same("get node 1000, never made", show(l.get(1000)), "None");
    run.same("len", l.len(), "103");
    run.same("the first 4 values", values(&l, 4), "5 21 25 100");
    run.same("head, tail", format!("{} {}", show(l.head()), show(l.tail())), "3 103");
    run.same("next(b), prev(b)", format!("{} {}", show(l.next(b)), show(l.prev(b))), "0 3");
}

// What a correct list must do, kept the simple way: the live nodes in list
// order, each node's value while it's alive, and the freed nodes waiting to
// be used again.
struct Model {
    order: Vec<u32>,
    value: Vec<Option<u64>>,
    free: Vec<u32>,
}

// A new node must be a freed one if any are waiting, and the next new index
// if not.
fn took(run: &Run, op: u64, model: &mut Model, node: u32, what: &str) {
    if model.free.is_empty() {
        run.check(op, node as usize == model.value.len(), || {
            format!("{what} returned node {node} with nothing free; the next new node is {}", model.value.len())
        });
        model.value.push(None);
    } else {
        let at = model.free.iter().position(|&f| f == node);
        run.check(op, at.is_some(), || {
            format!("{what} returned node {node}, but it isn't free, and {} freed nodes wait", model.free.len())
        });
        model.free.swap_remove(at.unwrap_or_default());
    }
}

// The links, both ways: head to tail by next() must visit the model's nodes
// in order, and tail to head by prev() the same nodes backwards. A walk stops
// at the first node that's wrong, so a loop in the links can't hang it.
fn links_agree(run: &Run, op: u64, l: &List, model: &Model) {
    let n = model.order.len();
    let mut at = l.head();
    for (i, &want) in model.order.iter().enumerate() {
        run.check(op, at == Some(want), || {
            let came = if i == 0 { "head()".to_string() } else { format!("next({})", model.order[i - 1]) };
            format!("walking head to tail, {came} gives {}, but the model has node {want} there, at position {i} of {n}", show(at))
        });
        at = l.next(want);
    }
    run.check(op, at.is_none(), || match model.order.last() {
        Some(last) => format!("walking head to tail, next({last}) gives {}, but {last} is the last of {n} nodes", show(at)),
        None => format!("head() gives {}, but the list is empty", show(at)),
    });
    let mut at = l.tail();
    for (i, &want) in model.order.iter().enumerate().rev() {
        run.check(op, at == Some(want), || {
            let came = if i + 1 == n { "tail()".to_string() } else { format!("prev({})", model.order[i + 1]) };
            format!("walking tail to head, {came} gives {}, but the model has node {want} there, at position {i} of {n}", show(at))
        });
        at = l.prev(want);
    }
    run.check(op, at.is_none(), || match model.order.first() {
        Some(first) => format!("walking tail to head, prev({first}) gives {}, but {first} is the first of {n} nodes", show(at)),
        None => format!("tail() gives {}, but the list is empty", show(at)),
    });
}

// Every node by its index, live and free.
fn all_agree(run: &Run, op: u64, l: &List, model: &Model) {
    for (node, &value) in model.value.iter().enumerate() {
        let node = node as u32;
        run.check(op, l.get(node) == value, || format!("get({node}) gives {}, the model says {}", show(l.get(node)), show(value)));
    }
}

fn fuzz(run: &Run) {
    run.fuzz();
    let mut rng = Rng::new(run.seed);
    let mut l = List::new();
    let mut model = Model { order: Vec::new(), value: Vec::new(), free: Vec::new() };
    for op in 1..=run.ops {
        let roll = rng.below(100);
        let value = op;
        if roll < 20 || model.order.is_empty() {
            run.before(op, format_args!("push_back({value})"));
            let node = l.push_back(value);
            took(run, op, &mut model, node, "push_back");
            model.value[node as usize] = Some(value);
            model.order.push(node);
        } else if roll < 30 {
            run.before(op, format_args!("push_front({value})"));
            let node = l.push_front(value);
            took(run, op, &mut model, node, "push_front");
            model.value[node as usize] = Some(value);
            model.order.insert(0, node);
        } else if roll < 45 {
            let at = rng.below(model.order.len() as u64) as usize;
            let after = model.order[at];
            run.before(op, format_args!("insert_after({after}, {value})"));
            let got = l.insert_after(after, value);
            run.check(op, got.is_some(), || format!("insert_after({after}, {value}) gave None, but node {after} is in the list"));
            let node = got.unwrap_or_default();
            took(run, op, &mut model, node, "insert_after");
            model.value[node as usize] = Some(value);
            model.order.insert(at + 1, node);
        } else if roll < 85 {
            let at = rng.below(model.order.len() as u64) as usize;
            let node = model.order.remove(at);
            let want = model.value[node as usize].take();
            run.before(op, format_args!("remove({node})"));
            let got = l.remove(node);
            run.check(op, got == want, || format!("remove({node}) returned {}, the model says {}", show(got), show(want)));
            model.free.push(node);
        } else if roll < 88 && !model.free.is_empty() {
            let node = rng.pick(&model.free);
            run.before(op, format_args!("remove({node}), a freed node, and insert_after({node}, {value})"));
            let len = l.len();
            let removed = l.remove(node);
            let inserted = l.insert_after(node, value);
            run.check(op, removed.is_none() && inserted.is_none() && l.len() == len, || {
                format!("node {node} is free, so remove({node}) and insert_after({node}, {value}) should give None and change nothing")
            });
            run.check(op, l.get(node).is_none() && l.next(node).is_none() && l.prev(node).is_none(), || {
                format!("node {node} is free, so get, next and prev should give None; they give {}, {}, {}",
                        show(l.get(node)), show(l.next(node)), show(l.prev(node)))
            });
        } else if roll < 96 {
            let node = model.order[rng.below(model.order.len() as u64) as usize];
            run.before(op, format_args!("get_mut({node}), then add 1"));
            if let Some(v) = l.get_mut(node) {
                *v += 1;
            }
            if let Some(v) = model.value[node as usize].as_mut() {
                *v += 1;
            }
            let want = model.value[node as usize];
            run.check(op, l.get(node) == want, || format!("after get_mut({node}), get({node}) gives {}, the model says {}", show(l.get(node)), show(want)));
        } else {
            let node = rng.below(model.value.len() as u64 + 2) as u32;
            run.before(op, format_args!("get({node})"));
            let want = model.value.get(node as usize).copied().flatten();
            run.check(op, l.get(node) == want, || format!("get({node}) gives {}, the model says {}", show(l.get(node)), show(want)));
        }
        run.check(op, l.len() == model.order.len(), || format!("len() is {}, the model has {}", l.len(), model.order.len()));
        links_agree(run, op, &l, &model);
        if op % 256 == 0 {
            all_agree(run, op, &l, &model);
        }
    }
    all_agree(run, run.ops, &l, &model);
    run.pass();
}

// The kata's main calls this.
pub fn run() {
    let run = Run::new("index", "8. index, not pointer");
    scenario(&run);
    fuzz(&run);
}
