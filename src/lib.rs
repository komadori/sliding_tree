#![no_std]
#![doc = include_str!("../README.md")]

pub use buffers::SlidingBuffers;
use core::{
    cell::Cell,
    fmt::{self, Debug, Formatter},
    mem, slice,
};

mod buffers;

/// A trait for types that have child nodes.
///
/// This trait primarily exists for documentation purposes. Consider calling
/// `children` before writing generic code over this trait.
pub trait HasChildren<'a, T> {
    /// Returns a reference to the children of this node.
    fn children(&self) -> &[Node<'a, T>];

    /// Returns true if this node has no children, false otherwise.
    fn is_empty(&self) -> bool;

    /// Returns the number of children of this node.
    fn len(&self) -> usize;

    /// Returns an iterator over references to the children of this node.
    fn iter(&self) -> slice::Iter<'_, Node<'a, T>>;

    /// Returns a reference to the child at the given index.
    fn at(&self, index: usize) -> &Node<'a, T>;
}

/// A trait for types that have mutable child nodes.
///
/// This trait primarily exists for documentation purposes. Consider calling
/// `children_mut` before writing generic code over this trait.
pub trait HasChildrenMut<'a, T>: HasChildren<'a, T> {
    /// Returns a mutable reference to the children of this node.
    fn children_mut(&mut self) -> NodeChildrenMut<'a, '_, T>;

    /// Sets the children of this node using the provided iterable.
    ///
    /// This function allocates new nodes for each item in the iterable and
    /// replaces the current children with these new nodes.
    fn set_children<I>(&mut self, iterable: I)
    where
        I: IntoIterator<Item = T>;

    /// Sets the children of this node using the provided iterable, allowing
    /// recursive construction of a subtree.
    ///
    /// This function allocates new nodes for each item in the iterable and
    /// replaces the current children with these new nodes. The `builder`
    /// function is called for each element during iteration, allowing further
    /// children to be created recursively.
    fn set_children_subtree<I, F, U>(&mut self, iterable: I, builder: F)
    where
        I: IntoIterator<Item = (T, U)>,
        F: FnMut(U, NodeMut<'a, '_, T>);

    /// Adopts the children of the node at the given index as the new children
    /// of this node.
    ///
    /// This replaces the current children of the node with the grandchildren.
    fn adopt_grandchildren_at(&mut self, index: usize);

    /// Moves the children of this node to the pending roots of the tree.
    fn set_pending_roots(&mut self);

    /// Returns an iterator over mutable references to the children of this node.
    fn iter_mut(&mut self) -> NodeIterMut<'a, '_, T>;

    /// Returns a mutable reference to the child at the given index.
    fn at_mut(&mut self, index: usize) -> NodeMut<'a, '_, T>;
}

/// A node in the tree, containing user data and child nodes.
#[derive(Debug)]
pub struct Node<'a, T> {
    data: T,
    children: &'a mut [Node<'a, T>],
}

/// A reference to a node in the tree.
impl<'a, T> Node<'a, T> {
    /// Returns a reference to the user data stored in this node.
    pub fn get(&self) -> &T {
        &self.data
    }
}

impl<'a, T> HasChildren<'a, T> for Node<'a, T> {
    fn children(&self) -> &[Node<'a, T>] {
        self.children
    }

    fn is_empty(&self) -> bool {
        self.children.is_empty()
    }

    fn len(&self) -> usize {
        self.children.len()
    }

    fn iter(&self) -> slice::Iter<'_, Node<'a, T>> {
        self.children.iter()
    }

    fn at(&self, index: usize) -> &Node<'a, T> {
        &self.children[index]
    }
}

/// A mutable reference to a node in the tree.
pub struct NodeMut<'a, 'b, T> {
    node: &'b mut Node<'a, T>,
    state: &'b SlidingTreeState<'a, T>,
}

impl<'a, 'b, T> NodeMut<'a, 'b, T> {
    /// Returns a reference to the user data stored in this node.
    pub fn get(&self) -> &T {
        &self.node.data
    }

    /// Returns a mutable reference to the user data stored in this node.
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.node.data
    }
}

