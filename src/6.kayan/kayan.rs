// 6. kayan
//
// Everything so far, in one small game. A turret at the centre shoots at wings
// of drones flying in from the edges. Bullets hurt drones, a badly hurt drone
// catches fire, and a drone at zero health is destroyed in a blast that hurts
// and ignites the drones around it. A drone that reaches the turret is a leak.
//
// The new idea is when structural changes happen. Spawning, despawning, adding
// and removing components all change tables, and systems are in the middle of
// walking tables when they decide to make those changes:
//
//   * a despawn swap-removes: the last row moves into the hole, behind the
//     loop, so that entity is not processed this frame;
//   * a spawn can grow a table, which moves every column of it to new memory;
//   * an add or a remove moves an entity into another table, which the same
//     query may walk later, so the entity is visited twice.
//
// In Rust, a system holding a slice of a table cannot call any of them: the
// slice borrows the world, and each change needs the whole world. That rules
// out the stale slice. It does not rule out the skipped or doubled entity,
// which a loop over row numbers can still produce.
//
// So systems do not make structural changes. They record them in a command
// buffer, and the frame applies the buffer once every system has run. Plain
// writes to components are fine during a query: a hit lowers a drone's health
// in place. Only changes to which table an entity is in wait for the end of
// the frame. Until then no row moves, so within a frame a row number is as
// good as a reference, and the systems below use both.
//
// The buffer is a frame arena made of Vecs. Commands and their payloads are
// pushed during the frame, and the Vecs are cleared after the buffer is
// applied, keeping their memory, so nothing in them is freed one at a time. A
// command against an entity that died earlier in the same buffer is skipped,
// which the handle's generation check makes cheap to detect.

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

#[path = "../../includes/kayan.rs"]
mod kayan;

use kayan::{Component, Dead, Entity, Query, World, bit, bytes_of, component, mask_of, pod};
use std::fmt;

enum ComponentId {
    Position,
    Velocity,
    Health,
    Damage,
    Lifetime,
    Turret,
    Burning,
}

pod!(Vec2 { x: f32, y: f32 });
component!(Position { at: Vec2 });
component!(Velocity { per_second: Vec2 });
component!(Health { points: f32 });
component!(Damage { amount: f32 });
component!(Lifetime { seconds: f32 });
component!(Turret { cooldown: f32, timer: f32 });
component!(Burning { per_second: f32 });

// ---------------------------------------------------------------------------
// The command buffer
// ---------------------------------------------------------------------------

// One recorded change. A spawned entity has no handle until the buffer is
// applied, so the commands for it name it by `pending`: how many spawns were
// recorded before it. A payload is `len` bytes at `start` in the byte store.
// The buffer is complete, though this game never removes a component.
#[allow(dead_code)]
#[derive(Clone, Copy)]
enum Command {
    Spawn { signature: u64 },
    SetSpawned { pending: usize, id: u32, start: usize, len: usize },
    Add { entity: Entity, id: u32, start: usize, len: usize },
    Remove { entity: Entity, id: u32 },
    Despawn(Entity),
}

// The frame's commands, the bytes of their payloads, and, while the buffer is
// applied, the handles its spawns made. All three are cleared after.
#[derive(Default)]
struct Commands {
    list: Vec<Command>,
    bytes: Vec<u8>,
    spawns: usize,
    spawned: Vec<Entity>,
}

#[derive(Default)]
struct Applied {
    applied: u32,
    skipped: u32,
}

impl Commands {
    // A copy of `value` in the byte store, kept until the buffer is applied,
    // as the bytes the component's column will receive.
    fn payload<T: Component>(&mut self, value: &T) -> (usize, usize) {
        let start = self.bytes.len();
        self.bytes.extend_from_slice(bytes_of(value));
        (start, size_of::<T>())
    }

    fn spawn(&mut self, signature: u64) -> usize {
        self.list.push(Command::Spawn { signature });
        self.spawns += 1;
        self.spawns - 1
    }

