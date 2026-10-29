// Dojo 6, the playground: bitsets.
//
// A set of component ids fits in a u64: bit `id` is set when `id` is in the
// set. In an ECS that set is a signature, what a kind of entity is made of,
// and a query is two sets: the ids an entity must have, and the ids it must
// not. A query against a signature is then two ANDs and two compares,
// however many components there are:
//
//   (signature & all) == all  &&  (signature & none) == 0
//
// This program gives five kinds of thing a signature each, draws them as rows
// of bits, runs queries over them, and changes two of them. Each kind keeps
// its ids twice: the fast way, a signature, and the slow way, a bool per id.
// After every step, verify() checks the fast way against the slow way and
// says what disagrees; break inside its `if` to stop on the first.
//
// The switch: set ALL_NOT_ANY to false and run it again. contains_all then
// asks whether the signature has any of the ids, not all of them, and the
// turret, which can't move, matches the movement query. It's a static, read
// each time it's used, so a debugger can flip it mid-run too: stop on the
// first `query` in main, set it to false in the watch window, and continue.

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

use std::sync::atomic::{AtomicBool, Ordering::Relaxed};

static ALL_NOT_ANY: AtomicBool = AtomicBool::new(true);

const NAMES: [&str; 8] = ["position", "velocity", "health", "sprite", "player", "turret", "frozen", "loot"];
const POSITION: u32 = 0;
const VELOCITY: u32 = 1;
const HEALTH: u32 = 2;
const SPRITE: u32 = 3;
const PLAYER: u32 = 4;
const TURRET: u32 = 5;
const FROZEN: u32 = 6;

fn bit(id: u32) -> u64 {
    1 << id
}

fn with(set: u64, id: u32) -> u64 {
    set | bit(id)
}

fn without(set: u64, id: u32) -> u64 {
    set & !bit(id)
}

fn has(set: u64, id: u32) -> bool {
    set & bit(id) != 0
}

fn contains_all(set: u64, wanted: u64) -> bool {
    if ALL_NOT_ANY.load(Relaxed) { set & wanted == wanted } else { set & wanted != 0 }
}

fn matches(set: u64, all: u64, none: u64) -> bool {
    contains_all(set, all) && set & none == 0
}

fn of(ids: &[u32]) -> u64 {
    ids.iter().fold(0, |set, &id| with(set, id))
}

// Eight bits, highest id first, the way a number is written.
fn bits(set: u64) -> String {
    (0..8).rev().map(|id| if has(set, id) { '1' } else { '0' }).collect()
}

fn names(set: u64) -> String {
    let named: Vec<&str> = (0..8).filter(|&id| has(set, id)).map(|id| NAMES[id as usize]).collect();
    format!("{{{}}}", named.join(" "))
}

// Up to eight kinds, each with its signature and, the slow way, a bool per id.
struct Kinds {
    names: [&'static str; 8],
    signatures: [u64; 8],
    parts: [[bool; 8]; 8],
    count: usize,
}

impl Kinds {
    fn add(&mut self, name: &'static str, ids: &[u32]) -> usize {
        let k = self.count;
        self.names[k] = name;
        self.signatures[k] = of(ids);
        for &id in ids {
            self.parts[k][id as usize] = true;
        }
        self.count += 1;
        k
    }

    fn set(&mut self, k: usize, id: u32, on: bool) {
        self.signatures[k] = if on { with(self.signatures[k], id) } else { without(self.signatures[k], id) };
        self.parts[k][id as usize] = on;
        println!("    {:<8}{}  {}", self.names[k], bits(self.signatures[k]), names(self.signatures[k]));
    }

    fn draw(&self) {
        for k in 0..self.count {
            println!("    {:<8}{}  {}", self.names[k], bits(self.signatures[k]), names(self.signatures[k]));
        }
    }
}

// What a system asks for: all of these ids, and none of those.
#[derive(Clone, Copy)]
struct Query {
    all: u64,
    none: u64,
}

fn query(kinds: &Kinds, q: Query) {
    println!("    all     {}", bits(q.all));
    println!("    none    {}", bits(q.none));
    for k in 0..kinds.count {
        let answer = if matches(kinds.signatures[k], q.all, q.none) { "matches" } else { "no" };
        println!("    {:<8}{}  {answer}", kinds.names[k], bits(kinds.signatures[k]));
    }
}

// The fast way against the slow way, for every kind: the signature's bits
// against its bools, and the query's answer, if there is one, against one id
// at a time.
fn verify(kinds: &Kinds, q: Option<Query>) {
    for k in 0..kinds.count {
        let (name, set, parts) = (kinds.names[k], kinds.signatures[k], kinds.parts[k]);
        for id in 0..8 {
            if has(set, id) != parts[id as usize] {
                println!("    verify: {name}'s signature and its parts disagree about {}", NAMES[id as usize]);
            }
        }
        let Some(q) = q else { continue };
        let missing = (0..8).find(|&id| has(q.all, id) && !parts[id as usize]);
        let barred = (0..8).find(|&id| has(q.none, id) && parts[id as usize]);
        let slow = missing.is_none() && barred.is_none();
        if matches(set, q.all, q.none) != slow {
            match (missing, barred) {
                (Some(id), _) => println!("    verify: {name} matches, but it has no {}", NAMES[id as usize]),
                (_, Some(id)) => println!("    verify: {name} matches, but it has {}", NAMES[id as usize]),
                _ => println!("    verify: {name} should match, and doesn't"),
            }
        }
    }
}

fn main() {
    let mut kinds = Kinds { names: [""; 8], signatures: [0; 8], parts: [[false; 8]; 8], count: 0 };
    let move_query = Query { all: of(&[POSITION, VELOCITY]), none: of(&[FROZEN]) };
    let draw_query = Query { all: of(&[POSITION, SPRITE]), none: 0 };
    let think_query = Query { all: of(&[VELOCITY, HEALTH]), none: of(&[PLAYER]) };

    println!("five kinds of thing, one bit per component, highest id first");
    kinds.add("player", &[POSITION, VELOCITY, HEALTH, SPRITE, PLAYER]);
    let drone = kinds.add("drone", &[POSITION, VELOCITY, HEALTH, SPRITE]);
    kinds.add("turret", &[POSITION, HEALTH, SPRITE, TURRET]);
    kinds.add("rock", &[POSITION, SPRITE]);
    let ghost = kinds.add("ghost", &[POSITION, VELOCITY, SPRITE, FROZEN]);
    kinds.draw();
    verify(&kinds, None);

    println!("query move: all {{position velocity}}, none {{frozen}}");
    query(&kinds, move_query);
    verify(&kinds, Some(move_query));

    println!("query draw: all {{position sprite}}, none {{}}");
    query(&kinds, draw_query);
    verify(&kinds, Some(draw_query));

    println!("query think: all {{velocity health}}, none {{player}}");
    query(&kinds, think_query);
    verify(&kinds, Some(think_query));

    println!("the drone is frozen: with(drone, frozen)");
    kinds.set(drone, FROZEN, true);
    verify(&kinds, None);

    println!("the ghost thaws: without(ghost, frozen)");
    kinds.set(ghost, FROZEN, false);
    verify(&kinds, None);

    println!("query move again: the drone stops, the ghost starts");
    query(&kinds, move_query);
    verify(&kinds, Some(move_query));

    let player = kinds.signatures[0];
    let ids: Vec<String> = (0..64).filter(|&id| has(player, id)).map(|id| id.to_string()).collect();
    println!("the player's signature is the number {player}: {} ids, {}", player.count_ones(), ids.join(" "));
}
