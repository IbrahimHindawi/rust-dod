# 7. An arena

`dojo/7.arena/play_arena.rs`

Most of what a frame allocates dies with the frame: commands, scratch lists,
text for the screen. An **arena** hands all of it out of one buffer and keeps
one number, how much of the buffer is used. A push rounds that number up to
the alignment asked for, hands out the offset, and moves the number past the
new bytes. Nothing is freed on its own. A **mark** remembers the number, a
**rewind** puts it back, and a **clear** sets it to 0: everything pushed since
goes at once, in one store, with no list of what to free.

    push(4, 4) with 1 used:   round 1 up to 4, hand out 4, used is now 8

For an `align` that is a power of two, rounding up is
`(used + align - 1) & !(align - 1)`: adding `align - 1` reaches the next
multiple unless `used` is one already, and the mask drops what's past it.

**The invariant:** every push not yet let go of starts at a multiple of its
alignment, ends at or before `used`, and stops before the next push starts;
`used` never passes the end of the buffer.

In the course: rin-dod allocates everything from arenas, and rust-dod uses
`Vec`s instead; [0](../../docs/0_from_kayan.md) says why. The arena's way of
freeing is still there. The command buffer in
[3.3](../../ecs/docs/3.3_commands.md) keeps its bytes in a frame arena made of
a `Vec`, cleared after every frame, and so does kayan's in
[6](../../docs/6_kayan.md).

## Watch it: the playground

    cargo run --bin play_arena
    python tools/dojo.py play arena

It pushes things of different sizes and alignments into a 64-byte buffer,
rewinds, fills it to the last byte, and clears it. After each step it draws
the buffer, 16 bytes to a group: a letter for each byte of a push, `-` for
padding, `.` for free.

```
push b: a u32 count, 4 bytes, align 4
    offset 4, used 8 of 64: 3 bytes of padding first
    a---bbbb........|................|................|................
...
rewind to the mark: e and f are gone at once
    a---bbbbcccccccc|dddddddddddddddd|................|................
```

Each push is filled with its letter, and `verify()` checks after every step
that each push is aligned, inside `used`, clear of the next, and still holds
its letter. Step through it: break on `push` and watch `used`. The buffer is
a `#[repr(C, align(16))]` struct, so an offset that is a multiple of 16 is an
address that is one too.

Then flip the switch at the top, `ALIGN_OFFSETS`, to `false` and run it again.
Pushes are handed out at `used` as it is, packed with no padding, and the
`u32`, the `u64` and the 16-byte vector land at offsets 1, 5 and 13:

```
    verify: b is at offset 1, not a multiple of 4: its address isn't aligned
```

The playground only ever writes bytes, so nothing breaks yet. Read a `u32`
from there through a `&u32`, though, and it's undefined behaviour in Rust: on
x86 it usually works, slower when it straddles two cache lines; an aligned
SIMD load faults; and the optimiser is allowed to assume it never happens.

## Do reps: the kata

You write `Arena`, over bytes the caller lends it:

```rust
pub struct Arena<'a> {
    bytes: &'a mut [u8],
    // Bytes before `used` are handed out; bytes after it are free.
    used: usize,
}
```

Its one function with an idea in it:

```rust
    pub fn push(&mut self, size: usize, align: usize) -> Option<usize> {
```

an offset that is a multiple of `align`, or `None`, changing nothing, when
the bytes would end past the last one. Plus `new(bytes)`, `bytes(offset, len)`
and `bytes_mut(offset, len)` to read and write a push, `used()`,
`capacity()`, `mark()`, `rewind(mark)` and `clear()`.

Push hands out an offset, not a `&mut [u8]`: a slice from `push` would borrow
the whole arena for as long as you kept it, and you couldn't push again until
it was gone. An offset is a number, and `bytes_mut` borrows only while you
write. It's the idea of dojo 8, index, not pointer, in miniature.

| level | you write | target |
|---|---|---|
| 1 | `push`, in an otherwise finished file | 2:00 |
| 2 | every function body; the fields and signatures are given | 6:00 |
| 3 | the whole file, from a blank page | 8:00 |

    python tools/dojo.py start arena 2
    python tools/dojo.py arena

The harness runs the playground's steps as a scenario, then 20,000 random
pushes, marks, rewinds and clears on a 256-byte buffer, against a model that
keeps the used count and every push not yet let go of. Pushes are 1 to 24
bytes at alignments 1 to 16, with some sized to end exactly at the last byte
and some one byte past it. Each push is filled with a pattern through
`bytes_mut` and read back through `bytes`, so a push that lands on another
shows.

**Level 4, variations.** The same harness, different insides:

- **Align the address, not the offset**: round
  `self.bytes.as_ptr() as usize + used` up, and subtract the start again.
  rin's `memops_arena` does it this way. The answers are the same for the
  harness's buffers, which start 16-aligned; what changes for a buffer that
  doesn't?
- **`push_zeroed`**: the same, with the bytes zeroed. Rewind, push again, and
  read the bytes before zeroing: what's there? (Not checked by the harness;
  write your own test.)
- **A scope that rewinds itself**: `scope(&mut self)` returns a guard that
  holds the mark, derefs to the arena, and rewinds in its `Drop`. What does
  the borrow checker let you do with the arena while the guard is alive?
  (Not checked by the harness; write your own test.)

## When it fails

    FAIL in the scenario: push 4 bytes, align 4: 3 bytes of padding first gave 1, it should give 4

No rounding up at all: the offset is `used` as it is. The fuzz run is what
finds the rest:

    FAIL at operation 15 (seed 1): the 14 bytes at 84, pushed at operation 13, have changed: byte 0 is 0, it should be 13
        debug it: build/dojo/arena/harness --seed 1 --stop 15
        with a breakpoint on stop_here, which runs just before that operation

A push whose bytes changed was written over: two pushes share bytes, or
`bytes_mut` wrote somewhere other than where it was asked to. That one is a
`bytes_mut` that ignores `offset`. Run the harness under your debugger with
those arguments and a breakpoint on `stop_here`, and step back to operation
13 with `--stop 13` to watch the push that the pattern went into.
