use core::{
    cell::{RefCell, RefMut},
    cmp::{self, Reverse},
    ops::Range,
    slice,
};

extern crate alloc;
use alloc::collections::VecDeque;
use alloc::vec::Vec;
use smallvec::SmallVec;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Generation(usize);

impl Generation {
    const FIRST: Generation = Generation(0);

    fn advance(&mut self) {
        self.0 += 1;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct GenerationSpan {
    start: Generation,
    end: Generation,
}

impl GenerationSpan {
    const INVALID: GenerationSpan = GenerationSpan {
        start: Generation(usize::MAX),
        end: Generation(usize::MAX),
    };

    fn is_older_than(&self, other: GenerationSpan) -> bool {
        self.end < other.start
    }
}

impl From<Generation> for GenerationSpan {
    fn from(gen: Generation) -> Self {
        GenerationSpan {
            start: gen,
            end: gen,
        }
    }
}

struct Buffer<T> {
    vec: Vec<T>,
    ptr_range: Range<*const T>,
    generation: GenerationSpan,
}

impl<T> Buffer<T> {
    fn new(capacity: usize, generation: GenerationSpan) -> Buffer<T> {
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
        self.generation = GenerationSpan::INVALID;
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
    depth: usize,
    flat_allocation: bool,
}

impl<T> SlidingBuffersState<T> {
    fn take_current_buffer(&mut self, required: usize) -> Buffer<T> {
        if required > self.capacity {
            // Increase the size of future buffers. Older smaller buffers
            // will eventually be freed rather than recycled.
            self.capacity = cmp::max(required, self.capacity * 2);
        }
        self.depth += 1;
        let first_recursion = self.depth > 1 && self.flat_allocation;
        if first_recursion {
            // When starting a recursive allocation, advance the
            // generation before any new buffer might be needed.
            self.current_generation.advance();
            self.flat_allocation = false;
        }
        loop {
            let buf = match self.current.pop() {
                Some(buf) => {
                    if buf.vec.capacity() - buf.vec.len() < required {
                        // The buffer is too small.
                        self.finished.push_back(buf);
                        continue;
                    }
                    buf
                }
                None => {
                    if self.flat_allocation {
                        // Outside of a recursive allocation, advance the
                        // generation once for each new buffer.
                        self.current_generation.advance();
                    }
                    match self.recycle.pop() {
                        Some(mut buf) => {
                            // Use a recycled buffer.
                            buf.generation = self.current_generation.into();
                            buf
                        }
                        None => {
                            // No free buffers, allocate a new one.
                            Buffer::new(
                                self.capacity,
                                self.current_generation.into(),
                            )
                        }
                    }
                }
            };
            debug_assert_eq!(buf.vec.as_ptr(), buf.ptr_range.start);
            debug_assert_eq!(
                unsafe { buf.vec.as_ptr().add(buf.vec.capacity()) },
                buf.ptr_range.end
            );
            return buf;
        }
    }

    fn handle_full_buffer(
        &mut self,
        buf: Buffer<T>,
        start_offset: usize,
        remaining_lower_bound: usize,
    ) -> (Buffer<T>, usize) {
        let required = buf.vec.len() - start_offset + 1 + remaining_lower_bound;

        // Put the old buffer back first to preserve order.
        self.depth -= 1;
        let old_buf_idx = self.finished.len();
        self.finished.push_back(buf);

        // Get a new buffer.
        let mut new_buf = self.take_current_buffer(required);
        let new_start_offset = new_buf.vec.len();

        // Index is still valid because only push_back() is called.
        let old_buf = &mut self.finished[old_buf_idx];

        // Copy already iterated nodes to the new buffer.
        new_buf.vec.extend(old_buf.vec.drain(start_offset..));
        (new_buf, new_start_offset)
    }

    fn put_back(&mut self, mut buf: Buffer<T>) {
        debug_assert_eq!(buf.vec.as_ptr(), buf.ptr_range.start);

        // Extend the range of the buffer to the current generation.
        buf.generation.end = self.current_generation;

        // Exit scope.
        self.depth -= 1;
        self.flat_allocation |= self.depth == 0;

        // Put the buffer back.
        if buf.is_full() {
            self.finished.push_back(buf);
        } else {
            self.current.push(buf);
        }

        // Sort buffers so that the most filled buffer will be used first.
        self.current
            .sort_by_key(|buf| Reverse(buf.vec.capacity() - buf.vec.len()));
    }

    fn find_generation(&self, ptr: *const T) -> GenerationSpan {
        self.finished
            .iter()
            .chain(self.current.iter())
            .find(|buf| buf.contains(ptr))
            .map(|buf| buf.generation)
            .expect("slice not present in this SlidingBuffers")
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
                current_generation: Generation::FIRST,
                depth: 0,
                flat_allocation: true,
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
                .push(Buffer::new(capacity, GenerationSpan::INVALID))
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
        debug_assert!(buf.contains(slice.as_ptr()));
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
    /// # Panics
    ///
    /// Panics if the supplied `slice` is not a valid reference to an
    /// allocation from this `SlidingBuffers`.
    ///
    /// # Safety
    ///
    /// This function is unsafe because it assumes that all existing references
    /// to allocated slices older than the supplied `slice` are no longer in
    /// use. Calling this function while there are outstanding references may
    /// lead to undefined behaviour.
    pub unsafe fn recycle_older_than(&mut self, slice: &[T]) {
        let mut cell = self.borrow_mut();
        let generation = cell.find_generation(slice.as_ptr());
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

    /// Asserts that `src` can safely reference `dst`.
    ///
    /// This function is provided to assist in testing and debugging data
    /// structures built on top of `SlidingBuffers`.
    ///
    /// # Panics
    ///
    /// May panic if `src` was allocated after `dst` and it is therefore unsafe
    /// for data in `src` to reference `dst`. However, this detection is
    /// performed with buffer-level granularity and so it is not guaranteed to
    /// detect all such cases.
    ///
    /// Panics if either `src` or `dst` are not valid references to allocations
    /// from this `SlidingBuffers`, with the exception that `dst` may be an
    /// empty slice.
    pub fn assert_can_reference(&self, src: &[T], dst: &[T]) {
        let cell = self.state.borrow();
        let src_generation = cell.find_generation(src.as_ptr());
        if !dst.is_empty() {
            let dst_generation = cell.find_generation(dst.as_ptr());
            assert!(!dst_generation.is_older_than(src_generation),
                "src in generation {}:{} cannot reference dst in generation {}:{}",
                src_generation.start.0,
                src_generation.end.0,
                dst_generation.start.0,
                dst_generation.end.0
            );
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
