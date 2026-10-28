// 7. Layout follows access
//
// Lesson 1 found the layout SIMD wants: one array per float. This lesson asks
// what that layout costs everywhere else. The same bodies are stored four ways,
// and two kinds of loop run over each:
//
//   stream   every body, in order, the same math: integration.
//   lookup   bodies picked by index, two at a time, in no order: contacts,
//            the way collision and constraints reach them.
//
// The four layouts:
//
//   aos      Vec<Body>, everything about a body together
//   columns  Vec<Vec3> per field, as an ECS table stores components
//   split    Vec<f32> per coordinate, as lesson 1's move_split
//   blocks   Vec<BodyBlock>, eight bodies per block, one [f32; 8] per
//            coordinate
//
// A stream wants consecutive floats, so SIMD can load four or eight at once.
// A lookup wants one body's fields close together, so one cache line brings
// them all. No layout is best at both; the program prints how many cache lines
// a lookup touches in each, and with `--time` how long each loop takes.

// A body carries fields neither loop reads, which is part of the comparison.
// Dead-code warnings about them would be noise.
#![allow(dead_code)]
// `unsafe` is an error, except where an item allows it by name. Nothing here
// allows it.
#![deny(unsafe_code)]

use std::process::ExitCode;
use std::time::Instant;

const BODY_COUNT: usize = 1048576;
const CONTACT_COUNT: usize = 1048576;
const DT: f32 = 1.0 / 60.0;
const GRAVITY: f32 = -9.8;
const STIFFNESS: f32 = 0.01;

#[derive(Clone, Copy, Default)]
struct Vec3 {
    x: f32,
    y: f32,
    z: f32,
}

// aos: 32 bytes, the two fields both loops use and two they do not. Aligned
// to its size, so no body straddles a cache line.
#[derive(Clone, Copy, Default)]
#[repr(C, align(32))]
struct Body {
    position: Vec3,
    velocity: Vec3,
    inverse_mass: f32,
    radius: f32,
}

// blocks: eight bodies, each coordinate a run of eight floats. 192 bytes, three
// cache lines, and aligned to start on one.
#[derive(Clone, Copy, Default)]
#[repr(C, align(64))]
struct BodyBlock {
    px: [f32; 8],
    py: [f32; 8],
    pz: [f32; 8],
    vx: [f32; 8],
    vy: [f32; 8],
    vz: [f32; 8],
}

// Two bodies a contact pushes apart.
#[derive(Clone, Copy)]
struct Contact {
    a: u32,
    b: u32,
}

#[derive(Default)]
struct Columns {
    positions: Vec<Vec3>,
    velocities: Vec<Vec3>,
}

#[derive(Default)]
struct Split {
    px: Vec<f32>,
    py: Vec<f32>,
    pz: Vec<f32>,
    vx: Vec<f32>,
    vy: Vec<f32>,
    vz: Vec<f32>,
}

// A small LCG, so every layout starts from the same bodies and contacts on
// every machine. The arithmetic is meant to wrap, and says so.
struct Random {
    state: u64,
}

impl Random {
    fn step(&mut self) -> u64 {
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.state
    }

    fn next01(&mut self) -> f32 {
        (self.step() >> 40) as f32 / 16777216.0
    }

    fn index(&mut self, n: usize) -> u32 {
        ((self.step() >> 33) % n as u64) as u32
    }
}

// ---------------------------------------------------------------- stream
// Every body, in order, the same math. #[inline(never)] keeps each kernel a
// function of its own, to find in the assembly and to time alone.

#[inline(never)]
fn integrate_aos(bodies: &mut [Body], dt: f32) {
    for b in bodies {
        b.velocity.y += GRAVITY * dt;
        b.position.x += b.velocity.x * dt;
        b.position.y += b.velocity.y * dt;
        b.position.z += b.velocity.z * dt;
    }
}

