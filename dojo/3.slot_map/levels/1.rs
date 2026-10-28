// Level 1: one function. Everything here works except remove(). Write its body.
//
// Start it with: python tools/dojo.py start slot_map 1

use crate::Handle;
use std::num::NonZeroU32;

// One slot per index: the life it is in now (or next, if it is free), and the
// row its value is in while it is alive.
#[derive(Clone, Copy)]
struct Slot {
    generation: NonZeroU32,
    row: u32,
}

pub struct SlotMap<T> {
    slots: Vec<Slot>,
    free: Vec<u32>,
    // Packed: values[row] belongs to handles[row].
    values: Vec<T>,
    handles: Vec<Handle>,
}

impl<T> SlotMap<T> {
    pub fn new() -> SlotMap<T> {
        SlotMap { slots: Vec::new(), free: Vec::new(), values: Vec::new(), handles: Vec::new() }
    }

    // The value goes in a new row at the end; its handle reuses a freed index
    // before making a new one.
    pub fn insert(&mut self, value: T) -> Handle {
        let index = match self.free.pop() {
            Some(index) => index,
            None => {
                self.slots.push(Slot { generation: NonZeroU32::MIN, row: 0 });
                self.slots.len() as u32 - 1
            }
        };
        let slot = &mut self.slots[index as usize];
        slot.row = self.values.len() as u32;
        let h = Handle { index, generation: slot.generation };
        self.values.push(value);
        self.handles.push(h);
        h
    }

    // Free the index (dojo 1), then swap-remove the row (dojo 2).
    pub fn remove(&mut self, h: Handle) -> Option<T> {
        todo!("level 1: free h's index and take its value out, keeping the rows packed")
    }

    pub fn contains(&self, h: Handle) -> bool {
        self.slots.get(h.index as usize).is_some_and(|s| s.generation == h.generation)
    }

    // Handle, slot, row, value.
    pub fn get(&self, h: Handle) -> Option<&T> {
        self.contains(h).then(|| &self.values[self.slots[h.index as usize].row as usize])
    }

    pub fn get_mut(&mut self, h: Handle) -> Option<&mut T> {
        if !self.contains(h) {
            return None;
        }
        let row = self.slots[h.index as usize].row as usize;
        Some(&mut self.values[row])
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    // The values, packed, in row order: what a system loops over.
    pub fn values(&self) -> &[T] {
        &self.values
    }

    // handles()[row] is the handle that owns values()[row].
    pub fn handles(&self) -> &[Handle] {
        &self.handles
    }
}
