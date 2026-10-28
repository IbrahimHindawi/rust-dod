# 1. AoS vs SoA

`src/1.aos_vs_soa/aos_vs_soa.rs`

The same 262,144 particles, stored three times: as a `Vec` of 68-byte
`Particle` structs (AoS), as seven `Vec`s, one per field (SoA), and with
position and velocity split one step further into six `Vec<f32>`, one per
coordinate (split). Sixty steps of movement run over each, and the program
checks that all three end with the same positions.

## What the loop fetches

Memory reaches the CPU in 64-byte cache lines. A loop that reads 24 bytes of
each 68-byte particle still pulls every byte of every line those particles
sit in, so about two thirds of what movement fetches in the AoS loop is tint,
age, lifetime, size and name. In the SoA loop the two arrays it reads are the
only thing it fetches, and the hardware prefetcher sees two simple streams.
The split loop fetches the same 24 bytes per particle as SoA, as six streams.

The program prints that arithmetic:

    bytes fetched per step: 17825792     (AoS: 68 bytes x 262144)
    bytes fetched per step: 6291456      (SoA: 24 bytes x 262144)
    bytes fetched per step: 6291456      (split: 6 x 4 bytes x 262144)

`Particle` is `#[repr(C)]`, so its fields sit in the order they're written.
Without it Rust may reorder a struct's fields to save padding. Here there's
none to save, so the size would be 68 either way.

## What the CPU does with each layout

SSE registers hold four floats, AVX registers eight. One instruction adds four
(or eight) pairs of floats at once, but only if they arrive side by side. Here
is what rustc 1.97 at `-C opt-level=3` (Release) makes of each loop on x86-64
Linux, read from the assembly:

| loop | particles per pass | instructions per pass | per particle | shuffles |
|---|---|---|---|---|
| `move_aos` | 2 | 29 | about 15 | 2 |
| `move_soa` | 4 | 77 | about 19 | 30 |
| `move_split` | 4 | 20 | 5 | 0 |
| `move_split`, `-C target-cpu=native` | 8 | 14 | under 2 | 0 |

- **AoS** goes one particle at a time. LLVM unrolls the loop twice and does
  each particle's x and y as a pair, but the next particle's fields are 68
  bytes away, so there is nothing to load four of. At `opt-level=2` it's
  plainly scalar, 17 instructions a particle.
- **SoA** is vectorized, four particles per pass, but each `Vec3` keeps x, y
  and z side by side. To get four x's into one register the compiler loads
  them one float at a time and stitches them together, then takes them apart
  again to store. About a tenth of the loop is arithmetic, and per particle it
  costs about what AoS does.
- **Split** is what SIMD wants. Each line of `move_split` becomes one load of
  four consecutive floats, one multiply, one add, one store:

      movups (%rcx,%rbx), %xmm3      four vx
      mulps  %xmm1, %xmm3            vx * dt
      movups (%rdi,%rbx), %xmm4      four px
      addps  %xmm3, %xmm4            px + vx * dt
      movups %xmm4, (%rdi,%rbx)      four px back

The SoA and split loops are the ones clang made of rin-dod's C, instruction
for instruction: rustc and clang share LLVM, and the layout decides the code,
not the language.

A layout is right for SIMD when its vector loop is mostly full-width loads,
math and stores. When it is mostly `movss`, `unpcklps` and `shufps`, the data
is fighting the registers.

Split is right for this loop, which runs over every particle in order. It is
wrong for a loop that reaches particles by index, where x, y and z in three
arrays are three trips to memory instead of one. Lesson 7 measures both kinds
of loop over four layouts.

## Reading the assembly

Ask rustc for it:

    rustc --edition 2024 -C opt-level=3 -C debug-assertions=off --emit asm -o aos_vs_soa.s src/1.aos_vs_soa/aos_vs_soa.rs

and search the `.s` file for `move_split:`. Rust mangles names, so the label
is `_RNv...10move_split:`, but the plain name is in it. Each kernel is marked
`#[inline(never)]` so it stays a function you can find; inlined into `main`,
the three would blend into one block of code.

`objdump -d --no-show-raw-insn` on a Release binary works too. The
third-party `cargo-show-asm` (`cargo asm --bin aos_vs_soa move_split`) prints
one function at a time; it's a convenience, and nothing in the course needs
it.

## Timings

`--time` prints how long each layout took. Release builds, best of seven
runs, on the 2.8 GHz Xeon these lessons were checked on, ms:

