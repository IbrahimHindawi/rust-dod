// The harness for dojo 11, a reflection walk. It is fixed. Your work is
// kata.rs, beside it: kata.rs brings this file in as a module and its main
// calls run(), so the two are one program. The reference, solution.rs, and
// each level file do the same, so each of them builds on its own, and this
// file never does. Build and run yours with
//
//   python tools/dojo.py reflect
//
// It gives the kata what it walks: reflect records, Info and Field, made the
// way the course's 2.7 makes them, and four test types with records. Lamp has
// padding after `on`, and Room has a Lamp, and a hidden Vec2. The kata
// reads values as bytes, laid out as the compiler lays them out, and finds
// its way through them by the records alone.
//
// First a short scenario, the playground's steps, each printed as it passes.
// Then a fuzz run: thousands of random reads, writes and diffs of a Room from
// a seed, by path, each checked against the same fields reached directly, the
// compiler doing the offsets.

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

use crate::dojo::{Rng, Run};
use crate::{diff, get_f32, paths, set_f32};
use std::alloc::Layout;
use std::mem::offset_of;

// A reflect record: a type's name and layout, and its fields in the order the
// struct declares them.
pub struct Info {
    pub name: &'static str,
    pub layout: Layout,
    pub fields: &'static [Field],
}

// One field: where its bytes are in its struct (`offset`, `size`), its type's
// name, the type's own record when it's a struct (None for a builtin: f32 or
// u8 here), and whether a tool should leave it out.
pub struct Field {
    pub name: &'static str,
    pub offset: usize,
    pub size: usize,
    pub ty: &'static str,
    pub info: Option<&'static Info>,
    pub hidden: bool,
}

// Every type a test type is made of has a record, or None for a builtin.
trait Reflect {
    const RECORD: Option<&'static Info>;
}

// How the harness turns a value into the bytes the kata reads, and back: field
// by field, at the compiler's offsets, with the padding zero. No pointer
// casts, so no unsafe.
trait Bytes: Sized {
    fn put(&self, out: &mut [u8]);
    fn take(at: &[u8]) -> Self;
}

impl Reflect for f32 {
    const RECORD: Option<&'static Info> = None;
}

impl Bytes for f32 {
    fn put(&self, out: &mut [u8]) {
        out[..4].copy_from_slice(&self.to_ne_bytes());
    }
    fn take(at: &[u8]) -> f32 {
        f32::from_ne_bytes(at[..4].try_into().expect("4 bytes"))
    }
}

impl Reflect for u8 {
    const RECORD: Option<&'static Info> = None;
}

impl Bytes for u8 {
    fn put(&self, out: &mut [u8]) {
        out[0] = *self;
    }
    fn take(at: &[u8]) -> u8 {
        at[0]
    }
}

macro_rules! is_hidden {
    () => {
        false
    };
    (hidden) => {
        true
    };
}

