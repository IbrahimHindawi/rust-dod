// The harness for dojo 12, a watermark. It is fixed. Your work is kata.rs,
// beside it: kata.rs brings this file in as a module and its main calls run(),
// so the two are one program. The reference, solution.rs, and each level file
// do the same, so each of them builds on its own, and this file never does.
// Build and run yours with
//
//   python tools/dojo.py watermark
//
// The world's tables are here, reduced to what a query reads: a list of
// signatures that only ever grows at the end. First a short scenario, the
// playground's steps. Then a fuzz run: tables made at random, four queries
// updated at random and now and then replaced by new ones, against a model
// that keeps, for each query, the matching tables and how many tables it has
// been updated over. After every update the query must list exactly the
// matching tables, in order, and have looked at each table once.

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

use crate::dojo::{Rng, Run};
use crate::Query;

fn list(items: &[u32]) -> String {
    if items.is_empty() {
        return "none".to_string();
    }
    items.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(" ")
}

// A signature as the components in it: {0 2 5}.
fn set(bits: u64) -> String {
    let ids: Vec<String> = (0..64).filter(|i| (bits >> i) & 1 == 1).map(|i: u32| i.to_string()).collect();
    format!("{{{}}}", ids.join(" "))
}

const POSITION: u64 = 1 << 0;
const VELOCITY: u64 = 1 << 1;
const HEALTH: u64 = 1 << 2;
const BURNING: u64 = 1 << 3;

fn scenario(run: &Run) {
    println!("scenario");
    let mut tables = vec![POSITION, POSITION | VELOCITY, POSITION | VELOCITY | HEALTH];
    let mut movers = Query::new(POSITION | VELOCITY, 0);
    let mut healthy = Query::new(HEALTH, BURNING);
    run.same("a new query: checked, scanned, tables", format!("{} {} {}", movers.checked(), movers.scanned(), list(movers.tables())), "0 0 none");
    movers.update(&tables);
    run.same("update movers over tables 0 to 2: tables", list(movers.tables()), "1 2");
    run.same("checked, scanned", format!("{} {}", movers.checked(), movers.scanned()), "3 3");
    movers.update(&tables);
    run.same("update movers again, nothing new: tables", list(movers.tables()), "1 2");
    run.same("checked, scanned: it looked at nothing", format!("{} {}", movers.checked(), movers.scanned()), "3 3");
    tables.push(POSITION | VELOCITY | HEALTH | BURNING);
    tables.push(HEALTH);
    movers.update(&tables);
    run.same("make tables 3 and 4, update movers: tables", list(movers.tables()), "1 2 3");
    run.same("checked, scanned: only 3 and 4 were new", format!("{} {}", movers.checked(), movers.scanned()), "5 5");
    healthy.update(&tables);
    run.same("update healthy for the first time: tables", list(healthy.tables()), "2 4");
    run.same("checked, scanned: all five", format!("{} {}", healthy.checked(), healthy.scanned()), "5 5");
    tables.push(VELOCITY | HEALTH);
    healthy.update(&tables);
    run.same("make table 5, update healthy: tables", list(healthy.tables()), "2 4 5");
    run.same("checked, scanned", format!("{} {}", healthy.checked(), healthy.scanned()), "6 6");
    run.same("movers, not updated since: checked", movers.checked(), "5");
}

// What a query must have, kept the plain way: its masks, the tables that
// match among the ones it has been updated over, and how many those are.
struct Model {
    all: u64,
    none: u64,
    tables: Vec<u32>,
    seen: usize,
}

// Two random components that must be there, and sometimes two that mustn't.
// Each two may be one component twice, and the ones that mustn't may be ones
// that must, so some queries match nothing.
fn random_query(rng: &mut Rng) -> Model {
    let all = (1 << rng.below(8)) | (1 << rng.below(8));
    let none = if rng.below(3) == 0 { (1 << rng.below(8)) | (1 << rng.below(8)) } else { 0 };
    Model { all, none, tables: Vec::new(), seen: 0 }
}

// Where two lists of tables first differ, in words.
fn difference(got: &[u32], want: &[u32]) -> String {
    let at = got.iter().zip(want).take_while(|(a, b)| a == b).count();
    let show = |tables: &[u32]| tables.get(at).map_or("nothing".to_string(), |t| format!("table {t}"));
    format!(
        "it lists {}, the model {}; the first difference is at position {at}: {}, where the model has {}",
        got.len(),
        want.len(),
        show(got),
        show(want)
    )
}

fn fuzz(run: &Run) {
    run.fuzz();
    let mut rng = Rng::new(run.seed);
    let mut signatures: Vec<u64> = Vec::new();
    let mut models: Vec<Model> = (0..4).map(|_| random_query(&mut rng)).collect();
    let mut queries: Vec<Query> = models.iter().map(|m| Query::new(m.all, m.none)).collect();
    for op in 1..=run.ops {
        let roll = rng.below(100);
        if roll < 20 {
            let signature = rng.below(256);
            run.before(op, format_args!("make table {} {}", signatures.len(), set(signature)));
            signatures.push(signature);
        } else if roll < 90 {
            let k = rng.below(4) as usize;
            let m = &mut models[k];
            let what = format!("query {k} (all {}, none {})", set(m.all), set(m.none));
            run.before(op, format_args!("update {what} over {} tables, {} new", signatures.len(), signatures.len() - m.seen));
            let q = &mut queries[k];
            q.update(&signatures);
            for (ti, &s) in signatures.iter().enumerate().skip(m.seen) {
                if (s & m.all) == m.all && (s & m.none) == 0 {
                    m.tables.push(ti as u32);
                }
            }
            m.seen = signatures.len();
            run.check(op, q.checked() == m.seen, || {
                format!("after update(), {what} says checked() is {}, but signatures.len() is {}", q.checked(), m.seen)
            });
            run.check(op, q.tables() == m.tables, || format!("after update(), {what}: {}", difference(q.tables(), &m.tables)));
            run.check(op, q.scanned() == m.seen, || {
                format!(
                    "{what} has scanned {} in all, but signatures.len() is {}, and each table should be looked at once, by the first update after it was made",
                    q.scanned(),
                    m.seen
                )
            });
        } else {
            let k = rng.below(4) as usize;
            models[k] = random_query(&mut rng);
            let m = &models[k];
            run.before(op, format_args!("Query::new(all {}, none {}) as query {k}", set(m.all), set(m.none)));
            let q = Query::new(m.all, m.none);
            run.check(op, q.checked() == 0 && q.scanned() == 0 && q.tables().is_empty(), || {
                format!(
                    "a new query should have checked nothing; it says checked {}, scanned {}, tables {}",
                    q.checked(),
                    q.scanned(),
                    list(q.tables())
                )
            });
            queries[k] = q;
        }
    }
    run.pass();
}

// The kata's main calls this.
pub fn run() {
    let run = Run::new("watermark", "12. a watermark");
    scenario(&run);
    fuzz(&run);
}