impl<'a, T> HasChildren<'a, T> for NodeMut<'a, '_, T> {
    fn children(&self) -> &[Node<'a, T>] {
        self.node.children
    }

    fn is_empty(&self) -> bool {
        self.node.children.is_empty()
    }

    fn len(&self) -> usize {
        self.node.children.len()
    }

    fn iter(&self) -> slice::Iter<'_, Node<'a, T>> {
        self.node.children.iter()
    }

    fn at(&self, index: usize) -> &Node<'a, T> {
        &self.node.children[index]
    }
}

impl<'a, T> HasChildrenMut<'a, T> for NodeMut<'a, '_, T> {
    fn children_mut(&mut self) -> NodeChildrenMut<'a, '_, T> {
        NodeChildrenMut {
            children: &mut self.node.children,
            state: self.state,
        }
    }

    fn set_children<I>(&mut self, iterable: I)
    where
        I: IntoIterator<Item = T>,
    {
        self.node.children = self.state.alloc_iter(iterable);
    }

    fn set_children_subtree<I, F, U>(&mut self, iterable: I, builder: F)
    where
        I: IntoIterator<Item = (T, U)>,
        F: FnMut(U, NodeMut<'a, '_, T>),
    {
        self.node.children = self.state.alloc_iter_recursive(iterable, builder);
    }

    fn adopt_grandchildren_at(&mut self, index: usize) {
        let node = &mut self.node.children[index];
        self.node.children = mem::take(&mut node.children);
    }

    fn set_pending_roots(&mut self) {
        let children = mem::take(&mut self.node.children);
        self.state.pending_roots.set(Some(children));
    }

    fn iter_mut(&mut self) -> NodeIterMut<'a, '_, T> {
        NodeIterMut {
            iter: self.node.children.iter_mut(),
            state: self.state,
        }
    }

    fn at_mut(&mut self, index: usize) -> NodeMut<'a, '_, T> {
        NodeMut {
            node: &mut self.node.children[index],
            state: self.state,
        }
    }
}

impl<'a, T> AsRef<Node<'a, T>> for NodeMut<'a, '_, T> {
    fn as_ref(&self) -> &Node<'a, T> {
        self.node
    }
}

impl<'a, T> Debug for NodeMut<'a, '_, T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.node.fmt(f)
    }
}

/// A mutable reference to the children of a node.
pub struct NodeChildrenMut<'a, 'b, T> {
    children: &'b mut &'a mut [Node<'a, T>],
    state: &'b SlidingTreeState<'a, T>,
}

impl<'a, T> HasChildrenMut<'a, T> for NodeChildrenMut<'a, '_, T> {
    fn children_mut(&mut self) -> NodeChildrenMut<'a, '_, T> {
        NodeChildrenMut {
            children: self.children,
            state: self.state,
        }
    }

    fn set_children<I>(&mut self, iterable: I)
    where
        I: IntoIterator<Item = T>,
    {
        *self.children = self.state.alloc_iter(iterable);
    }

    fn set_children_subtree<I, F, U>(&mut self, iterable: I, builder: F)
    where
        I: IntoIterator<Item = (T, U)>,
        F: FnMut(U, NodeMut<'a, '_, T>),
    {
        *self.children = self.state.alloc_iter_recursive(iterable, builder);
    }

    fn adopt_grandchildren_at(&mut self, index: usize) {
        let node = &mut self.children[index];
        *self.children = mem::take(&mut node.children);
    }

    fn set_pending_roots(&mut self) {
        let children = mem::take(self.children);
        self.state.pending_roots.set(Some(children));
    }

    fn iter_mut(&mut self) -> NodeIterMut<'a, '_, T> {
        NodeIterMut {
            iter: self.children.iter_mut(),
            state: self.state,
        }
    }

    fn at_mut(&mut self, index: usize) -> NodeMut<'a, '_, T> {
        NodeMut {
            node: &mut self.children[index],
            state: self.state,
        }
    }
}

impl<'a, T> HasChildren<'a, T> for NodeChildrenMut<'a, '_, T> {
    fn children(&self) -> &[Node<'a, T>] {
        self.children
    }

