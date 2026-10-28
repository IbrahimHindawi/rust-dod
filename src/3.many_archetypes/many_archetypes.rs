// 3. Many archetypes
//
// A game has more than one combination of components. Rocks only have a
// position. Ships move and can be hurt. Bullets move and hurt. Turrets stand
// still, can be hurt and hurt back. Each combination is a table of its own, and
// a system has to visit every table that has the columns it needs.
//
// kayan wrote a C struct per archetype, gave each a hand-set signature, and at
// query time looked each column up by name in a string map. Here one table type
// serves every archetype:
//
//   * A component is a member of the `ComponentId` enum, so its id is a small
//     integer, and a set of components fits in a u64 mask, one bit each.
//   * A table's columns are `[Option<Column>; 64]`, indexed by component id
//     and None where the table has no such column. Finding a column is an array
//     index, not a string lookup.
//   * A column is bytes, and its row size comes from the component's reflect
//     record, which the `component!` macro writes from the compiler's own
//     layout of the type.
//   * A table's signature is computed from the components it is created with,
//     so the signature cannot disagree with the columns. In kayan it could, and
//     it did: archetype D held positions and velocities, its signature left them
//     out, and the movement query skipped it.
//
// A query is two masks: a table matches when it has every component in `all`
// and none in `none`. Typed access comes back through generics: `column::<T>`
// finds T's column by T's id and returns it as a &[T] of the rows in use.

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

use std::alloc::Layout;

/// Plain old data: what lets a column hold a component as bytes, zero a row,
/// and hand the bytes back as the type.
///
/// # Safety
///
/// The type is Copy, has no padding bytes, and every bit pattern is a valid
/// value of it. That is a promise the compiler cannot check by itself, so the
/// trait is unsafe; pod! makes the promise only after checking it at compile
/// time.
#[allow(unsafe_code)]
unsafe trait Pod: Copy + 'static {}

#[allow(unsafe_code)]
unsafe impl Pod for f32 {}

macro_rules! pod {
    ($name:ident { $($field:ident: $ty:ty),* $(,)? }) => {
        #[derive(Clone, Copy, Default, Debug)]
        #[repr(C)]
        struct $name {
            $($field: $ty),*
        }

        #[allow(unsafe_code)]
        unsafe impl Pod for $name {}

        const _: () = {
            const fn field_is_pod<T: Pod>() {}
            $(field_is_pod::<$ty>();)*
            assert!(size_of::<$name>() == 0 $(+ size_of::<$ty>())*, "a component has padding");
        };
    };
}

// A component's reflect record: its id, its name, and the size and alignment a
// column needs to store it as bytes.
struct Info {
    id: u32,
    name: &'static str,
    layout: Layout,
}

trait Component: Pod {
    const INFO: &'static Info;
}

// The components, one member each. A component's id is its member's position.
#[derive(Clone, Copy)]
enum ComponentId {
    Position,
    Velocity,
    Health,
    Damage,
}

// Declares a component and its record. The id is the enum member with the
// struct's own name, so a component missing from the enum does not compile,
// and a record cannot belong to the wrong type.
macro_rules! component {
    ($name:ident { $($field:ident: $ty:ty),* $(,)? }) => {
        pod!($name { $($field: $ty),* });

        impl Component for $name {
            const INFO: &'static Info = &Info {
                id: ComponentId::$name as u32,
                name: stringify!($name),
                layout: Layout::new::<$name>(),
            };
        }
    };
}

pod!(Vec2 { x: f32, y: f32 });
component!(Position { at: Vec2 });
component!(Velocity { per_second: Vec2 });
component!(Health { points: f32, max: f32 });
component!(Damage { per_hit: f32 });

fn bit(id: u32) -> u64 {
    1 << id
}

fn mask_of<T: Component>() -> u64 {
    bit(T::INFO.id)
}

// 16 bytes, aligned to 16: the unit a column's storage comes in, so the start
// of every column is aligned for any component.
#[derive(Clone, Copy, Default)]
#[repr(C, align(16))]
struct Block([u8; 16]);

// One component's rows, as bytes, with room for a fixed number of rows. The
// record says how many bytes make a row.
struct Column {
    blocks: Vec<Block>,
    info: &'static Info,
}

impl Column {
    fn new(info: &'static Info, capacity: usize) -> Column {
        assert!(info.layout.align() <= 16, "{} needs more alignment than a column has", info.name);
        Column { blocks: vec![Block::default(); (info.layout.size() * capacity).div_ceil(16)], info }
    }

