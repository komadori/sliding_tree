Sliding Tree
============

This crate is a Rust library which provides a sliding tree structure that grows from the leaves and recedes from the root. It is intended to be suitable for implementing game tree search, where new leaves are repeatedly added by the search algorithm and then the root pointer is advanced as moves are taken.

It uses a queue of allocation buffers under the hood to manage memory. This provides low-cost high-locality arena allocation while allowing older buffers to be safely recycled once their contents are unreachable below the root of the tree.

## Dependency

```toml
[dependencies]
sliding_tree = "0.3"
```

## Growing the Tree

A `SlidingTree` is created empty and grown by attaching groups of children onto it. The `HasChildrenMut` and `HasChildren` traits provide methods for attaching and accessing child nodes. These traits are implemented both by the tree itself for working with the roots, and also by the smart references `Node` and `NodeMut` provided for traversing the nodes.

```rust
use sliding_tree::{HasChildren, HasChildrenMut, SlidingTree};

let mut tree: SlidingTree<i32> = SlidingTree::new();
tree.set_children([1, 2, 3]);
assert_eq!(tree.len(), 3);

// Set and access grandchildren
tree.at_mut(1).set_children([40, 50]);
assert_eq!(tree.at(0).len(), 0);
let at_1 = tree.at(1);
assert_eq!(at_1.len(), 2);
assert_eq!(*at_1.at(0).get(), 40);
```

Child nodes can be accessed via index with `at` or iterated over with `iter`, or with their mutable counterparts `at_mut` and `iter_mut`. Each node stores a payload value `T` which can be accessed with `get` or modified with `get_mut`.

Child nodes are created as groups of siblings with `set_children` given their payloads. Alternatively, an entire subtree can be created at once with `set_children_subtree` and a closure to populate the children. 

```rust
# use sliding_tree::{HasChildren, HasChildrenMut, SlidingTree};
let mut tree: SlidingTree<i32> = SlidingTree::new();
tree.set_children_subtree([(1, ()), (2, ())], |mut node, _meta| {
    node.set_children([10, 20]);
});
assert_eq!(*tree.at(0).at(0).get(), 10);
assert_eq!(*tree.at(1).at(0).get(), 10);
```

## Sliding the Root

Calling `move_children_to_root` on a node promotes its children to be the new roots of the tree. Its former siblings, ancestors, and their subtrees thereby become unreachable.

```rust
# use sliding_tree::{HasChildren, HasChildrenMut, SlidingTree};
let mut tree: SlidingTree<i32> = SlidingTree::new();
tree.set_children([1, 2, 3]);
tree.at_mut(0).set_children([10, 11]);

// Advance the root onto node 0's children, discarding nodes 1 and 2.
tree.at_mut(0).move_children_to_root();
assert_eq!(tree.len(), 2);
assert_eq!(*tree.at(0).get(), 10);

// Reclaim the memory of the discarded branches so it can be reused.
tree.recycle();
```

Once the root has been advanced, `recycle` prepares any buffers that no longer hold reachable nodes to be reused for subsequent growth.

An example demonstrating how to use the crate to implement Monte Carlo Tree Search for a simple game is provided in `tests/mcts.rs`.

## Advanced

The underlying arena, `SlidingBuffers`, is exposed for advanced use. It provides unsafe methods to free buffers under the invariant that an allocation may reference later allocations but never earlier ones. `SlidingTree` is a safe abstraction built on top of it.

## Licence

This crate is licensed under the Apache License, Version 2.0 (see
LICENCE-APACHE or <http://www.apache.org/licenses/LICENSE-2.0>) or the MIT
licence (see LICENCE-MIT or <http://opensource.org/licenses/MIT>), at your
option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
