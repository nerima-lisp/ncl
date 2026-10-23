use super::{MetadataBudget, MetadataCharge, MetadataError};
use core::alloc::Layout;
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;
use core::sync::atomic::{AtomicPtr, Ordering};
use std::alloc::{alloc, dealloc, realloc};
use std::marker::PhantomData;

#[derive(Debug)]
struct MetadataAllocation<T> {
    pointer: AtomicPtr<T>,
    capacity: usize,
    layout: Option<Layout>,
    charge: MetadataCharge,
}

impl<T> MetadataAllocation<T> {
    fn new(budget: MetadataBudget) -> Self {
        let capacity = if core::mem::size_of::<T>() == 0 {
            usize::MAX
        } else {
            0
        };
        Self {
            pointer: AtomicPtr::new(NonNull::dangling().as_ptr()),
            capacity,
            layout: None,
            charge: budget.into_empty_charge(),
        }
    }

    fn grow_to(&mut self, new_capacity: usize) -> Result<(), MetadataError> {
        let new_layout =
            Layout::array::<T>(new_capacity).map_err(|_| MetadataError::CapacityOverflow)?;
        let additional_charge = new_layout
            .size()
            .checked_sub(self.charge.bytes())
            .ok_or(MetadataError::CapacityOverflow)?;
        self.charge.reserve_more(additional_charge)?;

        let new_pointer = if let Some(old_layout) = self.layout {
            // SAFETY: `pointer` was allocated with `old_layout`; `new_layout` has the same
            // alignment, and its larger size is the requested replacement allocation size.
            unsafe {
                realloc(
                    self.pointer.load(Ordering::Relaxed).cast(),
                    old_layout,
                    new_layout.size(),
                )
            }
        } else {
            // SAFETY: `new_layout` is non-zero for non-ZST vectors and was validated above.
            unsafe { alloc(new_layout) }
        };

        let Some(new_pointer) = NonNull::new(new_pointer.cast::<T>()) else {
            let released = self.charge.release(additional_charge);
            debug_assert!(released.is_ok());
            return Err(MetadataError::AllocationFailed);
        };

        self.pointer.store(new_pointer.as_ptr(), Ordering::Relaxed);
        self.capacity = new_capacity;
        self.layout = Some(new_layout);
        Ok(())
    }
}

impl<T> Drop for MetadataAllocation<T> {
    fn drop(&mut self) {
        if let Some(layout) = self.layout {
            // SAFETY: `pointer` is the live allocation returned by `alloc` or `realloc` for
            // this exact layout, and no other owner deallocates it.
            unsafe { dealloc(self.pointer.load(Ordering::Relaxed).cast(), layout) };
        }
    }
}

/// The value returned by [`MetadataVec::push`] when capacity cannot grow.
#[derive(Debug)]
pub struct MetadataPushError<T> {
    /// The capacity or allocation failure.
    pub error: MetadataError,
    /// The value that was not inserted.
    pub value: T,
}

/// A fallibly growing vector charged to a shared metadata budget.
///
/// Its charge is the byte size of the requested `Layout::array` capacity. It
/// accounts for vector storage, not allocator overhead or process RSS.
#[derive(Debug)]
pub struct MetadataVec<T> {
    allocation: MetadataAllocation<T>,
    len: usize,
    marker: PhantomData<T>,
}

impl<T> MetadataVec<T> {
    /// Create an empty vector charged to `budget` when it grows.
    #[must_use]
    pub fn new(budget: MetadataBudget) -> Self {
        Self {
            allocation: MetadataAllocation::new(budget),
            len: 0,
            marker: PhantomData,
        }
    }

    /// Ensure capacity for `additional` elements without changing the contents
    /// if the charge or allocation fails.
    pub fn reserve(&mut self, additional: usize) -> Result<(), MetadataError> {
        let required = self
            .len
            .checked_add(additional)
            .ok_or(MetadataError::CapacityOverflow)?;
        if required <= self.allocation.capacity {
            return Ok(());
        }

        let grown = self
            .allocation
            .capacity
            .checked_mul(2)
            .map_or(required, |doubled| doubled.max(required));
        match self.allocation.grow_to(grown) {
            Ok(()) => Ok(()),
            Err(_) if grown != required => self.allocation.grow_to(required),
            Err(error) => Err(error),
        }
    }

