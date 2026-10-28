# 2. Swap-remove and the back-reference

`dojo/2.swap_remove/play_swap_remove.rs`

Systems loop over values, so values are kept **packed**: one after another,
no holes, nothing to skip. Removing one from the middle would leave a hole, so
the last value moves into it instead. That's **swap-remove**: one copy,
however many values there are, at the price of order.

Values move, so an id can't be the position of its value. Two arrays keep
track:

- **sparse, by id:** `rows[id]` is the row id's value is in now;
- **packed, by row:** `ids[row]` is the id that owns the row. This is the
  **back-reference**: when the last row moves into a hole, it says whose entry
  in `rows` to fix.

The whole structure is what [EnTT](https://github.com/skypjack/entt) calls a
sparse set.

**The invariant:** the rows are packed, and the two arrays point at each
other: for every row, `rows[ids[row]] == row`, and an id with no value has no
row.

In the course: [1.4](../../ecs/docs/1.4_entity_index.md) packs entities into
rows this way, [2.3](../../ecs/docs/2.3_tables.md) does it for every column of
a table, and [2.6](../../ecs/docs/2.6_add_remove.md) does it when an entity
moves between tables.

## Watch it: the playground

    cargo run --bin play_swap_remove
    python tools/dojo.py play swap_remove

It inserts four values, removes one from the middle, and draws both sides
after each step:

```
remove 7, in row 1: the last row, 5's, moves into the hole
    row         0    1    2
    ids         3    5    1
    values     30   50   10
    rows    1->2  3->0  5->1  (id -> row)
```

Step through it: break on `remove`, and watch `ids` and `rows` as the last
row moves. Then flip the switch at the top, `FIX_BACK_REFERENCE`, to `false`
and run it again. Id 5's row still says 3, past the end, so `get 5` finds
nothing. Then `insert 9` puts 9's value in row 3, and `get 5` returns 90,
another id's value, with nothing to say it's wrong. That's the bug every
packed array invites, and the reason the back-reference exists.

## Do reps: the kata

You write `SparseSet<T>`. The heart of it is `remove`:

```rust
    pub fn remove(&mut self, id: u32) -> Option<T> {
        let row = self.rows.get_mut(id as usize)?.take()? as usize;
        let value = self.values.swap_remove(row);
        self.ids.swap_remove(row);
        if let Some(&moved) = self.ids.get(row) {
            self.rows[moved as usize] = Some(row as u32);
        }
        Some(value)
    }
```

That's the reference, so don't read it before level 1 if you'd rather not
know. The full API:

```rust
    pub fn insert(&mut self, id: u32, value: T) -> Option<T> {
```

```rust
    pub fn get(&self, id: u32) -> Option<&T> {
```

plus `new()`, `get_mut`, `len()`, `values()` (the packed values, in row
order) and `ids()` (`ids()[row]` owns `values()[row]`).

| level | you write | target |
|---|---|---|
| 1 | `remove`, in an otherwise finished file | 2:00 |
| 2 | every function body; the fields and signatures are given | 8:00 |
| 3 | the whole file, from a blank page | 10:00 |

    python tools/dojo.py start swap_remove 2
    python tools/dojo.py swap_remove

The harness runs the playground's steps as a scenario, then 20,000 random
inserts and removes over a small range of ids, so the same ids come and go
many times, against a plain map from id to value. Every 64 operations it
checks the packed rows themselves: as many as the map has, and each row's
owner holding exactly that row's value.

**Level 4, variations.** The same harness, different insides:

- **Pages:** `rows` as `Vec<Option<Box<[u32; 64]>>>`, 64 ids a page, a page
  made only when an id in it gets a value. A big id no longer means a big
  `rows`. This is what EnTT does.
- **No `Option`:** `rows: Vec<u32>` with `u32::MAX` for "no row". Compare
  `size_of::<Option<u32>>()` with `size_of::<u32>()`. Then try
  `Option<NonZeroU32>` holding `row + 1`: same size as `u32`, and still an
  `Option`.
- **By hand:** swap-remove without `Vec::swap_remove`: copy the last row over
  the hole, then `pop`. Where does the "removing the last row" case go?

And one that **fails on purpose**: a remove that keeps order, shifting every
later row down and fixing each one's back-reference. The scenario stops it at
the first `ids` line. Why does the harness insist on swap-remove's order, and
what does keeping order cost?

## When it fails

A forgotten back-reference often shows up as a panic in a `get` that is
right, not in the `remove` that is wrong:

    FAIL in the scenario: it panicked after "values" passed
        panicked at dojo/2.swap_remove/kata.rs:46:26:
    index out of bounds: the len is 3 but the index is 3

The line says where the panic happened, and the first line says which step
was last to pass, so the bug is in what ran between them. In the fuzz run, a
failure gives the operation and the seed:

    FAIL at operation 64 (seed 1): row 4 holds 43's value 37043, but get(43) gives 43020
        debug it: build/dojo/swap_remove/harness --seed 1 --stop 64

Run the harness under your debugger with those arguments and a breakpoint on
`stop_here`, which runs just before that operation.
