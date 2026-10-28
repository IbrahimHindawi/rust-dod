# 1. Free list and generations

`dojo/1.free_list/play_free_list.rs`

Handles name things that come and go: entities, textures, sounds, network
connections. A handle is two numbers, `index:generation`, and two small ideas
make it safe to keep:

- **A free list.** A freed index is reused before a new one is made, so the
  arrays an index points into stay as long as the most things ever alive at
  once, not the most things ever made.
- **A generation per index.** Freeing bumps the index's generation, so every
  handle made for the old life stops matching. A reused index can't be
  mistaken for whatever had it before.

**The invariant:** a handle is alive exactly when its generation is the one
its index holds now. A free index already holds its *next* generation, which
no handle has yet, so "free" needs no flag of its own. Every index is either
alive or on the free list, once.

In the course: [1.2](../../ecs/docs/1.2_despawn.md) builds the free list and
meets its bug, [1.3](../../ecs/docs/1.3_generations.md) adds generations, and
every later chapter keeps both.

## Watch it: the playground

    cargo run --bin play_free_list
    python tools/dojo.py play free_list

It allocates four handles, frees two, reuses one, and draws both arrays after
each step:

```
free b, then d: their generations go up now, before anyone reuses them
    generations  0:1  1:2  2:1  3:2
    free list    [1 3]
alloc e: alloc takes from the end of the free list, so index 3, in its next life
    e 3:2
```

Step through it: break on `alloc` and `free`, and watch `generations` and
`free` change. Then flip the switch at the top, `BUMP_ON_FREE`, to `false`
and run it again. The freed `b` is still alive, the reused index comes back
with the old generation so `d` and `e` are the same handle, and freeing `d`
again frees `e` and puts index 3 on the free list twice. That's the bug from
[1.2](../../ecs/docs/1.2_despawn.md), in four lines of output.

## Do reps: the kata

You write `Handles`, an allocator of handles:

```rust
    pub fn alloc(&mut self) -> Handle {
```

```rust
    pub fn free(&mut self, h: Handle) -> Result<(), Dead> {
```

```rust
    pub fn alive(&self, h: Handle) -> bool {
```

plus `new()`, `live()` (how many are alive) and `slots()` (how many indices
have been handed out). `Handle` and `Dead` are given, in `dojo/dojo.rs`:
`Handle { index: u32, generation: NonZeroU32 }`, shown as `index:generation`.

| level | you write | target |
|---|---|---|
| 1 | `free`, in an otherwise finished file | 2:00 |
| 2 | every function body; the fields and signatures are given | 8:00 |
| 3 | the whole file, from a blank page | 10:00 |

    python tools/dojo.py start free_list 2      kata.rs from level 2, and the clock starts
    python tools/dojo.py free_list              your kata.rs against the harness

The harness first runs a short scenario, the playground's steps, and prints
each as it passes. Then it runs 20,000 random allocs and frees from a seed,
against a model that keeps every handle ever handed out. It checks what your
allocator does, not how: a handle is never handed out twice, a freed index is
reused before a new one is made, `alive` agrees with the model for every
handle, live and dead.

**Level 4, variations.** The same harness, different insides:

- Keep the free list **inside the dead slots**: no `free` Vec, but a slot
  that is either `Alive { generation }` or `Free { generation, next }`, and a
  `first_free: Option<u32>`. Rust enums are made for this.
- **16-bit generations** in the slots, stored as `u16`, that wrap from 65535
  back to 1, never 0. What does wrapping give up?
- Reuse in **first-in, first-out** order (`VecDeque`), so an index rests as
  long as possible before it comes back. Why might a game want that?

## When it fails

    FAIL at operation 266 (seed 1): after free(43:1), alive(43:1) is still true
        debug it: build/dojo/free_list/harness --seed 1 --stop 266
        with a breakpoint on stop_here, which runs just before that operation

`python tools/dojo.py free_list` built that harness with your kata. Run it
under your debugger with those arguments and a breakpoint on `stop_here`
(`gdb --args build/dojo/free_list/harness --seed 1 --stop 266`, then
`break stop_here`, `run`). It stops just before the operation that fails; step
out of `stop_here` and into your code.
