// Dojo 11, the playground: a reflection walk.
//
// A reflect record lists a struct's fields: each one's name, where its bytes
// start in the struct, how many there are, and its type, with the type's own
// record if it's a struct too. With records, one piece of code can list,
// read, write and compare the fields of any type it's handed, by name, the way
// an inspector, an editor, a save file or a network diff does. It never names
// the type: it walks the bytes by the record.
//
// rin's compiler writes these records. Here they are written out by hand, with
// offset_of! for the offsets, as 2.7's macro writes them. The lamp is kept as
// bytes, as an ECS column keeps it, and verify() runs after every step: every
// f32 the walk reads by path must be what the lamp holds, read where the
// compiler put it. Put a breakpoint inside its failure branch to stop where
// they differ.
//
// The switch: set USE_RECORD_OFFSETS to false and run it again. The walk adds
// up the sizes of the fields before each one instead, as if structs had no
// padding. That's right for `x` and `y`, and wrong from `power` on: it starts
// at 4, after 3 bytes of padding, not at 1. It's a static, read on every step,
// so a debugger can flip it mid-run too: stop on the first `set_f32` in main,
// set it to false in the watch window, and continue.

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

use std::mem::offset_of;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};

static USE_RECORD_OFFSETS: AtomicBool = AtomicBool::new(true);

#[repr(C)]
struct Vec2 {
    x: f32,
    y: f32,
}

#[repr(C)]
struct Lamp {
    on: u8,
    power: f32,
    at: Vec2,
    flicker: f32,
}

struct Info {
    size: usize,
    fields: &'static [Field],
}

struct Field {
    name: &'static str,
    offset: usize,
    size: usize,
    ty: &'static str,
    info: Option<&'static Info>,
    hidden: bool,
}

static VEC2: Info = Info {
    size: size_of::<Vec2>(),
    fields: &[
        Field { name: "x", offset: offset_of!(Vec2, x), size: 4, ty: "f32", info: None, hidden: false },
        Field { name: "y", offset: offset_of!(Vec2, y), size: 4, ty: "f32", info: None, hidden: false },
    ],
};

static LAMP: Info = Info {
    size: size_of::<Lamp>(),
    fields: &[
        Field { name: "on", offset: offset_of!(Lamp, on), size: 1, ty: "u8", info: None, hidden: false },
        Field { name: "power", offset: offset_of!(Lamp, power), size: 4, ty: "f32", info: None, hidden: false },
        Field { name: "at", offset: offset_of!(Lamp, at), size: 8, ty: "Vec2", info: Some(&VEC2), hidden: false },
        Field { name: "flicker", offset: offset_of!(Lamp, flicker), size: 4, ty: "f32", info: None, hidden: true },
    ],
};

// Where field i starts in its struct: what the record says or, with the
// switch off, the sizes of the fields before it added up.
fn offset_in(info: &Info, i: usize) -> usize {
    if USE_RECORD_OFFSETS.load(Relaxed) { info.fields[i].offset } else { info.fields[..i].iter().map(|f| f.size).sum() }
}

// Every field a tool shows, depth first: its path, where its bytes start, and
// its record. Struct fields are walked into, and hidden ones left out.
fn leaves(info: &'static Info, prefix: &str, base: usize, out: &mut Vec<(String, usize, &'static Field)>) {
    for (i, f) in info.fields.iter().enumerate().filter(|(_, f)| !f.hidden) {
        let path = format!("{prefix}{}", f.name);
        match f.info {
            Some(inner) => leaves(inner, &format!("{path}."), base + offset_in(info, i), out),
            None => out.push((path, base + offset_in(info, i), f)),
        }
    }
}

// The field a path names, and where its bytes start.
fn find(info: &'static Info, path: &str) -> Option<(usize, &'static Field)> {
    let (mut info, mut at, mut found) = (Some(info), 0, None);
    for name in path.split('.') {
        let fields = info?.fields;
        let i = fields.iter().position(|f| f.name == name)?;
        at += offset_in(info?, i);
        info = fields[i].info;
        found = Some(&fields[i]);
    }
    found.map(|f| (at, f))
}

