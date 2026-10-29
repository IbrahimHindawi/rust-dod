// The harness for dojo 6, bitsets. It is fixed. Your work is kata.rs, beside
// it: kata.rs brings this file in as a module and its main calls run(), so
// the two are one program. The reference, solution.rs, and each level file do
// the same, so each of them builds on its own, and this file never does. Build
// and run yours with
//
//   python tools/dojo.py bitset
//
// First a short scenario, the playground's kinds and queries, each step
// printed as it passes. Then a fuzz run: a set changed and asked about
// thousands of times from a seed, against a model that keeps a bool per id
// and answers every question one id at a time. Ids go up to 63, so a shift
// done in 32 bits shows.

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

use crate::dojo::{Rng, Run};
use crate::{bit, contains_all, count, has, ids, intersects, matches, with, without};

// The model: a set as a bool per id.
type Bools = [bool; 64];

fn to_set(b: &Bools) -> u64 {
    (0..64).filter(|&i| b[i]).fold(0, |set, i| set | 1u64 << i)
}

// A set as its ids, {1 5 40}.
fn list(set: u64) -> String {
    let ids: Vec<String> = (0..64u32).filter(|&i| set >> i & 1 == 1).map(|i| i.to_string()).collect();
    format!("{{{}}}", ids.join(" "))
}

fn words(ids: &[u32]) -> String {
    ids.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(" ")
}

fn of(ids: &[u32]) -> u64 {
    ids.iter().fold(0, |set, &id| with(set, id))
}

fn scenario(run: &Run) {
    println!("scenario");
    let (position, velocity, health, sprite, player, turret, frozen) = (0, 1, 2, 3, 4, 5, 6);
    let hero = of(&[position, velocity, health, sprite, player]);
    run.same("with 0, 1, 2, 3 and 4, from 0: the player", hero, "31");
    run.same("bit(3)", bit(3), "8");
    run.same("has(player, sprite)", has(hero, sprite), "true");
    run.same("has(player, turret)", has(hero, turret), "false");
    let gun = of(&[position, health, sprite, turret]);
    run.same("the turret", gun, "45");
    let moving = of(&[position, velocity]);
    run.same("contains_all(player, {position velocity})", contains_all(hero, moving), "true");
    run.same("contains_all(turret, {position velocity}): no velocity", contains_all(gun, moving), "false");
    run.same("intersects(turret, {velocity frozen})", intersects(gun, of(&[velocity, frozen])), "false");
    run.same("intersects(turret, {position frozen})", intersects(gun, of(&[position, frozen])), "true");
    let ghost = of(&[position, velocity, sprite, frozen]);
    run.same("matches(ghost, move): it is frozen", matches(ghost, moving, bit(frozen)), "false");
    let thawed = without(ghost, frozen);
    run.same("without(ghost, frozen)", thawed, "11");
    run.same("matches(that, move)", matches(thawed, moving, bit(frozen)), "true");
    let thinking = of(&[velocity, health]);
    run.same("matches(player, think): it is the player", matches(hero, thinking, bit(player)), "false");
    run.same("matches(turret, think): no velocity", matches(gun, thinking, bit(player)), "false");
    run.same("matches(rock, all {}, none {}): no conditions", matches(of(&[position, sprite]), 0, 0), "true");
    run.same("count(player)", count(hero), "5");
    run.same("ids(player)", words(&ids(hero)), "0 1 2 3 4");
    run.same("with(0, 63)", with(0, 63), "9223372036854775808");
    run.same("ids of {0 31 32 63}", words(&ids(of(&[0, 31, 32, 63]))), "0 31 32 63");
    run.same("count(every bit)", count(u64::MAX), "64");
}

// A random set: sparse, about half, dense, or empty or full.
fn random(rng: &mut Rng) -> Bools {
    let mut b = [false; 64];
    let kind = rng.below(4);
    let full = kind == 3 && rng.below(2) == 0;
    for x in &mut b {
        *x = match kind {
            0 => rng.below(16) == 0,
            1 => rng.below(2) == 0,
            2 => rng.below(16) != 0,
            _ => full,
        };
    }
    b
}