#[inline(never)]
fn integrate_columns(positions: &mut [Vec3], velocities: &mut [Vec3], dt: f32) {
    for (p, v) in positions.iter_mut().zip(velocities) {
        v.y += GRAVITY * dt;
        p.x += v.x * dt;
        p.y += v.y * dt;
        p.z += v.z * dt;
    }
}

// Every slice cut to px's length first, as in lesson 1: the bounds checks
// happen once, at the top, and the loop has none.
#[inline(never)]
fn integrate_split(px: &mut [f32], py: &mut [f32], pz: &mut [f32], vx: &[f32], vy: &mut [f32], vz: &[f32], dt: f32) {
    let n = px.len();
    let (py, pz, vx, vy, vz) = (&mut py[..n], &mut pz[..n], &vx[..n], &mut vy[..n], &vz[..n]);
    for i in 0..n {
        vy[i] += GRAVITY * dt;
        px[i] += vx[i] * dt;
        py[i] += vy[i] * dt;
        pz[i] += vz[i] * dt;
    }
}

// The inner loop is eight floats per coordinate: one AVX register, or two SSE.
// The arrays are [f32; 8], so `l` is in bounds by type and nothing is checked.
#[inline(never)]
fn integrate_blocks(blocks: &mut [BodyBlock], dt: f32) {
    for k in blocks {
        for l in 0..8 {
            k.vy[l] += GRAVITY * dt;
            k.px[l] += k.vx[l] * dt;
            k.py[l] += k.vy[l] * dt;
            k.pz[l] += k.vz[l] * dt;
        }
    }
}

// ---------------------------------------------------------------- lookup
// Each contact moves its two bodies a little toward each other. The bodies are
// anywhere in the arrays, so every contact is two jumps through memory.
//
// Both bodies are in one slice, and may even be the same body, so Rust will
// not lend a `&mut` to each at once. It doesn't need to: each loop reads both
// bodies, then writes one, then the other, one borrow at a time. The arithmetic
// is the same in every layout, in the same order, so the results match to the
// bit.

#[inline(never)]
fn contacts_aos(bodies: &mut [Body], cs: &[Contact]) {
    for c in cs {
        let (a, b) = (c.a as usize, c.b as usize);
        let (p, q) = (&bodies[a].position, &bodies[b].position);
        let (dx, dy, dz) = (q.x - p.x, q.y - p.y, q.z - p.z);
        let p = &mut bodies[a].position;
        p.x += dx * STIFFNESS;
        p.y += dy * STIFFNESS;
        p.z += dz * STIFFNESS;
        let q = &mut bodies[b].position;
        q.x -= dx * STIFFNESS;
        q.y -= dy * STIFFNESS;
        q.z -= dz * STIFFNESS;
    }
}

#[inline(never)]
fn contacts_columns(positions: &mut [Vec3], cs: &[Contact]) {
    for c in cs {
        let (a, b) = (c.a as usize, c.b as usize);
        let (p, q) = (&positions[a], &positions[b]);
        let (dx, dy, dz) = (q.x - p.x, q.y - p.y, q.z - p.z);
        let p = &mut positions[a];
        p.x += dx * STIFFNESS;
        p.y += dy * STIFFNESS;
        p.z += dz * STIFFNESS;
        let q = &mut positions[b];
        q.x -= dx * STIFFNESS;
        q.y -= dy * STIFFNESS;
        q.z -= dz * STIFFNESS;
    }
}

// The indices are random, so each one is checked, Release included. Cut to one
// length, the three slices need two checks a contact instead of six.
#[inline(never)]
fn contacts_split(px: &mut [f32], py: &mut [f32], pz: &mut [f32], cs: &[Contact]) {
    let n = px.len();
    let (py, pz) = (&mut py[..n], &mut pz[..n]);
    for c in cs {
        let (a, b) = (c.a as usize, c.b as usize);
        let dx = px[b] - px[a];
        let dy = py[b] - py[a];
        let dz = pz[b] - pz[a];
        px[a] += dx * STIFFNESS;
        py[a] += dy * STIFFNESS;
        pz[a] += dz * STIFFNESS;
        px[b] -= dx * STIFFNESS;
        py[b] -= dy * STIFFNESS;
        pz[b] -= dz * STIFFNESS;
    }
}