    // T must be in the pending spawn's signature: the spawn decides the table.
    fn set_spawned<T: Component>(&mut self, pending: usize, value: T) {
        let (start, len) = self.payload(&value);
        self.list.push(Command::SetSpawned { pending, id: T::ID, start, len });
    }

    fn add<T: Component>(&mut self, entity: Entity, value: T) {
        let (start, len) = self.payload(&value);
        self.list.push(Command::Add { entity, id: T::ID, start, len });
    }

    #[allow(dead_code)]
    fn remove<T: Component>(&mut self, entity: Entity) {
        self.list.push(Command::Remove { entity, id: T::ID });
    }

    fn despawn(&mut self, entity: Entity) {
        self.list.push(Command::Despawn(entity));
    }

    // Every command, in the order it was recorded. Then the Vecs are cleared,
    // which lets go of every command and payload at once.
    fn apply(&mut self, w: &mut World) -> Applied {
        let mut result = Applied::default();
        for &cmd in &self.list {
            let done = match cmd {
                Command::Spawn { signature } => {
                    self.spawned.push(w.spawn(signature));
                    Ok(())
                }
                Command::SetSpawned { pending, id, start, len } => {
                    let e = self.spawned[pending];
                    if write_component(w, e, id, &self.bytes[start..start + len]).is_none() {
                        panic!("set_spawned: spawn {pending} has no {} in its signature", w.info(id).name);
                    }
                    Ok(())
                }
                Command::Add { entity, id, start, len } => {
                    w.move_to(entity, w.signature(entity) | bit(id)).map(|()| {
                        write_component(w, entity, id, &self.bytes[start..start + len]).expect("it was just added");
                    })
                }
                Command::Remove { entity, id } => w.move_to(entity, w.signature(entity) & !bit(id)),
                Command::Despawn(entity) => w.despawn(entity),
            };
            match done {
                Ok(()) => result.applied += 1,
                Err(Dead) => result.skipped += 1,
            }
        }
        self.list.clear();
        self.bytes.clear();
        self.spawned.clear();
        self.spawns = 0;
        result
    }
}

// Copy a payload into e's component. None if e is dead or has no such
// component.
fn write_component(w: &mut World, e: Entity, id: u32, payload: &[u8]) -> Option<()> {
    w.get_bytes_mut(e, id)?.copy_from_slice(payload);
    Some(())
}

// ---------------------------------------------------------------------------
// The game
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Stats {
    waves: u32,
    drones: u32,
    destroyed: u32,
    leaked: u32,
    shots: u32,
    hits: u32,
    misses: u32,
}

// Each frame's events are printed under one "frame N" line.
#[derive(Default)]
struct Log {
    frame: i32,
    printed: i32,
}

impl Log {
    fn event(&mut self, what: fmt::Arguments) {
        if self.printed != self.frame {
            println!("frame {}", self.frame);
            self.printed = self.frame;
        }
        println!("    {what}");
    }
}

// The game's state is four separate fields, so a system can walk the world's
// tables and record commands, count and log at the same time.
struct Game {
    world: World,
    commands: Commands,
    stats: Stats,
    log: Log,
}

fn drone_mask() -> u64 {
    mask_of::<Position>() | mask_of::<Velocity>() | mask_of::<Health>()
}

fn bullet_mask() -> u64 {
    mask_of::<Position>() | mask_of::<Velocity>() | mask_of::<Damage>() | mask_of::<Lifetime>()
}

fn distance(a: Vec2, b: Vec2) -> f32 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    (dx * dx + dy * dy).sqrt()
}

fn toward(from: Vec2, to: Vec2, speed: f32) -> Vec2 {
    let d = distance(from, to);
    Vec2 { x: (to.x - from.x) / d * speed, y: (to.y - from.y) / d * speed }
}

