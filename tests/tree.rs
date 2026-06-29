use std::ops::RangeBounds;
use std::panic::{self, AssertUnwindSafe};
use std::slice;

use sliding_tree::{
    HasChildren, HasChildrenMut, Node, NodeIterMut, SlidingTree,
};

mod common;
use common::{Counters, DropCounter, PanicAfter, PanicOnSizeHint};

struct HideSizeHint<I>(I);

impl<I: Iterator> Iterator for HideSizeHint<I> {
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}

fn deepen_tree<R>(iter: NodeIterMut<'_, '_, usize>, range: R)
where
    R: RangeBounds<usize> + Iterator<Item = usize> + Clone,
{
    for mut node in iter {
        if node.is_empty() {
            node.set_children(range.clone());
        } else {
            deepen_tree(node.iter_mut(), range.clone());
        }
    }
}

fn deepen_tree_no_hint<R>(iter: NodeIterMut<'_, '_, usize>, range: R)
where
    R: RangeBounds<usize> + Iterator<Item = usize> + Clone,
{
    for mut node in iter {
        if node.is_empty() {
            node.set_children(HideSizeHint(range.clone()));
        } else {
            deepen_tree(node.iter_mut(), range.clone());
        }
    }
}

fn count_nodes(node: slice::Iter<'_, Node<'_, usize>>) -> usize {
    let mut count = 0;
    for child in node {
        count += 1 + count_nodes(child.iter());
    }
    count
}

fn stats(tree: &SlidingTree<usize>) -> (usize, usize, usize, usize) {
    let (f, c, r) = tree.buffer_stats();
    (count_nodes(tree.iter()), f, c, r)
}

#[test]
fn test_empty() {
    let mut tree: SlidingTree<usize> = SlidingTree::new();
    assert!(tree.is_empty());
    assert_eq!(format!("{:?}", tree), "SlidingTree { roots: [] }");
    tree.preallocate(0);
    tree.recycle();
    tree.trim();
    tree.clear();
}

#[test]
fn test_root_data() {
    let mut tree: SlidingTree<usize> = SlidingTree::new();
    tree.set_children(0..10);
    assert!(!tree.is_empty());
    assert_eq!(tree.len(), 10);

    for (i, a) in tree.iter().enumerate() {
        assert_eq!(*tree.at(i).get(), i);
        assert_eq!(a.get(), &i);
    }

    for mut a in tree.iter_mut() {
        *a.get_mut() += 1;
    }

    for i in 0..tree.len() {
        *tree.at_mut(i).get_mut() += 1;
        assert_eq!(*tree.at(i).get(), i + 2);
    }
}

#[test]
fn test_node_data() {
    let mut tree: SlidingTree<usize> = SlidingTree::new();
    tree.set_children(0..1);
    let mut node = tree.at_mut(0);
    node.set_children(0..10);
    assert!(!node.is_empty());
    assert_eq!(node.len(), 10);

    for (i, a) in node.iter().enumerate() {
        assert_eq!(*node.at(i).get(), i);
        assert_eq!(*a.get(), i);
    }

    for mut a in node.iter_mut() {
        *a.get_mut() += 1;
    }

    for i in 0..node.len() {
        *node.at_mut(i).get_mut() += 1;
        assert_eq!(*node.at(i).get(), i + 2);
    }

    // Check immutable Node reference
    let node = node.as_ref();
    for (i, a) in node.iter().enumerate() {
        assert_eq!(*node.at(i).get(), i + 2);
        assert_eq!(*a.get(), i + 2);
    }
}

#[test]
fn test_grow_capacity_100() {
    let mut tree: SlidingTree<usize> = SlidingTree::with_capacity(100);
    assert_eq!(tree.buffer_stats(), (0, 0, 0));

    tree.set_children(0..10);
    assert_eq!(stats(&tree), (10, 0, 1, 0));
    deepen_tree(tree.iter_mut(), 0..10);
    assert_eq!(stats(&tree), (110, 1, 1, 0));
    deepen_tree(tree.iter_mut(), 0..10);
    assert_eq!(stats(&tree), (1110, 11, 1, 0));

    tree.clear();
    assert_eq!(stats(&tree), (0, 0, 0, 12));
}

#[test]
fn test_grow_capacity_49_with_hint() {
    let mut tree: SlidingTree<usize> = SlidingTree::with_capacity(49);
    assert_eq!(tree.buffer_stats(), (0, 0, 0));

    tree.set_children(0..10);
    assert_eq!(stats(&tree), (10, 0, 1, 0));
    deepen_tree(tree.iter_mut(), 0..10);
    assert_eq!(stats(&tree), (110, 2, 1, 0));
    deepen_tree(tree.iter_mut(), 0..10);
    assert_eq!(stats(&tree), (1110, 27, 1, 0));

    tree.clear();
    assert_eq!(stats(&tree), (0, 0, 0, 28));
}