    #[allow(unsafe_code)]
    fn bytes(&self) -> &[u8] {
        // SAFETY: a Block is 16 initialised bytes with no padding, so the Vec's
        // buffer is `len * 16` initialised bytes.
        unsafe { std::slice::from_raw_parts(self.blocks.as_ptr().cast::<u8>(), self.blocks.len() * 16) }
    }

    #[allow(unsafe_code)]
    fn bytes_mut(&mut self) -> &mut [u8] {
        // SAFETY: as in bytes(); `&mut self` makes the view the only one.
        unsafe { std::slice::from_raw_parts_mut(self.blocks.as_mut_ptr().cast::<u8>(), self.blocks.len() * 16) }
    }

    fn zero_row(&mut self, row: usize) {
        let size = self.info.layout.size();
        self.bytes_mut()[row * size..(row + 1) * size].fill(0);
    }

    // The first `count` rows as T's. T must be the column's component.
    #[allow(unsafe_code)]
    fn slice<T: Component>(&self, count: usize) -> &[T] {
        assert!(self.info.id == T::INFO.id, "a {} column read as {}", self.info.name, T::INFO.name);
        let bytes = &self.bytes()[..count * size_of::<T>()];
        // SAFETY: T is the column's component, so the bytes are `count` T's;
        // the storage is aligned to 16 and T needs at most 16; T is Pod, so
        // any bytes are a valid T.
        unsafe { std::slice::from_raw_parts(bytes.as_ptr().cast::<T>(), count) }
    }

    #[allow(unsafe_code)]
    fn slice_mut<T: Component>(&mut self, count: usize) -> &mut [T] {
        assert!(self.info.id == T::INFO.id, "a {} column read as {}", self.info.name, T::INFO.name);
        let bytes = &mut self.bytes_mut()[..count * size_of::<T>()];
        // SAFETY: as in slice(). T has no padding, so writing a T leaves every
        // byte initialised, and the byte views stay valid.
        unsafe { std::slice::from_raw_parts_mut(bytes.as_mut_ptr().cast::<T>(), count) }
    }
}

// One archetype. `columns[id]` is Some when the table has component `id`. 64
// slots, one per bit a signature can hold.
struct Table {
    name: &'static str,
    signature: u64,
    count: usize,
    capacity: usize,
    columns: [Option<Column>; 64],
}

struct Query {
    all: u64,
    none: u64,
}

impl Table {
    // A column per component in `components`, and the signature from the same
    // list, so the two cannot disagree.
    fn new(name: &'static str, components: &[&'static Info], capacity: usize) -> Table {
        let mut t = Table { name, signature: 0, count: 0, capacity, columns: [const { None }; 64] };
        for &info in components {
            t.signature |= bit(info.id);
            t.columns[info.id as usize] = Some(Column::new(info, capacity));
        }
        t
    }

    fn has<T: Component>(&self) -> bool {
        self.signature & mask_of::<T>() != 0
    }

    fn matches(&self, q: &Query) -> bool {
        (self.signature & q.all) == q.all && (self.signature & q.none) == 0
    }

    // A new row with every component zeroed. The caller fills it in.
    fn add_row(&mut self) -> usize {
        assert!(self.count < self.capacity, "table {} is full", self.name);
        let row = self.count;
        for c in self.columns.iter_mut().flatten() {
            c.zero_row(row);
        }
        self.count += 1;
        row
    }

    // The rows in use of T's column. Asking a table for a column it does not
    // have is a bug in the caller, so it stops the program rather than return
    // nothing.
    fn column<T: Component>(&self) -> &[T] {
        self.columns[T::INFO.id as usize]
            .as_ref()
            .unwrap_or_else(|| panic!("table {} has no {} column", self.name, T::INFO.name))
            .slice::<T>(self.count)
    }

    fn column_mut<T: Component>(&mut self) -> &mut [T] {
        let count = self.count;
        self.columns[T::INFO.id as usize]
            .as_mut()
            .unwrap_or_else(|| panic!("table {} has no {} column", self.name, T::INFO.name))
            .slice_mut::<T>(count)
    }

    // Two of the table's columns at once. Both live in `columns`, so the
    // borrow checker must be shown they are two different ones: the same
    // component twice does not compile, and get_disjoint_mut lends out two
    // slots of the array after checking the indices differ.
    fn pair_mut<A: Component, B: Component>(&mut self) -> (&mut [A], &mut [B]) {
        const { assert!(A::INFO.id != B::INFO.id, "pair_mut needs two different components") };
        let count = self.count;
        let [a, b] = self.columns.get_disjoint_mut([A::INFO.id as usize, B::INFO.id as usize])
            .expect("two different columns");
        let a = a.as_mut().unwrap_or_else(|| panic!("table {} has no {} column", self.name, A::INFO.name));
        let b = b.as_mut().unwrap_or_else(|| panic!("table {} has no {} column", self.name, B::INFO.name));
        (a.slice_mut::<A>(count), b.slice_mut::<B>(count))
    }

    fn set<T: Component>(&mut self, row: usize, value: T) {
        self.column_mut::<T>()[row] = value;
    }
}

