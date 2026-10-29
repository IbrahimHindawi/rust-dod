# 4. Entity handles

`src/4.entity_handles/entity_handles.rs`, `includes/kayan.rs`

**In the dojo:** [1. free list and generations](../dojo/1.free_list/README.md), [3. the slot map](../dojo/3.slot_map/README.md), [9. interning](../dojo/9.interning/README.md), each on its own, to watch and to practise.

The world: tables made on demand, entity handles that survive row moves,
despawning with index reuse, and moving an entity between archetypes when a
component is added or removed. The code is `includes/kayan.rs`. The lesson
program calls each part and prints what happened.

## One file, three programs

Lessons 4, 5 and 6 share the world. Each program brings it in as a module:

```rust
#[path = "../../includes/kayan.rs"]
mod kayan;
```

`mod kayan;` alone would look for `kayan.rs` next to the program. `#[path]`
says where it is instead, so every program is still one `rustc` command, with
no crate and no Cargo needed. The module starts with `#![allow(dead_code)]`,
because no single program calls everything in it.

A program declares its components with the module's macros, after an enum
that gives each one its id:

```rust
enum ComponentId {
    Position,
    Velocity,
    Health,
}

pod!(Vec2 { x: f32, y: f32 });
component!(Position { at: Vec2 });
component!(Velocity { per_second: Vec2 });
component!(Health { points: f32 });
```

`component!` is the ECS course's macro (2.1, 2.2): a plain-data struct, checked
at compile time, with its reflect record and the id of the `ComponentId`
member with the same name. The macro is written in `kayan.rs`, but
`ComponentId` is resolved where it's used, in the program, so the library
never needs to know the program's components.

## Handles, slots, rows

```rust
pub struct Entity {
    pub index: u32,
    pub generation: NonZeroU32,
}
```

```rust
pub struct Slot {
    pub generation: NonZeroU32,
    pub table: u32,
    pub row: u32,
}
```

A handle is what the rest of a program keeps. Its `index` picks a slot, and
the slot says where the entity lives now. Rows move all the time: every
despawn and every archetype change swap-removes. When one does, the table's
`entities` column says which entity owned the moved row, and that entity's
slot gets the new row number. Handles never change.

`generation` is what makes a handle safe to keep after its entity dies.
Despawning bumps the slot's generation and frees the index, and the next spawn
reuses the index with the new generation:

```
spawn d: it reuses a's index with the next generation
    d = 0:2 in table 0 row 2 {Position Velocity} at (4.0, 0.0)
    a = 0:1 is dead
```

`alive` is one comparison: the handle's generation against the slot's.

```rust
    pub fn alive(&self, e: Entity) -> bool {
        self.slots.get(e.index as usize).is_some_and(|s| s.generation == e.generation)
    }
```

A free slot already holds its next generation, which no handle carries yet,
so no separate "alive" flag is needed. `a` can't reach `d`'s data, `get`
returns `None` for it, and despawning it again returns `Err(Dead)` and
changes nothing.

Generations start at 1, and `NonZeroU32` makes that a rule of the type. Rust
uses the forbidden 0 to store `None`, so "no entity" is `Option<Entity>`, at no
cost:

```
    no entity is None, and an Option<Entity> is 8 bytes, an Entity's size
```

rin-dod used a zeroed handle, `0:0`, for "no entity". It works, and nothing
stops you from using one as if it were real. `None` has to be checked before
it can be used.

## Results

`despawn`, `add`, `remove` and `move_to` return `Result<(), Dead>`: `Ok(())`,
or `Err(Dead)` when the handle no longer names a live entity.

```rust
pub struct Dead;
```

That's the only way a change can fail here, so the error type is a single
value with no fields. A `Result` is must-use: dropping one unread is a
warning, and these programs build with warnings as errors. Where the lesson
knows the handle is alive, it says so:

```rust
    w.add(b, Health { points: 75.0 }).expect("b is alive");
```

`add` itself shows `?`, Rust's early return for errors. If the move fails,
`?` returns the `Err(Dead)` from `add` at once; if it works, `add` writes the
value:

```rust
    pub fn add<T: Component>(&mut self, e: Entity, value: T) -> Result<(), Dead> {
        self.move_to(e, self.signature(e) | mask_of::<T>())?;
        *self.get_mut::<T>(e).expect("its table has a T now") = value;
        Ok(())
    }
```

## Tables on demand

```rust
    pub fn table_for(&mut self, signature: u64) -> u32 {
        if let Some(&index) = self.by_signature.get(&signature) {
            return index;
        }
        let index = self.tables.len() as u32;
        self.tables.push(Table::new(&self.infos, signature));
        self.by_signature.insert(signature, index);
        index
    }
```

