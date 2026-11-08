use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::marker::PhantomData;
use std::rc::Rc;

#[derive(Clone, Copy)]
struct AllocationState {
    active: bool,
    bytes: u128,
    overflowed: bool,
}

impl AllocationState {
    const INACTIVE: Self = Self {
        active: false,
        bytes: 0,
        overflowed: false,
    };
}

thread_local! {
    static ALLOCATION_STATE: Cell<AllocationState> = const {
        Cell::new(AllocationState::INACTIVE)
    };
    static TRACKING_ALLOCATOR_INSTALLED: Cell<bool> = const { Cell::new(false) };
}

/// System allocator that counts requested bytes inside an active thread scope.
#[derive(Debug, Default)]
pub struct AllocationTrackingAllocator;

impl AllocationTrackingAllocator {
    /// Mark this allocator as installed by the executable's `#[global_allocator]`.
    pub fn activate(&self) {
        let _ = TRACKING_ALLOCATOR_INSTALLED.try_with(|installed| installed.set(true));
    }
}

// SAFETY: all operations delegate to `System`; tracking state is thread-local.
unsafe impl GlobalAlloc for AllocationTrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller guarantees `layout` satisfies `GlobalAlloc::alloc`.
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            record_allocation(layout.size());
        }
        pointer
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller guarantees `layout` satisfies `GlobalAlloc::alloc_zeroed`.
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            record_allocation(layout.size());
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: the caller guarantees this pointer and layout came from `System`.
        unsafe { System.dealloc(pointer, layout) };
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: the caller guarantees this pointer and layout came from `System`.
        let resized = unsafe { System.realloc(pointer, layout, new_size) };
        if !resized.is_null() {
            record_allocation(new_size);
        }
        resized
    }
}

fn record_allocation(bytes: usize) {
    let _ = ALLOCATION_STATE.try_with(|state| {
        let mut current = state.get();
        if current.active {
            let Ok(bytes) = u64::try_from(bytes) else {
                current.overflowed = true;
                state.set(current);
                return;
            };
            if let Some(total) = current.bytes.checked_add(u128::from(bytes)) {
                current.bytes = total;
            } else {
                current.overflowed = true;
            }
            state.set(current);
        }
    });
}

/// Error starting thread-local allocation tracking.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AllocationTrackingError {
    /// The executable did not install [`AllocationTrackingAllocator`].
    AllocatorNotInstalled,
    /// This thread already has an active allocation-tracking scope.
    ScopeAlreadyActive,
    /// Thread-local tracking state is unavailable during thread teardown.
    ThreadStateUnavailable,
}

impl Display for AllocationTrackingError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::AllocatorNotInstalled => {
                formatter.write_str("allocation tracker is not installed")
            }
            Self::ScopeAlreadyActive => {
                formatter.write_str("allocation tracking scope is already active")
            }
            Self::ThreadStateUnavailable => {
                formatter.write_str("allocation tracking state is unavailable")
            }
        }
    }
}

impl Error for AllocationTrackingError {}

/// A thread-bound allocation counter scope.
#[derive(Debug)]
pub struct AllocationTrackingScope {
    _thread_bound: PhantomData<Rc<()>>,
}

impl AllocationTrackingScope {
    /// Start counting successful allocator requests on the current thread.
    ///
    /// `alloc`, `alloc_zeroed`, and successful `realloc` calls contribute their
    /// requested sizes. A reallocation contributes its full new size. Deallocation
    /// does not subtract bytes. Allocations on other threads are excluded.
    ///
    /// # Errors
    /// Returns an error if the tracking allocator is not installed, a scope is
    /// already active on this thread, or its thread-local state is unavailable.
    pub fn start() -> Result<Self, AllocationTrackingError> {
        let installed = TRACKING_ALLOCATOR_INSTALLED
            .try_with(Cell::get)
            .map_err(|_| AllocationTrackingError::ThreadStateUnavailable)?;
        if !installed {
            return Err(AllocationTrackingError::AllocatorNotInstalled);
        }
        let mut already_active = false;
        ALLOCATION_STATE
            .try_with(|state| {
                if state.get().active {
                    already_active = true;
                } else {
                    state.set(AllocationState {
                        active: true,
                        bytes: 0,
                        overflowed: false,
                    });
                }
            })
            .map_err(|_| AllocationTrackingError::ThreadStateUnavailable)?;
        if already_active {
            return Err(AllocationTrackingError::ScopeAlreadyActive);
        }
        Ok(Self {
            _thread_bound: PhantomData,
        })
    }

    /// Return the cumulative requested bytes so far, or `None` after overflow.
    #[must_use]
    pub fn allocated_bytes(&self) -> Option<u128> {
        ALLOCATION_STATE
            .try_with(|state| {
                let current = state.get();
                (!current.overflowed).then_some(current.bytes)
            })
            .ok()
            .flatten()
    }
}

