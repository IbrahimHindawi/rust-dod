// Level 1: one function. Everything here works except remove(). Write its body.
//
// Start it with: python tools/dojo.py start swap_remove 1

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

#[path = "../dojo.rs"]
mod dojo;
// The harness: main hands it the program, and it checks the code below.
mod harness;

pub struct SparseSet<T> {
    // Packed: values[row] belongs to ids[row]. No holes, in no order.
    values: Vec<T>,
    ids: Vec<u32>,
    // Sparse: rows[id] is the row id's value is in, or None.
    rows: Vec<Option<u32>>,
}

impl<T> SparseSet<T> {
    pub fn new() -> SparseSet<T> {
        SparseSet { values: Vec::new(), ids: Vec::new(), rows: Vec::new() }
    }

    // Give `id` a value. If it has one already, the new value replaces it and
    // the old one comes back; otherwise it goes in a new row at the end.
    pub fn insert(&mut self, id: u32, value: T) -> Option<T> {
        if let Some(old) = self.get_mut(id) {
            return Some(std::mem::replace(old, value));
        }
        let i = id as usize;
        if i >= self.rows.len() {
            self.rows.resize(i + 1, None);
        }
        self.rows[i] = Some(self.values.len() as u32);
        self.values.push(value);
        self.ids.push(id);
        None
    }

    // Swap-remove: the last row moves into the hole, and the id that owned the
    // last row is told where its value went.
    pub fn remove(&mut self, id: u32) -> Option<T> {
        todo!("level 1: take id's value out, keeping the rows packed and every id's row right")
    }

    pub fn get(&self, id: u32) -> Option<&T> {
        let row = (*self.rows.get(id as usize)?)?;
        Some(&self.values[row as usize])
    }

    pub fn get_mut(&mut self, id: u32) -> Option<&mut T> {
        let row = (*self.rows.get(id as usize)?)?;
        Some(&mut self.values[row as usize])
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    // The values, packed, in row order: what a system loops over.
    pub fn values(&self) -> &[T] {
        &self.values
    }

    // ids()[row] is the id that owns values()[row].
    pub fn ids(&self) -> &[u32] {
        &self.ids
    }
}

fn main() {
    harness::run();
}
