# 6. kayan

`src/6.kayan/kayan.rs`

**In the dojo:** [5. a command buffer](../dojo/5.commands/README.md), [7. an arena](../dojo/7.arena/README.md), each on its own, to watch and to practise.

A turret at the centre, five wings of three drones, 140 frames. Seven systems
run each frame, and every structural change they want goes through a command
buffer applied at the end of the frame.

## The frame

    spawner    wings of drones enter                    records spawns
    turret     shoot at the nearest drone               records spawns
    movement   position += velocity * dt                writes
    lifetime   bullets expire                           writes, records despawns
    collision  live bullets against drones              writes health, records despawn and add
    burning    burning drones lose health               writes
    outcome    destroy dead drones, catch leaks         records despawns
      blast    a destroyed drone hurts its neighbours   writes health, records adds
    apply      every recorded change, in order

Writes to components happen in place, during queries. Changes to which table
an entity is in are recorded, and applied once nothing is iterating.

```rust
    for frame in 1..=frames {
        g.log.frame = frame;
        spawner_system(&mut g);
        turret_system(&mut g, dt);
        movement_system(&mut g.world, dt);
        lifetime_system(&mut g, dt);
        collision_system(&mut g);
        burning_system(&mut g.world, dt);
        outcome_system(&mut g);
        skipped += g.commands.apply(&mut g.world).skipped;
    }
```

Lifetime runs before collision, and collision skips bullets whose lifetime is
used up. Those bullets are only still in their table because nothing
structural happens until the end of the frame. Without the skip, a bullet that
hit on its last frame would be despawned twice and counted as both a hit and
a miss.

## The game's state, in four fields

```rust
struct Game {
    world: World,
    commands: Commands,
    stats: Stats,
    log: Log,
}
```

A system takes `&mut Game` and uses its fields directly: `g.world.tables`,
`g.commands`, `g.stats`. The compiler treats different fields as different
borrows, so a system can walk the world's tables and record a command in the
same loop. If recording a command were a method on `Game`, it would borrow
the whole game, tables included, and the loop couldn't call it. That's why
the event log is a field of its own, with its own method:

```rust
    fn event(&mut self, what: fmt::Arguments) {
        if self.printed != self.frame {
            println!("frame {}", self.frame);
            self.printed = self.frame;
        }
        println!("    {what}");
    }
```

`g.log.event(format_args!("drone {e} is destroyed"))` borrows only the log.
`format_args!` builds the text's pieces without making a `String`; `println!`
puts them together.

## Systems and kernels

`movement_system` and `burning_system` are each a query and a kernel:

```rust
fn integrate(ps: &mut [Position], vs: &[Velocity], dt: f32) {
    for (p, v) in ps.iter_mut().zip(vs) {
        p.at.x += v.per_second.x * dt;
        p.at.y += v.per_second.y * dt;
    }
}

fn movement_system(w: &mut World, dt: f32) {
    let movers = Query { all: mask_of::<Position>() | mask_of::<Velocity>(), none: 0 };
    for t in w.tables.iter_mut().filter(|t| t.matches(movers)) {
        let (ps, vs) = t.pair_mut::<Position, Velocity>();
        integrate(ps, vs, dt);
    }
}
```

The system walks the tables and calls the kernel once for each table that
matches. The kernel sees two slices, so the compiler sees a straight loop
over two arrays. rin-dod split the kernels out because `noalias` is a
parameter qualifier, and the loop needed a proc of its own to carry it. Rust
needs no qualifier: a `&mut` slice is always the only way to its memory. The
kernels stay split because it reads well, and because a kernel is what you
time and read the assembly of.

`pair_mut` (from the ECS course, 3.1) lends out two columns of one table, the
first to write and the second to read, after checking at compile time that
they're two different components.

`burn` is the cleanest loop in the game. `Health` and `Burning` are each one
`f32`, so their columns are plain runs of floats. `integrate` works on `Vec2`s
and spends some of each pass separating x from y; lesson 1 shows what the
fully split layout looks like.

