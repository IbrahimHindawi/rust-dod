// The harness for dojo 9, interning. It is fixed. Your work is kata.rs,
// beside it: kata.rs brings this file in as a module and its main calls run(),
// so the two are one program. The reference, solution.rs, and each level file
// do the same, so each of them builds on its own, and this file never does.
// Build and run yours with
//
//   python tools/dojo.py interning
//
// First a short scenario, the playground's steps, each printed as it passes.
// Then a fuzz run: thousands of random interns and finds from a seed, against
// a model that is std's HashMap and a Vec of the keys in the order they came.
// The keys are the kinds a hash table meets in an ECS: small numbers, 0
// among them; signatures with a few bits set; keys that differ only in their
// high bits; and any u64 at all. Ids must be dense, in the order keys were
// first interned, and never change; find must agree with intern; and key(id)
// must give the key back.

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

use crate::dojo::{Rng, Run};
use crate::Interner;
use std::collections::HashMap;

fn show(value: Option<u32>) -> String {
    value.map_or("None".to_string(), |v| v.to_string())
}

fn scenario(run: &Run) {
    println!("scenario");
    let mut t = Interner::new();
    let (player, drone, turret, rock, chest, tank, bird) = (31, 47, 103, 7, 135, 111, 15);
    let ids = [t.intern(player), t.intern(drone), t.intern(turret)];
    run.same("intern player, drone, turret", format!("{} {} {}", ids[0], ids[1], ids[2]), "0 1 2");
    run.same("intern drone again", t.intern(drone), "1");
    let ids = [t.intern(rock), t.intern(chest)];
    run.same("intern rock, chest", format!("{} {}", ids[0], ids[1]), "3 4");
    run.same("find tank", show(t.find(tank)), "None");
    run.same("intern tank", t.intern(tank), "5");
    run.same("intern bird: the table grows", t.intern(bird), "6");
    run.same("find turret", show(t.find(turret)), "2");
    run.same("key(2)", t.key(2), "103");
    run.same("len", t.len(), "7");
    run.same("intern 0", t.intern(0), "7");
    run.same("find 0", show(t.find(0)), "7");
    let ids = [t.intern(1 << 63), t.intern(1 << 62)];
    run.same("intern 1 << 63, 1 << 62: only high bits", format!("{} {}", ids[0], ids[1]), "8 9");
    run.same("find 1 << 61", show(t.find(1 << 61)), "None");
    run.same("key(8)", t.key(8), "9223372036854775808");
    let firsts: Vec<u32> = (0..10).map(|i| t.intern(1000 + i)).collect();
    run.same("intern 10 more: the table grows again; the first", firsts[0], "10");
    run.same("find each key so far", (0..20).all(|id| t.find(t.key(id)) == Some(id)), "true");
}

// A key of the kinds an ECS's tables meet. The same draws, in the same order,
// as rin-dod's harness, so a seed is the same run in both.
fn fresh_key(rng: &mut Rng) -> u64 {
    match rng.below(4) {
        0 => rng.below(64),
        1 => {
            let bits = rng.below(4) + 1;
            let mut signature = 0;
            for _ in 0..bits {
                signature |= 1 << rng.below(64);
            }
            signature
        }
        2 => rng.below(1 << 16) << 48,
        _ => rng.next(),
    }
}

struct Model {
    ids: HashMap<u64, u32>,
    keys: Vec<u64>,
}

// Everything at once: every id's key, and every key's id.
fn all_agree(run: &Run, op: u64, t: &Interner, model: &Model) {
    run.check(op, t.len() == model.keys.len(), || format!("len() is {}, the model has {}", t.len(), model.keys.len()));
    for (id, &key) in model.keys.iter().enumerate() {
        let id = id as u32;
        run.check(op, t.key(id) == key, || format!("key({id}) gives {}, but id {id} is {key}'s", t.key(id)));
        run.check(op, t.find(key) == Some(id), || format!("find({key}) gives {}, but {key} is id {id}", show(t.find(key))));
    }
}

fn fuzz(run: &Run) {
    run.fuzz();
    let mut rng = Rng::new(run.seed);
    let mut t = Interner::new();
    let mut model = Model { ids: HashMap::new(), keys: Vec::new() };
    for op in 1..=run.ops {
        let roll = rng.below(100);
        if roll < 45 || model.keys.is_empty() {
            let key = fresh_key(&mut rng);
            run.before(op, format_args!("intern({key})"));
            let got = t.intern(key);
            let next = model.keys.len() as u32;
            let want = *model.ids.entry(key).or_insert(next);
            if want == next {
                model.keys.push(key);
            }
            run.check(op, got == want, || {
                if want == next {
                    format!("intern({key}) gives {got}, but {key} is new, so its id is the next one, {want}")
                } else {
                    format!("intern({key}) gives {got}, but {key} was interned before, as {want}")
                }
            });
        } else if roll < 70 {
            let id = rng.below(model.keys.len() as u64) as u32;
            let key = model.keys[id as usize];
            run.before(op, format_args!("intern({key}), interned before"));
            let got = t.intern(key);
            run.check(op, got == id, || format!("intern({key}) gives {got}, but {key} was interned before, as {id}"));
        } else if roll < 85 {
            let key = fresh_key(&mut rng);
            run.before(op, format_args!("find({key})"));
            let got = t.find(key);
            let want = model.ids.get(&key).copied();
            run.check(op, got == want, || format!("find({key}) gives {}, the model says {}", show(got), show(want)));
        } else if roll < 95 {
            let id = rng.below(model.keys.len() as u64) as u32;
            let key = model.keys[id as usize];
            run.before(op, format_args!("find({key}), interned before"));
            let got = t.find(key);
            run.check(op, got == Some(id), || format!("find({key}) gives {}, but {key} is id {id}", show(got)));
        } else {
            let id = rng.below(model.keys.len() as u64) as u32;
            run.before(op, format_args!("key({id})"));
            let got = t.key(id);
            run.check(op, got == model.keys[id as usize], || format!("key({id}) gives {got}, but id {id} is {}'s", model.keys[id as usize]));
        }
        run.check(op, t.len() == model.keys.len(), || format!("len() is {}, the model has {}", t.len(), model.keys.len()));
        if op % 256 == 0 {
            all_agree(run, op, &t, &model);
        }
    }
    all_agree(run, run.ops, &t, &model);
    run.pass();
}

// The kata's main calls this.
pub fn run() {
    let run = Run::new("interning", "9. interning");
    scenario(&run);
    fuzz(&run);
}