| build | AoS | SoA | split |
|---|---|---|---|
| `-C opt-level=3` | 71.0 | 37.4 | 18.8 |
| `-C opt-level=2` | 92.1 | 37.7 | 18.6 |
| `-C opt-level=3 -C target-cpu=native` | 160.7 | 21.0 | 18.2 |

The same loops with 8,192 particles, so everything stays in L2, and 1,920
steps so the work is the same:

| build | AoS | SoA | split |
|---|---|---|---|
| `-C opt-level=3` | 24.2 | 36.6 | 9.0 |
| `-C opt-level=2` | 32.8 | 36.7 | 9.1 |
| `-C opt-level=3 -C target-cpu=native` | 123.9 | 16.4 | 8.6 |

- **Split is the fastest everywhere,** by two to four times.
- **In cache, SoA is slower than AoS.** Its 30 shuffles cost more than AoS's
  wasted bytes, once the bytes come from L2 instead of memory. The SoA loop
  is vectorized, and that's not the same as fast.
- **From the 6 MB case, SoA's smaller footprint shows.** That machine has
  33 MB of L3, so the data stays in cache between steps, but SoA and split
  still move a third of the bytes AoS does.
- **`target-cpu=native` made AoS two to five times slower.** This Xeon has
  AVX-512, and with it LLVM "vectorizes" `move_aos` eight particles at a time
  with `vgatherdps` and `vscatterdps`: instructions that load and store eight
  floats from eight different addresses. They work one lane at a time inside
  the CPU, and the loop is slower than doing it by hand. SoA gained, because
  wider shuffles cost less. Split barely moved: it was already waiting on
  memory, not on arithmetic.

`target-cpu=native` builds for the CPU you're on, so a binary built that way
may not run on another machine, and what it does to these loops depends on
which CPU that is. The course's builds use the default target, which every
x86-64 CPU runs.

The machine is shared; expect about 20 percent noise between runs, and more
for AoS, which went anywhere from 70 to 125 ms. rin-dod's C, built with clang
`-O2` and run turn about with the Rust build on the same day, came out the
same within that noise: they are the same loops. Both were slower that day
than rin-dod's own tables, which is the machine, not the language.

Debug numbers are not worth comparing: Debug checks every index and every add
for overflow, and vectorizes nothing.

For a Release build, `python tools/check.py --release aos_vs_soa` leaves one
in `build/check/release/aos_vs_soa`; run it with `--time`. Or
`cargo run --release --bin aos_vs_soa -- --time`.

## The parameter list is the data

`move_aos(ps: &mut [Particle], ...)` says it needs particles.
`move_soa(positions: &mut [Vec3], velocities: &mut [Vec3], ...)` and
`move_split(px, py, pz, vx, vy, vz, ...)` say exactly which data they read and
write, and with `&mut` or `&`, which of the two. In lessons 2 and 3 every
system is written that way. In lesson 6 each system runs its own query, then
works only on the columns it named.

Naming the columns is what lets a scheduler see that two systems touch
different data, and it is what makes a system easy to test: hand it two
small arrays.

## Every `&mut` is noalias

As far as a C compiler can tell, two `float *` arrays could overlap. So
before it works on several particles at once, it checks at run time that the
two are apart, and keeps a one-at-a-time copy of the loop for when they are
not. rin's `noalias` on a parameter promises that nothing else reaches the
memory behind it, and the check goes away.

In Rust, every `&mut` makes that promise, and the borrow checker is what
keeps it true. rustc passes it on to LLVM as `noalias` on every `&mut`
parameter, so the check is never there. `move_soa` starts with two
instructions that take the shorter of the two lengths, for `zip`, and goes
straight to the vector loop:

    cmpq   %rsi, %rcx
    cmovbq %rcx, %rsi

rin checked the promise in Debug, and a Release build ran on with wrong
results. Rust checks it when the program is built. Pass `soa.positions` as
both arguments and it doesn't compile:

```
error[E0499]: cannot borrow `soa.positions` as mutable more than once at a time
    |
218 |         move_soa(&mut soa.positions, &mut soa.positions, DT);
    |         -------- ------------------  ^^^^^^^^^^^^^^^^^^ second mutable borrow occurs here
    |         |        |
    |         |        first mutable borrow occurs here
    |         first borrow later used by call
```

rin-dod's lesson also needed `noalias` on `move_aos`'s *one* slice, because on
Windows a slice was passed by reference, and a float written through it might
have been the slice's own length. Rust passes a slice as two values, its
pointer and its length, in registers, so there's nothing in memory for a
write to change.

