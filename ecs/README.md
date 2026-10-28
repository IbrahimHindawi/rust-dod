# Build an ECS

An archetype entity component system, built from nothing in fifteen steps,
in Rust. It starts with eight entities in fixed arrays and ends with a game:
three lanes, turrets, waves of drones, fire. The game runs on about 700 lines
of ECS, the same design [flecs](https://github.com/SanderMertens/flecs) is
built on. The ECS is called kayan, after the C one it grew out of.

The order is the name: **E**, then **C**, then **S**.

- **Entities.** What an entity is, and how a reference to one stays honest
  when things die and their numbers are reused.
- **Components.** Any component, in any combination, stored as columns in
  tables, changed while the game runs, and printed from reflect records.
- **Systems.** Queries that find tables, systems that stream their columns,
  commands that change structure safely, and the game.

There is no renderer, no window and no crate to install: only data, and the
loops that stream it.

Every chapter is one program that compiles on its own with `rustc`. It holds
everything built so far, with the chapter's new code near the end, so any
chapter can be opened in a debugger and stepped through as a whole ECS.
Beside it is the output it must print. In `docs/` is a guide that builds the
chapter's new part step by step, and every code sample in it is copied from
the program. Each guide ends with a table of what changed from the rin
version, [rin-dod](https://github.com/IbrahimHindawi/rin-dod).

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
      docs/
        1.1_ids.md              the guide: steps, output, exercises
        ...

## Build and run

The chapters are in rust-dod's `Cargo.toml`, each named after its file:

    cargo run --bin archetypes

Or compile one by hand. It's a single file with no crate around it:

    rustc --edition 2024 ecs/src/2.components/2.4.archetypes/archetypes.rs -o archetypes
    ./archetypes

Two checks keep the course honest:

    python tools/check.py ecs           build, run and diff every chapter
    python tools/check_snippets.py      every sample in a guide is in its program

## How to read it

Read the guide first. It explains the chapter's new idea and builds its code
step by step. Then open the program: the samples are all in there, in
context, with everything they depend on above them. Run it and read the
output against the code. Each guide ends with exercises, most of which are
"break this on purpose and watch what happens".

Each chapter's file starts from the previous chapter's, so diffing two
neighbours shows exactly what a chapter changed:

    git diff --no-index ecs/src/2.components/2.5.typed_access/typed_access.rs ecs/src/2.components/2.6.add_remove/add_remove.rs

## The chapters

| | chapter | about |
|---|---|---|
| **1** | **entities** | |
| 1.1 | [an entity is a number](docs/1.1_ids.md) | ids, components as arrays indexed by them, a system as a loop |
| 1.2 | [despawning](docs/1.2_despawn.md) | a free list, reused numbers, and the stale reference they cause |
| 1.3 | [generations](docs/1.3_generations.md) | handles that know when they are stale |
| 1.4 | [the entity index](docs/1.4_entity_index.md) | slots, packed rows, swap-remove, `verify` |
| **2** | **components** | |
| 2.1 | [components and signatures](docs/2.1_components.md) | component ids, a registry of reflect records, 64-bit signatures |
| 2.2 | [columns](docs/2.2_columns.md) | one array of any component, as bytes, viewed as a typed slice |
| 2.3 | [a table](docs/2.3_tables.md) | the entity index and the columns, joined |
| 2.4 | [archetypes](docs/2.4_archetypes.md) | a table for every combination, found by signature |
| 2.5 | [typed access](docs/2.5_typed_access.md) | `get::<T>`, `set`; keep handles, borrow references |
| 2.6 | [adding and removing](docs/2.6_add_remove.md) | moving between tables, and the archetype graph |
| 2.7 | [inspecting](docs/2.7_inspecting.md) | any entity, field by field, from reflect records |
| **3** | **systems** | |
| 3.1 | [queries](docs/3.1_queries.md) | the tables a system runs over, matched once and kept |
| 3.2 | [systems](docs/3.2_systems.md) | kernels, systems, a schedule, a frame |
| 3.3 | [commands](docs/3.3_commands.md) | why a system cannot change structure, and the buffer that can |
| 3.4 | [a game](docs/3.4_game.md) | turrets, waves, bullets and fire, on everything above |

After the course, the [advanced lessons](../docs/README.md) go underneath it:
memory layout, aliasing and SIMD. Lesson 7, [layout follows
access](../docs/7_access_patterns.md), is the bridge: why an ECS stores
components the way it does.

The building blocks the chapters are made of (the free list, swap-remove,
the slot map, the type-erased column and the command buffer) each have a
playground and a kata in [the dojo](../dojo/README.md), to watch them work on
their own and to practise writing them. Each chapter's guide says which
blocks it uses.
