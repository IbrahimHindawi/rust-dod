// Dojo 1, the playground: a free list and generations.
//
// Handles name things that come and go: entities, textures, sounds, network
// connections. Two small ideas make them safe to keep:
//
//   * a free list: freed indices are reused before new ones are made, so the
//     arrays an index points into stay as small as the most things alive;
//   * a generation per index: freeing bumps it, so every handle made for the
//     old life stops matching, and a reused index can't be mistaken for the
//     thing that had it before.
//
// This program runs the allocator step by step and draws its two arrays after
// each step. To see it in a debugger, break on `alloc` and `free` and watch
// `generations` and `free` change.
//
// The switch: set BUMP_ON_FREE to false and run it again. The reused index
// comes back with the old generation, and a dead handle is alive again.

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

use std::fmt;

const BUMP_ON_FREE: bool = true;

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

struct Handles {
    // The life each index is in now, or will be in next if it is free.
    generations: Vec<u32>,
    // Freed indices, a stack: the last one freed is the first one reused.
    free: Vec<u32>,
}

impl Handles {
    fn alloc(&mut self) -> Handle {
        let index = match self.free.pop() {
            Some(index) => index,
            None => {
                self.generations.push(1);
                self.generations.len() as u32 - 1
            }
        };
        Handle { index, generation: self.generations[index as usize] }
    }

    fn free(&mut self, h: Handle) -> bool {
        if !self.alive(h) {
            return false;
        }
        if BUMP_ON_FREE {
            self.generations[h.index as usize] += 1;
        }
        self.free.push(h.index);
        true
    }

    fn alive(&self, h: Handle) -> bool {
        self.generations.get(h.index as usize) == Some(&h.generation)
    }

    // Each index with the generation it holds, then the free list.
    fn draw(&self) {
        let slots: Vec<String> = self.generations.iter().enumerate().map(|(i, g)| format!("{i}:{g}")).collect();
        let free: Vec<String> = self.free.iter().map(|i| i.to_string()).collect();
        println!("    generations  {}", slots.join("  "));
        println!("    free list    [{}]", free.join(" "));
    }
}

fn says(alive: bool) -> &'static str {
    if alive { "alive" } else { "dead" }
}

fn main() {
    let mut h = Handles { generations: Vec::new(), free: Vec::new() };
    println!("alloc a, b, c, d: four new indices, each in its first life");
    let (a, b, c, d) = (h.alloc(), h.alloc(), h.alloc(), h.alloc());
    println!("    a {a}  b {b}  c {c}  d {d}");
    h.draw();

    println!("free b, then d: their generations go up now, before anyone reuses them");
    h.free(b);
    h.free(d);
    h.draw();

    println!("alloc e: alloc takes from the end of the free list, so index 3, in its next life");
    let e = h.alloc();
    println!("    e {e}");
    h.draw();

    println!("the old handles");
    println!("    d {d} is {}", says(h.alive(d)));
    println!("    e {e} is {}", says(h.alive(e)));
    println!("    b {b} is {}", says(h.alive(b)));
    if d == e {
        println!("    d and e are the same handle: whoever kept d now reaches e");
    } else {
        println!("    d and e share index {} but not a generation, so d can never reach e", e.index);
    }

    println!("free d again");
    println!("    {}", if h.free(d) { "freed: d named e, so e is gone" } else { "refused: d is dead, nothing changes" });
    h.draw();

    println!("alloc f, g: index 1 first, then a new one");
    let (f, g) = (h.alloc(), h.alloc());
    println!("    f {f}  g {g}");
    h.draw();
    println!("    c {c} was never freed: still {}", says(h.alive(c)));
}
