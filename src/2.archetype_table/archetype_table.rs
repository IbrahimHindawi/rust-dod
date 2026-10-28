// 2. One archetype table
//
// An archetype is one combination of components. Its table holds every entity
// that has exactly that combination: one column per component, one row per
// entity. It is lesson 1's struct of arrays with two rules added.
//
// Every column has the same length, always. Spawn pushes to every column in
// one place and despawn removes from every column in one place. kayan appended
// to each column at the call site, and three appends that must happen together
// are three chances to forget one.
//
// Rows are packed. Despawn moves the last row into the hole it leaves (a
// swap-remove), so a system loops over every row with no gaps and no "alive"
// flag to test. The price is that a row is not a stable name for an entity: the
// last entity changes rows every time something before it dies. Lesson 4 deals
// with that.
//
// The sparks here have a position, a velocity and a life in seconds. Each frame
// they move, they age, and the ones whose life ran out are removed. Removal
// walks the rows backwards, so the row swapped into a hole has already been
// looked at this frame.

// `unsafe` is an error, except where an item allows it by name. Nothing here
// needs it: every column is a Vec of its own type.
#![deny(unsafe_code)]

#[derive(Clone, Copy, Default)]
struct Vec2 {
    x: f32,
    y: f32,
}

// The table for the archetype {position, velocity, life}. Each column is a Vec,
// and its length is the number of rows in use, the same for every column.
// `numbers` is not a component: it only lets the output say which spark sits in
// which row.
struct SparkTable {
    capacity: usize,
    numbers: Vec<u32>,
    positions: Vec<Vec2>,
    velocities: Vec<Vec2>,
    lives: Vec<f32>,
}

// Why a spawn did not happen.
#[derive(Debug)]
struct Full;

impl SparkTable {
    // Every column gets room for `capacity` rows once, up front.
    fn new(capacity: usize) -> SparkTable {
        SparkTable {
            capacity,
            numbers: Vec::with_capacity(capacity),
            positions: Vec::with_capacity(capacity),
            velocities: Vec::with_capacity(capacity),
            lives: Vec::with_capacity(capacity),
        }
    }

    fn count(&self) -> usize {
        self.numbers.len()
    }

    // The only place a row is added: every column gets its value here. Returns
    // the new row, or Full when there is no room.
    fn spawn(&mut self, number: u32, position: Vec2, velocity: Vec2, life: f32) -> Result<usize, Full> {
        if self.count() == self.capacity {
            return Err(Full);
        }
        self.numbers.push(number);
        self.positions.push(position);
        self.velocities.push(velocity);
        self.lives.push(life);
        Ok(self.count() - 1)
    }

    // The only place a row is removed. The last row moves into `row`, in every
    // column, and the table shrinks by one. Returns the row that moved, which is
    // `row` itself when the removed row was the last one.
    fn despawn(&mut self, row: usize) -> usize {
        let last = self.count() - 1;
        self.numbers.swap_remove(row);
        self.positions.swap_remove(row);
        self.velocities.swap_remove(row);
        self.lives.swap_remove(row);
        last
    }
}

// Systems. Each takes the columns it reads and writes, as slices, and knows
// nothing about tables. A `&mut` slice is the only way to reach its memory
// while the call runs, so the loop can be vectorized without checking whether
// the columns overlap.
fn move_system(positions: &mut [Vec2], velocities: &[Vec2], dt: f32) {
    for (p, v) in positions.iter_mut().zip(velocities) {
        p.x += v.x * dt;
        p.y += v.y * dt;
    }
}

fn age_system(lives: &mut [f32], dt: f32) {
    for life in lives {
        *life -= dt;
    }
}

// Structural change: rows leave the table. Backwards, so the row a swap-remove
// pulls in has already been tested.
fn expire_system(t: &mut SparkTable) {
    for i in (0..t.count()).rev() {
        if t.lives[i] <= 0.0 {
            let number = t.numbers[i];
            let moved = t.despawn(i);
            if moved != i {
                println!("    spark {number} in row {i} expired; spark {} moved from row {moved} into it", t.numbers[i]);
            } else {
                println!("    spark {number} in row {i} expired; it was the last row");
            }
        }
    }
}

fn print_table(t: &SparkTable) {
    for (row, ((n, p), life)) in t.numbers.iter().zip(&t.positions).zip(&t.lives).enumerate() {
        println!("    row {row}  spark {n}  position ({:6.2}, {:6.2})  life {life:.2}", p.x, p.y);
    }
}

fn main() {
    let mut sparks = SparkTable::new(8);
    println!("a table with room for {} sparks, {} bytes of components a row",
        sparks.capacity, size_of::<Vec2>() * 2 + size_of::<f32>());

    // Eight sparks with lives of 0.5s to 4s, plus one more than fits.
    for i in 0..9 {
        let f = i as f32;
        let life = 0.5 + ((i * 5) % 8) as f32 * 0.5;
        if let Err(Full) = sparks.spawn(i, Vec2 { x: f, y: 0.0 }, Vec2 { x: 1.0, y: f }, life) {
            println!("spark {i} did not fit: the table is full");
        }
    }
    print_table(&sparks);

    let dt = 0.5;
    for frame in 1..=4 {
        println!("frame {frame}");
        move_system(&mut sparks.positions, &sparks.velocities, dt);
        age_system(&mut sparks.lives, dt);
        expire_system(&mut sparks);
        print_table(&sparks);
    }
    println!("{} sparks left", sparks.count());
}
