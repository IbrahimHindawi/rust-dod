// The harness for dojo 13, a dispatch table. It is fixed. Your work is kata.rs,
// beside it: kata.rs brings this file in as a module and its main calls run(),
// so the two are one program. The reference, solution.rs, and each level file
// do the same, so each of them builds on its own, and this file never does.
// Build and run yours with
//
//   python tools/dojo.py dispatch
//
// The systems are here: twelve functions, each of which writes its own name
// in the context's log when it is called. So after a run the log says which
// systems ran, in what order, and that each name was run through its own
// function. First a short scenario. Then a fuzz run: random adds, inserts,
// removes, enables, disables and runs, against a model that keeps the
// schedule as a plain list of names, each enabled or not, with its count of
// calls. After every operation the order and the counts must match, and every
// run must log the model's enabled systems, in order.

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

use crate::dojo::{Rng, Run};
use crate::Schedule;

// What a system is given when it runs. Here only a log, which each system
// writes its name in.
pub struct Ctx {
    log: Vec<&'static str>,
}

impl Ctx {
    fn ran(&mut self, name: &'static str) {
        self.log.push(name);
    }
}

// A system as the harness hands it over: a name, and the function that logs
// that name.
type Named = (&'static str, fn(&mut Ctx));

// The systems the scenario and the fuzz run use.
const SYSTEMS: [Named; 12] = [
    ("input", |c| c.ran("input")),
    ("think", |c| c.ran("think")),
    ("move", |c| c.ran("move")),
    ("collide", |c| c.ran("collide")),
    ("damage", |c| c.ran("damage")),
    ("die", |c| c.ran("die")),
    ("spawn", |c| c.ran("spawn")),
    ("animate", |c| c.ran("animate")),
    ("sound", |c| c.ran("sound")),
    ("draw", |c| c.ran("draw")),
    ("net", |c| c.ran("net")),
    ("save", |c| c.ran("save")),
];

fn system(name: &str) -> Named {
    SYSTEMS.into_iter().find(|s| s.0 == name).expect("one of the harness's systems")
}

fn words(names: &[&str]) -> String {
    if names.is_empty() {
        return "nothing".to_string();
    }
    names.join(" ")
}

// One run, and what it logged.
fn frame(s: &mut Schedule, ctx: &mut Ctx) -> String {
    ctx.log.clear();
    s.run(ctx);
    words(&ctx.log)
}

fn scenario(run: &Run) {
    println!("scenario");
    let mut ctx = Ctx { log: Vec::new() };
    let mut s = Schedule::new();
    for name in ["move", "collide", "die"] {
        let (name, f) = system(name);
        s.add(name, f);
    }
    run.same("add move, collide, die: the order", words(&s.order()), "move collide die");
    let (name, f) = system("damage");
    run.same("insert damage before die", s.insert_before("die", name, f), "true");
    run.same("the order", words(&s.order()), "move collide damage die");
    run.same("run: what ran", frame(&mut s, &mut ctx), "move collide damage die");
    run.same("disable collide", s.set_enabled("collide", false), "true");
    run.same("run: what ran", frame(&mut s, &mut ctx), "move damage die");
    run.same("the order, with collide still in its place", words(&s.order()), "move collide damage die");
    run.same("calls of move, collide", format!("{} {}", s.calls("move"), s.calls("collide")), "2 1");
    let (name, f) = system("spawn");
    run.same("insert spawn before a system that isn't there", s.insert_before("nobody", name, f), "false");
    run.same("the order, unchanged", words(&s.order()), "move collide damage die");
    run.same("remove collide", s.remove("collide"), "true");
    run.same("the order", words(&s.order()), "move damage die");
    run.same("calls of collide, now it's gone", s.calls("collide"), "0");
    run.same("remove collide again", s.remove("collide"), "false");
    run.same("enable collide, which isn't there", s.set_enabled("collide", true), "false");
    let (name, f) = system("collide");
    s.add(name, f);
    run.same("add collide again: at the end, enabled, calls from 0", s.calls("collide"), "0");
    run.same("run: what ran", frame(&mut s, &mut ctx), "move damage die collide");
}

// A system as the model keeps it: which of SYSTEMS, whether it is enabled,
// and how many times it has run since it was added.
#[derive(Clone, Copy)]
struct Entry {
    id: usize,
    enabled: bool,
    calls: u32,
}

// The system an operation names: one in the schedule, three times in four,
// or any of the twelve, which may not be.
fn target(rng: &mut Rng, model: &[Entry]) -> usize {
    if !model.is_empty() && rng.below(4) != 0 {
        model[rng.below(model.len() as u64) as usize].id
    } else {
        rng.below(SYSTEMS.len() as u64) as usize
    }
}

// The model's schedule in words: draw (die off) damage.
fn describe(model: &[Entry]) -> String {
    let names: Vec<String> = model
        .iter()
        .map(|e| if e.enabled { SYSTEMS[e.id].0.to_string() } else { format!("({} off)", SYSTEMS[e.id].0) })
        .collect();
    if names.is_empty() { "an empty schedule".to_string() } else { names.join(" ") }
}

fn fuzz(run: &Run) {
    run.fuzz();
    let mut rng = Rng::new(run.seed);
    let mut s = Schedule::new();
    let mut ctx = Ctx { log: Vec::new() };
    let mut model: Vec<Entry> = Vec::new();
    for op in 1..=run.ops {
        let roll = rng.below(100);
        let what;
        if roll < 20 && model.len() < SYSTEMS.len() {
            let free: Vec<usize> = (0..SYSTEMS.len()).filter(|&id| model.iter().all(|e| e.id != id)).collect();
            let id = free[rng.below(free.len() as u64) as usize];
            let (name, f) = SYSTEMS[id];
            let entry = Entry { id, enabled: true, calls: 0 };
            if roll < 10 {
                what = format!("add({name})");
                run.before(op, &what);
                s.add(name, f);
                model.push(entry);
            } else {
                let before = target(&mut rng, &model);
                let at = model.iter().position(|e| e.id == before);
                what = format!("insert_before({}, {name})", SYSTEMS[before].0);
                run.before(op, &what);
                let got = s.insert_before(SYSTEMS[before].0, name, f);
                if let Some(at) = at {
                    model.insert(at, entry);
                }
                run.check(op, got == at.is_some(), || {
                    format!("{what} returned {got}, but {} is {}in the schedule", SYSTEMS[before].0, if at.is_some() { "" } else { "not " })
                });
            }
        } else if roll < 40 {
            let id = target(&mut rng, &model);
            let at = model.iter().position(|e| e.id == id);
            what = format!("remove({})", SYSTEMS[id].0);
            run.before(op, &what);
            let got = s.remove(SYSTEMS[id].0);
            if let Some(at) = at {
                model.remove(at);
            }
            run.check(op, got == at.is_some(), || {
                format!("{what} returned {got}, but {} was {}in the schedule", SYSTEMS[id].0, if at.is_some() { "" } else { "not " })
            });
        } else if roll < 55 {
            let id = target(&mut rng, &model);
            let on = rng.below(2) == 1;
            let at = model.iter().position(|e| e.id == id);
            what = format!("set_enabled({}, {on})", SYSTEMS[id].0);
            run.before(op, &what);
            let got = s.set_enabled(SYSTEMS[id].0, on);
            if let Some(at) = at {
                model[at].enabled = on;
            }
            run.check(op, got == at.is_some(), || {
                format!("{what} returned {got}, but {} is {}in the schedule", SYSTEMS[id].0, if at.is_some() { "" } else { "not " })
            });
        } else {
            let want: Vec<&str> = model.iter().filter(|e| e.enabled).map(|e| SYSTEMS[e.id].0).collect();
            what = format!("run() over {}", describe(&model));
            run.before(op, &what);
            ctx.log.clear();
            s.run(&mut ctx);
            for e in model.iter_mut().filter(|e| e.enabled) {
                e.calls += 1;
            }
            run.check(op, ctx.log == want, || {
                format!("{what} ran {}; it should run {}", words(&ctx.log), words(&want))
            });
        }
        let order = s.order();
        let want: Vec<&str> = model.iter().map(|e| SYSTEMS[e.id].0).collect();
        run.check(op, order == want, || format!("after {what}, order() is {}; it should be {}", words(&order), words(&want)));
        for (id, (name, _)) in SYSTEMS.iter().enumerate() {
            let want = model.iter().find(|e| e.id == id).map_or(0, |e| e.calls);
            run.check(op, s.calls(name) == want, || {
                format!("after {what}, calls({name}) is {}; the model counts {want} since it was added", s.calls(name))
            });
        }
    }
    run.pass();
}

// The kata's main calls this.
pub fn run() {
    let run = Run::new("dispatch", "13. a dispatch table");
    scenario(&run);
    fuzz(&run);
}