    /// Insert `value`, returning it together with the typed failure if growth
    /// cannot be completed.
    pub fn push(&mut self, value: T) -> Result<(), MetadataPushError<T>> {
        if let Err(error) = self.reserve(1) {
            return Err(MetadataPushError { error, value });
        }

        // SAFETY: `reserve(1)` guarantees one uninitialized slot at `len`, and
        // `len` is increased only after the value has been written there.
        unsafe {
            self.allocation
                .pointer
                .load(Ordering::Relaxed)
                .add(self.len)
                .write(value);
        }
        self.len += 1;
        Ok(())
    }

    /// Insert `value` at `index`, preserving the relative order of later elements.
    ///
    /// Returns `value` with an index or capacity error without changing the
    /// vector contents.
    pub fn insert(&mut self, index: usize, value: T) -> Result<(), MetadataPushError<T>> {
        if index > self.len {
            return Err(MetadataPushError {
                error: MetadataError::IndexOutOfBounds {
                    index,
                    len: self.len,
                },
                value,
            });
        }
        if let Err(error) = self.reserve(1) {
            return Err(MetadataPushError { error, value });
        }

        // SAFETY: capacity for one more element is reserved, the source range
        // is initialized, and `copy` supports the overlapping right shift.
        unsafe {
            let pointer = self.allocation.pointer.load(Ordering::Relaxed);
            core::ptr::copy(pointer.add(index), pointer.add(index + 1), self.len - index);
            pointer.add(index).write(value);
        }
        self.len += 1;
        Ok(())
    }

    /// Remove and return the last element, if present.
    pub fn pop(&mut self) -> Option<T> {
        let new_len = self.len.checked_sub(1)?;
        self.len = new_len;
        // SAFETY: `new_len` was the last initialized index before shortening
        // `len`, so reading it moves out exactly one live value.
        Some(unsafe {
            self.allocation
                .pointer
                .load(Ordering::Relaxed)
                .add(new_len)
                .read()
        })
    }

    /// Remove and return the element at `index`, shifting later elements left.
    ///
    /// Returns `None` if `index` is outside the initialized elements.
    pub fn remove(&mut self, index: usize) -> Option<T> {
        if index >= self.len {
            return None;
        }
        let old_len = self.len;
        let pointer = self.allocation.pointer.load(Ordering::Relaxed);

        // SAFETY: `index` is initialized. Reading moves that element out, and
        // `copy` shifts the remaining initialized range over the vacated slot.
        // The shortened length excludes the duplicate bytes at the old end.
        unsafe {
            let removed = pointer.add(index).read();
            core::ptr::copy(
                pointer.add(index + 1),
                pointer.add(index),
                old_len - index - 1,
            );
            self.len = old_len - 1;
            Some(removed)
        }
    }

    /// Remove all elements while retaining the allocated capacity.
    pub fn clear(&mut self) {
        self.truncate(0);
    }

    /// Keep the elements accepted by `predicate`, preserving their order.
    ///
    /// If the predicate or a removed element's destructor panics, the vector
    /// remains valid and contains the retained prefix and all unprocessed items.
    pub fn retain<F>(&mut self, mut predicate: F)
    where
        F: FnMut(&T) -> bool,
    {
        struct BackshiftOnDrop<'a, T> {
            vector: &'a mut MetadataVec<T>,
            processed_len: usize,
            deleted_count: usize,
            original_len: usize,
        }

