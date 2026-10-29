# 2. One archetype table

`src/2.archetype_table/archetype_table.rs`

**In the dojo:** [2. swap-remove and the back-reference](../dojo/2.swap_remove/README.md), [4. a type-erased column](../dojo/4.column/README.md), each on its own, to watch and to practise.

Eight sparks in one table with three component columns, `positions`,
`velocities` and `lives`, plus a `numbers` column that exists only so the
output can say which spark is which. Each frame they move, they age, and the
ones that ran out are removed.

## Two rules

**Columns never disagree.** `spawn` and `despawn` are the only places rows
come and go, and each touches every column. A new component means a new
column, one line in each of those two methods, and one more argument to
`spawn`. A call site cannot add a row to some columns and forget the others.

```rust
        self.numbers.push(number);
        self.positions.push(position);
        self.velocities.push(velocity);
        self.lives.push(life);
```

Each column is a `Vec`, and its length is the number of rows in use. There's
no separate `count` to keep in step: `count()` is `numbers.len()`, and the
two rules keep every other column the same length.

**Rows are packed.** Systems loop over every row and never test whether a row
is alive. Removing a row moves the last row into the hole, which `Vec` does
with `swap_remove`:

```
    spark 0 in row 0 expired; spark 7 moved from row 7 into it
```

That one move per column is the whole cost of a despawn, whatever the table's
size. The price is that a row number is not a name for a spark. After that
line, "row 0" means spark 7. Lesson 4 adds names that don't move.

## Removing while iterating

`expire_system` walks the rows backwards. When it removes row `i`, the row
that moves in comes from the end, and the end has already been tested:

```rust
    for i in (0..t.count()).rev() {
```

`(0..t.count())` is computed once, before the loop starts, and `.rev()` hands
out its numbers from the top. A forward loop would have to stay on `i` after a
removal, and it's easy to get wrong. Lesson 6 removes the question entirely by
not removing anything during a loop.

Why numbers, and not an iterator over `lives`? Because an iterator borrows the
column for as long as it runs, and `despawn` needs the whole table:

```rust ignore
    for (i, life) in t.lives.iter().enumerate().rev() {
        if *life <= 0.0 {
```

```
error[E0502]: cannot borrow `*t` as mutable because it is also borrowed as immutable
    |
111 |     for (i, life) in t.lives.iter().enumerate().rev() {
    |                      --------------------------------
    |                      |
    |                      immutable borrow occurs here
    |                      immutable borrow later used here
...
114 |             let moved = t.despawn(i);
    |                         ^^^^^^^^^^^^ mutable borrow occurs here
```

The compiler doesn't know that walking backwards makes the despawn harmless.
It only knows the column could change under the iterator, and refuses. A loop
over row numbers borrows nothing between iterations, so it compiles, and
getting its order right is up to you. The [ECS course](../ecs/docs/3.3_commands.md)
meets the same choice in 3.3.

## Systems take slices

```rust
fn move_system(positions: &mut [Vec2], velocities: &[Vec2], dt: f32) {
    for (p, v) in positions.iter_mut().zip(velocities) {
        p.x += v.x * dt;
        p.y += v.y * dt;
    }
}
```

`move_system` gets the rows in use and knows nothing about tables. `&[Vec2]`
says it only reads velocities. The call hands it two fields of the table at
once, one mutably and one shared, which the compiler allows because they're
different fields:

```rust
        move_system(&mut sparks.positions, &sparks.velocities, dt);
```

`zip` walks the two slices together and stops at the shorter one, so the
loop has no index and nothing to bounds-check. Indexing `positions[i]` would
be checked in every build, Release included, against the slice's length,
which is the row count. There's no way to reach the spare capacity behind it.

`&mut [Vec2]` also makes a promise to the compiler: while the call runs,
nothing else reaches the memory `positions` covers. rin writes that promise
as `noalias`, and has to remember to. In Rust every `&mut` makes it, and the
borrow checker is what keeps it true. So the compiler vectorizes the loop
with no run-time check that the columns are apart.

rin needed `noalias` even on `age_system`'s one slice, because on Windows a
slice arrives by reference, and without the promise clang-cl assumed a float
written through it might be the slice's own pointer or length. Rust passes a
slice as a pointer and a length in two registers, on every platform, and a
`&mut` is the only way to its memory in any case, so there's nothing to add.
The loop is eight floats per pass:

```
movups (%r9,%rdi,4), %xmm0       four lives
movups 16(%r9,%rdi,4), %xmm1     four more
addps  %xmm2, %xmm0              plus -dt
addps  %xmm2, %xmm1
movups %xmm0, (%r9,%rdi,4)
movups %xmm1, 16(%r9,%rdi,4)
```

The systems were inlined into `main` and `dt` folded into a constant, so
the subtraction became an add of `-0.5`. To look yourself:
`rustc --edition 2024 -C opt-level=3 --emit asm src/2.archetype_table/archetype_table.rs`,
then search the `.s` file for `addps`.

`positions` and `velocities` are `Vec2`, x and y side by side. Lesson 1
measures what that costs when x and y are treated differently. Here they
aren't: both get `+= v * dt`, so the compiler treats each column as one run of
floats, and `move_system` is also a plain `mulps` and `addps` loop, four
sparks per pass, with no shuffling.

## Status codes

`spawn` returns `Result<usize, Full>`: the new row, or why there wasn't one.
A `Result` is must-use, so calling `spawn` and ignoring it is a warning, and
this course builds with warnings as errors. rin's `spawn_status` was an
`enum[must_use]` with the row through an out-parameter; Rust puts both in the
return value. The ninth spark shows the `Full` case:

```rust
        if let Err(Full) = sparks.spawn(i, Vec2 { x: f, y: 0.0 }, Vec2 { x: 1.0, y: f }, life) {
            println!("spark {i} did not fit: the table is full");
        }
```

## Rust notes

| rin | Rust |
|---|---|
| `positions: []vec2` from an arena, plus `count` | `positions: Vec<Vec2>`: its length is the count |
| copy the last row over the hole, in every column | `swap_remove(row)`, in every column |
| `while (i > 0) { i -= 1; ... }` | `for i in (0..t.count()).rev()` |
| `noalias []vec2` on every written slice | `&mut [Vec2]`: every `&mut` is already "noalias" |
| `for (i = 0; i < positions.length; ...)` | `positions.iter_mut().zip(velocities)`: no index, no bounds checks |
| `spawn_status` and a `row: *u64` out-parameter | `Result<usize, Full>` |
| `_ = spark_spawn(...)` | `let _ = sparks.spawn(...)` |

## Try

1. Rewrite `expire_system` to walk forwards with `for i in 0..t.count()`. Run
   it: it panics in frame 1. Why? (The range was computed before the loop
   despawned anything.) Now write it as a `while i < t.count()` loop that
   rereads the count, like rin's forward loop. It runs, and the output doesn't
   change, because no frame here expires a spark together with the one in the
   last row. Change two lives so that one does, and find the spark that
   outlives its life by a frame. Then fix the loop.
2. Delete the line in `despawn` that swap-removes `lives`. What does the output
   show, and how long did it take to notice?
3. Call `sparks.spawn(...)` as a statement, ignoring its result, and build.
   Read the error. Then discard it on purpose with `let _ = sparks.spawn(...)`.
4. Make the table grow instead of returning `Full`. With Vecs, that's deleting
   two lines. What did the fixed capacity give you that growing doesn't? (Hint:
   what happens to a reference into a column when it grows? Lesson 4.)
