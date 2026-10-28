// 1. AoS vs SoA
//
// The first decision in data-oriented code is how a collection sits in memory,
// and it is made before any system is written.
//
// An array of structs (AoS) keeps each particle's fields together: position,
// velocity, tint, age, lifetime, size and a name, one particle after another.
// A struct of arrays (SoA) keeps each field together instead: every position,
// then every velocity, and so on, each in an array of its own.
//
// Both hold exactly the same data. What differs is what a loop drags through
// the cache. Movement reads position and velocity, 24 bytes of each particle.
// The CPU fetches memory 64 bytes at a time, so in AoS most of every line it
// fetches for movement is tint, age and name, which the loop never looks at. In
// SoA the two arrays movement reads are all it fetches.
//
// SoA still stores each position as a Vec3, x y z side by side. A third layout
// splits those too: one array of f32 per coordinate. Each array is then a
// straight run of floats, which is what a SIMD register loads: four floats at
// once with SSE, eight with AVX, nothing to rearrange first.
//
// The movement functions below also differ in what they ask for. move_aos takes
// the whole particle. move_soa and move_split take the arrays they use and
// nothing else, so their parameter lists are lists of their data dependencies.
// That habit is what the rest of these lessons build on.
//
// The program runs the same movement over all three layouts and checks that
// they agree. With `--time` it also times them. Build Release before believing
// the numbers: Debug checks every slice index and every add for overflow.

// A particle carries fields no loop reads, which is the point of the lesson.
// Dead-code warnings about them would be noise.
#![allow(dead_code)]
// `unsafe` is an error, except where an item allows it by name. Nothing here
// allows it.
#![deny(unsafe_code)]

use std::process::ExitCode;
use std::time::Instant;

const PARTICLE_COUNT: usize = 262144;
const STEPS: usize = 60;
const DT: f32 = 1.0 / 60.0;
// A constant is a value, not a place in memory, so no write through a slice
// can change it and the compiler never has to read it again.
const GRAVITY: f32 = -9.8;

#[derive(Clone, Copy, Default)]
struct Vec3 {
    x: f32,
    y: f32,
    z: f32,
}

#[derive(Clone, Copy, Default)]
struct Color {
    r: f32,
    g: f32,
    b: f32,
    a: f32,
}

#[derive(Clone, Copy, Default)]
struct ParticleName {
    text: [u8; 16],
}

// One particle, the way a particle system tends to grow. repr(C) keeps the
// fields in the order written, as C would lay them out; without it Rust may
// reorder fields, though here it would not change the size.
#[derive(Clone, Copy, Default)]
#[repr(C)]
struct Particle {
    position: Vec3,
    velocity: Vec3,
    tint: Color,
    age: f32,
    lifetime: f32,
    size: f32,
    name: ParticleName,
}

// The same particles, one Vec per field. Every Vec has the same length, and
// index i in each of them is particle i.
#[derive(Default)]
struct Particles {
    positions: Vec<Vec3>,
    velocities: Vec<Vec3>,
    tints: Vec<Color>,
    ages: Vec<f32>,
    lifetimes: Vec<f32>,
    sizes: Vec<f32>,
    names: Vec<ParticleName>,
}

// Position and velocity split one step further, one Vec per coordinate.
#[derive(Default)]
struct SplitColumns {
    px: Vec<f32>,
    py: Vec<f32>,
    pz: Vec<f32>,
    vx: Vec<f32>,
    vy: Vec<f32>,
    vz: Vec<f32>,
}

// A small LCG, so every layout starts from the same particles on every
// machine. The arithmetic is meant to wrap, and wrapping_mul and wrapping_add
// say so: a plain `*` would stop a Debug build on overflow.
struct Random {
    state: u64,
}

impl Random {
    fn next01(&mut self) -> f32 {
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.state >> 40) as f32 / 16777216.0
    }
}

fn make_particle(rng: &mut Random, i: usize) -> Particle {
    let mut p = Particle {
        position: Vec3 { x: rng.next01() * 100.0, y: rng.next01() * 100.0, z: rng.next01() * 100.0 },
        velocity: Vec3 { x: rng.next01() - 0.5, y: rng.next01() * 4.0, z: rng.next01() - 0.5 },
        tint: Color { r: rng.next01(), g: rng.next01(), b: rng.next01(), a: 1.0 },
        ..Particle::default()
    };
    p.lifetime = 2.0 + rng.next01() * 3.0;
    p.size = 0.1 + rng.next01();
    p.name.text[0] = b'p';
    p.name.text[1] = b'0' + (i % 10) as u8;
    p
}

// #[inline(never)] keeps each kernel a function of its own, so it can be
// found in the assembly by name and timed alone. Inlined into main, the three
// would blend into one block of code.
#[inline(never)]
fn move_aos(ps: &mut [Particle], dt: f32) {
    for p in ps {
        p.velocity.y += GRAVITY * dt;
        p.position.x += p.velocity.x * dt;
        p.position.y += p.velocity.y * dt;
        p.position.z += p.velocity.z * dt;
    }
}

