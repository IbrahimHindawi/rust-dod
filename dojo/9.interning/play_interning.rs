// Dojo 9, the playground: interning.
//
// An interner gives each distinct key a small id, the first time it sees the
// key, and the same id every time after: find it, or make it. The ECS does
// this with signatures, to find the table for a set of components. Behind it
// is a hash table: a row of slots, each empty or holding an id. A key's hash
// says which slot to look in first; if that one holds another key, look in the
// next, and the next, until the key or an empty slot turns up. A table more
// than 3/4 full makes those runs long, so it doubles before that, and every id
// is put back where its key's hash says in the bigger table.
//
// Here the table has 8 slots, then 16, small enough to see whole in a
// debugger's watch window. The keys are the signatures of a few kinds of
// thing in a game: a bit per component, with the common components first.
//
// verify() runs after every step: every id must be reachable from its key's
// hash without crossing an empty slot, and the table at most 3/4 full. Put a
// breakpoint on `broken` to stop where it finds something wrong.
//
// The switch: MIX_HASH. Break on the first intern, set it to false in the
// watch window, and continue. The hash is then the key itself, and the slot
// its lowest bits: the components every kind of thing has. Watch the runs get
// long, and the count of slots looked at at the end. Flip it later instead,
// with ids already in the table, and verify says why a table's hash can't
// change under it.

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};

static MIX_HASH: AtomicBool = AtomicBool::new(true);

// Components, a bit each. The ones nearly everything has were made first, so
// they have the lowest bits.
const POSITION: u64 = 1 << 0;
const SPRITE: u64 = 1 << 1;
const HEALTH: u64 = 1 << 2;
const VELOCITY: u64 = 1 << 3;
const PLAYER: u64 = 1 << 4;
const AI: u64 = 1 << 5;
const DAMAGE: u64 = 1 << 6;
const LOOT: u64 = 1 << 7;

const KINDS: [(&str, u64); 7] = [
    ("player", POSITION | SPRITE | HEALTH | VELOCITY | PLAYER),
    ("drone", POSITION | SPRITE | HEALTH | VELOCITY | AI),
    ("turret", POSITION | SPRITE | HEALTH | AI | DAMAGE),
    ("rock", POSITION | SPRITE | HEALTH),
    ("chest", POSITION | SPRITE | HEALTH | LOOT),
    ("tank", POSITION | SPRITE | HEALTH | VELOCITY | AI | DAMAGE),
    ("bird", POSITION | SPRITE | HEALTH | VELOCITY),
];

fn name(key: u64) -> &'static str {
    KINDS.iter().find(|k| k.1 == key).map_or("?", |k| k.0)
}

// splitmix64's finisher, or, with the switch off, the key as it is.
fn hash(key: u64) -> u64 {
    if !MIX_HASH.load(Ordering::Relaxed) {
        return key;
    }
    let mut x = key;
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58476D1CE4E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D049BB133111EB);
    x ^ (x >> 31)
}

struct Interner {
    // slots[..capacity] is the table: 0 is empty, anything else an id + 1.
    slots: [u32; 16],
    capacity: usize,
    keys: [u64; 12],
    len: usize,
    // Slots looked at, in all.
    probes: usize,
}

impl Interner {
    // Where `key` is, or the empty slot where it would go, printing the slots
    // it looks at on the way.
    fn slot_of(&mut self, key: u64) -> usize {
        let mask = self.capacity - 1;
        let mut at = hash(key) as usize & mask;
        print!("    slots looked at:");
        loop {
            print!(" {at}");
            self.probes += 1;
            let s = self.slots[at];
            if s == 0 || self.keys[s as usize - 1] == key {
                return at;
            }
            at = (at + 1) & mask;
        }
    }

    fn find(&mut self, key: u64) -> Option<u32> {
        let at = self.slot_of(key);
        self.slots[at].checked_sub(1)
    }