**And `GRAVITY` is a `const`.** rin-dod made `gravity` `static`, so that gcc
would know no write through a slice could change it. A Rust `const` is a
value, not a place in memory: it's written into the code where it's used, and
there's nothing to change.

## Bounds checks

What Rust adds is a bounds check on every index, Release included. A check
that might fail inside a loop is a problem for SIMD: the loop must stop at
exactly the right particle, so it can't do four at once past a check it
hasn't made. `move_soa` has no index at all, because `zip` walks both slices
and stops at the shorter. `move_split` indexes six slices, so it cuts them to
one length first:

```rust
    let n = px.len();
    let (py, pz, vx, vy, vz) = (&mut py[..n], &mut pz[..n], &vx[..n], &mut vy[..n], &vz[..n]);
    for i in 0..n {
```

Now every slice is exactly `n` long, `i < n` is in bounds for all six, and the
checks are five compares at the top of the function, none in the loop. If a
slice is too short, the program stops before anything is written.

Without the cut, LLVM still vectorized this loop on this machine: it worked
out the shortest of the six lengths, ran the vector loop up to there, and
kept a checked one-at-a-time loop for the rest. The cut makes that certain
rather than lucky.

## Fused multiply-add

A fused multiply-add computes `p + v * dt` in one instruction that rounds
once instead of twice. C compilers fuse on their own when allowed to (rin-dod's
checksums moved with `-mfma`). **Rust never fuses on its own.** With
`-C target-cpu=native` on this AVX-512 machine, `move_split` is eight floats a
pass with separate `vmulps` and `vaddps`, and the output is byte for byte the
same as the default build's.

To get the fused instruction you ask for it, `v.x.mul_add(dt, p.x)`, and then
the checksum's last digits move. Without FMA in the target, `mul_add` calls a
much slower software routine to round once anyway.

## When AoS is right

When the code that touches a record touches most of its fields, AoS fetches
nothing wasted, and keeping a record together is simpler. Data-oriented
design does not mean SoA everywhere. It means choosing the layout from the
loops that run over it. An entity component system is the case where many
small loops each touch a few fields.

## Rust notes

| rin | Rust |
|---|---|
| `positions: noalias []vec3` | `positions: &mut [Vec3]`: every `&mut` is noalias, checked when the program is built |
| a Debug check that two `noalias` slices don't overlap | the same call is a compile error (E0499) |
| `gravity: static f32` | `const GRAVITY: f32`: a value, not a place in memory |
| `push_slice<T>(arena&, count)`, zeroed | `Vec::with_capacity(count)`, then `push` |
| `random_state`, a global changed by `random01()` | `struct Random`, passed as `&mut`: a global you change needs `unsafe` in Rust |
| `random_state * 6364136223846793005'u64`, which wraps | `wrapping_mul`: a plain `*` that overflows stops a Debug build |
| `NDEBUG` compiles bounds checks out of Release | Release keeps them; `zip`, or slices cut to one length, keep them out of loops |
| clang fuses multiply-adds with `-mfma` | rustc never does; `mul_add` asks for one |
| `clock()`, CPU time | `Instant::now()`, wall-clock time |
| `argc > 1 && strcmp(argv[1], "--time") == 0` | `std::env::args().nth(1).is_some_and(\|a\| a == "--time")` |

## Try

1. Run with `--time` in Release. Then remove `tint`, `age`, `lifetime` and
   `name` from `Particle` and run again. The AoS loop should close most of
   the gap, because the struct shrinks toward what the loop reads.
2. Build with `-C target-cpu=native` and compare `--time`. Then find
   `move_aos` in the assembly and count the `vgatherdps`. On a CPU without
   AVX-512 you'll see something else; what?
3. kayan stored positions as 16-byte `vec4`s. Give `Vec3` a fourth float and
   see what it does to the bytes-per-step line, to `move_soa`'s assembly and
   to the timing.
4. Store the split columns in blocks of eight, `struct Block { x: [f32; 8],
   y: [f32; 8], ... }`, and write the loop over blocks. Each block is a whole
   AVX register per coordinate, and a particle's coordinates stay near each
   other. (Lesson 7 does this, and finds a surprise with `target-cpu=native`.)
5. Pass `&mut soa.positions` as both arguments to `move_soa`, and read the
   error.
6. Delete the line that cuts the slices in `move_split` and compare its
   assembly. Then index `vx[i + 1]` instead of `vx[i]` (and stop one early).
   Does the loop still vectorize?
7. Write `p.x = v.x.mul_add(dt, p.x)` in `move_soa`, build with
   `-C target-cpu=native`, and watch the checksums.