`integrate` and `burn` are streams: every row of every matching table, in
order. `collision_system` and `blast_system` are closer to lookups: for each
bullet or each blast, they visit drones that could be anywhere. Lesson 7
measures what each layout costs for both kinds of loop, and why the
components here stay small structs.

`lifetime_system` is two passes, where rin-dod's was one loop:

```rust
        age(t.column_mut::<Lifetime>(), dt);
        for (&e, l) in t.entities.iter().zip(t.column::<Lifetime>()) {
            if l.seconds <= 0.0 {
                g.commands.despawn(e);
                g.stats.misses += 1;
            }
        }
```

A column to write and the handles to read are two borrows of one table, and
`column_mut` borrows all of it. So the kernel writes, then a second loop
reads. The output is the same, and the kernel, which calls nothing, can be
vectorized.

## Row numbers, within a frame

Some systems hold no slices at all. They read one row at a time, by number:

```rust
        for i in 0..g.world.tables[ti].count() {
            let t = &g.world.tables[ti];
            let (e, at, points) = (t.entities[i], t.column::<Position>()[i].at, t.column::<Health>()[i].points);
            if points <= 0.0 {
                g.commands.despawn(e);
                g.stats.destroyed += 1;
                g.log.event(format_args!("drone {e} is destroyed"));
                blast_system(g, e, at);
```

`outcome_system` can't hold its table's columns while it calls
`blast_system`, which writes the health of every drone near the blast,
including drones in the very table `outcome_system` is walking. A slice held
across that call would be a second way to the same memory, and the compiler
refuses it. So it copies what it needs out of row `i` (the handle, the
position, the health) and lets go before the blast. Each row is read afresh,
so a drone that an earlier blast in this loop took below zero is destroyed in
turn. That chain is the point of frame 113.

The ECS course spent several chapters on why a row number isn't safe to keep.
Here it is, and the reason is the command buffer: until the buffer is
applied, no row moves. `turret_system` does the same, reading a turret,
searching every table for the nearest drone, then writing the turret back.

`collision_system` finds the hit with slices and acts on it with numbers. The
search lends out each drone table's health and positions, and returns where
it hit:

```rust
fn hit_first_drone(w: &mut World, at: Vec2, damage: f32) -> Option<(usize, usize, f32)> {
    let drones = Query { all: drone_mask(), none: 0 };
    for (ti, t) in w.tables.iter_mut().enumerate().filter(|(_, t)| t.matches(drones)) {
        let (hs, ps) = t.pair_mut::<Health, Position>();
        for (row, (h, p)) in hs.iter_mut().zip(ps).enumerate() {
            if distance(at, p.at) <= 1.0 && h.points > 0.0 {
                let before = h.points;
                h.points -= damage;
                return Some((ti, row, before));
            }
        }
    }
    None
}
```

Then the caller, with the slices gone, looks up the drone's handle in that
table and row, and records the commands.

## Why not change tables during a query

Each of these would be a real bug with the world from lesson 4:

- **Despawn.** The swap-remove moves the last row into the hole, behind the
  loop, so that entity is not processed this frame.
- **Spawn.** A table that is full grows, and its columns move to new memory.
  In rin, a system holding a slice from before the spawn kept writing to the
  old copy.
- **Add or remove.** The entity moves to another table, which the same query
  may reach later. That entity is processed twice this frame.

