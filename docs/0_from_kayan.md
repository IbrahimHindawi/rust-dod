# 0. From kayan

[kayan](https://github.com/IbrahimHindawi/kayan) is a sketch of an archetype
entity component system in C, about 200 lines in `src/kayan.h`, with haikal
generating its containers. These lessons start from its idea and rebuild the
rest, first in rin ([rin-dod](https://github.com/IbrahimHindawi/rin-dod)) and
here in Rust. This page says what was kept, what was changed, and why, so the
lessons can be read as a review of it as much as a rewrite.

## What kayan does

Components are bits in an enum, `positions = 1 << 0`, `velocities = 1 << 1`
and so on. Each archetype is a C struct written by hand (`A`, `B`, `C`, `D`)
that starts with an `ArchetypeHeader` and then holds one haikal `Array_T` per
component. The header has a `u64` signature and a `Map_u64` from column name
to the column's byte offset in the struct.

A query walks an array of `void *` pointing at the archetypes, reads each
header, and tests `(signature & want) == want`. For a match it looks up
`"positions"` and `"velocities"` in the offset map, adds the offsets to the
struct's address, and runs the loop over the two columns.

## Kept

- **An archetype is a struct of arrays.** Each component is a contiguous column
  and every entity in the table has every column. That is the whole point,
  and kayan had it right from the start.
- **A signature is a u64 of component bits**, and a query is the test
  `(signature & all) == all`. Lesson 3 adds a `none` mask beside it.
- **Arena-style memory.** kayan and rin-dod allocate from arenas and never
  free one thing at a time. Here the world's data lives in `Vec`s that only
  grow while the game runs (tables are never removed), and the frame's
  commands live in `Vec`s that are cleared after every frame and keep their
  memory. That is an arena's allocation pattern, with the Vec doing the
  bookkeeping and freeing everything when the world is dropped.

## Changed

- **One table type for every archetype** (lesson 3). kayan needs a new C
  struct, a new signature line and new offset-map lines for every archetype.
  Here an archetype is data: a signature, and the world makes its table the
  first time something needs it (lesson 4).
- **Columns by component id, not by name** (lesson 3). kayan hashes the
  strings `"positions"` and `"velocities"` on every query. Here a table's
  columns are an array indexed by component id, so finding one is an index.
  Names are kept for tools, through reflect records (lesson 5).
- **The signature comes from the columns** (lesson 3). In kayan they are
  written separately and can disagree. In `D` they do: it has `positions` and
  `velocities` columns, but its signature is `health | damage | alive`, so
  the movement query never visits it.
- **A row is added and removed in one place** (lesson 2). kayan appends to
  each column at the call site, and keeps `entity_count` apart from the
  columns. For `B` and `D` it increments `a`'s count instead of their own
  (`kayan.h` lines 142, 145, 168, 174), so `A` reports seven entities and the
  others report none. Nothing breaks only because the query loop happens to
  use the column's own length. In Rust the row count *is* a length, of the
  table's `entities` Vec, so there is no second count to get wrong.
- **Entities have handles** (lesson 4). kayan's entity id is the value of an
  archetype's `entity_count` before it is bumped: meant per table, in
  practice `A`'s, as above. Nothing records where an entity lives, so there is
  no way to refer to one entity from elsewhere, to remove it, or to move it to
  another archetype. The lessons use an `{index, generation}` handle and a
  slot list that tracks each entity's table and row.
- **Entities move between archetypes** (lesson 4). Adding or removing a
  component moves the entity to the matching table, copying the components
  both tables share.
- **Structural changes wait for the end of the frame** (lesson 6), through a
  command buffer. kayan never changes structure while a query runs, but only
  because it never changes structure at all.
- **Reflect records replace hand-written tables** (lessons 3 to 5). A
  component's size, alignment, field names and offsets are written down by the
  macro that declares it, from what the compiler knows. The offset map goes
  away entirely.

## What Rust adds

- **Components are checked to be plain data.** Columns hold bytes, and turning
  bytes back into a component is only sound if any bytes are a valid value.
  `pod!` checks at compile time that every field is plain data and the struct
  has no padding. kayan, like C, trusts you.
- **`unsafe` is fenced in.** Every program starts with
  `#![deny(unsafe_code)]`. Lessons 1, 2 and 7 have no `unsafe` at all.
  Lessons 3 to 6 have the byte views of a column, four one-line blocks, and
  lessons 4 to 6 add one more, `bytes_of`, all with a `// SAFETY:` reason, and
  they run clean under Miri (`python tools/check.py --miri`).
- **Some of kayan's quiet bugs don't compile.** A pointer to a table kept
  across the creation of another, a slice kept across a spawn, two `&mut` to
  one column: each is a borrow-checker error in these lessons, where in C or
  rin the write would just go somewhere stale. What the borrow checker can't
  see is a stale *number*, a row or an entity index, and handles and the
  command buffer are still how those are handled.

## Not carried over

- **`vec4s` for positions.** kayan pads 3D vectors to 16 bytes, which suits
  SIMD loads. The lessons use `Vec2` and `Vec3` and plain loops, and leave
  vectorizing to the compiler. Every `&mut` slice in Rust promises that
  nothing else reaches its memory, which is what rin's `noalias` and C's
  `restrict` say, so kernels that take slices vectorize without run-time
  overlap checks. Lesson 1 adds a third layout, one `f32` array per
  coordinate, which is the one SIMD runs cleanly over. Lesson 7 weighs that
  layout against lookups, which it is bad at.
- **cglm and the haikal containers.** Rust's standard library (`Vec`,
  `HashMap`, slices) covers what they did, and the lessons have no
  dependencies at all.

## Small things in kayan.h

- The query's inner loop declares `i32 i`, shadowing the outer `i`
  (line 201). The Rust lessons mostly avoid the question: a loop over an
  iterator, or two zipped together, has no index to shadow.
- Each lookup on lines 199 and 200 hashes and compares a string, once per
  column per archetype per query. That is cheap next to a large loop, but it
  is work that an array index does for nothing.
