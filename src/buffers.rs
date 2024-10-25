use core::{
    cell::{RefCell, RefMut},
    cmp,
    ops::Range,
    slice,
};

extern crate alloc;
use alloc::collections::VecDeque;
use alloc::vec::Vec;
use smallvec::SmallVec;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Generation(usize);

impl Generation {
    pub const INVALID: Generation = Generation(usize::MAX);

    fn advance(&mut self) {
        self.0 += 1;
    }

    fn is_older_than(&self, other: Generation) -> bool {
        self.0 < other.0
    }
}

struct Buffer<T> {
    vec: Vec<T>,
    ptr_range: Range<*const T>,
    generation: Generation,
}

impl<T> Buffer<T> {
    fn new(capacity: usize, generation: Generation) -> Buffer<T> {
        let vec = Vec::<T>::with_capacity(capacity);
        let start = vec.as_ptr();
        // SAFETY: The pointer `end` is never dereferenced and the memory
        // between `start` and `end` is part of the same allocation.
        let end = unsafe { start.add(vec.capacity()) };
        Buffer {
            vec,
            ptr_range: start..end,
            generation,
        }
    }

    fn clear(&mut self) {
        self.vec.clear();
        self.generation = Generation::INVALID;
    }

    #[inline]
    fn is_full(&self) -> bool {
        self.vec.len() == self.vec.capacity()
    }

    #[inline]
    fn contains(&self, ptr: *const T) -> bool {
        self.ptr_range.contains(&ptr)
    }
}

struct SlidingBuffersState<T> {
    capacity: usize,
    finished: VecDeque<Buffer<T>>,
    current: SmallVec<[Buffer<T>; 4]>,
    recycle: Vec<Buffer<T>>,
    current_generation: Generation,
    should_advance: bool,
}

impl<T> SlidingBuffersState<T> {
    fn take_current_buffer(&mut self, required: usize) -> Buffer<T> {
        if required > self.capacity {
            // Increase the size of future buffers. Older smaller buffers
            // will eventually be freed rather than recycled.
            self.capacity = cmp::max(required, self.capacity * 2);
        }
        loop {
            let buf = match self.current.pop() {
                Some(buf) => {
                    if buf.vec.capacity() - buf.vec.len() < required {
                        // The buffer is too small.
                        self.should_advance = true;
                        self.finished.push_back(buf);
                        continue;
                    }
                    buf
                }
                None => {
                    if self.should_advance {
                        self.current_generation.advance();
                        self.should_advance = false;
                    }
                    match self.recycle.pop() {
                        Some(mut buf) => {
                            // Use a recycled buffer.
                            buf.generation = self.current_generation;
                            buf
                        }
                        None => {
                            // No free buffers, allocate a new one.
                            Buffer::new(self.capacity, self.current_generation)
                        }
                    }
                }
            };
            debug_assert_eq!(buf.generation, self.current_generation);
            debug_assert_eq!(buf.vec.as_ptr(), buf.ptr_range.start);
            return buf;
        }
    }

    fn handle_full_buffer(
        &mut self,
        mut buf: Buffer<T>,
        start_offset: usize,
        remaining_lower_bound: usize,
    ) -> (Buffer<T>, usize) {
        let required = buf.vec.len() - start_offset + 1 + remaining_lower_bound;

        self.should_advance = true;
        let mut new_buf = self.take_current_buffer(required);
        let new_start_offset = new_buf.vec.len();

        // Copy already iterated nodes to the new buffer.
        new_buf.vec.extend(buf.vec.drain(start_offset..));
        self.finished.push_back(buf);
        (new_buf, new_start_offset)
    }

    fn put_back(&mut self, buf: Buffer<T>) {
        debug_assert_eq!(buf.vec.as_ptr(), buf.ptr_range.start);
        if buf.is_full() {
            self.should_advance = true;
        }
        if buf.is_full()
            || buf.generation.is_older_than(self.current_generation)
        {
            self.finished.push_back(buf);
        } else {
            self.current.push(buf);
        }
    }
}

/// A specialised arena allocator which can recycle memory.
///
/// This allocator allocates slices of `T` from a queue of allocation buffers.
/// It is intended to work under the invariant that `T` does not contain any
/// references to any previously allocated slices, but may contain references
/// to slices allocated afterwards.
///
/// The unsafe function [`Self::recycle_older_than`] provides a mechanism for
/// reusing the memory taken up by older allocations if you can guarantee that
/// no references to these allocations exist.
///
/// This raison d'être of this type is to support the implementation of
/// [`crate::SlidingTree`]
pub struct SlidingBuffers<T> {
    state: RefCell<SlidingBuffersState<T>>,
}

