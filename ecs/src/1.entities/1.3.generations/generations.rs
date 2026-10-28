// 1.3 Generations
//
// 1.2's bug: a number outlives the entity it named, and when the number is
// reused the old reference names the new entity. The fix is to count lives.
// Each number gets a generation, and a reference carries both: entity 1 in its
// first life is 1:1, and the entity that reuses the number is 1:2. Despawning
// bumps the generation, so every reference to the dead entity stops matching
// at once, wherever it is kept.
//
// A reference with a generation is called a handle. Checking one is a single
// comparison, so every access can afford it.

use std::fmt;
use std::num::NonZeroU32;

#[derive(Clone, Copy, Default)]
struct Vec2 {
    x: f32,
    y: f32,
}

// A handle: which number, and which life of that number. Generations start at
// 1, and NonZeroU32 makes that a rule of the type: a generation can never be 0.
// Rust uses the forbidden 0 to store Option<Entity>'s None, so "no entity" is
// None, and an Option<Entity> is no bigger than an Entity.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Entity {
    index: u32,
    generation: NonZeroU32,
}

// A handle prints as index:generation.
impl fmt::Display for Entity {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:{}", self.index, self.generation)
    }
}

// Why an operation did nothing. `Dead`: the handle no longer names a live
// entity. Operations return Result<(), Error>, and a Result is must_use:
// dropping one unread is a warning, and the build treats warnings as errors.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Error {
    Dead,
}

// How the program prints a Result.
fn status(r: Result<(), Error>) -> &'static str {
    match r {
        Ok(()) => "ok",
        Err(Error::Dead) => "dead",
    }
}

const ONE: NonZeroU32 = NonZeroU32::MIN;

// `generations[i]` is the life number `i` is in, or will be in next if `i` is
// free. The `alive` array from 1.2 is gone: the generations say it.
#[derive(Default)]
struct World {
    count: u32,
    generations: [Option<NonZeroU32>; 8],
    free: [u32; 8],
    free_count: u32,
    positions: [Vec2; 8],
    has_position: [bool; 8],
    velocities: [Vec2; 8],
    has_velocity: [bool; 8],
    healths: [f32; 8],
    has_health: [bool; 8],
}

impl World {
    // A handle is alive when its generation is the one its number is in now.
    // Despawning bumps the number's generation straight away, so a free number
    // already holds a generation no handle has been given yet, and no handle
    // matches it.
    fn alive(&self, e: Entity) -> bool {
        e.index < self.count && self.generations[e.index as usize] == Some(e.generation)
    }

    fn spawn(&mut self) -> Entity {
        let index = if self.free_count > 0 {
            self.free_count -= 1;
            self.free[self.free_count as usize]
        } else {
            assert!(self.count < 8, "the world is full");
            self.generations[self.count as usize] = Some(ONE);
            self.count += 1;
            self.count - 1
        };
        let generation = self.generations[index as usize].expect("a number in use has a generation");
        Entity { index, generation }
    }

    // A dead handle reports `Dead` and changes nothing, so despawning twice is
    // harmless.
    fn despawn(&mut self, e: Entity) -> Result<(), Error> {
        if !self.alive(e) {
            return Err(Error::Dead);
        }
        let i = e.index as usize;
        self.has_position[i] = false;
        self.has_velocity[i] = false;
        self.has_health[i] = false;
        self.generations[i] = e.generation.checked_add(1);
        self.free[self.free_count as usize] = e.index;
        self.free_count += 1;
        Ok(())
    }

    fn set_position(&mut self, e: Entity, p: Vec2) {
        self.positions[e.index as usize] = p;
        self.has_position[e.index as usize] = true;
    }

    fn set_velocity(&mut self, e: Entity, v: Vec2) {
        self.velocities[e.index as usize] = v;
        self.has_velocity[e.index as usize] = true;
    }