    fn is_empty(&self) -> bool {
        self.children.is_empty()
    }

    fn len(&self) -> usize {
        self.children.len()
    }

    fn iter(&self) -> slice::Iter<'_, Node<'a, T>> {
        self.children.iter()
    }

    fn at(&self, index: usize) -> &Node<'a, T> {
        &self.children[index]
    }
}

/// An iterator over a slice of mutable node references.
pub struct NodeIterMut<'a, 'b, T> {
    iter: slice::IterMut<'b, Node<'a, T>>,
    state: &'b SlidingTreeState<'a, T>,
}

impl<'a, 'b, T> Iterator for NodeIterMut<'a, 'b, T> {
    type Item = NodeMut<'a, 'b, T>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|node| NodeMut {
            node,
            state: self.state,
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<'a, 'b, T> DoubleEndedIterator for NodeIterMut<'a, 'b, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.iter.next_back().map(|node| NodeMut {
            node,
            state: self.state,
        })
    }
}

impl<T> ExactSizeIterator for NodeIterMut<'_, '_, T> {}

struct SlidingTreeState<'a, T> {
    pending_roots: Cell<Option<&'a mut [Node<'a, T>]>>,
    buffers: SlidingBuffers<Node<'a, T>>,
}

impl<'a, T> SlidingTreeState<'a, T> {
    fn with_capacity(capacity: usize) -> SlidingTreeState<'a, T> {
        SlidingTreeState {
            pending_roots: Cell::new(None),
            buffers: SlidingBuffers::with_capacity(capacity),
        }
    }

    fn alloc_iter<I>(&self, iter: I) -> &'a mut [Node<'a, T>]
    where
        I: IntoIterator<Item = T>,
    {
        self.buffers.alloc_iter(iter.into_iter().map(|data| Node {
            data,
            children: &mut [],
        }))
    }

    fn alloc_iter_recursive<'b, I, F, U>(
        &'b self,
        iter: I,
        mut builder: F,
    ) -> &'a mut [Node<'a, T>]
    where
        I: IntoIterator<Item = (T, U)>,
        F: FnMut(U, NodeMut<'a, '_, T>),
    {
        self.buffers
            .alloc_iter(iter.into_iter().map(|(data, recursion)| {
                let mut node = Node {
                    data,
                    children: &mut [],
                };
                let node_mut = NodeMut {
                    node: &mut node,
                    state: self,
                };
                builder(recursion, node_mut);
                node
            }))
    }
}

/// A tree that grows from the leaves and recedes from the root.
///
/// The tree can be traversed via references from its roots and from nodes to
/// their children. New nodes can be added at the leaves of the tree and are
/// allocated efficiently using a [`crate::SlidingBuffers`]. Hence, the root
/// of the tree can be advanced through the tree making ancestor nodes
/// inaccessible and allowing their memory to be reused.
pub struct SlidingTree<'a, T> {
    roots: &'a mut [Node<'a, T>],
    state: SlidingTreeState<'a, T>,
}

