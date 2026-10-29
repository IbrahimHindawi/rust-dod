# 11. A reflection walk

`dojo/11.reflect/play_reflect.rs`

A **reflect record** lists a struct's fields: each one's name, where its
bytes start in the struct, how many there are, and its type, with the type's
own record when it's a struct too. With records, one piece of code can list,
read, write and compare the fields of any type it's handed, by name: an
inspector, an editor, a save file, a network diff. It never names the type.
It walks the bytes by the record:

    "at.y" in a Lamp:  Lamp's field at, offset 8  ->  Vec2's field y, offset 4  ->  bytes 12..16

Each offset counts from the start of the field's own struct, so a path adds
them up as it goes down. And an offset is the record's to say, not something
to work out from the sizes before it: a `u8` followed by an `f32` leaves 3
bytes of padding, and the `f32` starts at 4.

**The invariant:** the walk reads and writes exactly the bytes the compiler
gave each field. Read by path, every field is what the struct holds.

In the course: [2.7](../../ecs/docs/2.7_inspecting.md) prints any entity
field by field from records. The Advanced [lesson 5](../../docs/5_inspector.md)
builds an inspector, and an editor's write by name.

## Watch it: the playground

    cargo run --bin play_reflect
    python tools/dojo.py play reflect

It prints a lamp's record, walks the lamp by it, changes fields by path, and
diffs the lamp with how it was:

```
a lamp's record: 20 bytes
    on: offset 0, size 1, u8
    power: offset 4, size 4, f32
    at: offset 8, size 8, a struct
    flicker: offset 16, size 4, f32, hidden
...
set at.x to 5.00: true
diff with the lamp before: power at.x
set flicker to 0.50: true
diff with the lamp before: power at.x
```

The records are written out by hand, with `offset_of!`, as 2.7's macro
writes them; rin's compiler writes its own. The lamp is kept as bytes, the
way a column keeps it. Step through it: break on `find` and watch `at` add
up. `verify()` runs after every step and checks every `f32` the walk reads
against the same field read where the compiler put it. Then flip the switch
at the top, `USE_RECORD_OFFSETS`, to `false` and run it again. The walk adds
up the sizes before each field instead, as if nothing had padding: `power` is
read from offset 1, three padding bytes and one byte of the real `power`, and
writing `at.x` lands on `power`. The record printed above it still says 4.

## Do reps: the kata

You write the walk. The harness hands you records, `Info` and `Field`, made
the way 2.7 makes them:

```rust
pub struct Field {
    pub name: &'static str,
    pub offset: usize,
    pub size: usize,
    pub ty: &'static str,
    pub info: Option<&'static Info>,
    pub hidden: bool,
}
```

and you list the fields a tool shows, by path, walking into the ones that are
structs:

```rust
fn leaves(info: &Info, prefix: &str, base: usize, out: &mut Vec<(String, usize, usize)>) {
    for f in info.fields.iter().filter(|f| !f.hidden) {
        let path = format!("{prefix}{}", f.name);
        match f.info {
            Some(inner) => leaves(inner, &format!("{path}."), base + f.offset, out),
            None => out.push((path, base + f.offset, f.size)),
        }
    }
}
```

and build on it, and on a walk down one path:

```rust
pub fn paths(info: &Info) -> Vec<String> {
```

```rust
pub fn diff(info: &Info, a: &[u8], b: &[u8]) -> Vec<String> {
```

```rust
pub fn get_f32(info: &Info, bytes: &[u8], path: &str) -> Option<f32> {
```

```rust
pub fn set_f32(info: &Info, bytes: &mut [u8], path: &str, value: f32) -> bool {
```

A hidden field is left out of `paths` and `diff`, with everything inside it,
but a path still reaches it: hiding is for tools that list, not for code that
asks by name. `get_f32` and `set_f32` refuse a path that names a struct, a
`u8`, or nothing.

| level | you write | target |
|---|---|---|
| 1 | `leaves`, the walk, in an otherwise finished file | 4:00 |
| 2 | every function body; the signatures are given | 12:00 |
| 3 | the whole file, from a blank page | 15:00 |

    python tools/dojo.py start reflect 2
    python tools/dojo.py reflect

The harness's test types have the traps in them: `Lamp` has padding after
`on: u8`, `Room` has a `Lamp` inside it (paths three deep, offsets added
twice), and a hidden `draft: Vec2`. It turns values into bytes and back field
by field, with `offset_of!`, so it needs no `unsafe` and neither does the
kata. It runs the playground's steps as a scenario, then 20,000 random reads,
writes and diffs of a `Room` by path, each checked against the same fields
reached directly. The paths include ones that name a struct, a `u8`, or
nothing: `light.atx.x`, `corner.x.y`, `light.`.

**Level 4, variations.** The same harness, different insides:

- **`find` by recursion**: split off the first name, find that field, and
  recurse into its record with the rest of the path.
- **Paths without allocating**: `leaves` builds a `String` per field. Walk
  with a callback, `fn walk(info, base, &mut impl FnMut(&[&str], usize, &Field))`,
  that gets the names on the way down instead.
- **A tiny text format**: write a value as `path = value` lines and read it
  back. That's a save file and a network message. (Not checked by the
  harness; write your own test.)

## When it fails

    FAIL at operation 37 (seed 1): get_f32("light.a.x") gives -135.00, it should give None
        debug it: build/dojo/reflect/harness --seed 1 --stop 37

There's a field `at` but no field `a`: names that match by prefix find the
wrong field. Run the harness under your debugger with those arguments and a
breakpoint on `stop_here`, which runs just before that operation, then step
through the walk down the path, watching each name and offset.
