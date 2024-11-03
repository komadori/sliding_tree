use core::{cell::Cell, ptr};

pub(crate) struct RefSliceCell<'a, T> {
    pub cell: Cell<&'a mut [T]>,
}

impl<'a, T> RefSliceCell<'a, T> {
    #[inline]
    pub fn new(value: &'a mut [T]) -> Self {
        Self {
            cell: Cell::new(value),
        }
    }

    #[inline]
    pub fn get(&self) -> &[T] {
        // Creates an immutable reference which points to the same memory as
        // the inner mutable reference.
        //
        // SAFETY: This is safe because it borrows the inner reference so that
        // it cannot be used to mutate data while any immutable references exist.
        // Crucially, the interior mutability offered by this type is limited to
        // overwriting the inner reference, and it does not allow the inner
        // reference to be taken out without a mutable reference to the
        // containing cell. This would permit aliasing, which is undefined
        // behaviour.
        unsafe { ptr::read(self.cell.as_ptr() as *const &[T]) }
    }

    #[inline]
    pub fn set(&self, slice: &'a mut [T]) {
        self.cell.set(slice);
    }

    #[inline]
    pub fn get_mut(&mut self) -> &mut &'a mut [T] {
        self.cell.get_mut()
    }
}
