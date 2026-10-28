# rust-dod

[rin-dod](https://github.com/IbrahimHindawi/rin-dod) in Rust: data-oriented
design, and an archetype entity component system built from nothing,
chapter by chapter. Work in progress: Part 1 of the ECS course is here, the
rest is coming.

Every chapter is one file that builds with plain `rustc` and prints what its
`.expected` file says:

    rustc --edition 2024 -g ecs/src/1.entities/1.1.ids/ids.rs -o ids && ./ids
    python tools/check.py