The signature is looked up in a `HashMap<u64, u32>`, and the table is made if
it's new. Nobody declares archetypes; they appear when an entity first needs
that combination. Tables live in a `Vec<Table>` and are referred to by index,
because adding one can move the whole list.

Tables start with room for 16 rows and double when full. Growing a column
resizes its Vec, which copies the rows to a bigger allocation and frees the
old one. rin-dod's arena left the old columns in place, unused, until the
whole arena went. The last part of the program spawns 40 entities so the
first table grows twice, despawns every other one, and checks the result two
ways.

## Checking the world

Reading back each survivor's position is a weak check. Swap-remove copies
rows, so a stale row often still holds the right numbers. `verify` checks the
invariant instead: every row of every table belongs to a live entity whose
slot points back at that table and row, and the rows add up to the live
count. It prints each problem it finds and returns how many there were. The
lesson prints `verify: 0 problems` and fails if that is not so:

```rust
    if right == 20 && problems == 0 { ExitCode::SUCCESS } else { ExitCode::FAILURE }
```

`main` returns an `ExitCode`, so `tools/check.py` sees the failure.

`get` reads the row through a slice cut to the rows in use, so a slot that
points past the end of its table stops the program at the first lookup, in
Release too: Rust checks every index.

## Moving between archetypes

```
give b health: b moves to a new table and keeps its position
```

`add::<Health>` computes `signature | bit(Health)`, finds or makes that table,
appends a row, copies every component the two tables share, then
swap-removes the old row. `remove` is the same move with the bit cleared.

```rust
        let to = self.table_for(signature);
        let [old, new] = self.tables.get_disjoint_mut([from as usize, to as usize]).expect("two different tables");
```

The move needs two tables of the `Vec` at once, one to read and one to write.
Two `&mut self.tables[i]` would be two mutable borrows of one Vec, which the
compiler refuses whatever the indices are. `get_disjoint_mut` checks at run
time that the two are different, then lends out both. And the order matters:
`table_for` can push a table and move the rest, so no table is borrowed until
it returns. In rin that was a comment. Here, borrowing first doesn't compile.

A move costs two row copies, one into the new table and one to fill the hole
in the old, so components that come and go every frame are better as a field
than as a component. A burning rate in `Health`, say, instead of a `Burning`
component.

## Rust notes

| rin | Rust |
|---|---|
| `import "../../includes/kayan.rin"` | `#[path = "../../includes/kayan.rs"] mod kayan;` |
| `component_of: proc<position>() -> component = {...}`, once per type | `component!(Position { ... })`, with the id from `ComponentId::Position` |
| `kayan_status: enum[must_use] = { ok, dead }` | `Result<(), Dead>`, must-use built in |
| `_ = kayan_add<health>(...)` | `.expect("b is alive")` |
| `{}`, a zeroed handle, as "no entity" | `None`, which `NonZeroU32` makes free |
| `kayan_get<T>` returning `null` | `get::<T>` returning `Option<&T>` |
| `hashmap<u64, u32>` | `std::collections::HashMap<u64, u32>` |
| old columns left in the arena | the Vec frees them as it grows |
| `return wrong == 0 && problems == 0 ? 0 : 1;` | `ExitCode::SUCCESS` or `ExitCode::FAILURE` |

## Try

1. Despawn `b` and keep using its handle. Which calls notice, and which would
   not if `alive` did not check the generation?
2. Remove the lines in `Table::remove_row` that update the moved entity's
   slot. Run it and read what happens first: a lookup through a slot that
   points past the end stops the program, while one that points at another
   live row reads the wrong entity quietly. Then skip the position check and
   let `verify` list every broken slot.
3. At the end of the lesson, table 0 has 22 rows and room for 64. Take a
   `&mut [Position]` from `column_mut`, spawn 43 more so the table grows, then
   write through the old slice. In rin-dod the write was lost without a word.
   Here:

   ```rust ignore
   let ps = w.tables[0].column_mut::<Position>();
   for i in 0..43 {
       spawn_mover(&mut w, i as f32);
   }
   ps[0].at.x = 99.0;
   ```

   ```
   error[E0499]: cannot borrow `w` as mutable more than once at a time
       |
   120 |     let ps = w.tables[0].column_mut::<Position>();
       |              -------- first mutable borrow occurs here
   121 |     for i in 0..43 {
   122 |         spawn_mover(&mut w, i as f32);
       |                     ^^^^^^ second mutable borrow occurs here
   123 |     }
   124 |     ps[0].at.x = 99.0;
       |     ----- first borrow later used here
   ```

   The stale slice is a compile error. Lesson 6 defers spawns anyway: find
   out why the borrow checker isn't enough there.
4. Add `has::<T>(e) -> bool` to the world without moving anything.
