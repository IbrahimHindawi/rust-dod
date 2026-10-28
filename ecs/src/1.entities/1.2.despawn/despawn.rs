// 1.2 Despawning, and reusing numbers
//
// Games destroy things all the time: a bullet hits, a drone dies. 1.1 could
// only add. Here an entity can be despawned, and its number goes on a free
// list, so the next spawn takes it instead of growing the world. Without that,
// a game that fires a bullet a frame would run out of numbers in seconds.
//
// Reuse has a cost, and this program walks straight into it. Something that
// kept an entity's number, the turret's target, still holds it after the
// entity is gone. When the number is handed to a new entity, the old
// reference silently names the new one. 1.3 fixes that.
//
// Rust does not catch this one. The target is a number, not a reference, so
// the borrow checker has nothing to check: a stale number is a logic bug, and
// logic bugs compile.

#[derive(Clone, Copy, Default)]
struct Vec2 {
    x: f32,
    y: f32,
}

#[derive(Clone, Copy)]
struct Entity {
    index: u32,
}

// `alive` says which numbers are in use. `free` holds the numbers of despawned
// entities, `free_count` of them, ready to be handed out again.
#[derive(Default)]
struct World {
    count: u32,
    alive: [bool; 8],
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
    // A free number is reused before a new one is made. A reused number must
    // start with no components: whatever the last entity at this number had
    // was cleared when it was despawned.
    fn spawn(&mut self) -> Entity {
        let index = if self.free_count > 0 {
            self.free_count -= 1;
            self.free[self.free_count as usize]
        } else {
            assert!(self.count < 8, "the world is full");
            self.count += 1;
            self.count - 1
        };
        self.alive[index as usize] = true;
        Entity { index }
    }

    // The entity's components are dropped by lowering their flags, and its
    // number goes on the free list. The values stay in the arrays, unread.
    fn despawn(&mut self, e: Entity) {
        let i = e.index as usize;
        self.alive[i] = false;
        self.has_position[i] = false;
        self.has_velocity[i] = false;
        self.has_health[i] = false;
        self.free[self.free_count as usize] = e.index;
        self.free_count += 1;
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
}

// Despawned entities have no flags left, so the system skips them without
// knowing anything about despawning.
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
        if !w.alive[i] {
            println!("    entity {i}  (free)");
            continue;
        }
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

    // The turret remembers what it is aiming at, by number.
    let target = drone_a;
    println!("the turret targets entity {}, a drone", target.index);
    print_world(&w);

    println!("entity {} is destroyed and despawned", drone_a.index);
    w.despawn(drone_a);
    print_world(&w);

    // The turret fires. The bullet takes the first free number.
    let bullet = w.spawn();
    w.set_position(bullet, Vec2 { x: 0.0, y: 0.0 });
    w.set_velocity(bullet, Vec2 { x: 4.0, y: 4.0 });
    println!("the turret fires: the bullet is entity {}", bullet.index);
    movement_system(&mut w, 0.5);
    print_world(&w);

    // Nothing told the turret its target died. Its number now names the bullet.
    let t = target.index as usize;
    println!("the turret aims at entity {}, at ({:.2}, {:.2}), moving ({:.2}, {:.2})", target.index,
        w.positions[t].x, w.positions[t].y, w.velocities[t].x, w.velocities[t].y);
    println!("    that is the turret's own bullet, not the drone: the number was reused");
}