    fn intern(&mut self, key: u64) -> u32 {
        let mut at = self.slot_of(key);
        if self.slots[at] != 0 {
            println!("; found: id {}", self.slots[at] - 1);
            return self.slots[at] - 1;
        }
        if (self.len + 1) * 4 > self.capacity * 3 {
            println!("; not there");
            println!("    {} ids would fill more than 3/4 of {} slots: the table grows to {}, and every id",
                     self.len + 1, self.capacity, self.capacity * 2);
            println!("    goes where its key's hash says now");
            self.capacity *= 2;
            self.slots = [0; 16];
            for id in 0..self.len {
                let mask = self.capacity - 1;
                let mut i = hash(self.keys[id]) as usize & mask;
                while self.slots[i] != 0 {
                    i = (i + 1) & mask;
                }
                self.slots[i] = id as u32 + 1;
            }
            at = self.slot_of(key);
        }
        let id = self.len as u32;
        self.keys[self.len] = key;
        self.len += 1;
        self.slots[at] = id + 1;
        println!("; new: id {id}, in slot {at}");
        id
    }

    fn draw(&self) {
        let used: Vec<String> = (0..self.capacity)
            .filter(|&i| self.slots[i] != 0)
            .map(|i| format!("{i}:{}", name(self.keys[self.slots[i] as usize - 1])))
            .collect();
        println!("    table    {}, {} of {} slots used", used.join(" "), self.len, self.capacity);
    }
}

#[inline(never)]
fn broken(what: fmt::Arguments) {
    println!("    verify: {what}");
}

// The table's invariant: each id is in one slot, reachable from its key's
// hash without crossing an empty slot, and the table is at most 3/4 full.
fn verify(t: &Interner) {
    let mask = t.capacity - 1;
    let used = t.slots[..t.capacity].iter().filter(|&&s| s != 0).count();
    if used != t.len {
        broken(format_args!("{used} slots are used, but there are {} ids", t.len));
    }
    if t.len * 4 > t.capacity * 3 {
        broken(format_args!("{} ids in {} slots is more than 3/4 full", t.len, t.capacity));
    }
    for id in 0..t.len {
        let key = t.keys[id];
        let start = hash(key) as usize & mask;
        let mut at = start;
        let mut found = false;
        for _ in 0..t.capacity {
            if t.slots[at] as usize == id + 1 {
                found = true;
                break;
            }
            if t.slots[at] == 0 {
                break;
            }
            at = (at + 1) & mask;
        }
        if !found {
            broken(format_args!("id {id}, {}, can't be found: its run from slot {start} meets the empty slot {at} first", name(key)));
        }
    }
}

fn main() {
    let mut t = Interner { slots: [0; 16], capacity: 8, keys: [0; 12], len: 0, probes: 0 };
    let [player, drone, turret, rock, chest, tank, bird] = KINDS.map(|k| k.1);
    println!("an interner with 8 slots; a slot holds an id + 1, or 0 when it's empty");

    println!("intern player, signature {player}");
    t.intern(player);
    t.draw();
    verify(&t);

    println!("intern drone, signature {drone}");
    t.intern(drone);
    t.draw();
    verify(&t);

    println!("intern turret, signature {turret}");
    t.intern(turret);
    t.draw();
    verify(&t);

    println!("intern drone again");
    t.intern(drone);
    verify(&t);

    println!("intern rock, signature {rock}");
    t.intern(rock);
    t.draw();
    verify(&t);

    println!("intern chest, signature {chest}");
    t.intern(chest);
    t.draw();
    verify(&t);

    println!("find tank, signature {tank}");
    match t.find(tank) {
        Some(id) => println!("; found: id {id}"),
        None => println!("; not there"),
    }
    verify(&t);

    println!("intern tank");
    t.intern(tank);
    t.draw();
    verify(&t);

    println!("intern bird, signature {bird}");
    t.intern(bird);
    t.draw();
    verify(&t);

    println!("find turret");
    match t.find(turret) {
        Some(id) => println!("; found: id {id}, and id {id}'s key is {}", t.keys[id as usize]),
        None => println!("; not there"),
    }
    verify(&t);

    println!("slots looked at, in all: {}", t.probes);
}
