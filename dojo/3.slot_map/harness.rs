// The harness for dojo 3, the slot map. It is fixed. Your work is kata.rs,
// beside it: kata.rs brings this file in as a module and its main calls run(),
// so the two are one program. The reference, solution.rs, and each level file
// do the same, so each of them builds on its own, and this file never does.
// Build and run yours with
//
//   python tools/dojo.py slot_map
//
// First a short scenario, the same steps as the playground, each printed as
// it passes. Then a fuzz run: thousands of random inserts, removes, reads and
// writes from a seed, against a model that maps every live handle to its value
// and keeps every dead one. It checks dojo 1's rules for the handles (never
// handed out twice, a freed index reused first, dead ones stay dead) and dojo
// 2's for the rows (packed, each row's handle holding that row's value).

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

use crate::dojo::{Handle, Rng, Run};
use crate::SlotMap;
use std::collections::{HashMap, HashSet};

fn list<T: ToString>(items: &[T]) -> String {
    items.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(" ")
}

fn show(value: Option<&u64>) -> String {
    value.map_or("None".to_string(), |v| v.to_string())
}

fn scenario(run: &Run) {
    println!("scenario");
    let mut m = SlotMap::new();
    let (a, b, c) = (m.insert(10), m.insert(20), m.insert(30));
    run.same("insert 10, 20, 30 as a, b, c", format!("{a} {b} {c}"), "0:1 1:1 2:1");
    run.same("remove b", show(m.remove(b).as_ref()), "20");
    run.same("c moved into b's row: handles by row", list(m.handles()), "0:1 2:1");
    run.same("values by row", list(m.values()), "10 30");
    run.same("get c, through its slot", show(m.get(c)), "30");
    run.same("get b", show(m.get(b)), "None");
    run.same("contains b", m.contains(b), "false");
    run.same("remove b again", show(m.remove(b).as_ref()), "None");
    let d = m.insert(40);
    run.same("insert 40 as d: b's index, next life", d, "1:2");
    run.same("get b, now that its index is d's", show(m.get(b)), "None");
    if let Some(v) = m.get_mut(a) {
        *v += 1;
    }
    run.same("get_mut a, add 1", show(m.get(a)), "11");
    run.same("len", m.len(), "3");
}

struct Model {
    alive: HashMap<Handle, u64>,
    live: Vec<Handle>,
    dead: Vec<Handle>,
    issued: HashSet<Handle>,
    free: HashSet<u32>,
    indices: usize,
}

// Everything at once: the rows against the model, and every handle, live and
// dead.
fn all_agree(run: &Run, op: u64, m: &SlotMap<u64>, model: &Model) {
    run.check(op, m.len() == model.alive.len(), || format!("len() is {}, the model has {}", m.len(), model.alive.len()));
    run.check(op, m.values().len() == m.len() && m.handles().len() == m.len(), || {
        format!("len() is {}, values() has {}, handles() has {}", m.len(), m.values().len(), m.handles().len())
    });
    for (row, (&h, &value)) in m.handles().iter().zip(m.values()).enumerate() {
        run.check(op, model.alive.get(&h) == Some(&value), || {
            format!("row {row} says {h} holds {value}; the model says {h} holds {}", show(model.alive.get(&h)))
        });
    }
    for (&h, &value) in &model.alive {
        run.check(op, m.get(h) == Some(&value), || format!("get({h}) gives {}, the model says {value}", show(m.get(h))));
    }
    for &h in &model.dead {
        run.check(op, !m.contains(h) && m.get(h).is_none(), || format!("{h} was removed, but the map still has it"));
    }
}

fn fuzz(run: &Run) {
    run.fuzz();
    let mut rng = Rng::new(run.seed);
    let mut m = SlotMap::new();
    let mut model = Model {
        alive: HashMap::new(),
        live: Vec::new(),
        dead: Vec::new(),
        issued: HashSet::new(),
        free: HashSet::new(),
        indices: 0,
    };
    for op in 1..=run.ops {
        let roll = rng.below(100);
        if roll < 45 || model.live.is_empty() {
            let value = op;
            run.before(op, format_args!("insert({value})"));
            let h = m.insert(value);
            run.check(op, !model.issued.contains(&h), || format!("insert returned {h}, which was handed out before"));
            if model.free.is_empty() {
                run.check(op, h.index as usize == model.indices, || {
                    format!("insert returned {h} with nothing free; the next new index is {}", model.indices)
                });
                model.indices += 1;
            } else {
                run.check(op, model.free.remove(&h.index), || {
                    format!("insert returned {h}, but index {} is not free, and {} freed indices wait", h.index, model.free.len())
                });
            }
            run.check(op, m.get(h) == Some(&value), || format!("insert({value}) returned {h}, and get({h}) gives {}", show(m.get(h))));
            model.issued.insert(h);
            model.alive.insert(h, value);
            model.live.push(h);
        } else if roll < 75 {
            let at = rng.below(model.live.len() as u64) as usize;
            let h = model.live.swap_remove(at);
            let want = model.alive.remove(&h);
            run.before(op, format_args!("remove({h})"));
            let got = m.remove(h);
            run.check(op, got == want, || format!("remove({h}) returned {}, the model says {}", show(got.as_ref()), show(want.as_ref())));
            run.check(op, !m.contains(h), || format!("after remove({h}), contains({h}) is still true"));
            model.free.insert(h.index);
            model.dead.push(h);
        } else if roll < 80 && !model.dead.is_empty() {
            let h = rng.pick(&model.dead);
            run.before(op, format_args!("remove({h}), a dead handle"));
            let len = m.len();
            let got = m.remove(h);
            run.check(op, got.is_none() && m.len() == len, || format!("remove({h}) of a dead handle should give None and change nothing"));
        } else if roll < 90 {
            let h = if !model.dead.is_empty() && rng.below(3) == 0 { rng.pick(&model.dead) } else { rng.pick(&model.live) };
            run.before(op, format_args!("get({h})"));
            let got = m.get(h).copied();
            run.check(op, got.as_ref() == model.alive.get(&h), || {
                format!("get({h}) gives {}, the model says {}", show(got.as_ref()), show(model.alive.get(&h)))
            });
        } else {
            let h = rng.pick(&model.live);
            run.before(op, format_args!("get_mut({h}), then add 1"));
            if let Some(v) = m.get_mut(h) {
                *v += 1;
            }
            if let Some(v) = model.alive.get_mut(&h) {
                *v += 1;
            }
            run.check(op, m.get(h) == model.alive.get(&h), || {
                format!("after get_mut({h}), get({h}) gives {}, the model says {}", show(m.get(h)), show(model.alive.get(&h)))
            });
        }
        run.check(op, m.len() == model.alive.len(), || format!("len() is {}, the model has {}", m.len(), model.alive.len()));
        if op % 256 == 0 {
            all_agree(run, op, &m, &model);
        }
    }
    all_agree(run, run.ops, &m, &model);
    run.pass();
}

// The kata's main calls this.
pub fn run() {
    let run = Run::new("slot_map", "3. the slot map");
    scenario(&run);
    fuzz(&run);
}
