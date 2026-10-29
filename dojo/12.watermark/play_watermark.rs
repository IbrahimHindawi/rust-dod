// Dojo 12, the playground: a watermark.
//
// A query keeps the list of tables it matches, so a system doesn't search for
// them every frame. Tables are only ever added, at the end, and never removed.
// So the query remembers how many it has looked at, its watermark, and an
// update looks only at the tables past it:
//
//   tables   0 P   1 PV   2 PVH  |  3 PVHB   4 H
//                                ^ movers has checked 3: an update looks at 3 and 4
//
// A frame with no new tables costs one comparison, and each query looks at
// each table once, ever. This is the course's query (3.1), with the world's
// tables reduced to their signatures.
//
// This program makes tables a few at a time and updates two queries. After
// each step it draws the tables, and under them what each query has looked
// at, with a | at its watermark. Then verify() checks each query and says
// what is wrong, if anything; break inside its `if` to stop there. To see an
// update in a debugger, break on `update` and watch `from`, `checked` and
// `found`.
//
// The switch: set USE_WATERMARK to false and run it again. Every update looks
// at every table, from 0, so a query lists its tables again each time. It's a
// static, read on every update, so a debugger can flip it mid-run too: stop
// on `update`, set it to false in the watch window, and continue.

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

use std::sync::atomic::{AtomicBool, Ordering::Relaxed};

static USE_WATERMARK: AtomicBool = AtomicBool::new(true);

const CAPACITY: usize = 8;

// Four components, a bit each.
const POSITION: u64 = 1 << 0;
const VELOCITY: u64 = 1 << 1;
const HEALTH: u64 = 1 << 2;
const BURNING: u64 = 1 << 3;
const LETTERS: [char; 4] = ['P', 'V', 'H', 'B'];

// The world's tables, as far as a query cares: a signature each, added at the
// end and never removed.
struct Tables {
    signatures: [u64; CAPACITY],
    count: usize,
}

impl Tables {
    fn make(&mut self, signature: u64) {
        self.signatures[self.count] = signature;
        self.count += 1;
    }
}

struct Query {
    name: &'static str,
    all: u64,
    none: u64,
    // The matching tables, by index.
    found: [u32; CAPACITY],
    found_count: usize,
    // The watermark: tables 0 to checked - 1 have been looked at.
    checked: usize,
    // How many times update has looked at a table.
    scanned: usize,
}

fn matches(signature: u64, all: u64, none: u64) -> bool {
    (signature & all) == all && (signature & none) == 0
}

impl Query {
    fn new(name: &'static str, all: u64, none: u64) -> Query {
        Query { name, all, none, found: [0; CAPACITY], found_count: 0, checked: 0, scanned: 0 }
    }

    fn update(&mut self, t: &Tables) {
        let from = if USE_WATERMARK.load(Relaxed) { self.checked } else { 0 };
        for ti in from..t.count {
            self.scanned += 1;
            if matches(t.signatures[ti], self.all, self.none) {
                self.found[self.found_count] = ti as u32;
                self.found_count += 1;
            }
        }
        self.checked = t.count;
    }

    fn found(&self) -> &[u32] {
        &self.found[..self.found_count]
    }
}

fn list(tables: &[u32]) -> String {
    if tables.is_empty() {
        return "none".to_string();
    }
    tables.iter().map(|t| t.to_string()).collect::<Vec<_>>().join(" ")
}

// The invariant: a query lists the tables below its watermark that match,
// each once, in order, and it has looked at each of those tables once.
fn verify(t: &Tables, q: &Query) {
    let want: Vec<u32> =
        (0..q.checked).filter(|&ti| matches(t.signatures[ti], q.all, q.none)).map(|ti| ti as u32).collect();
    if q.found() != want {
        println!("    verify: {} lists tables {}; it should list {}", q.name, list(q.found()), list(&want));
    }
    if q.scanned != q.checked {
        println!(
            "    verify: {} has looked at {} tables to check {}; each should be looked at once",
            q.name, q.scanned, q.checked
        );
    }
}

fn letters(signature: u64) -> String {
    (0..4).filter(|i| (signature >> i) & 1 == 1).map(|i| LETTERS[i]).collect()
}

// The tables, then a row per query: yes or - for each table it has looked at,
// and a | at its watermark.
fn show(t: &Tables, queries: [&Query; 2]) {
    let mut table = String::from("    table     ");
    let mut has = String::from("    has       ");
    for ti in 0..t.count {
        table += &format!("{ti:<5}");
        has += &format!("{:<5}", letters(t.signatures[ti]));
    }
    println!("{}", table.trim_end());
    println!("{}", has.trim_end());
    for q in queries {
        let mut row = format!("    {:<10}", q.name);
        for ti in 0..q.checked {
            row += if q.found().contains(&(ti as u32)) { "yes  " } else { "-    " };
        }
        row += "|    ";
        for _ in q.checked..t.count {
            row += "     ";
        }
        println!("{row}checked {}, scanned {}, tables {}", q.checked, q.scanned, list(q.found()));
    }
    for q in queries {
        verify(t, q);
    }
}

fn main() {
    let mut t = Tables { signatures: [0; CAPACITY], count: 0 };
    let mut movers = Query::new("movers", POSITION | VELOCITY, 0);
    let mut healthy = Query::new("healthy", HEALTH, BURNING);
    println!("components: P position, V velocity, H health, B burning");
    println!("movers: all {{P V}}, none {{}}; healthy: all {{H}}, none {{B}}");

    println!("make tables 0 to 2");
    t.make(POSITION);
    t.make(POSITION | VELOCITY);
    t.make(POSITION | VELOCITY | HEALTH);
    show(&t, [&movers, &healthy]);

    println!("update movers: it looks at tables 0 to 2");
    movers.update(&t);
    show(&t, [&movers, &healthy]);

    println!("update movers again: no new tables, so it looks at nothing");
    movers.update(&t);
    show(&t, [&movers, &healthy]);

    println!("make tables 3 and 4");
    t.make(POSITION | VELOCITY | HEALTH | BURNING);
    t.make(HEALTH);
    show(&t, [&movers, &healthy]);

    println!("update movers: it looks at tables 3 and 4 only");
    movers.update(&t);
    show(&t, [&movers, &healthy]);

    println!("update healthy, for the first time: it looks at all five");
    healthy.update(&t);
    show(&t, [&movers, &healthy]);

    println!("make table 5");
    t.make(VELOCITY | HEALTH);
    show(&t, [&movers, &healthy]);

    println!("update healthy: it looks at table 5 only");
    healthy.update(&t);
    show(&t, [&movers, &healthy]);
}
