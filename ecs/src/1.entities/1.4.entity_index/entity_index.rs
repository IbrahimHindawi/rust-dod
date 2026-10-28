// 1.4 The entity index
//
// So far every array has room for every number, and systems walk all of them,
// skipping holes. This chapter packs the data instead. Live entities sit in
// rows 0 to len-1 with no holes, and a system is a straight loop over the
// rows: the stream lesson 1 measured.
//
// Packing means an entity's data no longer sits at its number, and it moves.
// Removing a row moves the last row into the hole (swap-remove), so whatever
// was last changes row. Two things keep track:
//
//   * each row records which entity owns it, `entities[row]`;
//   * each number has a slot recording its generation and its current row.
//
// A handle finds its data through its slot. When a row moves, the moved
// entity's slot is updated. That one line is the invariant everything else in
// this course relies on, and `verify` checks it.
//
// The packed rows are Vecs, and Vec has swap_remove built in. What no Vec can
// do for you is the second half: telling the moved entity's slot where it went.
//
// To keep the chapter about the index, every entity here is the same kind of
// thing: a particle with a position and a velocity. Part 2 brings back
// different kinds, each packed in a table of its own.

use std::fmt;
use std::num::NonZeroU32;

#[derive(Clone, Copy, Default)]
struct Vec2 {
    x: f32,
    y: f32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Entity {
    index: u32,
    generation: NonZeroU32,
}

impl fmt::Display for Entity {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:{}", self.index, self.generation)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Error {
    Dead,
}

const ONE: NonZeroU32 = NonZeroU32::MIN;

// Where a number's entity is: its generation, and the row its data is in.
#[derive(Clone, Copy)]
struct Slot {
    generation: NonZeroU32,
    row: u32,
}

// Two halves. The slots, indexed by the entity's number, which never moves. The
// rows, packed, indexed by row, which moves: each row's owner and its
// components side by side, all the same length.
#[derive(Default)]
struct World {
    slots: Vec<Slot>,
    free: Vec<u32>,

    entities: Vec<Entity>,
    positions: Vec<Vec2>,
    velocities: Vec<Vec2>,
}

impl World {
    fn alive(&self, e: Entity) -> bool {
        self.slots.get(e.index as usize).is_some_and(|s| s.generation == e.generation)
    }

    // A number, as in 1.3, and the next row at the end of the packed rows.
    fn spawn(&mut self, p: Vec2, v: Vec2) -> Entity {
        let index = match self.free.pop() {
            Some(index) => index,
            None => {
                self.slots.push(Slot { generation: ONE, row: 0 });
                self.slots.len() as u32 - 1
            }
        };
        let slot = &mut self.slots[index as usize];
        slot.row = self.entities.len() as u32;
        let e = Entity { index, generation: slot.generation };
        self.entities.push(e);
        self.positions.push(p);
        self.velocities.push(v);
        e
    }

    // Swap-remove: the last row moves into the hole, so the rows stay packed.
    // The entity that owned the last row now lives at `row`, and its slot must
    // say so.
    fn despawn(&mut self, e: Entity) -> Result<(), Error> {
        if !self.alive(e) {
            return Err(Error::Dead);
        }
        let slot = &mut self.slots[e.index as usize];
        let row = slot.row as usize;
        slot.generation = slot.generation.checked_add(1).expect("a number has lived 4 billion lives");
        self.entities.swap_remove(row);
        self.positions.swap_remove(row);
        self.velocities.swap_remove(row);
        if let Some(moved) = self.entities.get(row) {
            self.slots[moved.index as usize].row = row as u32;
        }
        self.free.push(e.index);
        Ok(())
    }

    // Handle, then slot, then row: the data is wherever the row is now.
    fn get_position(&self, e: Entity) -> Option<&Vec2> {
        if !self.alive(e) {
            return None;
        }
        Some(&self.positions[self.slots[e.index as usize].row as usize])
    }

    // The invariant, checked: every row belongs to a live entity whose slot
    // points back at that row, and there are as many rows as live entities.
    // Returns how many checks failed, printing each one.
    fn verify(&self) -> u32 {
        let mut problems = 0;
        for (row, &e) in self.entities.iter().enumerate() {
            if !self.alive(e) {
                println!("verify: row {row} holds dead entity {e}");
                problems += 1;
            } else if self.slots[e.index as usize].row as usize != row {
                println!("verify: {e} is in row {row}, its slot says row {}", self.slots[e.index as usize].row);
                problems += 1;
            }
        }
        let live = self.slots.len() - self.free.len();
        if live != self.entities.len() {
            println!("verify: {} rows, {live} live entities", self.entities.len());
            problems += 1;
        }
        problems
    }
}

// No flags and no holes: every row is a live particle. This is a stream over
// two packed slices.
fn movement_system(positions: &mut [Vec2], velocities: &[Vec2], dt: f32) {
    for (p, v) in positions.iter_mut().zip(velocities) {
        p.x += v.x * dt;
        p.y += v.y * dt;
    }
}

fn print_rows(w: &World) {
    for (row, (e, p)) in w.entities.iter().zip(&w.positions).enumerate() {
        println!("    row {row}  {e}  position ({:5.2}, {:5.2})", p.x, p.y);
    }
    // A slot is in use when the row it points at is owned by its number, in
    // its current generation. A free slot still holds the row it last had.
    print!("    slots:");
    for (i, s) in w.slots.iter().enumerate() {
        match w.entities.get(s.row as usize) {
            Some(owner) if owner.index as usize == i && owner.generation == s.generation => {
                print!("  {i} -> row {}", s.row);
            }
            _ => print!("  {i} free"),
        }
    }
    println!();
}

fn main() {
    let mut w = World::default();
    let p: Vec<Entity> = (0..5)
        .map(|i| w.spawn(Vec2 { x: i as f32, y: 0.0 }, Vec2 { x: 0.0, y: 1.0 }))
        .collect();
    println!("spawned 5 particles");
    print_rows(&w);

    println!("despawn {}, in row 1: the last row moves into the hole", p[1]);
    w.despawn(p[1]).expect("alive");
    print_rows(&w);

    println!("despawn {}, which the last despawn moved: its handle still finds it", p[4]);
    w.despawn(p[4]).expect("alive");
    print_rows(&w);

    let q = w.spawn(Vec2 { x: 9.0, y: 0.0 }, Vec2 { x: 0.0, y: 1.0 });
    println!("spawn {q}: it reuses a number and takes the next row");
    print_rows(&w);

    movement_system(&mut w.positions, &w.velocities, 0.5);
    movement_system(&mut w.positions, &w.velocities, 0.5);
    println!("two steps of movement, a straight loop over {} rows", w.entities.len());
    print_rows(&w);

    let at = w.get_position(p[2]).expect("alive");
    println!("{} is at ({:.2}, {:.2}), wherever its row is", p[2], at.x, at.y);
    println!("{} has a position: {}", p[4],
        if w.get_position(p[4]).is_some() { "yes" } else { "no, it is dead" });
    println!("verify: {} problems", w.verify());
}
