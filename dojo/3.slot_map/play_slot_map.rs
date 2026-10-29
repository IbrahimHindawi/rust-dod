// Dojo 3, the playground: a slot map.
//
// Dojos 1 and 2, put together. Values live packed in rows, so a loop over them
// has no holes (dojo 2). The rest of the program keeps handles, which survive
// rows moving and say when their value is gone (dojo 1). In between is a slot
// per index:
//
//   handle 1:2  ->  slots[1] { generation 2, row 0 }  ->  values[0]
//
// A lookup is two steps: the handle's generation must be the slot's, and the
// slot says the row. A remove is both dojos' work: bump the generation and
// free the index, then swap-remove the row and fix the slot of whoever moved.
// This is the ECS course's entity index (1.4), with one value per entity, and
// what game engines use for textures, meshes and sounds.
//
// This program inserts three values, removes one, reuses its index, and
// draws the slots and the rows after each step. To see it in a debugger,
// break on `remove` and watch `slots`, `handles` and `values`.
//
// The switch: set CHECK_GENERATION to false and run it again. A removed
// handle still finds a slot, and reads whatever value is in that slot's row
// now. It's a static, read each time it's used, so a debugger can flip it
// mid-run too: stop on `remove`, set it to false in the watch window, and
// continue.

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};

static CHECK_GENERATION: AtomicBool = AtomicBool::new(true);

#[derive(Clone, Copy, PartialEq, Eq)]
struct Handle {
    index: u32,
    generation: u32,
}

impl fmt::Display for Handle {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:{}", self.index, self.generation)
    }
}

#[derive(Clone, Copy)]
struct Slot {
    generation: u32,
    row: u32,
}

struct SlotMap {
    slots: Vec<Slot>,
    free: Vec<u32>,
    values: Vec<&'static str>,
    handles: Vec<Handle>,
}

impl SlotMap {
    fn insert(&mut self, value: &'static str) -> Handle {
        let index = match self.free.pop() {
            Some(index) => index,
            None => {
                self.slots.push(Slot { generation: 1, row: 0 });
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

    fn contains(&self, h: Handle) -> bool {
        match self.slots.get(h.index as usize) {
            Some(slot) => !CHECK_GENERATION.load(Relaxed) || slot.generation == h.generation,
            None => false,
        }
    }

    fn get(&self, h: Handle) -> Option<&'static str> {
        if !self.contains(h) {
            return None;
        }
        self.values.get(self.slots[h.index as usize].row as usize).copied()
    }

    fn remove(&mut self, h: Handle) -> Option<&'static str> {
        if !self.contains(h) {
            return None;
        }
        let slot = &mut self.slots[h.index as usize];
        let row = slot.row as usize;
        slot.generation += 1;
        self.free.push(h.index);
        let value = self.values.swap_remove(row);
        self.handles.swap_remove(row);
        if let Some(moved) = self.handles.get(row) {
            self.slots[moved.index as usize].row = row as u32;
        }
        Some(value)
    }

    fn draw(&self) {
        let slots: Vec<String> = self
            .slots
            .iter()
            .enumerate()
            .map(|(i, s)| {
                if self.free.contains(&(i as u32)) {
                    format!("{i}: generation {}, free", s.generation)
                } else {
                    format!("{i}: generation {}, row {}", s.generation, s.row)
                }
            })
            .collect();
        println!("    slots    {}", slots.join(" | "));
        let rows: Vec<String> = self.handles.iter().zip(&self.values).map(|(h, v)| format!("{h} {v}")).collect();
        println!("    rows     {}", rows.join(" | "));
    }
}

fn read(m: &SlotMap, h: Handle) -> String {
    m.get(h).map_or("nothing: it is gone".to_string(), |v| v.to_string())
}

fn main() {
    let mut m = SlotMap { slots: Vec::new(), free: Vec::new(), values: Vec::new(), handles: Vec::new() };
    println!("insert three textures");
    let grass = m.insert("grass");
    let stone = m.insert("stone");
    let water = m.insert("water");
    println!("    grass {grass}  stone {stone}  water {water}");
    m.draw();

    println!("remove stone: free index 1 in its next life, and water moves into row 1");
    m.remove(stone);
    m.draw();
    println!("    water {water}: {}", read(&m, water));
    println!("    stone {stone}: {}", read(&m, stone));

    println!("insert lava: index 1 again, generation 2, in a new row at the end");
    let lava = m.insert("lava");
    println!("    lava {lava}");
    m.draw();
    println!("    stone {stone}: {}", read(&m, stone));
    println!("    lava {lava}: {}", read(&m, lava));

    println!("remove grass: lava, in the last row, moves into row 0");
    m.remove(grass);
    m.draw();
    println!("    lava {lava}: {}  water {water}: {}  grass {grass}: {}", read(&m, lava), read(&m, water), read(&m, grass));
}
