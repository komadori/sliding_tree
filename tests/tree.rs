use std::{ops::RangeBounds, slice};

use sliding_tree::{Node, NodeIterMut, SlidingTree};

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
    tree.clear_pending_roots();
    tree.update_roots();
    tree.trim();
    tree.clear();
}

#[test]
fn test_root_data() {
    let mut tree: SlidingTree<usize> = SlidingTree::new();
    tree.set_roots(0..10);
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
    tree.set_roots(0..1);
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

    tree.set_roots(0..10);
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

    tree.set_roots(0..10);
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

    tree.set_roots(HideSizeHint(0..10));
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
    tree.set_roots(0..10);
    assert_eq!(stats(&tree), (10, 1, 0, 0));
    deepen_tree(tree.iter_mut(), 0..10);
    assert_eq!(stats(&tree), (110, 11, 0, 0));

    tree.iter_mut().last().unwrap().set_pending_roots();
    tree.update_roots();
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
    tree.set_roots(0..100);
    assert_eq!(stats(&tree), (100, 1, 0, 0));
    assert_eq!(tree.capacity(), 100);
}

#[test]
fn test_follow_first_child() {
    let mut tree: SlidingTree<usize> = SlidingTree::with_capacity(100);
    tree.set_roots(0..10);
    deepen_tree(tree.iter_mut(), 0..10);
    deepen_tree(tree.iter_mut(), 0..10);
    assert_eq!(stats(&tree), (1110, 11, 1, 0));

    tree.iter_mut().next().unwrap().set_pending_roots();
    tree.update_roots();
    assert_eq!(stats(&tree), (110, 11, 1, 0));

    tree.iter_mut().next().unwrap().set_pending_roots();
    tree.update_roots();
    assert_eq!(stats(&tree), (10, 10, 1, 1));

    tree.iter_mut().next().unwrap().set_pending_roots();
    tree.update_roots();
    assert_eq!(stats(&tree), (0, 0, 0, 12));
}

#[test]
fn test_follow_last_child() {
    let mut tree: SlidingTree<usize> = SlidingTree::with_capacity(100);
    tree.set_roots(0..10);
    deepen_tree(tree.iter_mut(), 0..10);
    deepen_tree(tree.iter_mut(), 0..10);
    assert_eq!(stats(&tree), (1110, 11, 1, 0));

    tree.iter_mut().next_back().unwrap().set_pending_roots();
    tree.update_roots();
    assert_eq!(stats(&tree), (110, 10, 1, 1));

    tree.iter_mut().next_back().unwrap().set_pending_roots();
    tree.update_roots();
    assert_eq!(stats(&tree), (10, 0, 1, 11));

    tree.iter_mut().next_back().unwrap().set_pending_roots();
    tree.update_roots();
    assert_eq!(stats(&tree), (0, 0, 0, 12));
}

#[test]
fn test_two_subtrees() {
    // Build a 3 level subtree.
    let mut tree: SlidingTree<usize> = SlidingTree::with_capacity(1001);
    tree.set_roots_subtree((0..10).map(|x| (x, ())), |_, mut node| {
        node.set_children_subtree((0..10).map(|x| (x, ())), |_, mut node| {
            node.set_children(0..10);
        });
    });
    assert_eq!(stats(&tree), (1110, 0, 3, 0));

    // Add another subtree to the first leaf.
    tree.at_mut(0).at_mut(0).at_mut(0).set_children_subtree(
        (0..10).map(|x| (x, ())),
        |_, mut node| {
            node.set_children_subtree(
                (0..10).map(|x| (x, ())),
                |_, mut node| {
                    node.set_children(0..10);
                },
            );
        },
    );
    assert_eq!(stats(&tree), (2220, 3, 2, 0));

    // Moving roots to the second subtree does not recycle the first subtree.
    tree.at_mut(0).at_mut(0).at_mut(0).set_pending_roots();
    tree.update_roots();
    assert_eq!(stats(&tree), (1110, 3, 2, 0));

    // Moving roots further forward does recycle the first subtree.
    let mut branch1 = tree.at_mut(0);
    let mut branch2 = branch1.at_mut(0);
    let mut leaf = branch2.at_mut(0);
    leaf.set_children(0..1);
    leaf.set_pending_roots();
    tree.update_roots();
    assert_eq!(stats(&tree), (1, 0, 2, 3));
}

#[test]
fn test_subtree_overflow() {
    // Create a 2 level tree.
    let mut tree: SlidingTree<usize> = SlidingTree::with_capacity(101);
    tree.set_roots_subtree((0..2).map(|x| (x, ())), |_, mut node| {
        node.set_children(0..50);
    });
    assert_eq!(stats(&tree), (102, 0, 2, 0));

    // Adding 100 further children cannot fit in either current buffer.
    tree.at_mut(0).at_mut(0).set_children(0..100);
    assert_eq!(stats(&tree), (202, 2, 1, 0));

    // Remove first two levels and first generation is recycled.
    tree.at_mut(0).at_mut(0).set_pending_roots();
    tree.update_roots();
    assert_eq!(stats(&tree), (100, 0, 1, 2));
}

#[test]
fn test_subtree_no_overflow() {
    // Create a 2 level tree.
    let mut tree: SlidingTree<usize> = SlidingTree::with_capacity(101);
    tree.set_roots_subtree((0..2).map(|x| (x, ())), |_, mut node| {
        node.set_children(0..50);
    });
    assert_eq!(stats(&tree), (102, 0, 2, 0));

    // Adding 99 further children fits in the first current buffer.
    tree.at_mut(0).at_mut(0).set_children(0..99);
    assert_eq!(stats(&tree), (201, 1, 1, 0));

    // Remove first two levels and nothing is recycled.
    tree.at_mut(0).at_mut(0).set_pending_roots();
    tree.update_roots();
    assert_eq!(stats(&tree), (99, 1, 1, 0));
}