impl<T> SlidingBuffers<T> {
    /// Creates a new `SlidingBuffers` with the given capacity.
    ///
    /// The `capacity` is the maximum number of elements that can be allocated
    /// in a single buffer.
    pub fn with_capacity(capacity: usize) -> SlidingBuffers<T> {
        SlidingBuffers {
            state: RefCell::new(SlidingBuffersState {
                capacity,
                finished: VecDeque::new(),
                current: SmallVec::new(),
                recycle: Vec::new(),
                current_generation: Generation::default(),
                should_advance: false,
            }),
        }
    }

    /// Preallocates recycled buffers.
    pub fn preallocate(&mut self, required: usize) {
        let mut cell = self.borrow_mut();
        let capacity = cell.capacity;
        cell.recycle.reserve(required);
        cell.finished.reserve(required);
        for _ in 0..required {
            cell.recycle
                .push(Buffer::new(capacity, Generation::INVALID))
        }
    }

    fn borrow_mut(&self) -> RefMut<'_, SlidingBuffersState<T>> {
        self.state.borrow_mut()
    }

    /// Populates a newly allocated slice with values from the iterator.
    ///
    /// Note that it is legal for the iterator to allocate more slices
    /// recursively from the same `SlidingBuffers`.
    pub fn alloc_iter<'a, I>(&self, mut iter: I) -> &'a mut [T]
    where
        I: Iterator<Item = T>,
    {
        let mut buf = self.borrow_mut().take_current_buffer(iter.size_hint().0);
        let mut start_offset = buf.vec.len();
        while let Some(value) = iter.next() {
            if buf.is_full() {
                // This only happens if the iterator is longer than the lower-bound size hint.
                (buf, start_offset) = self.borrow_mut().handle_full_buffer(
                    buf,
                    start_offset,
                    iter.size_hint().0,
                );
                debug_assert!(!buf.is_full());
            }
            buf.vec.push(value);
        }
        // SAFETY: The elements of `buf.vec` are only accessed via the slices
        // created here and these slices do not overlap with each other.
        let slice = unsafe {
            slice::from_raw_parts_mut(
                buf.vec.as_mut_ptr().add(start_offset),
                buf.vec.len() - start_offset,
            )
        };
        self.borrow_mut().put_back(buf);
        slice
    }

    /// Recycles all allocation buffers.
    ///
    /// # Safety
    ///
    /// This function is unsafe because it assumes that all existing references
    /// to allocated slices are no longer in use. Calling this function while
    /// there are outstanding references may lead to undefined behaviour.
    pub unsafe fn recycle_all(&mut self) {
        let mut cell = self.borrow_mut();
        while let Some(mut buf) = cell.finished.pop_front() {
            buf.clear();
            cell.recycle.push(buf);
        }
        while let Some(mut buf) = cell.current.pop() {
            buf.clear();
            cell.recycle.push(buf);
        }
    }

    /// Recycles all allocation buffers older than the supplied `slice`.
    ///
    /// # Safety
    ///
    /// This function is unsafe because it assumes that all existing references
    /// to allocated slices older than the supplied `slice` are no longer in
    /// use. Calling this function while there are outstanding references may
    /// lead to undefined behaviour.
    ///
    /// If `slice` was not allocated from this `SlidingBuffers`, it is not
    /// defined which buffers will be recycled.
    pub unsafe fn recycle_older_than(&mut self, slice: &[T]) {
        let ptr = slice.as_ptr();
        let mut cell = self.borrow_mut();
        let generation = cell
            .finished
            .iter()
            .find(|buf| buf.contains(ptr))
            .map(|buf| buf.generation)
            .unwrap_or(cell.current_generation);
        while let Some(peek_buf) = cell.finished.front_mut() {
            if peek_buf.generation.is_older_than(generation) {
                let mut buf = cell.finished.pop_front().unwrap();
                if buf.vec.capacity() >= cell.capacity {
                    buf.clear();
                    cell.recycle.push(buf);
                }
            } else {
                break;
            }
        }
    }

    /// Frees unused buffers to reduce memory usage.
    pub fn trim(&mut self) {
        self.borrow_mut().recycle.clear();
    }

    /// Returns the current buffer capacity.
    pub fn capacity(&self) -> usize {
        self.state.borrow().capacity
    }

    /// Returns the number of buffers in the finished, current, and recycled states.
    pub fn buffer_stats(&self) -> (usize, usize, usize) {
        let cell = self.state.borrow();
        (cell.finished.len(), cell.current.len(), cell.recycle.len())
    }
}
