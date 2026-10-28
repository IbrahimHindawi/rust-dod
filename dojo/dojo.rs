// What every harness in the dojo shares: the run's settings, a random number
// generator that gives the same numbers everywhere, how a step is checked, and
// how a failure is reported. It also holds the two types the katas take as
// given: a Handle, and Dead.
//
// Every kata file brings it in, along with the harness, the way lessons 4 to
// 6 bring in kayan:
//
//   #[path = "../dojo.rs"]
//   mod dojo;
//   mod harness;

// Not every harness uses every part.
#![allow(dead_code)]

use std::fmt;
use std::num::NonZeroU32;
use std::process::exit;
use std::sync::Mutex;

// Where the run is: the last scenario step that passed, or the fuzz operation
// running now. A panic in the kata reports it, since the panic itself only
// knows the line it happened on.
static WHERE: Mutex<String> = Mutex::new(String::new());

fn now_at(text: String) {
    if let Ok(mut at) = WHERE.lock() {
        *at = text;
    }
}

// A generational handle: an index, and which life of that index it was made
// for. Generations start at 1, so an Option<Handle> costs nothing extra.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Handle {
    pub index: u32,
    pub generation: NonZeroU32,
}

impl fmt::Display for Handle {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:{}", self.index, self.generation)
    }
}

// Why a change changed nothing: the handle no longer names anything alive.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Dead;

// One harness run: which block, the fuzz seed, how many operations, and where
// to stop for the debugger. The command line sets them:
//
//   harness --seed 42 --ops 5000 --stop 1337
pub struct Run {
    block: &'static str,
    pub seed: u64,
    pub ops: u64,
    stop: Option<u64>,
}

impl Run {
    pub fn new(block: &'static str, title: &str) -> Run {
        let mut run = Run { block, seed: 1, ops: 20000, stop: None };
        let args: Vec<String> = std::env::args().skip(1).collect();
        for pair in args.chunks(2) {
            let value = pair.get(1).and_then(|v| v.parse().ok());
            match (pair[0].as_str(), value) {
                ("--seed", Some(v)) => run.seed = v,
                ("--ops", Some(v)) => run.ops = v,
                ("--stop", Some(v)) => run.stop = Some(v),
                _ => {
                    println!("usage: harness [--seed N] [--ops N] [--stop N]");
                    exit(2);
                }
            }
        }
        println!("dojo: {title}");
        now_at("FAIL in the scenario: it panicked before the first step passed".to_string());
        std::panic::set_hook(Box::new(|info| {
            let at = WHERE.lock().map(|at| at.clone()).unwrap_or_default();
            println!("{at}");
            println!("    {info}");
        }));
        run
    }

    // A step of the scenario: what was done, what came back, and what should
    // have. Printed when it matches; the end of the run when it does not.
    pub fn same(&self, what: &str, got: impl fmt::Display, want: &str) {
        let got = got.to_string();
        if got != want {
            println!("FAIL in the scenario: {what} gave {got}, it should give {want}");
            exit(1);
        }
        println!("    {what}: {got}");
        now_at(format!("FAIL in the scenario: it panicked after \"{what}\" passed"));
    }

    pub fn fuzz(&self) {
        println!("fuzz: {} operations from seed {}, checked after each", self.ops, self.seed);
        now_at("FAIL in the fuzz run: it panicked before the first operation".to_string());
    }

    // Call before operation `op`. With --stop at this operation, it says what
    // is about to run and calls stop_here, so a breakpoint there lands just
    // before the operation that fails.
    pub fn before(&self, op: u64, what: impl fmt::Display) {
        now_at(format!(
            "FAIL at operation {op} (seed {}): it panicked, in {what}\n    debug it: build/dojo/{}/harness --seed {} --stop {op}",
            self.seed, self.block, self.seed
        ));
        if self.stop == Some(op) {
            println!("stopped before operation {op}: {what}");
            stop_here(op);
        }
    }

    // A check during the fuzz run. The message says what went wrong; the
    // second line says how to get back here in the debugger.
    pub fn check(&self, op: u64, holds: bool, what: impl FnOnce() -> String) {
        if !holds {
            println!("FAIL at operation {op} (seed {}): {}", self.seed, what());
            println!("    debug it: build/dojo/{}/harness --seed {} --stop {op}", self.block, self.seed);
            println!("    with a breakpoint on stop_here, which runs just before that operation");
            exit(1);
        }
    }

    pub fn pass(&self) {
        println!("PASS");
    }
}

// Set a breakpoint here, and run with --stop N.
#[inline(never)]
pub fn stop_here(op: u64) {
    std::hint::black_box(op);
}

// splitmix64: a few lines, fast, and the same numbers on every machine and in
// rin-dod's dojo, so a seed means the same run everywhere.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed)
    }

    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }

    // A number in 0..n.
    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    // One of `items`, at random.
    pub fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[self.below(items.len() as u64) as usize]
    }
}
