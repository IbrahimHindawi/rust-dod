# 5. A command buffer

`dojo/5.commands/play_commands.rs`

Systems walk tables, and while they walk, they decide to change which things
exist: a bullet's time is up, a turret fires, a drone catches fire. Doing it
on the spot breaks the walk. A despawn swap-removes, so the last row moves
into the hole, behind the loop, and is never looked at. So a system records
the change instead, as a **command**, and the frame applies every command, in
order, once nothing is walking anything.

Three details make it work:

- **Pending spawns.** A spawn has no handle until it is applied, so recording
  one hands back a pending number, and later commands in the same buffer can
  name the spawn by it.
- **Skipping the dead.** Two systems may both despawn the same thing, or set a
  value on something an earlier command removed. Applied in order, the second
  finds a dead handle and is skipped. Generations make that one comparison.
- **A frame arena.** After the buffer is applied it is cleared, and keeps its
  memory, so once a game has run a few frames, recording a command allocates
  nothing.

**The invariant:** recording changes nothing but the buffer. Applying it has
exactly the effect of running every command straight away, in the order it
was recorded, with each pending number meaning the handle its spawn made.

In the course: [3.3](../../ecs/docs/3.3_commands.md) builds the buffer and
shows the bug it fixes, and [3.4](../../ecs/docs/3.4_game.md)'s game runs on
it. The Advanced [lesson 6](../../docs/6_kayan.md) has one too.

## Watch it: the playground

    cargo run --bin play_commands
    python tools/dojo.py play commands

A packed world of five things, and one frame of three systems. The expire
system records a despawn for each thing whose life is up, the spawner records
a replacement and sets its life through its pending number, and a second
system despawns something the first already did. Then the buffer is applied:

```
apply the buffer, in the order it was recorded
    despawn 0:1: ok, 4:1 moves into 0:1's row
    despawn 2:1: ok, 3:1 moves into 2:1's row
    despawn 3:1: ok, it was the last row
    spawn with life 1: pending 0 is 3:2
    set pending 0 life 5: ok, 3:2 has life 5
    despawn 2:1: skipped, 2:1 is dead
```

Step through it: break on `apply` and walk the list. Then flip the switch at
the top, `DEFER`, to `false`, so every system changes the world on the spot,
and run it again. The expire system despawns `0:1`, `4:1` moves into its row
behind the loop, and when the loop ends, `3:1` is still there with no life:

```
things left with no life: none
```

becomes `things left with no life: 3:1, moved behind the loop and never
looked at`. That's [3.3](../../ecs/docs/3.3_commands.md)'s bug.

## Do reps: the kata

You write `Commands`. A command names its target either way:

```rust
pub enum Target {
    Entity(Handle),
    Pending(Pending),
}
```

and `From` conversions let `set` and `despawn` take a `Handle` or a `Pending`
alike:

```rust
    pub fn set(&mut self, target: impl Into<Target>, value: u32) {
```

```rust
    pub fn apply(&mut self, w: &mut World) -> Applied {
```

plus `new()`, `spawn(value)`, which returns a `Pending`, `despawn(target)`,
and `len()`, the commands waiting. The world is given, in the harness: a
generational store of numbers with `spawn`, `despawn`, `set` and `get`, where
`despawn` and `set` return `Err(Dead)` for a dead handle. `apply` returns
`Applied { applied, skipped }`.

| level | you write | target |
|---|---|---|
| 1 | `apply`, in an otherwise finished file | 3:00 |
| 2 | every function body; the types and signatures are given | 15:00 |
| 3 | the whole file, from a blank page | 20:00 |

    python tools/dojo.py start commands 2       kata.rs from level 2, and the clock starts
    python tools/dojo.py commands               your kata.rs against the harness

The harness first runs a short scenario: a set, a despawn, a spawn that
reuses the despawned index, a set through the spawn's pending number, and two
commands that find their target dead. Then a fuzz run, frame after frame:
random commands recorded against live handles, dead handles and the frame's
own pending spawns, then applied. A model runs the same commands straight
away on a copy of the world from before the frame, and the harness checks
the two worlds match, the applied and skipped counts match, and the buffer is
empty afterwards.

**Level 4, variations.** The same harness, different insides:

- Keep the spawned handles **in the command list** itself, as
  [3.3](../../ecs/docs/3.3_commands.md) does: a spawn command holds an
  `Option<Handle>` that `apply` fills in, and a pending number finds its spawn
  through it.
- Record the whole buffer as **one `Vec<u8>`**: a tag byte, then the
  command's fields as bytes. One allocation for any mix of commands, read back
  in order.
- Let the values be **any plain-data type**, not just `u32`: store each
  value's bytes in a byte store, as the course's `bytes_of` does, and keep
  the harness's `u32` as one case of it.

## When it fails

    FAIL at operation 28 (seed 1): apply() returned applied 7, skipped 4, the model says applied 9, skipped 2
        debug it: build/dojo/commands/harness --seed 1 --stop 28
        with a breakpoint on stop_here, which runs just before that operation

`python tools/dojo.py commands` built that harness with your kata. Run it
under your debugger with those arguments and a breakpoint on `stop_here`
(`gdb --args build/dojo/commands/harness --seed 1 --stop 28`, then
`break stop_here`, `run`). It stops just before the operation that fails;
step out of `stop_here` and into your code.
