# 3. Many archetypes

`src/3.many_archetypes/many_archetypes.rs`

Four archetypes (rock, ship, bullet, turret) and four components (Position,
Velocity, Health, Damage). One `Table` type holds all four archetypes, and two
queries decide which tables each system visits.

## A table without a type

Lesson 2's table had one typed field per column. That can't work for every
archetype at once, so the columns become bytes:

```rust
struct Table {
    name: &'static str,
    signature: u64,
    count: usize,
    capacity: usize,
    columns: [Option<Column>; 64],
}
```

`columns[id]` is `None` when the table has no such component. A column is a
run of 16-byte blocks and the component's reflect record:

```rust
struct Column {
    blocks: Vec<Block>,
    info: &'static Info,
}
```

The record says how many bytes make a row, `info.layout.size()`.
`add_row`, and lesson 4's moves, work only from sizes, so they never need to
know a component's type:

```rust
    fn zero_row(&mut self, row: usize) {
        let size = self.info.layout.size();
        self.bytes_mut()[row * size..(row + 1) * size].fill(0);
    }
```

Types come back at the edges. `column::<T>()` finds T's column by T's id and
returns it as a `&[T]` of the rows in use. Systems take `&mut [Position]`,
`&[Velocity]` and so on, exactly as in lesson 2.

```rust
    fn column<T: Component>(&self) -> &[T] {
        self.columns[T::INFO.id as usize]
            .as_ref()
            .unwrap_or_else(|| panic!("table {} has no {} column", self.name, T::INFO.name))
            .slice::<T>(self.count)
    }
```