#[test]
fn test_grow_capacity_49_without_hint() {
    let mut tree: SlidingTree<usize> = SlidingTree::with_capacity(49);
    assert_eq!(tree.buffer_stats(), (0, 0, 0));

    tree.set_children(HideSizeHint(0..10));
    assert_eq!(stats(&tree), (10, 0, 1, 0));
    deepen_tree_no_hint(tree.iter_mut(), 0..10);
    assert_eq!(stats(&tree), (110, 2, 1, 0));
    deepen_tree_no_hint(tree.iter_mut(), 0..10);
    assert_eq!(stats(&tree), (1110, 27, 1, 0));

    tree.clear();
    assert_eq!(stats(&tree), (0, 0, 0, 28));
}

#[test]
fn test_grow_capacity_10_and_trim() {
    let mut tree: SlidingTree<usize> = SlidingTree::with_capacity(10);
    tree.set_children(0..10);
    assert_eq!(stats(&tree), (10, 1, 0, 0));
    deepen_tree(tree.iter_mut(), 0..10);
    assert_eq!(stats(&tree), (110, 11, 0, 0));

    tree.iter_mut().last().unwrap().move_children_to_root();
    tree.recycle();
    assert_eq!(stats(&tree), (10, 1, 0, 10));
    tree.trim();
    assert_eq!(stats(&tree), (10, 1, 0, 0));

    tree.clear();
    assert_eq!(stats(&tree), (0, 0, 0, 1));
    tree.trim();
    assert_eq!(stats(&tree), (0, 0, 0, 0));
}

#[test]
fn test_capacity_too_small() {
    let mut tree: SlidingTree<usize> = SlidingTree::with_capacity(10);
    assert_eq!(tree.capacity(), 10);
    tree.set_children(0..100);
    assert_eq!(stats(&tree), (100, 1, 0, 0));
    assert_eq!(tree.capacity(), 100);
}

#[test]
fn test_follow_first_child() {
    let mut tree: SlidingTree<usize> = SlidingTree::with_capacity(100);
    tree.set_children(0..10);
    deepen_tree(tree.iter_mut(), 0..10);
    deepen_tree(tree.iter_mut(), 0..10);
    assert_eq!(stats(&tree), (1110, 11, 1, 0));

    tree.iter_mut().next().unwrap().move_children_to_root();
    tree.recycle();
    assert_eq!(stats(&tree), (110, 11, 1, 0));

    tree.iter_mut().next().unwrap().move_children_to_root();
    tree.recycle();
    assert_eq!(stats(&tree), (10, 10, 1, 1));

    tree.iter_mut().next().unwrap().move_children_to_root();
    tree.recycle();
    assert_eq!(stats(&tree), (0, 0, 0, 12));
}

#[test]
fn test_follow_last_child() {
    let mut tree: SlidingTree<usize> = SlidingTree::with_capacity(100);
    tree.set_children(0..10);
    deepen_tree(tree.iter_mut(), 0..10);
    deepen_tree(tree.iter_mut(), 0..10);
    assert_eq!(stats(&tree), (1110, 11, 1, 0));

    tree.iter_mut().next_back().unwrap().move_children_to_root();
    tree.recycle();
    assert_eq!(stats(&tree), (110, 10, 1, 1));

    tree.iter_mut().next_back().unwrap().move_children_to_root();
    tree.recycle();
    assert_eq!(stats(&tree), (10, 0, 1, 11));

    tree.iter_mut().next_back().unwrap().move_children_to_root();
    tree.recycle();
    assert_eq!(stats(&tree), (0, 0, 0, 12));
}

#[test]
fn test_two_subtrees() {
    // Build a 3 level subtree.
    let mut tree: SlidingTree<usize> = SlidingTree::with_capacity(1001);
    tree.set_children_subtree((0..10).map(|x| (x, ())), |mut node, _| {
        node.set_children_subtree((0..10).map(|x| (x, ())), |mut node, _| {
            node.set_children(0..10);
        });
    });
    assert_eq!(stats(&tree), (1110, 0, 3, 0));

    // Add another subtree to the first leaf.
    tree.at_mut(0).at_mut(0).at_mut(0).set_children_subtree(
        (0..10).map(|x| (x, ())),
        |mut node, _| {
            node.set_children_subtree(
                (0..10).map(|x| (x, ())),
                |mut node, _| {
                    node.set_children(0..20);
                },
            );
        },
    );
    assert_eq!(stats(&tree), (3220, 2, 3, 0));

    // Moving roots to the second subtree does not recycle the first subtree.
    tree.at_mut(0).at_mut(0).at_mut(0).move_children_to_root();
    tree.recycle();
    assert_eq!(stats(&tree), (2110, 2, 3, 0));

    // Moving roots further forward recycles both subtrees.
    let mut branch1 = tree.at_mut(0);
    let mut branch2 = branch1.at_mut(0);
    let mut leaf = branch2.at_mut(0);
    leaf.set_children(0..1000);
    leaf.move_children_to_root();
    tree.recycle();
    assert_eq!(stats(&tree), (1000, 0, 1, 5));
}

