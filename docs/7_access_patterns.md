# 7. Layout follows access

`src/7.access_patterns/access_patterns.rs`

Lesson 1 found the layout SIMD wants: one array per float. This lesson asks
what that layout costs everywhere else. 1,048,576 bodies are stored four ways,
and two kinds of loop run over each, and the program checks that all four end
with the same positions.

## Two kinds of loop

**Stream**: every body, in order, the same math. Integration is the example:

```rust
    for i in 0..n {
        vy[i] += GRAVITY * dt;
        px[i] += vx[i] * dt;
```

The next body is always right after the last one, so the hardware prefetcher
has it ready, and SIMD can take four or eight at once if they sit side by
side.

**Lookup**: bodies picked by index, two at a time, in no particular order.
Contacts are the example, the way collision and constraints reach bodies:

```rust
    for c in cs {
        let (a, b) = (c.a as usize, c.b as usize);
        let dx = px[b] - px[a];
```

Each body is somewhere else in memory. What matters is how many cache lines
one body's data sits in, because each one is a separate trip to memory. SIMD
does not help: there are no four neighbours to load at once.

## Four layouts

    aos      [ x y z vx vy vz m r ][ x y z vx vy vz m r ] ...      32 bytes a body
    columns  positions  [ x y z ][ x y z ][ x y z ] ...
             velocities [ vx vy vz ][ vx vy vz ] ...                 24 bytes a body
    split    px [ x x x x ... ]   py [ y y y y ... ]   pz ...  vx ...  vy ...  vz ...
    blocks   [ x0..x7 | y0..y7 | z0..z7 | vx0..vx7 | vy0..vy7 | vz0..vz7 ][ x8..x15 ...

- **aos** keeps everything about a body together, two fields neither loop
  uses included.
- **columns** is one array per component, each component a small struct. This
  is what an ECS table stores, and what lessons 2 to 6 use.
- **split** is one array per float, lesson 1's `move_split`.
- **blocks** is eight bodies at a time, each coordinate a run of eight floats.
  It is split cut into pieces of eight, with the pieces of one body kept near
  each other. The usual name is AoSoA, an array of structs of arrays.

The program prints how many 64-byte cache lines one body's position sits in:

    cache lines one lookup touches: aos 1.000, columns 1.125, split 3.000, blocks 2.000

A 12-byte `Vec3` crosses a line boundary 2 times in 16, hence 1.125. In
split, x, y and z are in three different arrays. In a block, x and y share a
line and z is in the next.

Those counts assume each array starts on a cache line. rin-dod's arena made
sure of it. A `Vec` only promises its element type's alignment, so the two
types whose count depends on it say so themselves:

```rust
#[derive(Clone, Copy, Default)]
#[repr(C, align(32))]
struct Body {
```

`align(32)` on a 32-byte body costs no padding and means no body ever
straddles a line. A `BodyBlock` is `align(64)`: 192 bytes, exactly three
lines. For `Vec3` and `f32` the count comes out the same wherever the array
starts.

## Measured

Release (`-C opt-level=3`), best of three runs, on the 2.8 GHz Xeon these
lessons were checked on: 4 MB of L2, 33 MB of L3. The body and contact counts
were changed in a copy of the file for the first and last columns. At 65,536
bodies each layout fits in L2; at the lesson's 1,048,576 each is about the
size of L3; at 8,388,608 each is 192 to 256 MB and comes from RAM.

**Stream, ns per body:**

| layout | 65,536 | 1,048,576 | 8,388,608 |
|---|---|---|---|
| aos | 1.82 | 4.57 | 4.64 |
| columns | 2.44 | 2.96 | 3.19 |
| split | 1.09 | **1.91** | **2.60** |
| blocks | **1.00** | 2.67 | 3.09 |

**Lookup, ns per contact:**

| layout | 65,536 | 1,048,576 | 8,388,608 |
|---|---|---|---|
| aos | 6.40 | 49.60 | 76.93 |
| columns | **3.85** | **22.65** | **64.14** |
| split | 6.89 | 44.92 | 103.67 |
| blocks | 7.47 | 55.41 | 92.91 |

These are slower than rin-dod's tables for the same lesson, lookups most. So
rin-dod's C, built with clang `-O2`, ran turn about with this program on the
same day, at a million bodies. The two came out the same within noise, about
50 ns a lookup for aos and split, 28 to 36 for columns: the difference is the
machine on the day, not the language. The order of the layouts is the same
as rin-dod's. Where two numbers are within about 20 percent, the order can
change from run to run.

What it says:

- **Split wins the stream and loses the lookup.** From RAM it streams a
  fifth faster than columns, and looks up 1.6 times slower: three lines a
  body instead of one.
- **Columns win the lookup everywhere** and come second or third in the
  stream. AoS also has one line a body, but its bodies are 32 bytes, so the
  same lookups spread over more memory. At a million bodies, columns'
  positions are 12 MB and sit comfortably in L3; AoS spreads them over
  32 MB, about all of it.
- **Blocks are the compromise.** In L2 they streamed as fast as split, and
  from L3 they sit between split and columns. From RAM their stream fell back
  to about columns' speed, as it did for rin-dod. They look up a little worse
  than split in cache, and better from RAM.
- **Out of cache, a lookup costs fifteen to forty times a streamed body.**
  From RAM, 64 to 104 ns a contact against 2.6 to 4.6 ns a body. Turning lookups
  into something closer to a stream, by sorting contacts or walking space in
  order, usually saves more than any layout choice.

## What the compiler makes of it

rustc 1.97, `-C opt-level=3`, x86-64 Linux, read from `--emit asm` (lesson 1
shows how):

| loop | bodies a pass | instructions a pass | shuffles |
|---|---|---|---|
| `integrate_aos` | 2 | 29 | 2 |
| `integrate_columns` | 4 | 77 | 30 |
| `integrate_split` | 4 | 20 | 0 |
| `integrate_blocks` | 8 | 37 | 0 |
| `integrate_split`, `-C target-cpu=native` | 8 | 14 | 0 |
| `contacts_columns` | 1 contact | 29 | 0 |
| `contacts_split` | 1 contact | 36 | 0 |

Blocks vectorize as cleanly as split: the inner loop over eight lanes is a
fixed-size loop over consecutive floats, which compilers turn into whole
registers. The contact loops are one contact a pass in every layout. The
columns, split and blocks streams are the loops clang made of rin-dod's C,
instruction for instruction, since both are LLVM. The contact loops are four
instructions longer than clang's, and the next paragraph is why.

**Bounds checks.** The contact loops index with numbers read from memory, so
each index is checked, in Release too: a compare and a branch that's never
taken. `contacts_columns` has two a contact, 4 of its 29 instructions.
`contacts_split` indexes three slices with each of `a` and `b`, which was six
checks and 44 instructions, until it cut the three to one length first, as
lesson 1's `move_split` does:

```rust
    let n = px.len();
    let (py, pz) = (&mut py[..n], &mut pz[..n]);
```

Now a check of `a` against `px` covers `py` and `pz` too: two checks, 36
instructions. In a loop that waits on memory for every contact, neither
number shows in the timing, at any of the three sizes. Bounds checks cost most where nothing
else does, in streams, and that's where `zip` and cut slices take them out.

**`target-cpu=native` and blocks.** On this AVX-512 machine, LLVM vectorized
`integrate_blocks` the wrong way round: not the eight lanes of a block, but
lane 0 of eight different blocks, then lane 1, gathering and scattering each
float with `vgatherdps` and `vscatterdps`. 267 instructions a pass, 80 of them
gathers and scatters, and about ten times slower than the default build:
9.6 ns a body at 65,536 bodies instead of 1.0. The same thing happened to
lesson 1's `move_aos`. "Vectorized" and "fast" are different claims; read
the assembly before believing either.

## Two bodies in one slice

A contact reads two bodies and writes both, and both are in the same slice,
sometimes the same body. In rin-dod, the loop took two pointers into the
array. Rust won't lend two `&mut` into one slice at once, and the loop doesn't
need them. It reads both bodies, then writes one, then the other:

```rust
        let (p, q) = (&bodies[a].position, &bodies[b].position);
        let (dx, dy, dz) = (q.x - p.x, q.y - p.y, q.z - p.z);
        let p = &mut bodies[a].position;
        p.x += dx * STIFFNESS;
```

Two shared borrows to read, then one `&mut` at a time to write. When `a` and
`b` are the same body, `dx` is 0 and both writes leave it as it was, exactly
as rin-dod's did, so the checksums match to the bit. The machine code is the
same loads and stores as rin-dod's: the borrows only decide what the source
may say.

## In between

Engines that have both kinds of loop do one of these:

- **Blocks**, as above. One layout, decent at both.
- **Gather, work, scatter.** Storage stays in columns or AoS, good for
  lookups. The hot loop copies a batch of bodies into a wide layout, x's in
  one register and y's in another, does its math, and copies them back. The
  copy pays off when the same batch is worked on many times, as a constraint
  solver does over several iterations. Box2D v3 solves contacts this way, four
  or eight at a time.
- **Two layouts for two jobs.** A particle system that only ever streams
  stores split. The bodies that collision looks up stay in columns.

## How an ECS decides

An ECS does not decide where each float goes. **The component is the unit it
stores together**, and the programmer decides what a component is.

- **Tables.** kayan, Unity's entities, Bevy and flecs keep one table per
  archetype, with one column per component. That is this lesson's columns
  layout: an array per component, and each component's fields side by side
  inside it. Unity cuts its tables into 16 KB chunks, each holding every
  component array for as many entities as fit, which is close to blocks at
  the level of components.
- **Sparse sets.** EnTT stores each component in its own dense array with a
  lookup table from entity to index, and can keep several components in the
  same order ("groups") so a system over them streams. Bevy lets each
  component choose: table storage, or a sparse set for components added and
  removed often.
- **Systems are streams.** A system is a query and a kernel, as in lesson 6:
  the ECS finds the tables that match, and hands the kernel each table's
  columns as slices. That stream over columns is the fast path every ECS is
  built for. Systems that say which columns they read (`&[T]`) and which they
  write (`&mut [T]`) are what lets a scheduler run them in parallel, and in
  Rust the borrow checker can check the split.
- **Lookups go through the handle.** Reaching one entity is handle, slot,
  table and row, then the column: `get` in lesson 4, and in the course's
  [2.5](../ecs/docs/2.5_typed_access.md). That is this lesson's lookup loop
  with an extra hop in front. ECS code keeps lookups for the places that need
  them, such as relationships between entities and collision, and finds the
  rest by query.

So the decisions that are yours:

1. **Keep components small**, to what one stream system reads. A component a
   system reads half of is AoS's wasted bytes again.
2. **Leave components as small structs**, `Position { x, y, z }`, unless a
   profiler shows a stream over them is the bottleneck. They are the best
   layout for lookups and good enough for most streams.
3. **For a hot stream over huge counts,** particles above all, store split or
   blocks, or gather into them for the hot loop.
4. **Make lookups rarer and less random** before tuning their layout.

## Rust notes

| rin | Rust |
|---|---|
| `push_slice<T>(arena&, count)`, 64-byte aligned by the arena | `Vec<T>`, aligned to `T`; `#[repr(align(64))]` on the type where it matters |
| `p: *vec3 = ...; q: *vec3 = ...;` into one array | read through `&`, then write through one `&mut` at a time |
| `px[a]`, unchecked in Release | checked in Release; cut slices to one length so one check covers several |
| `lines_per_lookup(layout: u32)`, with 0 to 3 | `enum Layout { Aos, Columns, Split, Blocks }` and a `match` |
| `t: [9]f64` of `clock()` readings | `ns_per(items, steps, \|\| kernel(...))`: a closure per loop |
| `k*.px[l] = p.x; k*.py[l] = p.y; ...` | `(k.px[l], k.py[l], k.pz[l]) = (p.x, p.y, p.z)`: assigning a tuple |
| `printf("%6.2f", ...)` | `{stream:6.2}` |

## Try

1. Sort `cs` by `a` before the timing loops (`cs.sort_by_key(|c| c.a)`) and
   run `--time` again. Which lookup numbers change, and by how much?
2. Set `BODY_COUNT` and `CONTACT_COUNT` to 8388608 and run `--time` in
   Release. Compare with the tables above.
3. Make the contacts push velocities too, `vx`, `vy` and `vz` of both
   bodies. How many lines does a lookup touch in each layout now?
4. Build with `-C target-cpu=native` and compare the stream column. Then
   rewrite `integrate_blocks` so the native build does eight lanes of one
   block at a time instead of gathering. (One way to start: a function that
   takes one block, marked `#[inline(never)]`.)
5. Write `contacts_gather`: for every eight contacts, copy the sixteen bodies'
   positions into `[f32; 8]` arrays, do the arithmetic over the arrays, and
   copy back. Contacts that share a body in the same eight break it; find the
   case and decide what to do about it.
6. Make `Body` 64 bytes with fields neither loop uses, and watch aos in both
   tables.
7. Remove `align(32)` from `Body`, print the address of `aos[0]` with
   `{:p}`, and work out whether the line count still holds for it.
