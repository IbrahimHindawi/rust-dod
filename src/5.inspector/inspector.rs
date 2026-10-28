// 5. The inspector
//
// Type-erased columns are fast and opaque: to a debugger a column is bytes.
// But every component already has a reflect record, registered in lesson 4 so
// the world knew its size. The same record lists the component's fields with
// their names, offsets and types, and links each field to the record of its
// own type. That is enough to print any entity, field by field, without
// writing a print function for any component.
//
// Rust has no reflection built in. The records come from the macro that
// declares each component: it asks the compiler for every field's offset and
// type, and writes them down as constants. They cost nothing until a tool
// reads them.
//
// This is where looking things up by name belongs. kayan looked up columns by
// name in every query, once per archetype, to find what to move. Here systems
// still find columns by id, as in lesson 3, and names are only used by tools:
// this inspector, and the editor-style write at the end that sets a field from
// its component and field names.
//
// A field written `#[hidden] name: T` is recorded as hidden, and the inspector
// skips it. That is how a component keeps bookkeeping out of a tool without
// the tool knowing that component exists.

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

#[path = "../../includes/kayan.rs"]
mod kayan;

use kayan::{Entity, Field, Info, Kind, World, bit, component, mask_of, pod, pod_enum};

enum ComponentId {
    Position,
    Health,
    Team,
    Weapon,
    Label,
}

pod!(Vec2 { x: f32, y: f32 });
pod_enum!(Side { RED = 0, BLUE = 1 });
component!(Position { at: Vec2, heading: f32 });
component!(Health { points: f32, max: f32, #[hidden] regen_timer: f32 });
component!(Team { side: Side });
component!(Weapon { damage: f32, cooldown: f32, ammo: u32 });
component!(Label { text: [u8; 16] });

fn number<const N: usize>(at: &[u8]) -> [u8; N] {
    at.try_into().expect("a field's bytes are its type's size")
}

// One value, from its bytes and its type's record: numbers by type, an enum by
// member name, text up to its first zero, and a struct field by field.
// from_ne_bytes reads a number from bytes, so this needs no unsafe.
fn print_value(info: &Info, at: &[u8], depth: usize) {
    match info.kind {
        Kind::F32 => println!(" {:.2}", f32::from_ne_bytes(number(at))),
        Kind::U32 => println!(" {}", u32::from_ne_bytes(number(at))),
        Kind::I32 => println!(" {}", i32::from_ne_bytes(number(at))),
        Kind::Text => {
            let end = at.iter().position(|&b| b == 0).unwrap_or(at.len());
            println!(" \"{}\"", String::from_utf8_lossy(&at[..end]));
        }
        Kind::Enum(members) => {
            let value = i32::from_ne_bytes(number(at));
            let name = members.iter().find(|m| m.1 == value).map_or("(not a member)", |m| m.0);
            println!(" {name}");
        }
        Kind::Struct(fields) => {
            println!();
            print_fields(fields, at, depth + 1);
        }
    }
}

// Every field of a struct whose bytes are `base`. Each field's bytes are cut
// from `base` by its offset and size, so a record that disagreed with the data
// would stop the program instead of printing garbage.
fn print_fields(fields: &[Field], base: &[u8], depth: usize) {
    for f in fields.iter().filter(|f| !f.hidden) {
        let at = &base[f.offset..f.offset + f.info.layout.size()];
        print!("{:indent$}{}:", "", f.name, indent = depth * 4);
        print_value(f.info, at, depth);
    }
}

fn inspect(w: &World, name: &str, e: Entity) {
    println!("{name} ({e})");
    let signature = w.signature(e);
    for id in (0..w.component_count).filter(|&id| signature & bit(id) != 0) {
        let info = w.info(id);
        println!("    {}", info.name);
        if let Kind::Struct(fields) = info.kind {
            print_fields(fields, w.get_bytes(e, id).expect("e has it"), 2);
        }
    }
}

// What each table holds: per component, its size, and its column's bytes in
// use out of the bytes allocated for the table's capacity.
fn print_layout(w: &World) {
    for (ti, t) in w.tables.iter().enumerate() {
        println!("table {ti} {}, rows in use: {} of {}", w.names(t.signature), t.count(), t.capacity);
        for c in t.columns.iter().flatten() {
            let (size, align) = (c.info.layout.size(), c.info.layout.align());
            println!("    {:<8} {size:2} bytes, align {align}, column {} of {} bytes",
                c.info.name, size * t.count(), size * t.capacity);
        }
    }
}

// The editor's write: set an f32 field by component and field name. Slow and
// flexible, which is right for a tool and wrong for a system. Each `?` gives
// up with None: no such component, no such f32 field, or e lacks it.
fn set_f32_by_name(w: &mut World, e: Entity, component: &str, field: &str, value: f32) -> Option<()> {
    let id = (0..w.component_count).find(|&id| w.info(id).name == component)?;
    let Kind::Struct(fields) = w.info(id).kind else {
        return None;
    };
    let f = fields.iter().find(|f| f.name == field && matches!(f.info.kind, Kind::F32))?;
    w.get_bytes_mut(e, id)?[f.offset..f.offset + 4].copy_from_slice(&value.to_ne_bytes());
    Some(())
}

fn written(r: Option<()>) -> &'static str {
    if r.is_some() { "written" } else { "refused" }
}

// Up to 15 bytes, the rest of the array zeroed, so a shorter label does not
// keep the tail of a longer one.
fn set_label(w: &mut World, e: Entity, text: &str) {
    if let Some(l) = w.get_mut::<Label>(e) {
        let n = text.len().min(15);
        *l = Label::default();
        l.text[..n].copy_from_slice(&text.as_bytes()[..n]);
    }
}

fn main() {
    let mut w = World::new();
    w.register::<Position>();
    w.register::<Health>();
    w.register::<Team>();
    w.register::<Weapon>();
    w.register::<Label>();

    let soldier = w.spawn(mask_of::<Position>() | mask_of::<Health>()
        | mask_of::<Team>() | mask_of::<Weapon>() | mask_of::<Label>());
    *w.get_mut::<Position>(soldier).expect("just spawned") = Position { at: Vec2 { x: 3.0, y: 4.0 }, heading: 90.0 };
    *w.get_mut::<Health>(soldier).expect("just spawned") = Health { points: 80.0, max: 100.0, regen_timer: 2.5 };
    *w.get_mut::<Team>(soldier).expect("just spawned") = Team { side: Side::BLUE };
    *w.get_mut::<Weapon>(soldier).expect("just spawned") = Weapon { damage: 12.5, cooldown: 0.25, ammo: 30 };
    set_label(&mut w, soldier, "soldier");

    let crate_ = w.spawn(mask_of::<Position>() | mask_of::<Label>());
    *w.get_mut::<Position>(crate_).expect("just spawned") = Position { at: Vec2 { x: -1.0, y: 0.5 }, heading: 0.0 };
    set_label(&mut w, crate_, "crate");

    inspect(&w, "soldier", soldier);
    inspect(&w, "crate", crate_);

    println!();
    println!("the editor writes three fields by name");
    println!("    soldier Health.points = 55: {}",
        written(set_f32_by_name(&mut w, soldier, "Health", "points", 55.0)));
    println!("    soldier Weapon.ammo = 1: {}, ammo is a u32, not an f32",
        written(set_f32_by_name(&mut w, soldier, "Weapon", "ammo", 1.0)));
    println!("    crate Health.points = 1: {}, the crate has no health",
        written(set_f32_by_name(&mut w, crate_, "Health", "points", 1.0)));
    inspect(&w, "soldier", soldier);

    println!();
    print_layout(&w);
}