In Rust, the second one can't happen: a system holding a slice can't call
`spawn`, which needs the whole world (lesson 4's Try). The first and third
can, in any system that walks rows by number, like `outcome_system`. The
borrow checker sees references, and a row number is not one.

## The buffer

```rust
enum Command {
    Spawn { signature: u64 },
    SetSpawned { pending: usize, id: u32, start: usize, len: usize },
    Add { entity: Entity, id: u32, start: usize, len: usize },
    Remove { entity: Entity, id: u32 },
    Despawn(Entity),
}
```

Each variant carries only what it needs. Spawned entities have no handle
until the buffer runs, so `spawn` returns a `pending` number, how many spawns
came before it, and `set_spawned::<T>` writes a component into "the entity
that pending spawn will create". T has to be in the spawn's signature, and
applying the buffer stops the program with a message if it is not.

Payloads are copies of the component value, as bytes, in the buffer's byte
store:

```rust
    fn payload<T: Component>(&mut self, value: &T) -> (usize, usize) {
        let start = self.bytes.len();
        self.bytes.extend_from_slice(bytes_of(value));
        (start, size_of::<T>())
    }
```

`bytes_of` is in `kayan.rs`, the library's one `unsafe` line outside its
columns: a `Pod` value seen as its bytes.

`apply` runs everything in order, then clears the Vecs:

```rust
        self.list.clear();
        self.bytes.clear();
        self.spawned.clear();
        self.spawns = 0;
```

rin-dod kept the buffer in a frame arena and freed it with one
`memops_arena_clear`. A cleared Vec keeps its memory the same way, so after
the first few frames the buffer allocates nothing: a frame arena made of
Vecs.

## Skipped commands

Frame 113 is the interesting one. A bullet destroys one drone, and its blast
takes two burning neighbours below zero in the same loop. Each of them is
destroyed in turn, and each blast reaches back to drones already recorded as
destroyed:

```
frame 113
    bullet 8:9 hits drone 6:1, health -13.2
    drone 6:1 is destroyed
    drone 7:1 is caught in the blast, health -14.4
    drone 4:2 is caught in the blast, health -4.8
    drone 7:1 is destroyed
    drone 6:1 is caught in the blast, health -54.0
...
```

The blast can't know `6:1` is doomed: its despawn is only recorded, and it is
still in its table. So it records an add of `Burning` against it. When the
buffer is applied the despawn comes first, `move_to` returns `Err(Dead)` for
the add, and the command is skipped. The summary counts three:

```
after 140 frames
    drones: 15 entered, 6 destroyed, 3 leaked
    bullets: 20 fired, 16 hit, 3 expired
    commands skipped because their entity was already gone: 3
```

Every line of the output is the same as rin-dod's: the same game, frame for
frame.

## Rust notes

| rin | Rust |
|---|---|
| `command: struct = { kind; entity; pending; signature; id; payload; }` | `enum Command`: one variant per kind, each with its own fields |
| `frame: memops_arena`, cleared after apply | `Vec`s, `clear`ed after apply, keeping their memory |
| `commands_payload<T>`: copy into the arena | `bytes_of(&value)`, then `extend_from_slice` |
| `game: struct`, passed as `*game` | `Game` of four fields: one per thing a system borrows |
| `noalias` on kernel parameters | nothing: every `&mut` slice is already the only way to its memory |
| slices held while `blast_system` writes the same table | row numbers, safe because no row moves until `apply` |
| `printf` after `event(g)` | `g.log.event(format_args!(...))` |
| `sqrtf` from `math.h` | `f32::sqrt`, the same correctly rounded square root |

## Try

1. Call `g.world.despawn(e)` directly in `outcome_system` instead of
   recording it. In rin-dod, frame 56 reported drone `3:1` destroyed while
   removing `1:1`, and the game went on. Here it stops in frame 56:
   `index out of bounds: the len is 1 but the index is 1`. Explain both, from
   the swap-remove and the loop's range.
2. Make `blast_system` skip neighbours whose health is already at or below
   zero. The skip count drops to zero. Is the buffer's check still needed?
3. Give the turret a `Health` component and let leaking drones damage it.
   When it reaches zero, despawn it through the buffer and end the game.
4. Print the buffer before applying it, as a log of every structural change
   the frame made.
5. Write `lifetime_system` as one loop, the way rin-dod did, by adding a
   `Table` method that returns the handles and one column to write at once.
   Which two fields of the table does it borrow?