impl Drop for AllocationTrackingScope {
    fn drop(&mut self) {
        let _ = ALLOCATION_STATE.try_with(|state| state.set(AllocationState::INACTIVE));
    }
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "test fixtures use constant-valid layouts and non-nested scopes"
)]
mod tests {
    use super::{AllocationTrackingAllocator, AllocationTrackingError, AllocationTrackingScope};
    use std::alloc::{GlobalAlloc, Layout};

    #[test]
    fn scope_counts_successful_allocator_requests_and_not_deallocations() {
        let allocator = AllocationTrackingAllocator;
        allocator.activate();
        let scope = AllocationTrackingScope::start().expect("scope starts");
        let layout = Layout::from_size_align(8, 8).expect("valid layout");
        // SAFETY: the layout is valid and the returned pointer is deallocated below.
        let pointer = unsafe { allocator.alloc(layout) };
        assert!(!pointer.is_null());
        assert_eq!(scope.allocated_bytes(), Some(8));
        let zeroed_layout = Layout::from_size_align(4, 4).expect("valid layout");
        // SAFETY: the layout is valid and the returned pointer is deallocated below.
        let zeroed = unsafe { allocator.alloc_zeroed(zeroed_layout) };
        assert!(!zeroed.is_null());
        assert_eq!(scope.allocated_bytes(), Some(12));
        // SAFETY: `pointer` was allocated with this allocator and layout.
        let resized = unsafe { allocator.realloc(pointer, layout, 16) };
        assert!(!resized.is_null());
        assert_eq!(scope.allocated_bytes(), Some(28));
        // SAFETY: `zeroed` was allocated with this allocator and layout.
        unsafe { allocator.dealloc(zeroed, zeroed_layout) };
        assert_eq!(scope.allocated_bytes(), Some(28));
        // SAFETY: `resized` was returned by realloc with this layout.
        unsafe { allocator.dealloc(resized, Layout::from_size_align(16, 8).expect("layout")) };
        assert_eq!(scope.allocated_bytes(), Some(28));
    }

    #[test]
    fn second_active_scope_is_rejected_without_stopping_the_first() {
        let result = std::thread::spawn(|| {
            let allocator = AllocationTrackingAllocator;
            allocator.activate();
            let layout = Layout::from_size_align(8, 8).expect("valid layout");
            let scope = AllocationTrackingScope::start().expect("scope starts");
            let second_scope = AllocationTrackingScope::start();
            let rejected = matches!(
                second_scope,
                Err(AllocationTrackingError::ScopeAlreadyActive)
            );
            // SAFETY: the layout is valid and the pointer is deallocated below.
            let pointer = unsafe { allocator.alloc(layout) };
            let bytes = scope.allocated_bytes();
            if !pointer.is_null() {
                // SAFETY: `pointer` was allocated with this allocator and layout.
                unsafe { allocator.dealloc(pointer, layout) };
            }
            (rejected, !pointer.is_null(), bytes)
        })
        .join()
        .expect("tracking worker completes");

        assert_eq!(
            result,
            (true, true, Some(8)),
            "the rejected second scope leaves the original counter active"
        );
    }

    #[test]
    fn counter_reports_overflow_as_unavailable() {
        let allocator = AllocationTrackingAllocator;
        allocator.activate();
        let scope = AllocationTrackingScope::start().expect("scope starts");
        super::ALLOCATION_STATE.with(|state| {
            state.set(super::AllocationState {
                active: true,
                bytes: u128::MAX,
                overflowed: false,
            });
        });
        let layout = Layout::from_size_align(1, 1).expect("valid layout");
        // SAFETY: the allocation is freed with the same allocator and layout.
        let pointer = unsafe { allocator.alloc(layout) };
        assert!(!pointer.is_null());
        assert_eq!(scope.allocated_bytes(), None);
        // SAFETY: `pointer` was allocated with this allocator and layout.
        unsafe { allocator.dealloc(pointer, layout) };
    }

    #[test]
    fn scopes_count_only_the_thread_that_started_them() {
        let allocator = AllocationTrackingAllocator;
        allocator.activate();
        let scope = AllocationTrackingScope::start().expect("scope starts");
        let other_allocator = AllocationTrackingAllocator;
        let other_thread = std::thread::spawn(move || {
            let layout = Layout::from_size_align(32, 8).expect("valid layout");
            // SAFETY: the allocation is freed with the same allocator and layout.
            let pointer = unsafe { other_allocator.alloc(layout) };
            assert!(!pointer.is_null());
            // SAFETY: `pointer` was allocated with this allocator and layout.
            unsafe { other_allocator.dealloc(pointer, layout) };
        });
        assert!(other_thread.join().is_ok());
        assert_eq!(scope.allocated_bytes(), Some(0));
    }

    #[test]
    fn scope_is_unavailable_without_the_profiling_allocator() {
        // This test uses a separate test process allocator in the test harness;
        // installation state is checked by an isolated helper thread.
        let unavailable = std::thread::spawn(|| AllocationTrackingScope::start().is_err()).join();
        assert!(unavailable.is_ok_and(|is_unavailable| is_unavailable));
    }
}
