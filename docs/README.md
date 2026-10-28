# Advanced: layout and SIMD

Seven lessons for after [Build an ECS](../README.md). The course builds the
ECS. These go underneath it, to the memory and the machine code:

- **Lesson 1, aos vs soa.** The same particles in three layouts: what a loop
  fetches per cache line, what a `&mut` slice promises the compiler, which
  loops it vectorizes, and how to check that in the assembly.
- **Lessons 2 to 6** build an archetype ECS again, compressed into five
  steps, and end with a small game. They came first and move fast. After the
  course, read them as a recap, or as a second take on the same design.
  Lesson 4 builds the world in `includes/kayan.rs`, and lessons 5 and 6
  bring it in unchanged with `#[path]`.
- **Lesson 7, layout follows access.** Four layouts under streams and under
  random lookups, timed, and how an ECS decides where each component lives.
- **Lesson 0, from kayan**, reviews the original C kayan: what these lessons
  kept from it, what they changed, and why.

| | lesson | about |
|---|---|---|
| 0 | [from kayan](0_from_kayan.md) | what was kept from the C kayan, what changed, and why |
| 1 | [aos vs soa](1_aos_vs_soa.md) | the same particles in three layouts, what a loop fetches, and what SIMD needs |
| 2 | [archetype table](2_archetype_table.md) | one table: columns, rows, spawn, swap-remove, systems over slices |
| 3 | [many archetypes](3_many_archetypes.md) | one table type for every archetype: ids, signatures, queries |
| 4 | [entity handles](4_entity_handles.md) | generational handles, slots, moving between archetypes |
| 5 | [inspector](5_inspector.md) | printing and editing any entity from reflect records |
| 6 | [kayan](6_kayan.md) | a small game, and deferring structural changes to a command buffer |
| 7 | [access patterns](7_access_patterns.md) | four layouts under streams and lookups, and how an ECS decides |

## How to read them

Read each lesson's `.rs` file from the top. It opens with the problem it
solves, and the code follows in the order it runs. Then run it and read the
output against the code: every lesson prints what it does, step by step. The
guides here give the background that doesn't fit in a file header, and end
with exercises.

Timings only mean something in a Release build:

    python tools/check.py --release aos_vs_soa
    build/check/release/aos_vs_soa --time

or `cargo run --release --bin aos_vs_soa -- --time`.

Debug builds check every slice index and every integer for overflow, and
don't vectorize. Lesson 1's guide shows how to read the assembly, and what to
look for in it.