// Two &mut slices are two slices nothing else can reach, so the compiler may
// work on several particles at once without checking that they don't overlap.
#[inline(never)]
fn move_soa(positions: &mut [Vec3], velocities: &mut [Vec3], dt: f32) {
    for (p, v) in positions.iter_mut().zip(velocities) {
        v.y += GRAVITY * dt;
        p.x += v.x * dt;
        p.y += v.y * dt;
        p.z += v.z * dt;
    }
}

// Each line of the loop is one SIMD load, multiply, add and store over
// consecutive floats. Cutting every slice to px's length first moves the
// bounds checks out of the loop: five length checks at the top, none inside,
// and a slice that is too short stops the program before anything is written.
#[inline(never)]
fn move_split(px: &mut [f32], py: &mut [f32], pz: &mut [f32], vx: &[f32], vy: &mut [f32], vz: &[f32], dt: f32) {
    let n = px.len();
    let (py, pz, vx, vy, vz) = (&mut py[..n], &mut pz[..n], &vx[..n], &mut vy[..n], &vz[..n]);
    for i in 0..n {
        vy[i] += GRAVITY * dt;
        px[i] += vx[i] * dt;
        py[i] += vy[i] * dt;
        pz[i] += vz[i] * dt;
    }
}

fn checksum_aos(ps: &[Particle]) -> f64 {
    ps.iter().map(|p| (p.position.x + p.position.y + p.position.z) as f64).sum()
}

fn checksum_soa(positions: &[Vec3]) -> f64 {
    positions.iter().map(|p| (p.x + p.y + p.z) as f64).sum()
}

fn checksum_split(xs: &[f32], ys: &[f32], zs: &[f32]) -> f64 {
    xs.iter().zip(ys).zip(zs).map(|((x, y), z)| (x + y + z) as f64).sum()
}

fn main() -> ExitCode {
    let timing = std::env::args().nth(1).is_some_and(|a| a == "--time");
    let mut rng = Random { state: 12345 };

    // The same particle goes into every layout: whole into aos, field by field
    // into soa, coordinate by coordinate into split.
    let mut aos = Vec::with_capacity(PARTICLE_COUNT);
    let mut soa = Particles::default();
    let mut split = SplitColumns::default();
    for i in 0..PARTICLE_COUNT {
        let p = make_particle(&mut rng, i);
        aos.push(p);
        soa.positions.push(p.position);
        soa.velocities.push(p.velocity);
        soa.tints.push(p.tint);
        soa.ages.push(p.age);
        soa.lifetimes.push(p.lifetime);
        soa.sizes.push(p.size);
        soa.names.push(p.name);
        split.px.push(p.position.x);
        split.py.push(p.position.y);
        split.pz.push(p.position.z);
        split.vx.push(p.velocity.x);
        split.vy.push(p.velocity.y);
        split.vz.push(p.velocity.z);
    }

    let t0 = Instant::now();
    for _ in 0..STEPS {
        move_aos(&mut aos, DT);
    }
    let t1 = Instant::now();
    for _ in 0..STEPS {
        move_soa(&mut soa.positions, &mut soa.velocities, DT);
    }
    let t2 = Instant::now();
    for _ in 0..STEPS {
        move_split(&mut split.px, &mut split.py, &mut split.pz, &split.vx, &mut split.vy, &split.vz, DT);
    }
    let t3 = Instant::now();

    let used = size_of::<Vec3>() * 2;
    println!("{PARTICLE_COUNT} particles, {STEPS} steps");
    println!("array of structs: a particle is {} bytes, movement uses {used} of them", size_of::<Particle>());
    println!("  bytes fetched per step: {}", size_of::<Particle>() * PARTICLE_COUNT);
    println!("struct of arrays: movement reads two arrays of {}-byte Vec3", size_of::<Vec3>());
    println!("  bytes fetched per step: {}", used * PARTICLE_COUNT);
    println!("split columns: movement reads six arrays of {}-byte f32", size_of::<f32>());
    println!("  bytes fetched per step: {}", used * PARTICLE_COUNT);

    let a = checksum_aos(&aos);
    let b = checksum_soa(&soa.positions);
    let c = checksum_split(&split.px, &split.py, &split.pz);
    println!("checksum aos {a:.3}");
    println!("checksum soa {b:.3}");
    println!("checksum split {c:.3}");
    let same = a == b && b == c;
    println!("same result: {}", if same { "yes" } else { "no" });

    if timing {
        let ms = |from: Instant, to: Instant| (to - from).as_secs_f64() * 1000.0;
        println!("aos {:.1} ms, soa {:.1} ms, split {:.1} ms", ms(t0, t1), ms(t1, t2), ms(t2, t3));
    }
    if same { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}
