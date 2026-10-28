// The harness for dojo 2, swap-remove and the back-reference. It is fixed:
// your work is kata.rs, beside it. Run both with
//
//   python tools/dojo.py swap_remove
//
// First a short scenario, the same steps as the playground, each printed as
// it passes. Then a fuzz run: thousands of random inserts and removes over a
// small range of ids, so the same ids come and go many times, each checked
// against a model that is a plain map from id to value. Every few operations
// it also checks the packed rows themselves: no holes, each row's owner holds
// exactly that row's value.

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

#[path = "../dojo.rs"]
mod dojo;
mod kata;

use dojo::{Rng, Run};
use kata::SparseSet;
use std::collections::HashMap;

fn list<T: ToString>(items: &[T]) -> String {
    items.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(" ")
}

fn show(value: Option<&u64>) -> String {
    value.map_or("None".to_string(), |v| v.to_string())
}

fn scenario(run: &Run) {
    println!("scenario");
    let mut s = SparseSet::new();
    for id in [3, 7, 1, 5] {
        s.insert(id, id as u64 * 10);
    }
    run.same("insert 3, 7, 1, 5: ids by row", list(s.ids()), "3 7 1 5");
    run.same("values by row", list(s.values()), "30 70 10 50");
    run.same("remove 7", show(s.remove(7).as_ref()), "70");
    run.same("the last row moved into row 1: ids", list(s.ids()), "3 5 1");
    run.same("values", list(s.values()), "30 50 10");
    run.same("get 5, through its fixed row", show(s.get(5)), "50");
    run.same("get 7", show(s.get(7)), "None");
    run.same("remove 7 again", show(s.remove(7).as_ref()), "None");
    run.same("insert 3 again: the old value comes back", show(s.insert(3, 33).as_ref()), "30");
    run.same("insert 7: a new row at the end", show(s.insert(7, 77).as_ref()), "None");
    run.same("ids", list(s.ids()), "3 5 1 7");
    if let Some(v) = s.get_mut(1) {
        *v += 1;
    }
    run.same("get_mut 1, add 1", show(s.get(1)), "11");
    run.same("len", s.len(), "4");
}

// The rows, checked against the model: packed, every row's owner holds that
// row's value, and every id the model has is in a row.
fn rows_agree(run: &Run, op: u64, s: &SparseSet<u64>, model: &HashMap<u32, u64>) {
    run.check(op, s.values().len() == s.len() && s.ids().len() == s.len(), || {
        format!("len() is {}, values() has {}, ids() has {}", s.len(), s.values().len(), s.ids().len())
    });
    run.check(op, s.len() == model.len(), || format!("len() is {}, the model has {}", s.len(), model.len()));
    for (row, (&id, &value)) in s.ids().iter().zip(s.values()).enumerate() {
        run.check(op, model.get(&id) == Some(&value), || {
            format!("row {row} says id {id} holds {value}; the model says id {id} holds {}", show(model.get(&id)))
        });
        run.check(op, s.get(id) == Some(&value), || {
            format!("row {row} holds {id}'s value {value}, but get({id}) gives {}", show(s.get(id)))
        });
    }
}

fn fuzz(run: &Run) {
    run.fuzz();
    let mut rng = Rng::new(run.seed);
    let mut s = SparseSet::new();
    let mut model: HashMap<u32, u64> = HashMap::new();
    for op in 1..=run.ops {
        // Mostly small ids, so the same ones come and go; now and then a big
        // one, so the sparse side has to grow.
        let id = if rng.below(50) == 0 { rng.below(5000) as u32 } else { rng.below(48) as u32 };
        let roll = rng.below(100);
        if roll < 45 {
            let value = op * 1000 + id as u64;
            run.before(op, format_args!("insert({id}, {value})"));
            let got = s.insert(id, value);
            let want = model.insert(id, value);
            run.check(op, got == want, || {
                format!("insert({id}, {value}) returned {}, the model says {}", show(got.as_ref()), show(want.as_ref()))
            });
        } else if roll < 85 {
            run.before(op, format_args!("remove({id})"));
            let got = s.remove(id);
            let want = model.remove(&id);
            run.check(op, got == want, || {
                format!("remove({id}) returned {}, the model says {}", show(got.as_ref()), show(want.as_ref()))
            });
        } else if roll < 95 {
            run.before(op, format_args!("get({id})"));
            let got = s.get(id).copied();
            run.check(op, got.as_ref() == model.get(&id), || {
                format!("get({id}) returned {}, the model says {}", show(got.as_ref()), show(model.get(&id)))
            });
        } else {
            run.before(op, format_args!("get_mut({id}), then add 1"));
            if let Some(v) = s.get_mut(id) {
                *v += 1;
            }
            if let Some(v) = model.get_mut(&id) {
                *v += 1;
            }
        }
        run.check(op, s.get(id) == model.get(&id), || {
            format!("after that, get({id}) is {}, the model says {}", show(s.get(id)), show(model.get(&id)))
        });
        if op % 64 == 0 {
            rows_agree(run, op, &s, &model);
        }
    }
    rows_agree(run, run.ops, &s, &model);
    run.pass();
}

fn main() {
    let run = Run::new("swap_remove", "2. swap-remove and the back-reference");
    scenario(&run);
    fuzz(&run);
}
