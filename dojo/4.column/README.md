# 4. A type-erased column

`dojo/4.column/play_column.rs`

A column is one component's rows, kept as bytes. It doesn't know the type it
holds, only how many bytes make a row and how they must be aligned. That is
enough to grow, to zero a new row, and to move the last row into a hole,
which is everything a table does to its columns. The type comes back only at
the edge: a typed view turns the bytes into a `&[T]`, for a `T` of the
column's size and alignment.

Two things make it work:

- **Aligned storage.** The bytes start on a 16-byte boundary, so every row of
  any type aligned to 16 or less lands where that type may live.
- **Growing by a factor.** A full column doubles, so the copies that growing
  costs add up to about one per push, however many rows there are.

**The invariant:** the column holds `len` rows of `size` bytes each, packed
from the start of a 16-byte-aligned allocation with room for `capacity` rows.
A typed view is only handed out for a `T` that is plain data, of exactly
`size` bytes, aligned to exactly `align`.

In the course: [2.2](../../ecs/docs/2.2_columns.md) builds the column,
[2.3](../../ecs/docs/2.3_tables.md) puts columns in a table, and
[2.5](../../ecs/docs/2.5_typed_access.md) is about what happens to a
reference into a column that grows.

## Watch it: the playground

    cargo run --bin play_column
    python tools/dojo.py play column

It fills a column of `Vec2`s and draws each row's bytes next to the value
they are:

```
push 3 zeroed rows, and write each through the typed view
    capacity 4, 3 rows in use, 2 blocks of storage
    row 0  00 00 80 3f 00 00 00 40   as a Vec2 (1, 2)
    row 1  00 00 40 40 00 00 80 40   as a Vec2 (3, 4)
```

Then it grows the column, swap-removes from it, and asks for the rows as two
wrong types. Step through it: break on `grow` and watch the rows move to the
new allocation.

The switch at the top, `DOUBLE`, picks how a full column grows. Set it to
`false`, so the column grows by one row, and run it again. The last line is
the point: the same 1000 pushes copy 1,020 rows when the column doubles, and
499,494 when it grows by one.

```
1000 pushes into a u32 column, growing by doubling
    9 grows, 1020 rows copied to make room
```

## Do reps: the kata

You write `Column`. Its bytes come from a `Vec` of 16-byte blocks:

```rust
#[derive(Clone, Copy, Default)]
#[repr(C, align(16))]
struct Block([u8; 16]);
```

and the typed views are the only place the column needs `unsafe`:

```rust
    pub fn slice<T: Pod>(&self) -> &[T] {
```

```rust
    pub fn slice_mut<T: Pod>(&mut self) -> &mut [T] {
```

plus `new(size, align)`, `len()`, `capacity()`, `push_zeroed()` (a zeroed row
at the end, and its row number), `swap_remove(row)`, and `row(row)` and
`row_mut(row)`, a row's bytes. `Pod` is given, in the harness: an unsafe
trait for plain data, which `slice` needs before it may turn bytes into a
`T`. The harness denies `unsafe` code except in functions marked
`#[allow(unsafe_code)]`, as the course does.

| level | you write | target |
|---|---|---|
| 1 | `slice` and `slice_mut`, the typed views, in an otherwise finished file | 3:00 |
| 2 | every function body; the fields and signatures are given | 15:00 |
| 3 | the whole file, from a blank page | 20:00 |

    python tools/dojo.py start column 2         kata.rs from level 2, and the clock starts
    python tools/dojo.py column                 your kata.rs against the harness
    python tools/dojo.py column --miri          the same under Miri, which checks your unsafe lines

The harness first runs a short scenario: three `Vec3`s written through
`slice_mut` and read back as bytes, a swap-remove, a row past the end, two
types the column must refuse, a column aligned to 16, and a column of tags,
zero-byte rows. Then it runs 20,000 random pushes, writes and swap-removes,
shared between four row types: 3 bytes aligned to 1, a `u32`, a 12-byte
`Vec3`, and a 16-byte `Vec4` aligned to 16. Each is checked against a
`Vec` of that type, through `slice` and through `row`, along with the
alignment of every view, and that a full column grows by at least half.

Miri is worth the wait here. A typed view over storage that isn't aligned
often works on your machine, because the allocator happened to return an
aligned address. Miri doesn't let it happen by luck.

**Level 4, variations.** The same harness, different insides:

- Grow by **1.5** instead of 2. Count the grows and the rows copied for 1000
  pushes. Why do some allocators prefer it?
- Allocate with **`std::alloc`** and the exact `Layout` (`size * capacity`,
  aligned to `align`) instead of a `Vec` of blocks: `alloc_zeroed`, `realloc`,
  and `dealloc` in a `Drop`. More `unsafe`, so run it with `--miri`. The tag
  column in the scenario will find the catch: a zero-size allocation is
  undefined behaviour.
- **Shrink**: when a swap-remove leaves the column less than a quarter full,
  halve its capacity. Why a quarter and not a half? (Push and remove one row,
  over and over, at the boundary.)

## When it fails

    FAIL at operation 11 (seed 1): the column was full at 4 rows and grew to 5; it should grow by at least half
        debug it: build/dojo/column/harness --seed 1 --stop 11
        with a breakpoint on stop_here, which runs just before that operation

`python tools/dojo.py column` built that harness with your kata. Run it under
your debugger with those arguments and a breakpoint on `stop_here`
(`gdb --args build/dojo/column/harness --seed 1 --stop 11`, then
`break stop_here`, `run`). It stops just before the operation that fails;
step out of `stop_here` and into your code.