const ORIGIN: Vec2 = Vec2 { x: 0.0, y: 0.0 };

// Drones come in wings of three, side by side, from each side in turn.
const ENTRY_POINTS: [Vec2; 4] = [
    Vec2 { x: 20.0, y: 0.0 },
    Vec2 { x: 0.0, y: -20.0 },
    Vec2 { x: -20.0, y: 0.0 },
    Vec2 { x: 0.0, y: 20.0 },
];
const WAVE_EVERY: i32 = 20;
const WAVES: u32 = 5;
const DRONE_SPEED: f32 = 2.0;

fn spawner_system(g: &mut Game) {
    if (g.log.frame - 1) % WAVE_EVERY != 0 || g.stats.waves == WAVES {
        return;
    }
    let centre = ENTRY_POINTS[g.stats.waves as usize % 4];
    let heading = toward(centre, ORIGIN, 1.0);
    for k in -1..=1 {
        // Side by side: 2 units apart, across the direction of flight.
        let side = 2.0 * k as f32;
        let at = Vec2 { x: centre.x - heading.y * side, y: centre.y + heading.x * side };
        let drone = g.commands.spawn(drone_mask());
        g.commands.set_spawned(drone, Position { at });
        g.commands.set_spawned(drone, Velocity { per_second: toward(at, ORIGIN, DRONE_SPEED) });
        g.commands.set_spawned(drone, Health { points: 100.0 });
        g.stats.drones += 1;
    }
    g.stats.waves += 1;
    g.log.event(format_args!("a wing of three drones enters at ({:.1}, {:.1})", centre.x, centre.y));
}

// The nearest drone to `from`, by walking every table a drone can be in.
fn nearest_drone(w: &World, from: Vec2) -> Option<Vec2> {
    let drones = Query { all: drone_mask(), none: 0 };
    let mut best = 1000000.0;
    let mut found = None;
    for t in w.tables.iter().filter(|t| t.matches(drones)) {
        for p in t.column::<Position>() {
            let d = distance(from, p.at);
            if d < best {
                best = d;
                found = Some(p.at);
            }
        }
    }
    found
}

// Each turret is read, then the drones are searched, then the turret is
// written: one borrow after another, never two at once, because the search
// reads every table and the turret's table is one of them.
fn turret_system(g: &mut Game, dt: f32) {
    let turrets = Query { all: mask_of::<Position>() | mask_of::<Turret>(), none: 0 };
    for ti in 0..g.world.tables.len() {
        if !g.world.tables[ti].matches(turrets) {
            continue;
        }
        for i in 0..g.world.tables[ti].count() {
            let t = &g.world.tables[ti];
            let p = t.column::<Position>()[i];
            let Turret { cooldown, timer } = t.column::<Turret>()[i];
            // The timer stops at zero, so an idle turret is ready to fire but
            // does not save up shots.
            let mut timer = (timer - dt).max(0.0);
            let target = if timer > 0.0 { None } else { nearest_drone(&g.world, p.at) };
            if let Some(target) = target {
                timer += cooldown;
                let bullet = g.commands.spawn(bullet_mask());
                g.commands.set_spawned(bullet, p);
                g.commands.set_spawned(bullet, Velocity { per_second: toward(p.at, target, 12.0) });
                g.commands.set_spawned(bullet, Damage { amount: 30.0 });
                g.commands.set_spawned(bullet, Lifetime { seconds: 2.0 });
                g.stats.shots += 1;
            }
            g.world.tables[ti].column_mut::<Turret>()[i].timer = timer;
        }
    }
}

// A system is a query and a kernel. The query finds the tables; the kernel
// runs over one table's columns. Each `&mut` slice is the only way to reach its
// memory, so the compiler can vectorize without checking for overlap.
fn integrate(ps: &mut [Position], vs: &[Velocity], dt: f32) {
    for (p, v) in ps.iter_mut().zip(vs) {
        p.at.x += v.per_second.x * dt;
        p.at.y += v.per_second.y * dt;
    }
}