#[test]
fn test_subtree_overflow_with_hint() {
    // Create a 2 level tree.
    let mut tree: SlidingTree<usize> = SlidingTree::with_capacity(101);
    tree.set_children_subtree((0..2).map(|x| (x, ())), |mut node, _| {
        node.set_children(0..50);
    });
    assert_eq!(stats(&tree), (102, 0, 2, 0));

    // Adding 100 further children cannot fit in either current buffer.
    tree.at_mut(0).at_mut(0).set_children(0..100);
    assert_eq!(stats(&tree), (202, 2, 1, 0));

    // Remove first two levels and first generation is recycled.
    tree.at_mut(0).at_mut(0).move_children_to_root();
    tree.recycle();
    assert_eq!(stats(&tree), (100, 0, 1, 2));
}

#[test]
fn test_subtree_overflow_without_hint() {
    // Fill most of first buffer.
    let mut tree: SlidingTree<usize> = SlidingTree::with_capacity(101);
    tree.set_children(0..100);
    assert_eq!(stats(&tree), (100, 0, 1, 0));

    // Create a 2 level subtree which overflows the first buffer.
    tree.at_mut(0).set_children_subtree(
        HideSizeHint(0..2).map(|x| (x, ())),
        |mut node, _| {
            node.set_children(0..50);
        },
    );
    assert_eq!(stats(&tree), (202, 2, 1, 0));

    // Remove first level and only one buffer should be recycled.
    tree.at_mut(0).move_children_to_root();
    tree.recycle();
    assert_eq!(stats(&tree), (102, 1, 1, 1));
}

#[test]
fn test_subtree_no_overflow() {
    // Create a 2 level tree.
    let mut tree: SlidingTree<usize> = SlidingTree::with_capacity(101);
    tree.set_children_subtree((0..2).map(|x| (x, ())), |mut node, _| {
        node.set_children(0..49);
    });
    assert_eq!(stats(&tree), (100, 0, 2, 0));

    // Adding 2 further children fits in the smaller current buffer.
    tree.at_mut(0).at_mut(0).set_children(0..2);
    assert_eq!(stats(&tree), (102, 0, 2, 0));

    // Remove first two levels and nothing is recycled.
    tree.at_mut(0).at_mut(0).move_children_to_root();
    tree.recycle();
    assert_eq!(stats(&tree), (2, 0, 2, 0));
}

fn root_data(tree: &SlidingTree<usize>) -> Vec<usize> {
    tree.iter().map(|n| *n.get()).collect()
}

#[test]
fn test_panic_replacing_roots_preserves_old_roots() {
    let mut tree: SlidingTree<usize> = SlidingTree::with_capacity(100);
    tree.set_children(0..3);
    assert_eq!(root_data(&tree), [0, 1, 2]);
    assert_eq!(tree.buffer_stats(), (0, 1, 0));

    // Replace the roots with an iterator that panics part way through. The new
    // nodes are allocated into the same buffer that still holds the live roots,
    // so a panic must not free that buffer.
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        tree.set_children(PanicAfter::new(2));
    }));
    assert!(result.is_err());

    // The buffer holding the original roots must still be present...
    assert_eq!(tree.buffer_stats(), (0, 1, 0));
    // ...and the original roots must be intact and traversable.
    assert_eq!(root_data(&tree), [0, 1, 2]);

    // The tree must remain usable afterwards.
    tree.set_children(10..14);
    assert_eq!(root_data(&tree), [10, 11, 12, 13]);
}

#[test]
fn test_panic_setting_child_nodes_preserves_tree() {
    let mut tree: SlidingTree<usize> = SlidingTree::with_capacity(100);
    tree.set_children(0..3);

    // Setting the children of a leaf allocates into the buffer that holds its
    // ancestors (the roots). A panic must not free those ancestors.
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        tree.at_mut(1).set_children(PanicAfter::new(2));
    }));
    assert!(result.is_err());

    // The roots (the panicking node's siblings and itself) must be intact.
    assert_eq!(root_data(&tree), [0, 1, 2]);
    assert!(tree.at(1).is_empty());

    // The tree must remain usable afterwards.
    tree.at_mut(1).set_children(20..23);
    assert_eq!(
        tree.at(1).iter().map(|n| *n.get()).collect::<Vec<_>>(),
        [20, 21, 22]
    );
}