        impl<T> Drop for BackshiftOnDrop<'_, T> {
            fn drop(&mut self) {
                let pointer = self.vector.allocation.pointer.load(Ordering::Relaxed);
                let tail_len = self.original_len - self.processed_len;
                if self.deleted_count != 0 && tail_len != 0 {
                    // SAFETY: the unprocessed tail is initialized at
                    // `processed_len`; its destination follows the retained
                    // prefix, and `copy` permits this overlap.
                    unsafe {
                        core::ptr::copy(
                            pointer.add(self.processed_len),
                            pointer.add(self.processed_len - self.deleted_count),
                            tail_len,
                        );
                    }
                }
                self.vector.len = self.original_len - self.deleted_count;
            }
        }

        let original_len = self.len;
        if original_len == 0 {
            return;
        }
        self.len = 0;

        let mut guard = BackshiftOnDrop {
            vector: self,
            processed_len: 0,
            deleted_count: 0,
            original_len,
        };
        let pointer = guard.vector.allocation.pointer.load(Ordering::Relaxed);

        for index in 0..original_len {
            // SAFETY: this index is either in the unprocessed initialized tail
            // or is being considered before its state is advanced below.
            let keep = unsafe { predicate(&*pointer.add(index)) };
            if keep {
                if guard.deleted_count != 0 {
                    // SAFETY: the current value is initialized and the target
                    // lies in the already processed deleted range.
                    unsafe {
                        core::ptr::copy(
                            pointer.add(index),
                            pointer.add(index - guard.deleted_count),
                            1,
                        );
                    }
                }
                guard.processed_len = index + 1;
            } else {
                guard.deleted_count += 1;
                guard.processed_len = index + 1;
                // SAFETY: the rejected item is initialized and is excluded from
                // the vector length before its destructor can unwind.
                unsafe { core::ptr::drop_in_place(pointer.add(index)) };
            }
        }
    }

    /// Return the number of initialized elements.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Return the number of elements that fit without another allocation.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.allocation.capacity
    }

    /// Return whether the vector contains no elements.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Borrow the initialized elements as a slice.
    #[must_use]
    pub fn as_slice(&self) -> &[T] {
        // SAFETY: the vector maintains `len <= capacity` initialized elements at
        // `pointer`, and the allocation remains alive for the returned borrow.
        unsafe {
            core::slice::from_raw_parts(self.allocation.pointer.load(Ordering::Relaxed), self.len)
        }
    }

    /// Mutably borrow the initialized elements as a slice.
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        // SAFETY: the vector maintains `len <= capacity` initialized elements at
        // `pointer`, and this exclusive borrow prevents other access to them.
        unsafe {
            core::slice::from_raw_parts_mut(
                self.allocation.pointer.load(Ordering::Relaxed),
                self.len,
            )
        }
    }

    /// Iterate over the initialized elements.
    pub fn iter(&self) -> core::slice::Iter<'_, T> {
        self.as_slice().iter()
    }

    /// Iterate mutably over the initialized elements.
    pub fn iter_mut(&mut self) -> core::slice::IterMut<'_, T> {
        self.as_mut_slice().iter_mut()
    }

    /// Shorten the vector to `new_len`, dropping removed elements.
    pub fn truncate(&mut self, new_len: usize) {
        if new_len >= self.len {
            return;
        }

        let old_len = self.len;
        self.len = new_len;
        // SAFETY: this range was initialized before `len` was shortened and does
        // not overlap any element that remains part of the vector.
        unsafe {
            let removed = self.allocation.pointer.load(Ordering::Relaxed).add(new_len);
            core::ptr::drop_in_place(core::ptr::slice_from_raw_parts_mut(
                removed,
                old_len - new_len,
            ));
        }
    }
}

impl<T> Drop for MetadataVec<T> {
    fn drop(&mut self) {
        // SAFETY: `as_mut_slice` covers exactly the initialized elements. The
        // allocation field is dropped afterward, including during unwinding.
        unsafe { core::ptr::drop_in_place(self.as_mut_slice()) };
    }
}

impl<T> Deref for MetadataVec<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<T> DerefMut for MetadataVec<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_slice()
    }
}

impl<'a, T> IntoIterator for &'a MetadataVec<T> {
    type Item = &'a T;
    type IntoIter = core::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T> IntoIterator for &'a mut MetadataVec<T> {
    type Item = &'a mut T;
    type IntoIter = core::slice::IterMut<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}
