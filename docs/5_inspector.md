# 5. The inspector

`src/5.inspector/inspector.rs`

**In the dojo:** [11. a reflection walk](../dojo/11.reflect/README.md), on its own, to watch and to practise.

A soldier and a crate, printed field by field by code that names no component
type, then edited by component and field name, then a table layout report.
Everything comes from the reflect records the world already holds for sizes.

## Where the records come from

rin's compiler writes a reflect record for every struct, `T<>`. Rust has no
reflection built in, so `includes/kayan.rs` writes them in the macro that
declares each component. A record is a type's name, its layout, and what its
bytes are:

```rust
pub struct Info {
    pub name: &'static str,
    pub layout: Layout,
    pub kind: Kind,
}

pub enum Kind {
    F32,
    U32,
    I32,
    // A [u8; N]: text, padded with zeros.
    Text,
    // An i32 that is one of these named values (pod_enum!).
    Enum(&'static [(&'static str, i32)]),
    // A struct, field by field (pod!).
    Struct(&'static [Field]),
}
```

A struct's record lists its fields, and each field links to the record of its
own type:

```rust
pub struct Field {
    pub name: &'static str,
    pub offset: usize,
    pub info: &'static Info,
    pub hidden: bool,
}
```

`pod!` fills them in. For each field, the compiler supplies the name
(`stringify!`), the byte offset (`offset_of!`), and the field type's record:

```rust
                    kind: Kind::Struct(&[$(Field {
                        name: stringify!($field),
                        offset: std::mem::offset_of!($name, $field),
                        info: <$ty as Reflect>::INFO,
                        hidden: $crate::kayan::is_hidden!($($attr)?),
                    }),*]),
```

Every type a component is made of implements `Reflect`, so the field types'
records are there to link to. `f32`, `u32` and `i32` have records written by
hand, a `[u8; N]` of any length has one through a generic impl, and every
struct made with `pod!` has one. A record is a constant, built when the
program is compiled, and it costs nothing until a tool reads it.

rin-dod's `kind` and `type` were strings like `"f32"`, compared with `strcmp`.
Here `Kind` is an enum, so a `match` on it has to handle every kind, and the
compiler says so if one is missing.

## Printing from records

`print_fields` takes the component's bytes, from `get_bytes`, and cuts each
field's bytes out by offset and size:

```rust
fn print_fields(fields: &[Field], base: &[u8], depth: usize) {
    for f in fields.iter().filter(|f| !f.hidden) {
        let at = &base[f.offset..f.offset + f.info.layout.size()];
        print!("{:indent$}{}:", "", f.name, indent = depth * 4);
        print_value(f.info, at, depth);
    }
}
```

A record that disagreed with the data would stop the program at the slice,
not print garbage. `print_value` is one `match` on the kind:

```rust
fn print_value(info: &Info, at: &[u8], depth: usize) {
    match info.kind {
        Kind::F32 => println!(" {:.2}", f32::from_ne_bytes(number(at))),
        Kind::U32 => println!(" {}", u32::from_ne_bytes(number(at))),
        Kind::I32 => println!(" {}", i32::from_ne_bytes(number(at))),
        Kind::Text => {
            let end = at.iter().position(|&b| b == 0).unwrap_or(at.len());
            println!(" \"{}\"", String::from_utf8_lossy(&at[..end]));
        }
        Kind::Enum(members) => {
            let value = i32::from_ne_bytes(number(at));
            let name = members.iter().find(|m| m.1 == value).map_or("(not a member)", |m| m.0);
            println!(" {name}");
        }
        Kind::Struct(fields) => {
            println!();
            print_fields(fields, at, depth + 1);
        }
    }
}
```

Numbers are read with `from_ne_bytes`, which turns an array of bytes into a
number. rin-dod cast a pointer into the bytes instead. Rust can do that too,
in an `unsafe` block, but there's no need: the inspector has no `unsafe` at
all. Because components are structs, even a single float like
`Health.points` has a name to print.

## Enums in components

```rust
pod_enum!(Side { RED = 0, BLUE = 1 });
```

```rust
component!(Team { side: Side });
```