// A test type, its record, and its bytes. As in 2.7, a field written
// `#[hidden] draft: Vec2` is recorded as hidden.
macro_rules! record {
    ($name:ident { $($(#[$attr:ident])? $field:ident: $ty:ty),* $(,)? }) => {
        #[derive(Clone, Copy, Default, PartialEq)]
        #[repr(C)]
        struct $name {
            $($field: $ty),*
        }

        impl Reflect for $name {
            const RECORD: Option<&'static Info> = Some(&Info {
                name: stringify!($name),
                layout: Layout::new::<$name>(),
                fields: &[$(Field {
                    name: stringify!($field),
                    offset: offset_of!($name, $field),
                    size: size_of::<$ty>(),
                    ty: stringify!($ty),
                    info: <$ty as Reflect>::RECORD,
                    hidden: is_hidden!($($attr)?),
                }),*],
            });
        }

        impl Bytes for $name {
            fn put(&self, out: &mut [u8]) {
                $(self.$field.put(&mut out[offset_of!($name, $field)..]);)*
            }
            fn take(at: &[u8]) -> $name {
                $name { $($field: <$ty>::take(&at[offset_of!($name, $field)..])),* }
            }
        }
    };
}

record!(Vec2 { x: f32, y: f32 });
record!(Rgb { r: u8, g: u8, b: u8 });
record!(Lamp { on: u8, power: f32, at: Vec2, color: Rgb, #[hidden] flicker: f32, range: f32 });
record!(Room { door: u8, light: Lamp, corner: Vec2, #[hidden] draft: Vec2 });

fn info<T: Reflect>() -> &'static Info {
    T::RECORD.expect("a test type has a record")
}

fn bytes<T: Bytes>(value: &T, info: &Info) -> Vec<u8> {
    let mut out = vec![0; info.layout.size()];
    value.put(&mut out);
    out
}

fn f32s(value: Option<f32>) -> String {
    value.map_or("None".to_string(), |v| format!("{v:.2}"))
}

fn scenario(run: &Run) {
    println!("scenario");
    let lamp_info = info::<Lamp>();
    let lamp = Lamp {
        on: 1,
        power: 60.0,
        at: Vec2 { x: 3.0, y: 4.0 },
        color: Rgb { r: 255, g: 200, b: 120 },
        flicker: 0.25,
        range: 10.0,
    };
    let mut row = bytes(&lamp, lamp_info);
    let record = format!("{}, {} bytes, {} fields", lamp_info.name, lamp_info.layout.size(), lamp_info.fields.len());
    run.same("Lamp's record", record, "Lamp, 28 bytes, 6 fields");
    run.same("paths(Vec2)", paths(info::<Vec2>()).join(" "), "x y");
    run.same("paths(Lamp): nested fields by path, flicker hidden", paths(lamp_info).join(" "), "on power at.x at.y color.r color.g color.b range");
    run.same(
        "paths(Room): the hidden draft left out whole",
        paths(info::<Room>()).join(" "),
        "door light.on light.power light.at.x light.at.y light.color.r light.color.g light.color.b light.range corner.x corner.y",
    );
    run.same("get_f32 power: past the padding after on", f32s(get_f32(lamp_info, &row, "power")), "60.00");
    run.same("get_f32 at.y: at's offset plus y's", f32s(get_f32(lamp_info, &row, "at.y")), "4.00");
    run.same("get_f32 range", f32s(get_f32(lamp_info, &row, "range")), "10.00");
    run.same("get_f32 flicker: hidden from lists, not from paths", f32s(get_f32(lamp_info, &row, "flicker")), "0.25");
    run.same("get_f32 on: a u8, not an f32", f32s(get_f32(lamp_info, &row, "on")), "None");
    run.same("get_f32 at: a struct", f32s(get_f32(lamp_info, &row, "at")), "None");
    run.same("get_f32 at.z: no such field", f32s(get_f32(lamp_info, &row, "at.z")), "None");
    let before = row.clone();
    run.same("set_f32 power 75", set_f32(lamp_info, &mut row, "power", 75.0), "true");
    run.same("set_f32 at.x 5", set_f32(lamp_info, &mut row, "at.x", 5.0), "true");
    let back = Lamp::take(&row);
    run.same("the lamp, read back: power and at.x", format!("{:.2} {:.2}", back.power, back.at.x), "75.00 5.00");
    run.same("diff with the lamp before", diff(lamp_info, &before, &row).join(" "), "power at.x");
    run.same("set_f32 flicker 0.5", set_f32(lamp_info, &mut row, "flicker", 0.5), "true");
    run.same("diff again: flicker is hidden", diff(lamp_info, &before, &row).join(" "), "power at.x");
    run.same("set_f32 on 0: a u8", set_f32(lamp_info, &mut row, "on", 0.0), "false");
    run.same("set_f32 color: a struct", set_f32(lamp_info, &mut row, "color", 0.0), "false");
    run.same("the lamp's on and color, unchanged", format!("{} {}", Lamp::take(&row).on, Lamp::take(&row).color.r), "1 255");
}

// Every leaf of a Room, in the order its record lists them, and whether a
// tool shows it.
const LEAVES: [(&str, bool); 14] = [
    ("door", true),
    ("light.on", true),
    ("light.power", true),
    ("light.at.x", true),
    ("light.at.y", true),
    ("light.color.r", true),
    ("light.color.g", true),
    ("light.color.b", true),
    ("light.flicker", false),
    ("light.range", true),
    ("corner.x", true),
    ("corner.y", true),
    ("draft.x", false),
    ("draft.y", false),
];

// What the fuzz run asks for by path: every leaf, then paths that name a
// struct, or nothing.
const ODD: [&str; 12] =
    ["light", "light.at", "draft", "corner.x.y", "light.at.z", "light.atx.x", "light.a.x", "lamp.power", "power", "door.x", "", "light."];

// A leaf of a Room reached directly, the compiler doing the offsets: what the
// walk has to agree with.
enum Leaf<'a> {
    F32(&'a mut f32),
    U8(&'a mut u8),
}

fn leaf<'a>(r: &'a mut Room, path: &str) -> Option<Leaf<'a>> {
    Some(match path {
        "door" => Leaf::U8(&mut r.door),
        "light.on" => Leaf::U8(&mut r.light.on),
        "light.power" => Leaf::F32(&mut r.light.power),
        "light.at.x" => Leaf::F32(&mut r.light.at.x),
        "light.at.y" => Leaf::F32(&mut r.light.at.y),
        "light.color.r" => Leaf::U8(&mut r.light.color.r),
        "light.color.g" => Leaf::U8(&mut r.light.color.g),
        "light.color.b" => Leaf::U8(&mut r.light.color.b),
        "light.flicker" => Leaf::F32(&mut r.light.flicker),
        "light.range" => Leaf::F32(&mut r.light.range),
        "corner.x" => Leaf::F32(&mut r.corner.x),
        "corner.y" => Leaf::F32(&mut r.corner.y),
        "draft.x" => Leaf::F32(&mut r.draft.x),
        "draft.y" => Leaf::F32(&mut r.draft.y),
        _ => return None,
    })
}

// A leaf's value, as text, to compare and to report.
fn read(r: &Room, path: &str) -> String {
    match leaf(&mut r.clone(), path) {
        Some(Leaf::F32(v)) => format!("{:.2}", *v),
        Some(Leaf::U8(v)) => v.to_string(),
        None => String::new(),
    }
}

// A quarter between -250 and 250: exact in an f32, and printed the same
// everywhere.
fn random_f32(rng: &mut Rng) -> f32 {
    (rng.below(2001) as i64 - 1000) as f32 / 4.0
}

// Change one leaf to a random value.
fn change(rng: &mut Rng, r: &mut Room, path: &str) {
    match leaf(r, path) {
        Some(Leaf::F32(v)) => *v = random_f32(rng),
        Some(Leaf::U8(v)) => *v = rng.below(256) as u8,
        None => {}
    }
}

fn random_room(rng: &mut Rng) -> Room {
    let mut r = Room::default();
    for (path, _) in LEAVES {
        change(rng, &mut r, path);
    }
    r
}

// The first leaf where the room in `row` and the model differ.
fn first_difference(row: &[u8], model: &Room) -> Option<(&'static str, String, String)> {
    let got = Room::take(row);
    LEAVES.iter().map(|&(path, _)| (path, read(&got, path), read(model, path))).find(|(_, g, w)| g != w)
}

fn fuzz(run: &Run) {
    run.fuzz();
    let room_info = info::<Room>();
    let mut rng = Rng::new(run.seed);
    let mut model = random_room(&mut rng);
    let mut row = bytes(&model, room_info);
    for op in 1..=run.ops {
        let roll = rng.below(100);
        if roll < 10 {
            run.before(op, "a new room");
            model = random_room(&mut rng);
            row = bytes(&model, room_info);
        } else if roll < 40 {
            let path = if rng.below(2) == 0 { LEAVES[rng.below(14) as usize].0 } else { rng.pick(&ODD) };
            run.before(op, format_args!("get_f32(\"{path}\")"));
            let got = get_f32(room_info, &row, path);
            let want = match leaf(&mut model, path) {
                Some(Leaf::F32(v)) => Some(*v),
                _ => None,
            };
            run.check(op, got == want, || format!("get_f32(\"{path}\") gives {}, it should give {}", f32s(got), f32s(want)));
        } else if roll < 70 {
            let path = if rng.below(2) == 0 { LEAVES[rng.below(14) as usize].0 } else { rng.pick(&ODD) };
            let value = random_f32(&mut rng);
            run.before(op, format_args!("set_f32(\"{path}\", {value:.2})"));
            let got = set_f32(room_info, &mut row, path, value);
            let want = match leaf(&mut model, path) {
                Some(Leaf::F32(v)) => {
                    *v = value;
                    true
                }
                _ => false,
            };
            run.check(op, got == want, || format!("set_f32(\"{path}\", {value:.2}) returns {got}, it should return {want}"));
            if let Some((at, g, w)) = first_difference(&row, &model) {
                run.check(op, false, || format!("after set_f32(\"{path}\", {value:.2}), the room's {at} is {g}; it should be {w}"));
            }
        } else {
            let mut other = model;
            let changes = rng.below(4);
            for _ in 0..changes {
                let path = LEAVES[rng.below(14) as usize].0;
                change(&mut rng, &mut other, path);
            }
            let want: Vec<&str> =
                LEAVES.iter().filter(|&&(path, shown)| shown && read(&model, path) != read(&other, path)).map(|&(path, _)| path).collect();
            run.before(op, format_args!("diff, after {changes} changes"));
            let after = bytes(&other, room_info);
            let got = diff(room_info, &row, &after);
            run.check(op, got == want, || format!("diff gives [{}]; the fields a tool shows that changed are [{}]", got.join(" "), want.join(" ")));
            model = other;
            row = after;
        }
    }
    run.pass();
}

// The kata's main calls this.
pub fn run() {
    let run = Run::new("reflect", "11. a reflection walk");
    scenario(&run);
    fuzz(&run);
}