Turning bytes back into `T`s is the one thing the compiler can't check, so it
is `unsafe`: four one-line views in `Column`, each with a `// SAFETY:`
comment, resting on the `Pod` promise that `pod!` checks at compile time. The
[ECS course's 2.2](../ecs/docs/2.2_columns.md) builds the same column step by
step, and explains why the blocks are 16 bytes. The program runs clean under
Miri.

## One list, and nothing to check

rin keeps three things in step by hand: the `component` enum, the
`component_info` array of records, and a `component_of<T>` specialisation per
type. It sizes the arrays by the enum's reflected member count, and a
`components_check` at startup stops the program if the enum and the records
were written in different orders.

Rust has no reflection to count an enum's members or read their names. It
doesn't need them here. The `component!` macro writes each component's record
as a constant of its type, with the id taken from the enum member that has the
struct's own name:

```rust
        impl Component for $name {
            const INFO: &'static Info = &Info {
                id: ComponentId::$name as u32,
                name: stringify!($name),
                layout: Layout::new::<$name>(),
            };
        }
```

So the record can't belong to the wrong type, and nothing is looked up by
position. A table is made from records, not ids:

```rust
        Table::new("ship", &[Position::INFO, Velocity::INFO, Health::INFO], 16),
```

The enum is the only list left, and a component that isn't in it doesn't
compile:

```
error[E0599]: no variant, associated function, or constant named `Armor` found for enum `ComponentId` in the current scope
...
111 | component!(Armor { points: f32 });
    |            ^^^^^ variant, associated function, or constant not found in `ComponentId`
```

There's no count either. `columns` has 64 slots, one per bit a signature can
hold, and the unused ones are `None`. A slot is 32 bytes, a Vec and a
reference, so that's 2 KB a table, and nothing next to its columns.

## Two columns of one table

`move_system` writes positions and reads velocities, two columns of the same
table. rin takes the velocities first and then the positions. The same in
Rust doesn't compile:

```rust ignore
                let velocities = t.column::<Velocity>();
                move_system(t.column_mut::<Position>(), velocities, dt);
```

```
error[E0502]: cannot borrow `*t` as mutable because it is also borrowed as immutable
    |
352 |                 let velocities = t.column::<Velocity>();
    |                                  - immutable borrow occurs here
353 |                 move_system(t.column_mut::<Position>(), velocities, dt);
    |                             ^^^^^^^^^^^^^^^^^^^^^^^^^^  ---------- immutable borrow later used here
    |                             |
    |                             mutable borrow occurs here
```

This isn't a bug in rin. The two columns really are separate memory. But
`column` borrows the whole table, and the compiler can't see which column
inside it a call reaches. So the table hands out both at once, through
`get_disjoint_mut`, which lends two slots of the `columns` array after
checking they're different:

```rust
    fn pair_mut<A: Component, B: Component>(&mut self) -> (&mut [A], &mut [B]) {
        const { assert!(A::INFO.id != B::INFO.id, "pair_mut needs two different components") };
        let count = self.count;
        let [a, b] = self.columns.get_disjoint_mut([A::INFO.id as usize, B::INFO.id as usize])
            .expect("two different columns");
```

The ids are constants, so `pair_mut::<Position, Position>()` is stopped by the
`const` assert when the program builds. The query loop then reads like rin's:

```rust
                let (positions, velocities) = t.pair_mut::<Position, Velocity>();
                move_system(positions, velocities, dt);
```

## Different types, the same loop

rin's guide found that clang vectorized this lesson's `move_system` without a
run-time overlap check, and lesson 2's only with one, before either had
`noalias`. Here the columns are different struct types, and clang reads C's
type rules as saying those can't share memory. In lesson 2 both were `vec2`.

Rust has no rules about types sharing memory. It has `&mut`: the positions
slice is the only way to its memory, whatever the types. So the two lessons
compile to the same loop, four rows a pass, with no overlap check. The only
extra work is `pair_mut`'s, once per table before the loop: the two column
ids and the two slice lengths.

## Queries

```rust
    let moving = Query { all: mask_of::<Position>() | mask_of::<Velocity>(), none: 0 };
    // Turrets have health too, but they repair by other means: no regen for
    // anything that deals damage.
    let healing = Query { all: mask_of::<Health>(), none: mask_of::<Damage>() };
```

A table matches when it has every bit in `all` and none in `none`. `moving`
visits ships and bullets. `healing` visits ships but not turrets, which have
health but also deal damage. The output prints each visit:

```
frame 1
    move visits ship
    regen visits ship
    move visits bullet
```

## The signature cannot lie

`Table::new` builds the signature from the same list it makes columns from:

```rust
        for &info in components {
            t.signature |= bit(info.id);
            t.columns[info.id as usize] = Some(Column::new(info, capacity));
        }
```

kayan wrote them separately, and archetype D's signature left out two of its
columns (see [0_from_kayan.md](0_from_kayan.md)).

## Four tables at once

Setting up the world needs a `&mut` to each of the four tables. rin takes
`tables[0]&`, `tables[1]&` and so on. Rust takes the array apart with a
pattern:

```rust
    let [rocks, ships, bullets, turrets] = &mut tables;
```

Four names, four `&mut Table`s, and the compiler knows they're four different
elements, because the pattern says so. Indexing `&mut tables[1]` twice would
be two borrows of the array, and only one may be held at a time.

## Rust notes

| rin | Rust |
|---|---|
| `columns: [component<>.count]^u8`, null if absent | `columns: [Option<Column>; 64]`, `None` if absent |
| `component_info[id]*.size` | `info.layout.size()`, from `Layout::new::<T>()` in the macro |
| `component_of: proc<position>() -> component` per type | `T::INFO.id`, a constant of the type |
| `components_check()` at startup | nothing to check: the id comes from the enum member with the type's name |
| `cast(t*.columns[c], ^T); raw[0..count]` | `slice::<T>(count)`, one `unsafe` line with its reason |
| `column<velocity>(t)` in a local, then `column<position>(t)` | `t.pair_mut::<Position, Velocity>()` |
| `tables[0]&`, `tables[1]&`, ... | `let [rocks, ships, bullets, turrets] = &mut tables;` |
| `printfmt(...); exit(1);` | `assert!` and `panic!`, with the same messages |

## Try

1. Add an `Armor` component, give it to turrets, and make a query that visits
   only tables with both `Health` and `Armor`. What did you have to touch?
2. Ask the rock table for `column::<Velocity>()`. Read the message, then find
   where it comes from.
3. Change `columns` to `[Option<Column>; 4]`, then add a fifth component.
   Where does it fail, and when: building or running?
4. Write `remove_row` with the swap-remove from lesson 2, working only from
   the columns' record sizes and `copy_within` on their bytes.
