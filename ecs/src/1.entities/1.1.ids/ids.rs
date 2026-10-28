// 1.1 An entity is a number
//
// An entity names one thing in the game: a drone, a bullet, the turret. It has
// no data of its own and no behaviour. It is a number, and everything about
// the thing it names is stored somewhere else, in arrays, at that number.
//
// This chapter builds the simplest ECS there is: one array per component, as
// long as the most entities the world can hold, and a flag beside each array
// saying which entities have that component. A system is a loop over every
// entity that checks the flags it needs.
//
// It works, and it already has the shape every later chapter keeps: entities
// are ids, components are plain data in arrays, systems are loops. It also
// wastes memory and time in ways the next chapters remove one at a time.

#[derive(Clone, Copy, Default)]
struct Vec2 {
    x: f32,
    y: f32,
}

// An entity is a number. Wrapping it in a struct costs nothing at run time and
// keeps it from being mixed up with any other u32: a function that takes an
// Entity cannot be handed a count or a row by mistake.
#[derive(Clone, Copy)]
struct Entity {
    index: u32,
}

// The world: how many entities exist, and for each component an array of
// values and an array of flags, both indexed by the entity's number. Every
// array has room for 8, the most entities this world can hold.
#[derive(Default)]
struct World {
    count: u32,
    positions: [Vec2; 8],
    has_position: [bool; 8],
    velocities: [Vec2; 8],
    has_velocity: [bool; 8],
    healths: [f32; 8],
    has_health: [bool; 8],
}

impl World {
    // A new entity is the next number. It has no components yet.
    fn spawn(&mut self) -> Entity {
        assert!(self.count < 8, "the world is full");
        let e = Entity { index: self.count };
        self.count += 1;
        e
    }

    // Giving an entity a component is writing the value at the entity's number
    // and raising the flag beside it.
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
}

// A system: everything with a position and a velocity moves. It visits every
// entity in the world and skips the ones that lack either component. Returns
// how many it moved.
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
        print!("    entity {i}");
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

// How many of the 8 slots of one component's array hold a value.
fn used(flags: &[bool; 8]) -> usize {
    flags.iter().filter(|&&f| f).count()
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

    let bullet = w.spawn();
    w.set_position(bullet, Vec2 { x: 0.0, y: 0.0 });
    w.set_velocity(bullet, Vec2 { x: 4.0, y: 4.0 });

    println!("spawned turret {}, drones {} and {}, bullet {}",
        turret.index, drone_a.index, drone_b.index, bullet.index);
    print_world(&w);

    for step in 1..=2 {
        let moved = movement_system(&mut w, 0.5);
        println!("step {step}: the movement system looked at {} entities and moved {moved}", w.count);
    }
    print_world(&w);

    println!("every component array has room for 8; {} entities exist", w.count);
    println!("    positions hold {} values, velocities {}, healths {}",
        used(&w.has_position), used(&w.has_velocity), used(&w.has_health));
}
