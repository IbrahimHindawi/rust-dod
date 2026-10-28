# 3. The slot map

`dojo/3.slot_map/play_slot_map.rs`

Dojos 1 and 2, put together. Values live **packed** in rows, so a loop over
them has no holes (dojo 2). The rest of the program keeps **handles**, which
survive rows moving and say when their value is gone (dojo 1). In between is
a **slot** per index:

    handle 1:2  ->  slots[1] { generation 2, row 0 }  ->  values[0]

A lookup is two steps: the handle's generation must be the slot's, and the
slot says the row. A remove is both dojos' work: bump the generation and free
the index, then swap-remove the row and fix the slot of whoever moved. Game
engines keep textures, meshes and sounds this way, and it is the ECS's entity
index.

**The invariant:** dojo 1's and dojo 2's together. A handle is alive exactly
when its generation is its slot's; the rows are packed; and for every row,
the slot of the row's handle points back at that row.

In the course: [1.4](../../ecs/docs/1.4_entity_index.md) is a slot map with
an entity's components as its value, and every chapter after keeps it, with a
table number added to the slot in [2.4](../../ecs/docs/2.4_archetypes.md).

## Watch it: the playground

    cargo run --bin play_slot_map
    python tools/dojo.py play slot_map

It keeps three textures, removes one, reuses its index, and draws the slots
and the rows after each step:

```
remove stone: free index 1 in its next life, and water moves into row 1
    slots    0: generation 1, row 0 | 1: generation 2, free | 2: generation 1, row 1
    rows     0:1 grass | 2:1 water
```

Step through it: break on `remove`, and watch the slot, the free list and the
rows change in that order. Then flip the switch at the top,
`CHECK_GENERATION`, to `false` and run it again. The removed `stone` still
finds slot 1, and reads whatever is in slot 1's row now: `water`, then
`lava`. In a game, that's a sound that plays the wrong sample, or a mesh
drawn with another mesh's texture.

## Do reps: the kata

You write `SlotMap<T>`. Its two-step lookup:

```rust
    pub fn get(&self, h: Handle) -> Option<&T> {
        self.contains(h).then(|| &self.values[self.slots[h.index as usize].row as usize])
    }
```

and the API around it:

```rust
    pub fn insert(&mut self, value: T) -> Handle {
```

```rust
    pub fn remove(&mut self, h: Handle) -> Option<T> {
```

plus `new()`, `contains`, `get_mut`, `len()`, `values()` (the packed values,
in row order) and `handles()` (`handles()[row]` owns `values()[row]`).
`Handle` comes from `dojo/dojo.rs`, as in dojo 1.

| level | you write | target |
|---|---|---|
| 1 | `remove`, in an otherwise finished file | 3:00 |
| 2 | every function body; the fields and signatures are given | 12:00 |
| 3 | the whole file, from a blank page | 15:00 |

    python tools/dojo.py start slot_map 2
    python tools/dojo.py slot_map

Level 3 is the one to come back to. Once you can write a slot map from a blank
page in fifteen minutes, dojos 1 and 2 are in your hands too.

The harness runs the playground's steps as a scenario, then 20,000 random
inserts, removes, reads and writes against a model that maps every live
handle to its value and keeps every dead one. It checks dojo 1's rules for
the handles (never handed out twice, a freed index reused before a new one is
made, a removed handle never alive again) and dojo 2's for the rows (packed,
each row's handle holding that row's value).

**Level 4, variations.** The same harness, different insides:

- **Free list inside the slots**: a slot is `Alive { generation, row }` or
  `Free { generation, next: Option<u32> }`, and the map keeps
  `first_free: Option<u32>`. No `free` Vec, and a slot can't be alive and free
  at once.
- **`remove` without `contains`**: look the slot up once, with `get_mut`, and
  bail out with `?` and a generation check. How many times does the
  reference look at the slot?
- **`retain(|h, v| keep)`**: remove every value the closure rejects, in one
  pass, keeping the rows packed. Which way should the loop walk? (Not checked
  by the harness; write your own test.)

## When it fails

    FAIL at operation 233 (seed 1): remove(33:1) returned 214, the model says 210
        debug it: build/dojo/slot_map/harness --seed 1 --stop 233

The wrong value usually means a slot pointing at the wrong row: an earlier
remove moved a row and didn't fix its slot. Run the harness under your
debugger with those arguments and a breakpoint on `stop_here`, which runs just
before that operation, then look at the slot of the handle being removed.