Why not a Rust `enum Side { Red, Blue }`? Because a column holds bytes, and a
component has to be valid whatever its bytes are (2.2's `Pod`). A Rust enum
isn't: its only valid values are its members, and a `Side` with the bytes of
7 would be undefined behaviour, not just a wrong answer. So `pod_enum!`
declares what C would: an `i32` with named values.

```rust
        struct $name(i32);
```

`Side::BLUE` is `Side(1)`. The record keeps the names, and a value that isn't
one prints as `(not a member)` instead of breaking anything.

## Tags

```rust
component!(Health { points: f32, max: f32, #[hidden] regen_timer: f32 });
```

`#[hidden]` is only text the macro matches, recorded as the field's `hidden`
flag. The inspector skips hidden fields. The component decides what a tool
shows, and the tool doesn't need to know the component.

## Names belong in tools

```rust
fn set_f32_by_name(w: &mut World, e: Entity, component: &str, field: &str, value: f32) -> Option<()> {
    let id = (0..w.component_count).find(|&id| w.info(id).name == component)?;
    let Kind::Struct(fields) = w.info(id).kind else {
        return None;
    };
    let f = fields.iter().find(|f| f.name == field && matches!(f.info.kind, Kind::F32))?;
    w.get_bytes_mut(e, id)?[f.offset..f.offset + 4].copy_from_slice(&value.to_ne_bytes());
    Some(())
}
```

It finds the component by name, the field by name, checks the field is an
`f32`, and writes it. Each `?` is a way to refuse: no such component, no such
`f32` field, or the entity doesn't have the component. It refuses
`Weapon.ammo`, a `u32`, and the crate's `Health`, which it doesn't have:

```
the editor writes three fields by name
    soldier Health.points = 55: written
    soldier Weapon.ammo = 1: refused, ammo is a u32, not an f32
    crate Health.points = 1: refused, the crate has no health
```

That's the flexibility an editor or a console needs. It's also the string
comparison kayan did in every query, once per archetype. Here it runs once
per edit, and systems keep using ids.

## Layout

`print_layout` lists each table's columns with size and alignment, and each
column's bytes in use out of the bytes allocated for the table's capacity:

```
table 0 {Position Health Team Weapon Label}, rows in use: 1 of 16
    Position 12 bytes, align 4, column 12 of 192 bytes
    Health   12 bytes, align 4, column 12 of 192 bytes
    Team      4 bytes, align 4, column 4 of 64 bytes
    Weapon   12 bytes, align 4, column 12 of 192 bytes
    Label    16 bytes, align 1, column 16 of 256 bytes
```

It's the kind of report that shows when a component has grown padding (here
it can't: `pod!` refuses padding), when a table has far more room than rows,
or when a table carries a column most of its systems never read.

## Rust notes

| rin | Rust |
|---|---|
| `position<>`, written by the compiler | `Position::INFO`, written by `pod!` from `stringify!`, `offset_of!` and `size_of` |
| `f*.type`, a string, compared with `strcmp` | `f.info.kind`, an enum, matched |
| `cast(at[0]&, *const f32)*` | `f32::from_ne_bytes(...)`: no pointer, no `unsafe` |
| `side: enum = { red, blue }` in a component | `pod_enum!(Side { RED = 0, BLUE = 1 })`: an `i32` with names |
| `regen_timer: f32 @ "hidden"` | `#[hidden] regen_timer: f32` |
| `text: [16]c8` | `text: [u8; 16]`, printed as UTF-8 up to the first zero |
| `return 0;` to refuse | `None`, through `?` |

## Try

1. Add a `u8`-sized flag to `Weapon` and see `pod!` refuse it. Then add a `u32`
   `armed` field instead, and give it its own `Kind::Bool` that prints `yes`
   or `no`. What has to change in `kayan.rs`, and what in the inspector?
2. Give `Position` a `#[readonly]` tag and make `set_f32_by_name` refuse
   fields that carry it. (`is_hidden!` shows how a macro turns a word into a
   flag.)
3. Write `set_by_name` for `u32` and `i32` as well, picking the write from
   the field's `kind`.
4. Write the bytes of 7 into the soldier's `Team` with `get_bytes_mut`, and
   inspect it. Why is that safe here, and why would it not be with a Rust
   `enum`?
5. Print a whole table as rows and columns: one line per entity, one column
   per field.
