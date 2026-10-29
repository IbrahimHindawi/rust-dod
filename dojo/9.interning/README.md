# 9. Interning

`dojo/9.interning/play_interning.rs`

An interner gives each distinct key a small number, an **id**: the next one,
0, 1, 2, the first time it sees the key, and the same one every time after.
Find it, or make it. The ids are dense, so they index plain Vecs: the key of
id 3 is `keys[3]`, and whatever else belongs to that key can live at index 3
of its own Vec.

Behind it is a hash table, written here by hand: a row of slots, each empty
or holding an id. A key's hash says which slot to look in first. If that one
holds another key, look in the next one, and the next, wrapping round at the
end, until the key or an empty slot turns up. This is **linear probing**. A
full table would make those runs long, and an absent key's run endless, so
the table doubles before it's more than 3/4 full, and every id is put back
where its key's hash says in the bigger table: a different slot, since the
hash now picks from twice as many.

**The invariant:** every id is in exactly one slot, and a walk from its key's
hash reaches that slot before it reaches an empty one; the table is never
more than 3/4 full; and ids are 0 to `len() - 1`, in the order their keys
first came.

In the course: [2.4](../../ecs/docs/2.4_archetypes.md)'s `table_for` is an
interner. Its keys are signatures and its ids are table indices, with std's
`HashMap` doing the lookup. Here you write the lookup too.
[2.6](../../ecs/docs/2.6_add_remove.md) then makes it rare, by remembering
where each add and remove leads, and dojo 10 practises that.

## Watch it: the playground

    cargo run --bin play_interning
    python tools/dojo.py play interning

It interns the signatures of a few kinds of thing in a game, in a table of 8
slots that grows to 16, and prints every slot it looks at:

```
intern rock, signature 7
    slots looked at: 4 5 6; new: id 3, in slot 6
    table    1:turret 4:drone 5:player 6:rock, 4 of 8 slots used
...
intern bird, signature 15
    slots looked at: 1 2; not there
    7 ids would fill more than 3/4 of 8 slots: the table grows to 16, and every id
    goes where its key's hash says now
    slots looked at: 9 10; new: id 6, in slot 10
    table    4:rock 5:player 7:chest 8:tank 9:turret 10:bird 12:drone, 7 of 16 slots used
```

After every step `verify()` checks the invariant; put a breakpoint on
`broken`, which it calls when something is wrong.

Then break on the first `intern`, set `MIX_HASH` to `false` in the watch
window, and continue. The hash is now the key itself, so the first slot is the
key's lowest bits. Those are the components nearly every kind has, position,
sprite and health, since they were made first, so every kind starts at the
same slot and the runs pile up:

    intern chest, signature 135
        slots looked at: 7 0 1 2 3; new: id 4, in slot 3
    ...
    slots looked at, in all: 41

against 17 with the hash. Nothing is wrong, only slow, so verify stays
quiet. Flip it later instead, with ids already in the table, and verify
complains straight away: the ids are where the old hash put them, and the new
hash looks somewhere else. A table's hash can't change under it.

## Do reps: the kata

You write `Interner`, its table included. The idea is in `intern`:

```rust
    pub fn intern(&mut self, key: u64) -> u32 {
        let mut at = self.slot_of(key);
        if self.slots[at] != 0 {
            return self.slots[at] - 1;
        }
        if (self.keys.len() + 1) * 4 > self.slots.len() * 3 {
            self.grow();
            at = self.slot_of(key);
        }
```

and the API around it:

```rust
    pub fn find(&self, key: u64) -> Option<u32> {
```

```rust
    pub fn key(&self, id: u32) -> u64 {
```

plus `new()` and `len()`. A slot holds `id + 1`, so that 0 can mean empty
and key 0 is a key like any other.

| level | you write | target |
|---|---|---|
| 1 | `intern`, in an otherwise finished file | 3:00 |
| 2 | every function body; the fields and signatures are given | 10:00 |
| 3 | the whole file, from a blank page | 12:00 |

    python tools/dojo.py start interning 2
    python tools/dojo.py interning

The harness runs the playground's steps as a scenario, then 20,000 random
interns and finds against a model, std's `HashMap` and a Vec of the keys in
order. The keys are the kinds an ECS's tables meet: small numbers, 0 among
them; signatures with a few bits set; keys that differ only in their high
bits; and any u64 at all. Ids must be dense, in the order keys were first
interned, and never change; `find` must agree with `intern`; and `key(id)`
must give the key back. It checks what the interner does, not how fast: an
interner with no mixing in its hash passes, and the playground shows what it
costs.

**Level 4, variations.** The same harness, different insides:

- **The hash beside the id**: slots of `(u32, u32)`, the id and the low bits
  of its key's hash, so a run compares hashes and reads `keys` only when they
  match, and `grow` never hashes a key again.
- **Robin Hood**: on the way along a run, an id that is closer to its home
  slot than the new one gives its slot up and moves on instead. The longest
  run gets much shorter. How does `find` stop early on an absent key?
- **Grow at 1/2**: what happens to the slots looked at in the playground, and
  to the memory?

## When it fails

    FAIL in the scenario: find turret gave None, it should give 2

A key that was interned and can't be found usually means `grow` copied the
slots across instead of putting each id back by its key's hash: in a table
twice the size, a key's run starts somewhere else. If the run stops and never
ends, the table filled up: a lookup for a key that isn't there walks round
and round, never meeting an empty slot to stop at. Break into it in your
debugger and look at `slots`.

In the fuzz run, a failure says which operation and seed, as in the other
blocks; run the harness under your debugger with the `--seed` and `--stop` it
gives, and a breakpoint on `stop_here`.
