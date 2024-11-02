use sliding_tree::SlidingBuffers;

#[test]
fn test_sliding_buffers() {
    let mut buffers = SlidingBuffers::<usize>::with_capacity(100);

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
    let mut buffers = SlidingBuffers::<usize>::with_capacity(101);

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
    let mut buffers = SlidingBuffers::<usize>::with_capacity(100);
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
