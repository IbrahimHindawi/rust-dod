# The dojo

An ECS is mostly a few small building blocks put together. None of them is
specific to an ECS: a renderer's texture handles are a slot map, a particle
system is swap-remove over packed arrays, undo and network replication are
command buffers. The course meets each block for the first time in the middle
of building something bigger, which is why its chapters can feel dense. The
dojo takes the blocks out and gives each one two halves:

- **a playground**, to understand it: a small program that runs the block
  step by step and draws its insides, with one switch at the top that breaks
  it the way it most often breaks. Step through it in a debugger, flip the
  switch, watch what goes wrong.
- **a kata**, to own it: you write the block yourself, and a harness checks
  it. Reading and stepping through code feels like learning, but writing it
  from a blank page, getting it wrong, and doing it again a few days later is
  what makes it yours.

| | block | what it is | in the course |
|---|---|---|---|
| 1 | [free list and generations](1.free_list/README.md) | handles that are safe to keep after what they name is gone | 1.2, 1.3 |
| 2 | [swap-remove and the back-reference](2.swap_remove/README.md) | values packed with no holes, found by id: a sparse set | 1.4, 2.3, 2.6 |
| 3 | [the slot map](3.slot_map/README.md) | 1 and 2 together: handles into packed values | 1.4, 2.4 |
| 4 | [a type-erased column](4.column/README.md) | an array of any type as bytes, viewed as a typed slice | 2.2, 2.3, 2.5 |
| 5 | [a command buffer](5.commands/README.md) | structural changes recorded now and applied later | 3.3, 3.4 |

Do them in order the first time: 3 is 1 and 2 put together, and 5 applies its
commands to something like 3. They can come before the course, alongside it,
or after it.

## Each block

    dojo/3.slot_map/
      README.md              the block, its invariant, the levels, the variations
      play_slot_map.rs       the playground
      harness.rs             fixed: a scenario, then a fuzz run against a model
      level1.rs ... 3.rs     where a kata starts
      solution.rs            the reference; don't peek
      kata.rs                yours: made by `start`, ignored by git

The harness doesn't know how your kata works, only what it must do. First it
runs a short scenario, the playground's steps, and prints each as it passes.
Then it runs 20,000 random operations from a seed against a simple model of
the right answers, and checks after each one. A failure says which operation,
which seed, and how to stop just before it in a debugger.

**Levels**, so the reps get harder:

1. **One function** in an otherwise finished file: the one with the idea in
   it, usually the one people forget.
2. **Every function body**, with the fields and signatures given.
3. **A blank page**, with only a list of what the harness calls.
4. **Variations**: the same harness, different insides, listed in each
   block's README.

Each README gives a target time for levels 1 to 3.

## Doing reps

    python tools/dojo.py                      the blocks, your reps, and what is due
    python tools/dojo.py play slot_map        the playground
    python tools/dojo.py start slot_map 2     a fresh kata.rs from level 2; the clock starts
    python tools/dojo.py slot_map             build the harness with your kata.rs and run it

A block can be named by its number, its name or the start of its name: `3`,
`slot_map` and `slot` are the same. While the kata fails, fix it and run it
again. When it passes, the time since `start` goes into `dojo/log.md`, and the
block is due again 1, 3, 7, 14 and then 30 days after each pass.
`python tools/dojo.py` with no arguments says what is due. Spacing reps out is
what makes them stick, much more than doing many in one day.

`start` won't overwrite a kata.rs you've changed unless you add `--force`.
Your log is a plain markdown table; commit it or don't.

**When a kata fails**, the harness says where:

    FAIL at operation 64 (seed 1): row 4 holds 43's value 37043, but get(43) gives 43020
        debug it: build/dojo/swap_remove/harness --seed 1 --stop 64
        with a breakpoint on stop_here, which runs just before that operation

Run that binary under a debugger with a breakpoint on `stop_here`. It stops
just before the failing operation, with your structure in the state that
leads to the failure. A panic in your kata reports the operation it happened
in the same way. `--seed N` reruns a different fuzz run, and `--release` builds
it optimised, for big `--ops`. Block 4's kata needs `unsafe`, and
`python tools/dojo.py column --miri` runs it under Miri.

## How it's built

Every kata file is a whole program. Yours, the solution and each level start
much the same way (block 2 imports nothing from dojo.rs):

    #[path = "../dojo.rs"]
    mod dojo;
    mod harness;

    use dojo::{Dead, Handle};

and end with a `main` that hands over to the harness:

    fn main() {
        harness::run();
    }

So any of them builds on its own, `rustc --edition 2024 dojo/3.slot_map/solution.rs`
for instance, and everything it uses is declared or imported in the file.
`mod harness;` is `harness.rs` beside it, and the harness reaches your code
through `crate::`, so all it needs is the names its README lists.
`dojo/dojo.rs` holds what every harness shares: `Handle` and `Dead`, the
run's settings, the random number generator, and the reporting. Block 4's
`Pod` and block 5's `World` are the harness's own, imported as
`harness::Pod` and `harness::World`. The runner builds `kata.rs` with
`rustc` directly, with the warnings for unused code off, since a kata in
progress is full of it.

`python tools/check.py dojo` keeps the dojo honest: every solution must build
on its own, pass its harness and print its `harness.expected`, level 1 must fail
(its gap must be a gap), level 2 must compile, and the playgrounds must print
their `.expected`. The harnesses were also checked against broken solutions,
the mistakes each block is about, and catch each one.

The same dojo exists in rin, in [rin-dod](https://github.com/IbrahimHindawi/rin-dod).
The random number generator is the same, so a seed is the same run in both,
and the playgrounds print the same thing, except where block 4's column gets
its memory: an arena in rin, a Vec in Rust.
