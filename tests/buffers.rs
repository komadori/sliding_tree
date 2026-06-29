use std::panic::{self, AssertUnwindSafe};

use sliding_tree::SlidingBuffers;

mod common;
use common::{Counters, DropCounter, PanicAfter, PanicOnSizeHint};

#[test]
fn test_sliding_buffers() {
    let buffers = SlidingBuffers::<usize>::with_capacity(100);

    // Test allocation
    let slice1 = buffers.alloc_iter(0..25);
    assert_eq!(slice1.len(), 25);
    assert_eq!(buffers.buffer_stats(), (0, 1, 0));

    let slice2 = buffers.alloc_iter(25..100);
    assert_eq!(slice2.len(), 75);
    assert_eq!(buffers.buffer_stats(), (1, 0, 0));

    let slice3 = buffers.alloc_iter(100..150);
    assert_eq!(slice3.len(), 50);
    assert_eq!(buffers.buffer_stats(), (1, 1, 0));

    // Test recycling older than
    unsafe {
        buffers.recycle_older_than(slice2);
    }
    assert_eq!(buffers.buffer_stats(), (1, 1, 0));
    unsafe {
        buffers.recycle_older_than(slice3);
    }
    assert_eq!(buffers.buffer_stats(), (0, 1, 1));

    // Test trimming
    buffers.trim();
    assert_eq!(buffers.buffer_stats(), (0, 1, 0));

    // Test recycling all
    unsafe {
        buffers.recycle_all();
    }
    assert_eq!(buffers.buffer_stats(), (0, 0, 1));

    // Test allocation after recycling
    let slice4 = buffers.alloc_iter(0..1);
    assert_eq!(slice4.len(), 1);
    assert_eq!(buffers.buffer_stats(), (0, 1, 0));
}

#[test]
fn test_recursive_allocation() {
    let buffers = SlidingBuffers::<usize>::with_capacity(101);

    let mut vec = Vec::new();
    let slice1 = buffers.alloc_iter((0..10).inspect(|_| {
        let slice1a = buffers.alloc_iter(0..10);
        assert_eq!(slice1a.len(), 10);
        vec.push(slice1a);
    }));
    for slice1a in vec.iter() {
        buffers.assert_can_reference(slice1, slice1a);
    }
    assert_eq!(slice1.len(), 10);
    assert_eq!(buffers.buffer_stats(), (0, 2, 0));

    let slice2 = buffers.alloc_iter(0..90);
    buffers.assert_can_reference(slice1, slice2);
    assert_eq!(slice2.len(), 90);
    assert_eq!(buffers.buffer_stats(), (1, 1, 0));

    let slice3 = buffers.alloc_iter(0..90);
    buffers.assert_can_reference(slice2, slice3);
    assert_eq!(slice3.len(), 90);
    assert_eq!(buffers.buffer_stats(), (2, 1, 0));

    // Test recycling older than
    unsafe {
        buffers.recycle_older_than(slice2);
    }
    assert_eq!(buffers.buffer_stats(), (2, 1, 0));
    unsafe {
        buffers.recycle_older_than(slice3);
    }
    assert_eq!(buffers.buffer_stats(), (0, 1, 2));
}

#[test]
#[should_panic(
    expected = "src in generation 2:2 cannot reference dst in generation 1:1"
)]
fn test_assert_can_reference_panics() {
    let buffers = SlidingBuffers::<usize>::with_capacity(1);
    let slice1 = buffers.alloc_iter(0..1);
    let slice2 = buffers.alloc_iter(0..1);
    assert_eq!(buffers.buffer_stats(), (2, 0, 0));
    buffers.assert_can_reference(slice2, slice1);
}

#[test]
fn test_preallocate() {
    let mut buffers = SlidingBuffers::<usize>::with_capacity(101);
    assert_eq!(buffers.buffer_stats(), (0, 0, 0));
    buffers.preallocate(10);
    assert_eq!(buffers.buffer_stats(), (0, 0, 10));
}

#[test]
fn test_capacity() {
    let buffers = SlidingBuffers::<usize>::with_capacity(100);
    let slice1 = buffers.alloc_iter(0..100);
    let slice2 = buffers.alloc_iter(0..199);
    assert_eq!(buffers.capacity(), 200);
    assert_eq!(buffers.buffer_stats(), (1, 1, 0));

    // Buffers smaller than the current capacity are freed.
    buffers.assert_can_reference(slice1, slice2);
    unsafe {
        buffers.recycle_older_than(slice2);
    }
    assert_eq!(buffers.buffer_stats(), (0, 1, 0));
}

