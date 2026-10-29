// The reference: a reflection walk. A record says where each field's bytes
// are; these functions list fields, read and write them by path, and compare
// two values, for any type with a record, without naming one.

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

#[path = "../dojo.rs"]
mod dojo;
// The harness: main hands it the program, and it checks the code below.
mod harness;

use harness::{Field, Info};

// Every field a tool shows, depth first in the record's order: its path, and
// where its bytes are in the outermost value. A struct field is walked into
// rather than listed, and its offset is added to everything inside it. A
// hidden field is left out, with everything inside it.
fn leaves(info: &Info, prefix: &str, base: usize, out: &mut Vec<(String, usize, usize)>) {
    for f in info.fields.iter().filter(|f| !f.hidden) {
        let path = format!("{prefix}{}", f.name);
        match f.info {
            Some(inner) => leaves(inner, &format!("{path}."), base + f.offset, out),
            None => out.push((path, base + f.offset, f.size)),
        }
    }
}

// The paths of the fields a tool shows: "at.x", "at.y".
pub fn paths(info: &Info) -> Vec<String> {
    let mut out = Vec::new();
    leaves(info, "", 0, &mut out);
    out.into_iter().map(|(path, _, _)| path).collect()
}

// The paths, of those, whose bytes differ between two values of the type.
pub fn diff(info: &Info, a: &[u8], b: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    leaves(info, "", 0, &mut out);
    out.into_iter().filter(|(_, at, size)| a[*at..at + size] != b[*at..at + size]).map(|(path, _, _)| path).collect()
}

// The field a path names, hidden or not, and where its bytes start in the
// outermost value: each name is looked up in the record of the field before
// it, and the offsets add up.
fn find(info: &Info, path: &str) -> Option<(usize, &'static Field)> {
    let mut fields = info.fields;
    let mut at = 0;
    let mut found = None;
    for name in path.split('.') {
        let f = fields.iter().find(|f| f.name == name)?;
        at += f.offset;
        fields = f.info.map_or(&[], |inner| inner.fields);
        found = Some(f);
    }
    found.map(|f| (at, f))
}

// The f32 a path names, or None if it names something else, or nothing.
pub fn get_f32(info: &Info, bytes: &[u8], path: &str) -> Option<f32> {
    match find(info, path)? {
        (at, f) if f.ty == "f32" => Some(f32::from_ne_bytes(bytes[at..at + 4].try_into().expect("4 bytes"))),
        _ => None,
    }
}

// Write the f32 a path names. False, and nothing written, if it names
// something else, or nothing.
pub fn set_f32(info: &Info, bytes: &mut [u8], path: &str, value: f32) -> bool {
    match find(info, path) {
        Some((at, f)) if f.ty == "f32" => {
            bytes[at..at + 4].copy_from_slice(&value.to_ne_bytes());
            true
        }
        _ => false,
    }
}

fn main() {
    harness::run();
}