#[inline(never)]
fn contacts_blocks(blocks: &mut [BodyBlock], cs: &[Contact]) {
    for c in cs {
        let (a, b) = (c.a as usize, c.b as usize);
        let (la, lb) = (a % 8, b % 8);
        let (p, q) = (&blocks[a / 8], &blocks[b / 8]);
        let (dx, dy, dz) = (q.px[lb] - p.px[la], q.py[lb] - p.py[la], q.pz[lb] - p.pz[la]);
        let p = &mut blocks[a / 8];
        p.px[la] += dx * STIFFNESS;
        p.py[la] += dy * STIFFNESS;
        p.pz[la] += dz * STIFFNESS;
        let q = &mut blocks[b / 8];
        q.px[lb] -= dx * STIFFNESS;
        q.py[lb] -= dy * STIFFNESS;
        q.pz[lb] -= dz * STIFFNESS;
    }
}

// ---------------------------------------------------------------- measuring

#[derive(Clone, Copy)]
enum Layout {
    Aos,
    Columns,
    Split,
    Blocks,
}

// How many 64-byte lines hold one body's x, y and z, given each one's byte
// offset from the start of its array. Each float is 4 bytes at a multiple of
// 4, so none of them crosses a line on its own.
fn lines_for(x: usize, y: usize, z: usize) -> usize {
    let (lx, ly, lz) = (x / 64, y / 64, z / 64);
    1 + usize::from(ly != lx) + usize::from(lz != lx && lz != ly)
}

// The average over the first 4096 bodies, which covers every repeating
// pattern. The offsets count from a line-aligned start: Body and BodyBlock are
// aligned so that it holds wherever the Vec puts them, and for Vec3 and f32
// the count comes out the same at any start.
fn lines_per_lookup(layout: Layout) -> f64 {
    let total: usize = (0..4096)
        .map(|i| match layout {
            Layout::Aos => {
                let base = i * size_of::<Body>();
                lines_for(base, base + 4, base + 8)
            }
            Layout::Columns => {
                let base = i * size_of::<Vec3>();
                lines_for(base, base + 4, base + 8)
            }
            // three separate arrays: never the same line
            Layout::Split => 3,
            Layout::Blocks => {
                let base = (i / 8) * size_of::<BodyBlock>() + (i % 8) * 4;
                lines_for(base, base + 32, base + 64)
            }
        })
        .sum();
    total as f64 / 4096.0
}

fn checksum_aos(bodies: &[Body]) -> f64 {
    bodies.iter().map(|b| (b.position.x + b.position.y + b.position.z) as f64).sum()
}

fn checksum_columns(positions: &[Vec3]) -> f64 {
    positions.iter().map(|p| (p.x + p.y + p.z) as f64).sum()
}

fn checksum_split(xs: &[f32], ys: &[f32], zs: &[f32]) -> f64 {
    xs.iter().zip(ys).zip(zs).map(|((x, y), z)| (x + y + z) as f64).sum()
}

fn checksum_blocks(blocks: &[BodyBlock]) -> f64 {
    blocks.iter().flat_map(|k| (0..8).map(move |l| (k.px[l] + k.py[l] + k.pz[l]) as f64)).sum()
}

// Run `f` `steps` times and return the time per item, in nanoseconds, for
// `items` items a run.
fn ns_per(items: usize, steps: usize, mut f: impl FnMut()) -> f64 {
    let start = Instant::now();
    for _ in 0..steps {
        f();
    }
    start.elapsed().as_secs_f64() * 1.0e9 / (items * steps) as f64
}