    fn set_health(&mut self, e: Entity, points: f32) {
        self.healths[e.index as usize] = points;
        self.has_health[e.index as usize] = true;
    }

    // Reading through a handle checks it first. A dead handle, or an entity
    // without a position, gives None instead of whatever is at the number.
    fn get_position(&self, e: Entity) -> Option<&Vec2> {
        if !self.alive(e) || !self.has_position[e.index as usize] {
            return None;
        }
        Some(&self.positions[e.index as usize])
    }
}

fn movement_system(w: &mut World, dt: f32) -> u32 {
    let mut moved = 0;
    for i in 0..w.count as usize {
        if w.has_position[i] && w.has_velocity[i] {
            w.positions[i].x += w.velocities[i].x * dt;
            w.positions[i].y += w.velocities[i].y * dt;
            moved += 1;
        }
    }
    moved
}

fn print_world(w: &World) {
    for i in 0..w.count as usize {
        let generation = w.generations[i].expect("a number in use has a generation");
        if !w.has_position[i] && !w.has_velocity[i] && !w.has_health[i] {
            println!("    entity {i}  (free, next generation {generation})");
            continue;
        }
        print!("    entity {i}:{generation}");
        if w.has_position[i] {
            print!("  position ({:5.2}, {:5.2})", w.positions[i].x, w.positions[i].y);
        } else {
            print!("  position -              ");
        }
        if w.has_velocity[i] {
            print!("  velocity ({:5.2}, {:5.2})", w.velocities[i].x, w.velocities[i].y);
        } else {
            print!("  velocity -              ");
        }
        if w.has_health[i] {
            print!("  health {:.0}", w.healths[i]);
        } else {
            print!("  health -");
        }
        println!();
    }
}

// The turret's aim: the target's position, or a new target if it is gone.
fn aim(w: &World, target: &mut Entity, fallback: Entity) {
    let p = match w.get_position(*target) {
        Some(p) => p,
        None => {
            println!("the turret's target {target} is dead; it picks {fallback} instead");
            *target = fallback;
            w.get_position(*target).expect("the fallback is alive and has a position")
        }
    };
    println!("the turret aims at {target}, at ({:.2}, {:.2})", p.x, p.y);
}

fn main() {
    let mut w = World::default();

    let turret = w.spawn();
    w.set_position(turret, Vec2 { x: 0.0, y: 0.0 });
    w.set_health(turret, 500.0);

    let drone_a = w.spawn();
    w.set_position(drone_a, Vec2 { x: 10.0, y: 0.0 });
    w.set_velocity(drone_a, Vec2 { x: -1.0, y: 0.5 });
    w.set_health(drone_a, 100.0);

    let drone_b = w.spawn();
    w.set_position(drone_b, Vec2 { x: 0.0, y: 10.0 });
    w.set_velocity(drone_b, Vec2 { x: 0.5, y: -1.0 });
    w.set_health(drone_b, 100.0);

    let mut target = drone_a;
    print_world(&w);
    aim(&w, &mut target, drone_b);

    println!("drone {drone_a} is destroyed and despawned");
    w.despawn(drone_a).expect("the drone is alive");

    let bullet = w.spawn();
    w.set_position(bullet, Vec2 { x: 0.0, y: 0.0 });
    w.set_velocity(bullet, Vec2 { x: 4.0, y: 4.0 });
    println!("the turret fires: the bullet is {bullet}, the same number in its next life");
    movement_system(&mut w, 0.5);
    print_world(&w);

    // The same moment as the end of 1.2, and this time the handle knows.
    aim(&w, &mut target, drone_b);

    println!("despawning {drone_a} again: {}", status(w.despawn(drone_a)));

    let none: Option<Entity> = None;
    println!("no entity is {}, and an Option<Entity> is {} bytes, the same as an Entity's {}",
        if none.is_none() { "None" } else { "Some" },
        std::mem::size_of::<Option<Entity>>(), std::mem::size_of::<Entity>());
}
