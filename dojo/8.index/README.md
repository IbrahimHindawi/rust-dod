# 8. Index, not pointer

`dojo/8.index/play_index.rs`

A linked list whose nodes live side by side in one Vec, and link to each other
by **index**: node 1's `next` is `Some(0)`, a position in the Vec, not an
address. When the Vec is full and grows, it moves: the nodes are copied to a
bigger place and the old one is freed. Every index is still right after the
move, because an index says where a node is *in the Vec*, wherever the Vec is.
A reference says where the node is in memory, so it would point at the old
place.

That's the whole idea, and it's everywhere in data-oriented code: an ECS
keeps tables by index because making a table can move the others, a mesh
keeps triangles as three vertex indices, a scene graph keeps a parent index.
A freed node goes on a free list and is used again before the Vec grows, as
in dojo 1.

**The invariant:** walking `next` from the head visits every live node once
and ends at the tail; each node's `prev` is the node before it; and a node's
index names it from the moment it's made until it's removed, however many
times the Vec moves in between.

In the course: [2.4](../../ecs/docs/2.4_archetypes.md) keeps tables by index,
because making a table can grow the `tables` Vec and move every table. It's
the same reason [2.2](../../ecs/docs/2.2_columns.md)'s columns can't be held
by reference across a grow, and [2.5](../../ecs/docs/2.5_typed_access.md)
says to keep handles and borrow references only to use them.

## Watch it: the playground

    cargo run --bin play_index
    python tools/dojo.py play index

Memory in the playground is an array of 16 cells, small enough to see whole
in a debugger, and the list takes 2 cells of it, then 4, then 8. It keeps node
`b` two ways: by its index, and by the cell it's in, which is the number a
pointer would hold. Then the list fills up and moves, and the program writes
to `b`:

```
push_back 30 as c: the list is full
    the list moves from cells 0..2 to cells 2..6
    c is node 2
...
write 21 into b through its index, 1: cell 3 now
    nodes    0: 10, prev -, next 1 | 1: 21, prev 0, next 2 | 2: 30, prev 1, next -
    list     10 21 30
    memory   0..2 old copy: 10 20 | 2..6 the list: 10 21 30 .
```

After every step `verify()` walks the links and checks that every node holds
what the program last wrote to it. Put a breakpoint on `broken`, which it
calls when something is wrong.

Then break on the line that writes 21 into `b`, set `USE_INDEX` to `false` in
the watch window, and continue. The write goes through the kept cell, into
the old copy, and the list never sees it:

    write 21 into b through what was kept, cell 1: where b was before the move
        ...
        memory   0..2 old copy: 10 21 | 2..6 the list: 10 20 30 .
        verify: node 1 holds 20, but the program last wrote 21 to it

The program keeps a cell number rather than a reference because Rust won't
let it keep the reference:

```rust ignore
    let kept = l.get_mut(b).expect("b is in the list");
    l.push_back(30);
    *kept = 21;
```

```
error[E0499]: cannot borrow `l` as mutable more than once at a time
    |
141 |     let kept = l.get_mut(b).expect("b is in the list");
    |                - first mutable borrow occurs here
142 |     l.push_back(30);
    |     ^ second mutable borrow occurs here
143 |     *kept = 21;
    |     ---------- first borrow later used here
```

A number, though, crosses anything, and a number that means *a place in
memory* goes stale the same way a pointer does. rin-dod's playground keeps a
real pointer, writes through it, and loses the write in just the same way.

## Do reps: the kata

You write `List`. The idea is in `remove`: the two neighbours link to each
other, past the node, and the node's index goes on the free list:

```rust
    pub fn remove(&mut self, node: u32) -> Option<u64> {
        let value = self.get(node)?;
        let Node { prev, next, .. } = self.nodes[node as usize];
        match prev {
            Some(prev) => self.nodes[prev as usize].next = next,
            None => self.head = next,
        }
        match next {
            Some(next) => self.nodes[next as usize].prev = prev,
            None => self.tail = prev,
        }
```

and the API around it:

```rust
    pub fn push_back(&mut self, value: u64) -> u32 {
```

```rust
    pub fn insert_after(&mut self, node: u32, value: u64) -> Option<u32> {
```

plus `new()`, `push_front`, `get`, `get_mut`, `len()`, and the links to walk
the list by: `head()`, `tail()`, `next(node)` and `prev(node)`, each an
`Option<u32>`.

| level | you write | target |
|---|---|---|
| 1 | `remove`, in an otherwise finished file | 3:00 |
| 2 | every function body; the fields and signatures are given | 12:00 |
| 3 | the whole file, from a blank page | 15:00 |

    python tools/dojo.py start index 2
    python tools/dojo.py index

The harness runs the playground's steps as a scenario, then 20,000 random
pushes, inserts, removes, reads and writes against a model that keeps the
live nodes in list order. After every operation it walks your list both ways,
`head()` to the end by `next`, and `tail()` back by `prev`, so a link left
wrong is caught by the operation that left it. The list grows to about a
thousand nodes, moving many times, and every node is checked by the index it
was given. A freed index must be handed out again before a new one is made.

**Level 4, variations.** The same harness, different insides:

- **The free list through the nodes**: a freed node's `next` is the next
  free node, and the list keeps `first_free: Option<u32>`. No `free` Vec.
- **An enum node**: `Live { value, prev, next }` or `Free { next_free }`, so
  `alive` goes away and a node can't be half of each.
- **`iter()`**: an iterator over the values, head to tail, holding a
  `&List` and the next index. (Not checked by the harness; write your own
  test.)

## When it fails

    FAIL at operation 17 (seed 1): walking head to tail, next(3) gives 2, but 3 is the last of 3 nodes
        debug it: build/dojo/index/harness --seed 1 --stop 17

A link pointing somewhere it shouldn't is usually a node from the free list
that kept its old links, or a neighbour that wasn't told about a change: an
insert at the tail that didn't move the tail, a remove that fixed `next` but
not `prev`. Run the harness under your debugger with those arguments and a
breakpoint on `stop_here`, which runs just before that operation, and look at
the nodes the message names.

An index alone can't tell a removed node from the new node that took its
place: `a` is node 0, and so is `e` after it. That's dojo 1's problem, and its
answer, a generation beside the index, works here unchanged.