// Systems take columns, as in lesson 2. The loops that feed them are queries.
fn move_system(positions: &mut [Position], velocities: &[Velocity], dt: f32) {
    for (p, v) in positions.iter_mut().zip(velocities) {
        p.at.x += v.per_second.x * dt;
        p.at.y += v.per_second.y * dt;
    }
}

fn regen_system(healths: &mut [Health], per_second: f32, dt: f32) {
    for h in healths {
        h.points = (h.points + per_second * dt).min(h.max);
    }
}

fn print_tables(tables: &[Table]) {
    for t in tables {
        for row in 0..t.count {
            print!("    {:<7} row {row}", t.name);
            if t.has::<Position>() {
                let p = t.column::<Position>()[row];
                print!("  at ({:5.1}, {:5.1})", p.at.x, p.at.y);
            }
            if t.has::<Health>() {
                let h = t.column::<Health>()[row];
                print!("  health {:5.1}/{:.0}", h.points, h.max);
            }
            if t.has::<Damage>() {
                print!("  damage {:.0}", t.column::<Damage>()[row].per_hit);
            }
            println!();
        }
    }
}

fn spawn_ship(t: &mut Table, x: f32, y: f32) {
    let row = t.add_row();
    t.set(row, Position { at: Vec2 { x, y } });
    t.set(row, Velocity { per_second: Vec2 { x: 2.0, y: 0.0 } });
    t.set(row, Health { points: 50.0, max: 100.0 });
}

fn spawn_bullet(t: &mut Table, x: f32, y: f32, dy: f32) {
    let row = t.add_row();
    t.set(row, Position { at: Vec2 { x, y } });
    t.set(row, Velocity { per_second: Vec2 { x: 0.0, y: dy } });
    t.set(row, Damage { per_hit: 10.0 });
}

fn main() {
    let mut tables = [
        Table::new("rock", &[Position::INFO], 16),
        Table::new("ship", &[Position::INFO, Velocity::INFO, Health::INFO], 16),
        Table::new("bullet", &[Position::INFO, Velocity::INFO, Damage::INFO], 16),
        Table::new("turret", &[Position::INFO, Health::INFO, Damage::INFO], 16),
    ];

    for t in &tables {
        print!("{} signature {}:", t.name, t.signature);
        for c in t.columns.iter().flatten() {
            print!(" {}", c.info.name);
        }
        println!();
    }

    // Four tables borrowed at once, one name each: an array pattern takes its
    // elements apart, and the compiler knows they are four different tables.
    let [rocks, ships, bullets, turrets] = &mut tables;
    let rock = rocks.add_row();
    rocks.set(rock, Position { at: Vec2 { x: 5.0, y: 5.0 } });
    spawn_ship(ships, 0.0, 0.0);
    spawn_ship(ships, 0.0, 10.0);
    spawn_bullet(bullets, 1.0, 0.0, 8.0);
    spawn_bullet(bullets, 2.0, 0.0, 8.0);
    spawn_bullet(bullets, 3.0, 0.0, -8.0);
    let turret = turrets.add_row();
    turrets.set(turret, Position { at: Vec2 { x: 20.0, y: 0.0 } });
    turrets.set(turret, Health { points: 40.0, max: 200.0 });
    turrets.set(turret, Damage { per_hit: 25.0 });
    print_tables(&tables);

    let moving = Query { all: mask_of::<Position>() | mask_of::<Velocity>(), none: 0 };
    // Turrets have health too, but they repair by other means: no regen for
    // anything that deals damage.
    let healing = Query { all: mask_of::<Health>(), none: mask_of::<Damage>() };

    let dt = 0.5;
    for frame in 1..=2 {
        println!("frame {frame}");
        for t in &mut tables {
            if t.matches(&moving) {
                println!("    move visits {}", t.name);
                let (positions, velocities) = t.pair_mut::<Position, Velocity>();
                move_system(positions, velocities, dt);
            }
            if t.matches(&healing) {
                println!("    regen visits {}", t.name);
                regen_system(t.column_mut::<Health>(), 10.0, dt);
            }
        }
        print_tables(&tables);
    }
}