fn movement_system(w: &mut World, dt: f32) {
    let movers = Query { all: mask_of::<Position>() | mask_of::<Velocity>(), none: 0 };
    for t in w.tables.iter_mut().filter(|t| t.matches(movers)) {
        let (ps, vs) = t.pair_mut::<Position, Velocity>();
        integrate(ps, vs, dt);
    }
}

fn age(ls: &mut [Lifetime], dt: f32) {
    for l in ls {
        l.seconds -= dt;
    }
}

// The kernel writes the lifetimes; the loop after it reads them with the
// handles. Two passes, because a column to write and the handles to read are
// two borrows of one table.
fn lifetime_system(g: &mut Game, dt: f32) {
    let q = Query { all: mask_of::<Lifetime>(), none: 0 };
    for t in g.world.tables.iter_mut().filter(|t| t.matches(q)) {
        age(t.column_mut::<Lifetime>(), dt);
        for (&e, l) in t.entities.iter().zip(t.column::<Lifetime>()) {
            if l.seconds <= 0.0 {
                g.commands.despawn(e);
                g.stats.misses += 1;
            }
        }
    }
}

// The first drone within reach of `at` that still has health loses `damage`.
// Returns its table, its row, and its health before the hit.
fn hit_first_drone(w: &mut World, at: Vec2, damage: f32) -> Option<(usize, usize, f32)> {
    let drones = Query { all: drone_mask(), none: 0 };
    for (ti, t) in w.tables.iter_mut().enumerate().filter(|(_, t)| t.matches(drones)) {
        let (hs, ps) = t.pair_mut::<Health, Position>();
        for (row, (h, p)) in hs.iter_mut().zip(ps).enumerate() {
            if distance(at, p.at) <= 1.0 && h.points > 0.0 {
                let before = h.points;
                h.points -= damage;
                return Some((ti, row, before));
            }
        }
    }
    None
}

// Every bullet against every drone. A hit lowers health in place, which is not
// a structural change; removing the bullet and setting the drone on fire are.
// A bullet that expired this frame is skipped: lifetime_system has already
// recorded its despawn, and it is only still here because nothing structural
// happens until the end of the frame.
fn collision_system(g: &mut Game) {
    let bullets = Query { all: bullet_mask(), none: 0 };
    for bi in 0..g.world.tables.len() {
        if !g.world.tables[bi].matches(bullets) {
            continue;
        }
        for b in 0..g.world.tables[bi].count() {
            let t = &g.world.tables[bi];
            if t.column::<Lifetime>()[b].seconds <= 0.0 {
                continue;
            }
            let (bullet, at, damage) = (t.entities[b], t.column::<Position>()[b].at, t.column::<Damage>()[b].amount);
            let Some((ti, row, before)) = hit_first_drone(&mut g.world, at, damage) else {
                continue;
            };
            let t = &g.world.tables[ti];
            let (drone, points) = (t.entities[row], t.column::<Health>()[row].points);
            g.stats.hits += 1;
            g.commands.despawn(bullet);
            g.log.event(format_args!("bullet {bullet} hits drone {drone}, health {points:.1}"));
            if before >= 50.0 && points < 50.0 && t.signature & mask_of::<Burning>() == 0 {
                g.commands.add(drone, Burning { per_second: 8.0 });
                g.log.event(format_args!("drone {drone} catches fire"));
            }
        }
    }
}

fn burn(hs: &mut [Health], bs: &[Burning], dt: f32) {
    for (h, b) in hs.iter_mut().zip(bs) {
        h.points -= b.per_second * dt;
    }
}

fn burning_system(w: &mut World, dt: f32) {
    let q = Query { all: mask_of::<Health>() | mask_of::<Burning>(), none: 0 };
    for t in w.tables.iter_mut().filter(|t| t.matches(q)) {
        let (hs, bs) = t.pair_mut::<Health, Burning>();
        burn(hs, bs, dt);
    }
}