fn main() -> ExitCode {
    let timing = std::env::args().nth(1).is_some_and(|a| a == "--time");
    let steps = if timing { 20 } else { 2 };
    let mut rng = Random { state: 12345 };

    let mut aos = Vec::with_capacity(BODY_COUNT);
    let mut col = Columns::default();
    let mut spl = Split::default();
    let mut blocks = vec![BodyBlock::default(); BODY_COUNT / 8];
    for i in 0..BODY_COUNT {
        let p = Vec3 { x: rng.next01() * 100.0, y: rng.next01() * 100.0, z: rng.next01() * 100.0 };
        let v = Vec3 { x: rng.next01() - 0.5, y: rng.next01() * 4.0, z: rng.next01() - 0.5 };
        aos.push(Body { position: p, velocity: v, inverse_mass: 1.0, radius: 0.5 });
        col.positions.push(p);
        col.velocities.push(v);
        spl.px.push(p.x);
        spl.py.push(p.y);
        spl.pz.push(p.z);
        spl.vx.push(v.x);
        spl.vy.push(v.y);
        spl.vz.push(v.z);
        let (k, l) = (&mut blocks[i / 8], i % 8);
        (k.px[l], k.py[l], k.pz[l]) = (p.x, p.y, p.z);
        (k.vx[l], k.vy[l], k.vz[l]) = (v.x, v.y, v.z);
    }
    let cs: Vec<Contact> = (0..CONTACT_COUNT)
        .map(|_| Contact { a: rng.index(BODY_COUNT), b: rng.index(BODY_COUNT) })
        .collect();

    // stream, then lookup, for each layout in turn
    let aos_stream = ns_per(BODY_COUNT, steps, || integrate_aos(&mut aos, DT));
    let aos_lookup = ns_per(CONTACT_COUNT, steps, || contacts_aos(&mut aos, &cs));
    let col_stream = ns_per(BODY_COUNT, steps, || integrate_columns(&mut col.positions, &mut col.velocities, DT));
    let col_lookup = ns_per(CONTACT_COUNT, steps, || contacts_columns(&mut col.positions, &cs));
    let spl_stream = ns_per(BODY_COUNT, steps, || {
        integrate_split(&mut spl.px, &mut spl.py, &mut spl.pz, &spl.vx, &mut spl.vy, &spl.vz, DT)
    });
    let spl_lookup = ns_per(CONTACT_COUNT, steps, || contacts_split(&mut spl.px, &mut spl.py, &mut spl.pz, &cs));
    let blk_stream = ns_per(BODY_COUNT, steps, || integrate_blocks(&mut blocks, DT));
    let blk_lookup = ns_per(CONTACT_COUNT, steps, || contacts_blocks(&mut blocks, &cs));

    println!("{BODY_COUNT} bodies, {CONTACT_COUNT} contacts, {steps} steps");
    println!("bytes per body: aos {}, columns {}, split {}, blocks {}",
        size_of::<Body>(), size_of::<Vec3>() * 2, size_of::<f32>() * 6, size_of::<BodyBlock>() / 8);
    println!("cache lines one lookup touches: aos {:.3}, columns {:.3}, split {:.3}, blocks {:.3}",
        lines_per_lookup(Layout::Aos), lines_per_lookup(Layout::Columns),
        lines_per_lookup(Layout::Split), lines_per_lookup(Layout::Blocks));

    let sums = [
        checksum_aos(&aos),
        checksum_columns(&col.positions),
        checksum_split(&spl.px, &spl.py, &spl.pz),
        checksum_blocks(&blocks),
    ];
    println!("checksum aos {:.3}", sums[0]);
    println!("checksum columns {:.3}", sums[1]);
    println!("checksum split {:.3}", sums[2]);
    println!("checksum blocks {:.3}", sums[3]);
    let same = sums.iter().all(|&s| s == sums[0]);
    println!("same result: {}", if same { "yes" } else { "no" });

    if timing {
        println!("ns per body     stream   ns per contact   lookup");
        for (name, stream, lookup) in [
            ("aos", aos_stream, aos_lookup),
            ("columns", col_stream, col_lookup),
            ("split", spl_stream, spl_lookup),
            ("blocks", blk_stream, blk_lookup),
        ] {
            println!("  {name:<14}{stream:6.2}                    {lookup:6.2}");
        }
    }
    if same { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}
