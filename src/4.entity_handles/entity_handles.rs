// 4. Entity handles
//
// Lesson 2 ended on a problem: swap-remove keeps a table packed, but it moves
// rows, so "spark 7 is row 7" stops being true the moment something before it
// dies. Anything that remembers an entity by its row (a target, a parent, a UI
// selection) ends up pointing at whoever moved in.
//
// The fix is one level of indirection that does not move:
//
//   * A handle is {index, generation}. The index picks a slot in the world's
//     slot list; the slot says which table and row the entity is in now.
//   * Every table keeps an `entities` column: the handle that owns each row.
//     When a swap-remove moves the last row, that column says whose slot to
//     update. That is the line lesson 2 could not write.
//   * Despawning bumps the slot's generation and puts the index on a free list.
//     The next spawn reuses the index with the new generation, so every old
//     handle to the dead entity fails the generation check instead of quietly
//     finding the newcomer. Generations start at 1, so 0 is never one, and
//     "no entity" is None at no cost.
//
// The same slot is what lets an entity change archetype. Adding a component
// moves the entity to the table with one more bit: a row is appended there,
// the components both tables share are copied, and the old row is
// swap-removed. Removing a component is the same move the other way. kayan had
// no way to do either: its entity id was a running count, and nothing recorded
// where an entity lived.
//
// The world itself is in includes/kayan.rs, which lessons 5 and 6 use as it
// is. This program walks through what it does and prints each step.

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

#[path = "../../includes/kayan.rs"]
mod kayan;

use kayan::{Entity, World, component, mask_of, pod, status};
use std::process::ExitCode;

enum ComponentId {
    Position,
    Velocity,
    Health,
}

pod!(Vec2 { x: f32, y: f32 });
component!(Position { at: Vec2 });
component!(Velocity { per_second: Vec2 });
component!(Health { points: f32 });

fn print_entity(w: &World, name: &str, e: Entity) {
    print!("    {name} = {e}");
    if !w.alive(e) {
        println!(" is dead");
        return;
    }
    let slot = w.slots[e.index as usize];
    print!(" in table {} row {} {}", slot.table, slot.row, w.names(w.signature(e)));
    if let Some(p) = w.get::<Position>(e) {
        print!(" at ({:.1}, {:.1})", p.at.x, p.at.y);
    }
    if let Some(h) = w.get::<Health>(e) {
        print!(" health {:.0}", h.points);
    }
    println!();
}

fn spawn_mover(w: &mut World, x: f32) -> Entity {
    let e = w.spawn(mask_of::<Position>() | mask_of::<Velocity>());
    *w.get_mut::<Position>(e).expect("just spawned") = Position { at: Vec2 { x, y: 0.0 } };
    *w.get_mut::<Velocity>(e).expect("just spawned") = Velocity { per_second: Vec2 { x: 0.0, y: 1.0 } };
    e
}

fn main() -> ExitCode {
    let mut w = World::new();
    w.register::<Position>();
    w.register::<Velocity>();
    w.register::<Health>();

    println!("spawn a, b, c");
    let a = spawn_mover(&mut w, 1.0);
    let b = spawn_mover(&mut w, 2.0);
    let c = spawn_mover(&mut w, 3.0);
    print_entity(&w, "a", a);
    print_entity(&w, "b", b);
    print_entity(&w, "c", c);

    println!("despawn a: c is swapped into a's row, and its slot follows");
    println!("    despawn returned {}", status(w.despawn(a)));
    print_entity(&w, "a", a);
    print_entity(&w, "c", c);

    println!("spawn d: it reuses a's index with the next generation");
    let d = spawn_mover(&mut w, 4.0);
    print_entity(&w, "d", d);
    print_entity(&w, "a", a);
    let found = if w.get::<Position>(a).is_none() { "None" } else { "found (a bug)" };
    println!("    a's position through the old handle is {found}");
    println!("    despawning a again returns {}", status(w.despawn(a)));
    println!("    no entity is None, and an Option<Entity> is {} bytes, an Entity's size", size_of::<Option<Entity>>());

    // The handle is known to be alive, so a failure would be a bug: expect
    // says so, and stops the program if it ever happens.
    println!("give b health: b moves to a new table and keeps its position");
    w.add(b, Health { points: 75.0 }).expect("b is alive");
    print_entity(&w, "b", b);
    print_entity(&w, "d", d);

    println!("take b's velocity: another move");
    w.remove::<Velocity>(b).expect("b is alive");
    print_entity(&w, "b", b);

    println!("the tables, with the handles in each row");
    w.print_tables();

    // Enough spawns to make a table grow past its first 16 rows. Then two
    // checks: every survivor still reads its own position, and verify finds
    // every row's owner and every owner's slot in agreement.
    println!("spawn 40 more");
    let many: Vec<Entity> = (0..40).map(|i| spawn_mover(&mut w, i as f32)).collect();
    for &e in many.iter().step_by(2) {
        w.despawn(e).expect("alive");
    }
    let right = many
        .iter()
        .enumerate()
        .skip(1)
        .step_by(2)
        .filter(|&(i, &e)| w.get::<Position>(e).is_some_and(|p| p.at.x == i as f32))
        .count();
    let problems = w.verify();
    println!("    despawned every other one; {right} of the 20 left read their own position");
    println!("    verify: {problems} problems");
    println!("    {} entities alive, {} tables", w.live, w.tables.len());
    if right == 20 && problems == 0 { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}
