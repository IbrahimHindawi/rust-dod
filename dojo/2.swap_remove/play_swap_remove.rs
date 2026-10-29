// Dojo 2, the playground: swap-remove and the back-reference.
//
// Systems loop over values, so values are kept packed: one after another, no
// holes, nothing to skip. Removing one from the middle would leave a hole, so
// the last value moves into it instead. That's swap-remove: one copy, however
// many values there are. The price is that values move, so an id can't be the
// position of its value. Two arrays keep track:
//
//   * sparse, by id: rows[id] is the row id's value is in now;
//   * packed, by row: ids[row] is the id that owns the row, the
//     back-reference. When the last row moves, it says whose entry in `rows`
//     to fix.
//
// This is what EnTT calls a sparse set, and what the ECS course's tables do
// in 1.4 and 2.3.
//
// This program inserts four values, removes one from the middle, and draws
// both sides after each step. To see it in a debugger, break on `remove` and
// watch `ids` and `rows` as the last row moves.
//
// The switch: set FIX_BACK_REFERENCE to false and run it again. The moved
// id's row still points where its value used to be, and the next insert
// writes another id's value there. It's a static, read each time it's used,
// so a debugger can flip it mid-run too: stop on `remove`, set it to false in
// the watch window, and continue.

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

use std::sync::atomic::{AtomicBool, Ordering::Relaxed};

static FIX_BACK_REFERENCE: AtomicBool = AtomicBool::new(true);

struct SparseSet {
    values: Vec<u64>,
    ids: Vec<u32>,
    rows: Vec<Option<u32>>,
}

impl SparseSet {
    fn insert(&mut self, id: u32, value: u64) {
        let i = id as usize;
        if i >= self.rows.len() {
            self.rows.resize(i + 1, None);
        }
        self.rows[i] = Some(self.values.len() as u32);
        self.values.push(value);
        self.ids.push(id);
    }

    fn remove(&mut self, id: u32) -> Option<u64> {
        let row = self.rows[id as usize].take()? as usize;
        let value = self.values.swap_remove(row);
        self.ids.swap_remove(row);
        if let Some(&moved) = self.ids.get(row)
            && FIX_BACK_REFERENCE.load(Relaxed)
        {
            self.rows[moved as usize] = Some(row as u32);
        }
        Some(value)
    }

    // id's value through its row. A row past the end reads nothing; a row
    // that is wrong but in range reads someone else's value.
    fn get(&self, id: u32) -> Option<u64> {
        let row = (*self.rows.get(id as usize)?)?;
        self.values.get(row as usize).copied()
    }

    fn draw(&self) {
        let cells = |items: Vec<String>| items.iter().map(|s| format!("{s:>5}")).collect::<String>();
        println!("    row     {}", cells((0..self.values.len()).map(|r| r.to_string()).collect()));
        println!("    ids     {}", cells(self.ids.iter().map(|i| i.to_string()).collect()));
        println!("    values  {}", cells(self.values.iter().map(|v| v.to_string()).collect()));
        let sparse: Vec<String> = self
            .rows
            .iter()
            .enumerate()
            .filter_map(|(id, row)| row.map(|r| format!("{id}->{r}")))
            .collect();
        println!("    rows    {}  (id -> row)", sparse.join("  "));
    }
}

fn read(s: &SparseSet, id: u32) -> String {
    s.get(id).map_or("nothing".to_string(), |v| v.to_string())
}

fn main() {
    let mut s = SparseSet { values: Vec::new(), ids: Vec::new(), rows: Vec::new() };
    println!("insert 3, 7, 1, 5, each with a value of ten times its id");
    for id in [3, 7, 1, 5] {
        s.insert(id, id as u64 * 10);
    }
    s.draw();

    println!("remove 7, in row 1: the last row, 5's, moves into the hole");
    s.remove(7);
    s.draw();
    println!("    get 5: {}", read(&s, 5));
    println!("    get 7: {}", read(&s, 7));

    println!("insert 9: a new row at the end");
    s.insert(9, 90);
    s.draw();
    println!("    get 5: {}", read(&s, 5));
    println!("    get 9: {}", read(&s, 9));

    println!("remove 3, in row 0: the last row, 9's, moves into it");
    s.remove(3);
    s.draw();
    println!("    get 9: {}  get 5: {}  get 1: {}", read(&s, 9), read(&s, 5), read(&s, 1));
}
