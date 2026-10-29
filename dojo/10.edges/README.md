# 10. Memoized edges

`dojo/10.edges/play_edges.rs`

In an archetype ECS, an entity that gains a component moves to the table
whose signature has one more bit. Finding that table by its signature is a
search. But a game moves entities between the same few tables over and over:
a drone catches fire, the fire goes out, it catches fire again. So each table
**remembers** where each component leads, once a move has found it:

    node 3 {position velocity health}  --+burning-->  node 4
    node 4 {position velocity health burning}  --burning-->  node 3

The first move from 3 with `burning` searches, and writes the answer on
**both** nodes: an add edge one way and a remove edge back. Every move after
that, in either direction, follows an edge. Remembering the answer to a slow
question is memoizing, and the nodes and their edges are the **archetype
graph** that flecs is built around.

**The invariant:** every known edge leads to the node one component away,
and that node has the edge back. Each move that changes a signature searches
at most once; after that, both directions are edges.

In the course: [2.6](../../ecs/docs/2.6_add_remove.md) adds `add_edges` and
`remove_edges` to each table, and moves entities along them.

## Watch it: the playground

    cargo run --bin play_edges
    python tools/dojo.py play edges

A drone gains and loses components, eight nodes at most. Each move says
where it went and how, and the graph is drawn twice:

```
catches fire: node 3 -> 4 {position velocity health burning}, by a lookup
the fire goes out: node 4 -> 3 {position velocity health}, by an edge
catches fire again: node 3 -> 4 {position velocity health burning}, by an edge
...
    node 3 {position velocity health}: +burning 4, -velocity 5, -health 2
...
moves that looked up a node by signature: 7; that followed an edge: 4
```

Step through it: break on `with` and watch the two edges get written after
`find_or_make`. `verify()` runs after every move and prints any edge that
leads to the wrong node or has no edge back. Then flip the switch at the
top, `MEMO`, to `false` and run it again. The drone goes to the same nodes,
but nothing is remembered: eleven moves, eleven lookups, and every node
drawn with `no edges known`. With a few hundred tables and a game moving
thousands of entities a frame, that's the difference.

## Do reps: the kata

You write `Graph`. The move that adds a component:

```rust
    pub fn with(&mut self, node: u32, id: u32) -> u32 {
        let signature = self.signature(node);
        if signature & bit(id) != 0 {
            return node;
        }
        if let Some(to) = self.nodes[node as usize].add[id as usize] {
            self.hits += 1;
            return to;
        }
        let to = self.find_or_make(signature | bit(id));
        self.nodes[node as usize].add[id as usize] = Some(to);
        self.nodes[to as usize].remove[id as usize] = Some(node);
        to
    }
```

and its mirror, `without`, plus `new()` (node 0 is the empty signature),
`signature(node)`, `len()`, `lookups()` (moves that searched) and `hits()`
(moves that followed an edge). The edges are `Option<u32>`, 64 per node each
way:

```rust
struct Node {
    signature: u64,
    // add[id] is the node with component id added, once a move has looked it
    // up; remove[id] is the node with it removed.
    add: [Option<u32>; 64],
    remove: [Option<u32>; 64],
}
```

| level | you write | target |
|---|---|---|
| 1 | `with`, in an otherwise finished file | 3:00 |
| 2 | every function body; the fields and signatures are given | 10:00 |
| 3 | the whole file, from a blank page | 12:00 |

    python tools/dojo.py start edges 2
    python tools/dojo.py edges

The harness runs the playground's moves as a scenario, then 20,000 random
moves from random nodes, with ids up to 63, against a model that knows each
node's signature and which edges a graph that writes both directions knows
by now. After every move it checks the node, its signature, and both
counters: a known edge is a hit, any other move that changes the signature is
one lookup, and a move that changes nothing (adding what the node has) is
neither.

**Level 4, variations.** The same harness, different insides:

- **One hash map of edges**, keyed by `(node, id, adding)`, instead of two
  64-wide arrays per node. Which takes less memory with 300 tables, and which
  is faster to follow?
- **A hash map for the slow path**: `find_or_make` searches every node. Keep
  a `HashMap<u64, u32>` from signature to node, as
  [2.4](../../ecs/docs/2.4_archetypes.md) does, or build one in dojo 9.
- **Removing a node.** An ECS that deletes empty tables must forget every
  edge into them. Which nodes have edges into node `n`? (Not checked by the
  harness; write your own test.)

## When it fails

    FAIL in the scenario: lookups and hits gave 5 1, it should give 4 2

Five lookups where the model counts four: the fire going out searched,
instead of following the edge back that catching fire should have written.
The edge back is the one people forget, or write on the wrong node: the move
from 3 to 4 sets node 3's add edge and node 4's remove edge.

The scenario's ids stop at 3. The fuzz run's go up to 63, and a failure there
says which operation and seed:

    FAIL at operation 1 (seed 1): it panicked, in without(0, 63)
        debug it: build/dojo/edges/harness --seed 1 --stop 1
        panicked at .../dojo/10.edges/kata.rs:15:5:
    attempt to shift left with overflow

That's a bit made in 32 bits, `(1u32 << id) as u64`. Run the harness under
your debugger with those arguments and a breakpoint on `stop_here`, which
runs just before that operation, and step into the move.