// What a query asks for: some of the set's ids, the same plus one more that
// may not be in it, or anything.
fn wanted(rng: &mut Rng, set: &Bools) -> Bools {
    let kind = rng.below(3);
    if kind == 2 {
        return random(rng);
    }
    let mut b = [false; 64];
    for i in 0..64 {
        if set[i] {
            b[i] = rng.below(2) == 0;
        }
    }
    if kind == 1 {
        b[rng.below(64) as usize] = true;
    }
    b
}

// What a query bars: some of the ids the set doesn't have, or anything.
fn barred(rng: &mut Rng, set: &Bools) -> Bools {
    if rng.below(2) == 1 {
        return random(rng);
    }
    let mut b = [false; 64];
    for i in 0..64 {
        if !set[i] {
            b[i] = rng.below(4) == 0;
        }
    }
    b
}

fn fuzz(run: &Run) {
    run.fuzz();
    let mut rng = Rng::new(run.seed);
    let mut model: Bools = [false; 64];
    for op in 1..=run.ops {
        let set = to_set(&model);
        let roll = rng.below(100);
        if roll < 5 {
            model = random(&mut rng);
            run.before(op, format_args!("a new set, {}", list(to_set(&model))));
        } else if roll < 40 {
            let id = rng.below(64) as u32;
            let adding = roll < 25;
            let name = if adding { "with" } else { "without" };
            run.before(op, format_args!("{name}({}, {id})", list(set)));
            let got = if adding { with(set, id) } else { without(set, id) };
            model[id as usize] = adding;
            let want = to_set(&model);
            run.check(op, got == want, || format!("{name}({}, {id}) gave {}, it should give {}", list(set), list(got), list(want)));
        } else if roll < 52 {
            let id = rng.below(64) as u32;
            run.before(op, format_args!("bit({id}) and has({}, {id})", list(set)));
            run.check(op, bit(id) == 1u64 << id, || format!("bit({id}) is {}, it should be {}", bit(id), 1u64 << id));
            let got = has(set, id);
            let want = model[id as usize];
            run.check(op, got == want, || {
                format!("has({}, {id}) is {got}, but {id} is {}in the set", list(set), if want { "" } else { "not " })
            });
        } else if roll < 67 {
            let w = wanted(&mut rng, &model);
            let wanted = to_set(&w);
            run.before(op, format_args!("contains_all({}, {})", list(set), list(wanted)));
            let got = contains_all(set, wanted);
            let want = (0..64).all(|i| !w[i] || model[i]);
            run.check(op, got == want, || format!("contains_all({}, {}) is {got}, it should be {want}", list(set), list(wanted)));
        } else if roll < 77 {
            let b = barred(&mut rng, &model);
            let other = to_set(&b);
            run.before(op, format_args!("intersects({}, {})", list(set), list(other)));
            let got = intersects(set, other);
            let want = (0..64).any(|i| b[i] && model[i]);
            run.check(op, got == want, || format!("intersects({}, {}) is {got}, it should be {want}", list(set), list(other)));
        } else if roll < 92 {
            let w = wanted(&mut rng, &model);
            let b = barred(&mut rng, &model);
            let (all, none) = (to_set(&w), to_set(&b));
            run.before(op, format_args!("matches({}, all {}, none {})", list(set), list(all), list(none)));
            let got = matches(set, all, none);
            let want = (0..64).all(|i| !w[i] || model[i]) && !(0..64).any(|i| b[i] && model[i]);
            run.check(op, got == want, || {
                format!("matches({}, all {}, none {}) is {got}, it should be {want}", list(set), list(all), list(none))
            });
        } else {
            run.before(op, format_args!("count and ids of {}", list(set)));
            let want: Vec<u32> = (0..64).filter(|&i| model[i as usize]).collect();
            run.check(op, count(set) as usize == want.len(), || {
                format!("count({}) is {}, it should be {}", list(set), count(set), want.len())
            });
            let got = ids(set);
            run.check(op, got == want, || format!("ids({}) gave {}, it should give {}", list(set), words(&got), words(&want)));
        }
    }
    run.pass();
}

// The kata's main calls this.
pub fn run() {
    let run = Run::new("bitset", "6. bitsets");
    scenario(&run);
    fuzz(&run);
}