impl<'a, T> SlidingTree<'a, T> {
    /// Creates a new empty `SlidingTree` with a default capacity based on
    /// the size of `T`.
    pub fn new() -> SlidingTree<'a, T> {
        SlidingTree::with_capacity(1000000 / size_of::<T>())
    }

    /// Creates a new empty `SlidingTree` with the specified capacity.
    ///
    /// The `capacity` is the maximum number of nodes that can be allocated
    /// in a single buffer.
    pub fn with_capacity(capacity: usize) -> SlidingTree<'a, T> {
        SlidingTree {
            roots: &mut [],
            state: SlidingTreeState::with_capacity(capacity),
        }
    }

    /// Preallocates recycled buffers.
    pub fn preallocate(&mut self, required: usize) {
        self.state.buffers.preallocate(required);
    }

    /// Clears the tree, removing the roots, all their descendants, and recycling all buffers.
    pub fn clear(&mut self) {
        self.roots = &mut [];
        self.state.pending_roots.set(None);
        // SAFETY: Once the roots have been cleared, previously allocated nodes
        // are inaccessible and can be recycled.
        unsafe {
            self.state.buffers.recycle_all();
        }
    }

    /// Clears any pending roots set with [`NodeMut::set_pending_roots`].
    ///
    /// This does not return the pending roots back to their original node.
    pub fn clear_pending_roots(&mut self) {
        self.state.pending_roots.set(None);
    }

    /// Adopts the pending roots as the new roots of the tree.
    ///
    /// This recycles older buffers containing nodes below the root to make
    /// space for new nodes. If there are no pending roots, this does nothing.
    pub fn update_roots(&mut self) {
        #[cfg(debug_assertions)]
        {
            fn sanity_check<'a, T>(
                src: &[Node<'a, T>],
                state: &SlidingTreeState<'a, T>,
            ) {
                for node in src.iter() {
                    state.buffers.assert_can_reference(src, node.children);
                    sanity_check(node.children, state);
                }
            }
            sanity_check(self.roots, &self.state);
        }
        if let Some(pending_roots) = self.state.pending_roots.take() {
            self.roots = pending_roots;
            if self.roots.is_empty() {
                self.clear();
            } else {
                // SAFETY: Once the roots have been updated, nodes allocated
                // before the new roots are inaccessible and can be recycled.
                unsafe {
                    self.state.buffers.recycle_older_than(self.roots);
                }
            }
        }
    }

    /// Frees unused buffers to reduce memory usage.
    pub fn trim(&mut self) {
        self.state.buffers.trim();
    }

    /// Returns the current buffer capacity.
    pub fn capacity(&self) -> usize {
        self.state.buffers.capacity()
    }

    /// Returns the number of buffers in the finished, current, and recycled states.
    pub fn buffer_stats(&self) -> (usize, usize, usize) {
        self.state.buffers.buffer_stats()
    }
}

impl<'a, T> Debug for SlidingTree<'a, T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("SlidingTree")
            .field("roots", &self.roots)
            .finish()
    }
}

impl<'a, T> Default for SlidingTree<'a, T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a, T> HasChildren<'a, T> for SlidingTree<'a, T> {
    fn children(&self) -> &[Node<'a, T>] {
        self.roots
    }

    fn is_empty(&self) -> bool {
        self.roots.is_empty()
    }

    fn len(&self) -> usize {
        self.roots.len()
    }

    fn iter(&self) -> slice::Iter<'_, Node<'a, T>> {
        self.roots.iter()
    }

    fn at(&self, index: usize) -> &Node<'a, T> {
        &self.roots[index]
    }
}

impl<'a, T> HasChildrenMut<'a, T> for SlidingTree<'a, T> {
    fn children_mut(&mut self) -> NodeChildrenMut<'a, '_, T> {
        NodeChildrenMut {
            children: &mut self.roots,
            state: &self.state,
        }
    }

    fn set_children<I>(&mut self, iterable: I)
    where
        I: IntoIterator<Item = T>,
    {
        self.clear();
        self.roots = self.state.alloc_iter(iterable);
    }

    fn set_children_subtree<I, F, U>(&mut self, iterable: I, builder: F)
    where
        I: IntoIterator<Item = (T, U)>,
        F: FnMut(U, NodeMut<'a, '_, T>),
    {
        self.clear();
        self.roots = self.state.alloc_iter_recursive(iterable, builder);
    }

    fn adopt_grandchildren_at(&mut self, index: usize) {
        let node = &mut self.roots[index];
        self.roots = mem::take(&mut node.children);
    }

    fn set_pending_roots(&mut self) {
        self.state
            .pending_roots
            .set(Some(mem::take(&mut self.roots)));
    }

    fn iter_mut(&mut self) -> NodeIterMut<'a, '_, T> {
        NodeIterMut {
            iter: self.roots.iter_mut(),
            state: &self.state,
        }
    }

    fn at_mut(&mut self, index: usize) -> NodeMut<'a, '_, T> {
        NodeMut {
            node: &mut self.roots[index],
            state: &self.state,
        }
    }
}
