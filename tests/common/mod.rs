//! Utilities shared between the integration test binaries, included via
//! `mod common;`. Not every binary uses every item.
#![allow(dead_code)]

use std::cell::Cell;

/// An iterator which yields `count` values and then panics.
pub struct PanicAfter {
    next: usize,
    count: usize,
}

impl PanicAfter {
    pub fn new(count: usize) -> Self {
        PanicAfter { next: 1000, count }
    }
}

impl Iterator for PanicAfter {
    type Item = usize;

    fn next(&mut self) -> Option<usize> {
        if self.count == 0 {
            panic!("boom");
        }
        self.count -= 1;
        self.next += 1;
        Some(self.next)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        // Hide the size so allocation reuses the existing partly-filled buffer.
        (0, None)
    }
}

/// An iterator whose `size_hint` panics on a given call while `next` keeps
/// yielding.
pub struct PanicOnSizeHint {
    value: usize,
    calls: Cell<usize>,
    panic_on_call: usize,
}

impl PanicOnSizeHint {
    pub fn new(panic_on_call: usize) -> Self {
        PanicOnSizeHint {
            value: 1000,
            calls: Cell::new(0),
            panic_on_call,
        }
    }
}

impl Iterator for PanicOnSizeHint {
    type Item = usize;

    fn next(&mut self) -> Option<usize> {
        self.value += 1;
        Some(self.value)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.calls.get() + 1;
        self.calls.set(n);
        if n >= self.panic_on_call {
            panic!("boom in size_hint");
        }
        (0, None)
    }
}

/// Shared count of how many `DropCounter`s have been constructed and dropped.
#[derive(Default)]
pub struct Counters {
    constructed: Cell<i64>,
    dropped: Cell<i64>,
}

impl Counters {
    pub fn new() -> Self {
        Counters::default()
    }

    pub fn constructed(&self) -> i64 {
        self.constructed.get()
    }

    pub fn dropped(&self) -> i64 {
        self.dropped.get()
    }

    pub fn balanced(&self) -> bool {
        self.constructed.get() == self.dropped.get()
    }
}

/// Data that counts its construction and destruction, so a test can detect
/// leaks and double-frees across a panic.
pub struct DropCounter<'c> {
    counters: &'c Counters,
}

impl<'c> DropCounter<'c> {
    pub fn new(counters: &'c Counters) -> Self {
        counters.constructed.set(counters.constructed.get() + 1);
        DropCounter { counters }
    }
}

impl Drop for DropCounter<'_> {
    fn drop(&mut self) {
        self.counters.dropped.set(self.counters.dropped.get() + 1);
    }
}