// A destroyed drone's blast hurts every drone near it and sets it on fire.
// Lowering health is a plain write, so a blast can finish off a neighbour later
// in the same loop, and that neighbour's blast reaches back to the drone that
// started it. That drone's despawn is only recorded, so it is still in its
// table and gets an add of Burning too. By the time the buffer is applied it
// is gone, and the add is skipped.
fn blast_system(g: &mut Game, from: Entity, at: Vec2) {
    let drones = Query { all: drone_mask(), none: 0 };
    for t in g.world.tables.iter_mut().filter(|t| t.matches(drones)) {
        for i in 0..t.count() {
            let e = t.entities[i];
            if e.index == from.index || distance(t.column::<Position>()[i].at, at) >= 4.0 {
                continue;
            }
            let h = &mut t.column_mut::<Health>()[i];
            h.points -= 40.0;
            let points = h.points;
            g.commands.add(e, Burning { per_second: 8.0 });
            g.log.event(format_args!("drone {e} is caught in the blast, health {points:.1}"));
        }
    }
}

// Drones at zero health are destroyed; drones that reach the turret leak. Each
// row is read afresh, by number, because a blast earlier in this loop may have
// lowered its health.
fn outcome_system(g: &mut Game) {
    let drones = Query { all: drone_mask(), none: 0 };
    for ti in 0..g.world.tables.len() {
        if !g.world.tables[ti].matches(drones) {
            continue;
        }
        for i in 0..g.world.tables[ti].count() {
            let t = &g.world.tables[ti];
            let (e, at, points) = (t.entities[i], t.column::<Position>()[i].at, t.column::<Health>()[i].points);
            if points <= 0.0 {
                g.commands.despawn(e);
                g.stats.destroyed += 1;
                g.log.event(format_args!("drone {e} is destroyed"));
                blast_system(g, e, at);
            } else if distance(at, ORIGIN) < 1.5 {
                g.commands.despawn(e);
                g.stats.leaked += 1;
                g.log.event(format_args!("drone {e} reaches the turret with {points:.1} health"));
            }
        }
    }
}

fn main() {
    let mut g = Game { world: World::new(), commands: Commands::default(), stats: Stats::default(), log: Log::default() };
    g.world.register::<Position>();
    g.world.register::<Velocity>();
    g.world.register::<Health>();
    g.world.register::<Damage>();
    g.world.register::<Lifetime>();
    g.world.register::<Turret>();
    g.world.register::<Burning>();

    let tower = g.world.spawn(mask_of::<Position>() | mask_of::<Turret>());
    *g.world.get_mut::<Turret>(tower).expect("just spawned") = Turret { cooldown: 0.6, timer: 0.6 };

    let dt = 0.1;
    let frames = 140;
    let mut skipped = 0;
    for frame in 1..=frames {
        g.log.frame = frame;
        spawner_system(&mut g);
        turret_system(&mut g, dt);
        movement_system(&mut g.world, dt);
        lifetime_system(&mut g, dt);
        collision_system(&mut g);
        burning_system(&mut g.world, dt);
        outcome_system(&mut g);
        skipped += g.commands.apply(&mut g.world).skipped;
    }

    let s = &g.stats;
    println!("after {frames} frames");
    println!("    drones: {} entered, {} destroyed, {} leaked", s.drones, s.destroyed, s.leaked);
    println!("    bullets: {} fired, {} hit, {} expired", s.shots, s.hits, s.misses);
    println!("    commands skipped because their entity was already gone: {skipped}");
    println!("    {} entities alive in {} tables", g.world.live, g.world.tables.len());
    for (ti, t) in g.world.tables.iter().enumerate() {
        println!("    table {ti} {}, rows in use: {}", g.world.names(t.signature), t.count());
    }
}
