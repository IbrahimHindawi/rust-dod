# rust-dod

Data-oriented design in Rust, in two courses:

1. **Build an ECS.** Start here. Fifteen chapters that build an archetype
   entity component system from nothing, in the order of its name: entities,
   then components, then systems. It ends with a game running on it. Each
   chapter is a program that compiles on its own, the output it must print,
   and a guide that builds it step by step.
2. **Advanced: layout and SIMD.** Seven lessons, for after. They cover what a
   loop fetches from memory, aliasing and what a `&mut` slice promises, what
   the compiler vectorizes and how to check it in the assembly, and which
   layout suits which access pattern.

There is no renderer, no window and no crate to install: only data, and the
loops that stream it. Every program is one file, and builds with plain
`rustc`.

This is the Rust twin of [rin-dod](https://github.com/IbrahimHindawi/rin-dod),
the same courses in [rin](https://github.com/IbrahimHindawi/rin). The programs
print the same output, line for line, and each guide has a table of what
changed between the two.

## Build an ECS

| | chapter | about |
|---|---|---|
| **1** | **entities** | |
| 1.1 | [an entity is a number](ecs/docs/1.1_ids.md) | ids, components as arrays indexed by them, a system as a loop |
| 1.2 | [despawning](ecs/docs/1.2_despawn.md) | a free list, reused numbers, and the stale reference they cause |
| 1.3 | [generations](ecs/docs/1.3_generations.md) | handles that know when they are stale |
| 1.4 | [the entity index](ecs/docs/1.4_entity_index.md) | slots, packed rows, swap-remove, `verify` |
| **2** | **components** | |
| 2.1 | [components and signatures](ecs/docs/2.1_components.md) | component ids, a registry of reflect records, 64-bit signatures |
| 2.2 | [columns](ecs/docs/2.2_columns.md) | one array of any component, as bytes, viewed as a typed slice |
| 2.3 | [a table](ecs/docs/2.3_tables.md) | the entity index and the columns, joined |
| 2.4 | [archetypes](ecs/docs/2.4_archetypes.md) | a table for every combination, found by signature |
| 2.5 | [typed access](ecs/docs/2.5_typed_access.md) | `get::<T>`, `set`; keep handles, borrow references |
| 2.6 | [adding and removing](ecs/docs/2.6_add_remove.md) | moving between tables, and the archetype graph |
| 2.7 | [inspecting](ecs/docs/2.7_inspecting.md) | any entity, field by field, from reflect records |
| **3** | **systems** | |
| 3.1 | [queries](ecs/docs/3.1_queries.md) | the tables a system runs over, matched once and kept |
| 3.2 | [systems](ecs/docs/3.2_systems.md) | kernels, systems, a schedule, a frame |
| 3.3 | [commands](ecs/docs/3.3_commands.md) | why a system cannot change structure, and the buffer that can |
| 3.4 | [a game](ecs/docs/3.4_game.md) | turrets, waves, bullets and fire, on everything above |

Read each chapter's guide first. It explains the new idea and builds its code
step by step, and every sample in it is copied from the chapter's program.
Then open the program: it holds everything built so far, with the chapter's
new code near the end, so any chapter can be opened in a debugger and stepped
through as a whole ECS. Run it and read the output against the code. Each
guide ends with exercises, most of them "break this on purpose and watch what
happens". In Rust, a good few of those end in a compiler error, and the guide
shows it.

Each chapter's file starts from the previous chapter's, so diffing two
neighbours shows exactly what a chapter changed:

    git diff --no-index ecs/src/2.components/2.5.typed_access/typed_access.rs ecs/src/2.components/2.6.add_remove/add_remove.rs

The ECS is called kayan, after the C sketch it grew out of.

## Advanced: layout and SIMD

| | lesson | about |
|---|---|---|
| 0 | [from kayan](docs/0_from_kayan.md) | what was kept from the C kayan, what changed, and why |
| 1 | [aos vs soa](docs/1_aos_vs_soa.md) | the same particles in three layouts, what a loop fetches, and what SIMD needs |
| 2 | [archetype table](docs/2_archetype_table.md) | one table: columns, rows, spawn, swap-remove, systems over slices |
| 3 | [many archetypes](docs/3_many_archetypes.md) | one table type for every archetype: ids, signatures, queries |
| 4 | [entity handles](docs/4_entity_handles.md) | generational handles, slots, moving between archetypes |
| 5 | [inspector](docs/5_inspector.md) | printing and editing any entity from reflect records |
| 6 | [kayan](docs/6_kayan.md) | a small game, and deferring structural changes to a command buffer |
| 7 | [access patterns](docs/7_access_patterns.md) | four layouts under streams and lookups, and how an ECS decides |

Lessons 1 and 7 are about memory, aliasing and the code the compiler makes.
Lessons 2 to 6 build an ECS again, compressed into five steps, so read them as
a recap of the course. [docs/README.md](docs/README.md) has more.

## The Rust in it

The style is rin-dod's: plain structs, plain loops, data in arrays, no
framework. What Rust adds is checked:

- **Components are plain data, checked.** A column holds bytes, and turning
  bytes back into a component is only sound if any bytes are a valid value.
  The `pod!` macro checks at compile time that every field is plain data and
  the struct has no padding.
- **`unsafe` is fenced in.** Every program starts with `#![deny(unsafe_code)]`.
  The whole ECS has five one-line `unsafe` blocks, each with a `// SAFETY:`
  reason: four views of a column's bytes, and one of a value's. Every program
  that has one runs clean under [Miri](https://github.com/rust-lang/miri),
  Rust's checker for undefined behaviour.
- **Several of rin-dod's quiet bugs don't compile.** A table reference kept
  across the creation of another (2.4), a component reference kept across a
  spawn (2.5), despawning while iterating a table (3.3). Each is a lesson in
  rin-dod and a compiler error here, and the guides show the error. What the
  borrow checker can't see is a stale *number*, a row or an entity index, and
  generations, handles and commands are how both languages handle those.
- **Vec, used like an arena.** The world's data lives in `Vec`s that only grow
  while the game runs, and the frame's commands in `Vec`s that are cleared
  every frame and keep their memory. That is an arena's allocation pattern,
  with nothing freed one at a time.
- **No dependencies.** The standard library has everything: `Vec`, `HashMap`,
  slices, `get_disjoint_mut` for two tables at once.

## Build and run

Needs Rust 1.86 or later (`rustup`, or your system's package). Every chapter
and lesson is its own program, named after its file:

    cargo run --bin archetypes

Or build one by hand, as one file with no crate around it:

    rustc --edition 2024 ecs/src/2.components/2.4.archetypes/archetypes.rs -o archetypes
    ./archetypes

`python tools/check.py` builds every program with `rustc` and warnings as
errors, runs it, and compares what it prints with its `.expected` file.
`--release` builds them optimised, `--clippy` runs clippy over everything,
`--miri` runs the programs with `unsafe` under Miri (it needs a nightly
toolchain with the `miri` component), `--bless` rewrites the expected files
after a deliberate change, and a word filters the programs.
`tools/check_snippets.py` checks that every sample in the guides is in its
program, and every output sample in its expected file:

    python tools/check.py ecs
    python tools/check.py --release --clippy
    python tools/check.py --miri
    python tools/check_snippets.py

## Reading it in a browser

Open `website/index.html`. It has every chapter's and lesson's guide, its
source with line numbers, and the output it must print, with an index on the
left. It is generated from the files in this repository, so after changing a
program or a guide run `python tools/site.py` to bring it up to date.

## Layout

    ecs/
      src/
        1.entities/
          1.1.ids/
            ids.rs              the program
            ids.expected        what it prints
          ...
        2.components/
        3.systems/
      docs/                     a guide a chapter: steps, output, exercises
      README.md                 the course on its own
    src/                        the advanced lessons, one folder each
    includes/
      kayan.rs                  lesson 4's world, used by lessons 5 and 6
    docs/                       the advanced lessons' guides
    website/
      index.html                everything as one page: guide, code, output
      template.html             its layout and styles
    tools/
      check.py                  build, run and compare every program
      check_snippets.py         check every sample in the guides is in its program
      site.py                   rebuild website/index.html from the repo
    Cargo.toml                  the program list, for cargo and check.py

## License

MIT. See [LICENSE](LICENSE).