#[test]
fn test_panic_preserves_prior_allocation() {
    let buffers = SlidingBuffers::<usize>::with_capacity(100);
    let slice1 = buffers.alloc_iter(0..3);
    assert_eq!(buffers.buffer_stats(), (0, 1, 0));

    // The panicking allocation reuses the buffer that still holds `slice1`, so
    // the panic must not free it.
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        buffers.alloc_iter(PanicAfter::new(2));
    }));
    assert!(result.is_err());

    // The buffer is still present and `slice1` is intact.
    assert_eq!(buffers.buffer_stats(), (0, 1, 0));
    assert_eq!(slice1, &[0, 1, 2]);

    // The allocator remains usable.
    let slice2 = buffers.alloc_iter(10..13);
    assert_eq!(slice2, &[10, 11, 12]);
}

#[test]
fn test_panic_spanning_buffers_preserves_prior_allocation() {
    // Small capacity so the new slice fills the buffer holding `slice1` and
    // spills into a second buffer before the panic.
    let buffers = SlidingBuffers::<usize>::with_capacity(10);
    let slice1 = buffers.alloc_iter(0..3);
    assert_eq!(buffers.buffer_stats(), (0, 1, 0));

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        buffers.alloc_iter(PanicAfter::new(9));
    }));
    assert!(result.is_err());

    // `slice1`'s buffer is now finished, the spilled buffer is back as current,
    // and `slice1` survived.
    assert_eq!(buffers.buffer_stats(), (1, 1, 0));
    assert_eq!(slice1, &[0, 1, 2]);

    // The allocator remains usable.
    let slice2 = buffers.alloc_iter(10..13);
    assert_eq!(slice2, &[10, 11, 12]);
}

#[test]
fn test_panic_in_size_hint_preserves_prior_allocation() {
    let buffers = SlidingBuffers::<usize>::with_capacity(100);
    let slice1 = buffers.alloc_iter(0..3);

    // The panic comes from `size_hint`, not `next`, while a buffer is in flight.
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        buffers.alloc_iter(PanicOnSizeHint::new(3));
    }));
    assert!(result.is_err());

    assert_eq!(buffers.buffer_stats(), (0, 1, 0));
    assert_eq!(slice1, &[0, 1, 2]);

    let slice2 = buffers.alloc_iter(10..13);
    assert_eq!(slice2, &[10, 11, 12]);
}

#[test]
fn test_panic_in_recursive_allocation_preserves_buffers() {
    let buffers = SlidingBuffers::<usize>::with_capacity(100);
    let outer = buffers.alloc_iter(0..3);
    assert_eq!(buffers.buffer_stats(), (0, 1, 0));

    // The outer iterator recursively allocates from the same buffers, and the
    // inner allocation panics. Both the in-flight inner and outer buffers must
    // be returned, leaving the earlier `outer` slice intact.
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        buffers.alloc_iter((0..5).inspect(|_| {
            buffers.alloc_iter(PanicAfter::new(2));
        }));
    }));
    assert!(result.is_err());

    assert_eq!(outer, &[0, 1, 2]);
    let after = buffers.alloc_iter(100..103);
    assert_eq!(after, &[100, 101, 102]);
}

#[test]
fn test_panic_does_not_leak_or_double_free() {
    let counters = Counters::new();
    {
        // Small capacity so the panicking allocation spans buffers, exercising
        // the drain that moves already-built elements into a new buffer.
        let buffers = SlidingBuffers::<DropCounter>::with_capacity(5);
        let _live =
            buffers.alloc_iter((0..3).map(|_| DropCounter::new(&counters)));

        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            buffers.alloc_iter(
                PanicAfter::new(6).map(|_| DropCounter::new(&counters)),
            );
        }));
        assert!(result.is_err());

        // Orphaned elements are still owned by the buffers, not dropped early.
        assert!(counters.constructed() > counters.dropped());
    }

    // Dropping the buffers must drop every constructed element exactly once; a
    // mismatch means a leak or a double-free.
    assert!(
        counters.balanced(),
        "leak or double-free across panic: constructed {}, dropped {}",
        counters.constructed(),
        counters.dropped(),
    );
}