fn read_f32(row: &[u8], at: usize) -> f32 {
    f32::from_ne_bytes(row[at..at + 4].try_into().expect("4 bytes"))
}

fn get_f32(info: &'static Info, row: &[u8], path: &str) -> Option<f32> {
    find(info, path).filter(|(_, f)| f.ty == "f32").map(|(at, _)| read_f32(row, at))
}

fn set_f32(info: &'static Info, row: &mut [u8], path: &str, value: f32) -> bool {
    let Some((at, _)) = find(info, path).filter(|(_, f)| f.ty == "f32") else { return false };
    row[at..at + 4].copy_from_slice(&value.to_ne_bytes());
    true
}

fn diff(info: &'static Info, a: &[u8], b: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    leaves(info, "", 0, &mut out);
    out.into_iter().filter(|(_, at, f)| a[*at..at + f.size] != b[*at..at + f.size]).map(|(path, _, _)| path).collect()
}

// The lamp's bytes, written where the compiler puts each field, padding zero.
fn row_of(l: &Lamp) -> Vec<u8> {
    let mut row = vec![0; size_of::<Lamp>()];
    row[offset_of!(Lamp, on)] = l.on;
    for (at, v) in [(offset_of!(Lamp, power), l.power), (offset_of!(Lamp, at), l.at.x), (offset_of!(Lamp, at) + 4, l.at.y), (offset_of!(Lamp, flicker), l.flicker)] {
        row[at..at + 4].copy_from_slice(&v.to_ne_bytes());
    }
    row
}

// Every f32 of the lamp read by path, against the same field read where the
// compiler put it. The first that differs is printed, and how many do.
fn verify(row: &[u8]) {
    let compiled = [("power", offset_of!(Lamp, power)), ("at.x", offset_of!(Lamp, at)), ("at.y", offset_of!(Lamp, at) + 4), ("flicker", offset_of!(Lamp, flicker))];
    let wrong: Vec<(&str, Option<f32>, f32)> = compiled
        .iter()
        .map(|&(path, at)| (path, get_f32(&LAMP, row, path), read_f32(row, at)))
        .filter(|&(_, walked, held)| walked != Some(held))
        .collect();
    if let Some(&(path, walked, held)) = wrong.first() {
        let walked = walked.map_or("nothing".to_string(), |v| format!("{v:.2}"));
        println!("verify: the walk reads {path} as {walked}, the lamp holds {held:.2}; {} of 4 f32s are wrong", wrong.len());
    }
}

fn show(row: &[u8]) {
    let mut out = Vec::new();
    leaves(&LAMP, "", 0, &mut out);
    for (path, at, f) in out {
        match f.ty {
            "f32" => println!("    {path}: {:.2}", read_f32(row, at)),
            _ => println!("    {path}: {}", row[at]),
        }
    }
}

fn set(row: &mut [u8], path: &str, value: f32) {
    println!("set {path} to {value:.2}: {}", set_f32(&LAMP, row, path, value));
    verify(row);
}

fn main() {
    println!("a lamp's record: {} bytes", LAMP.size);
    for f in LAMP.fields {
        let kind = if f.info.is_some() { "a struct" } else { f.ty };
        println!("    {}: offset {}, size {}, {kind}{}", f.name, f.offset, f.size, if f.hidden { ", hidden" } else { "" });
    }
    let lamp = Lamp { on: 1, power: 60.0, at: Vec2 { x: 3.0, y: 4.0 }, flicker: 0.25 };
    let mut row = row_of(&lamp);
    println!("the lamp, walked by its record:");
    show(&row);
    verify(&row);
    let before = row.clone();
    set(&mut row, "power", 75.0);
    set(&mut row, "at.x", 5.0);
    println!("diff with the lamp before: {}", diff(&LAMP, &before, &row).join(" "));
    set(&mut row, "flicker", 0.5);
    println!("diff with the lamp before: {}", diff(&LAMP, &before, &row).join(" "));
    set(&mut row, "on", 0.0);
    set(&mut row, "at", 1.0);
    set(&mut row, "at.z", 1.0);
    println!("the lamp, walked by its record:");
    show(&row);
}