#[test]
fn test_panic_in_subtree_builder_preserves_tree() {
    let mut tree: SlidingTree<usize> = SlidingTree::with_capacity(1000);
    tree.set_children(0..3);

    // The builder recursively allocates children and then panics. Both the
    // outer slice's buffer and the roots' buffer must survive.
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        let mut count = 0;
        tree.at_mut(0).set_children_subtree(
            (0..5).map(|x| (x, ())),
            |mut node, _| {
                node.set_children(0..4);
                count += 1;
                if count == 3 {
                    panic!("boom");
                }
            },
        );
    }));
    assert!(result.is_err());

    // The roots must be intact and the tree must remain usable.
    assert_eq!(root_data(&tree), [0, 1, 2]);
}

#[test]
fn test_panic_does_not_leak_or_double_free_node_data() {
    let counters = Counters::new();
    {
        let mut tree: SlidingTree<DropCounter> =
            SlidingTree::with_capacity(100);
        tree.set_children((0..3).map(|_| DropCounter::new(&counters)));

        // Panic partway through a recursive build that constructs many nodes.
        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            let mut built = 0;
            tree.at_mut(0).set_children_subtree(
                (0..5).map(|_| (DropCounter::new(&counters), ())),
                |mut node, _| {
                    node.set_children(
                        (0..4).map(|_| DropCounter::new(&counters)),
                    );
                    built += 1;
                    if built == 3 {
                        panic!("boom");
                    }
                },
            );
        }));
        assert!(result.is_err());

        // Nodes orphaned by the panic are still owned by the buffers, not
        // dropped early.
        assert!(counters.constructed() > counters.dropped());

        // The tree must remain usable afterwards.
        assert_eq!(tree.len(), 3);
        tree.at_mut(1)
            .set_children((0..4).map(|_| DropCounter::new(&counters)));
        assert_eq!(tree.at(1).len(), 4);
    }

    // Dropping the tree must drop every constructed node exactly once; a
    // mismatch means a leak or a double-free.
    assert!(
        counters.balanced(),
        "leak or double-free across panic: constructed {}, dropped {}",
        counters.constructed(),
        counters.dropped(),
    );
}

#[test]
fn test_panic_spanning_buffers_preserves_tree() {
    // Small capacity so the leaf's slice overflows the roots' buffer into a
    // second one before the panic.
    let mut tree: SlidingTree<usize> = SlidingTree::with_capacity(10);
    tree.set_children(0..3);
    assert_eq!(root_data(&tree), [0, 1, 2]);
    assert_eq!(tree.buffer_stats(), (0, 1, 0));

    // Fills the roots' buffer, spilling the rest into a fresh buffer that is in
    // flight when the iterator panics.
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        tree.at_mut(0).set_children(PanicAfter::new(9));
    }));
    assert!(result.is_err());

    // Roots' buffer is now finished, the spilled buffer is back as current, and
    // the roots survived.
    assert_eq!(tree.buffer_stats(), (1, 1, 0));
    assert_eq!(root_data(&tree), [0, 1, 2]);
    assert!(tree.at(0).is_empty());

    // The tree must remain usable afterwards.
    tree.at_mut(0).set_children(20..24);
    assert_eq!(
        tree.at(0).iter().map(|n| *n.get()).collect::<Vec<_>>(),
        [20, 21, 22, 23]
    );
    assert_eq!(root_data(&tree), [0, 1, 2]);
}

#[test]
fn test_panic_in_size_hint_preserves_tree() {
    let mut tree: SlidingTree<usize> = SlidingTree::with_capacity(100);
    tree.set_children(0..3);

    // The panic comes from `size_hint`, not `next`, while a buffer is in flight.
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        tree.at_mut(1).set_children(PanicOnSizeHint::new(3));
    }));
    assert!(result.is_err());

    assert_eq!(tree.buffer_stats(), (0, 1, 0));
    assert_eq!(root_data(&tree), [0, 1, 2]);
    assert!(tree.at(1).is_empty());

    // The tree must remain usable afterwards.
    tree.at_mut(1).set_children(30..33);
    assert_eq!(
        tree.at(1).iter().map(|n| *n.get()).collect::<Vec<_>>(),
        [30, 31, 32]
    );
}
