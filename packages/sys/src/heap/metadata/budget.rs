use core::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Failure while accounting for or allocating bounded metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MetadataError {
    /// The requested charge would exceed the shared budget's limit.
    CapacityExceeded {
        /// The budget limit in bytes.
        max_bytes: usize,
        /// The charge currently held by the budget in bytes.
        used_bytes: usize,
        /// The additional charge requested in bytes.
        requested_bytes: usize,
    },
    /// The requested capacity cannot be represented by `Layout` or `usize`.
    CapacityOverflow,
    /// The allocator could not provide the requested capacity.
    AllocationFailed,
    /// A release exceeded the charge held by this reservation.
    ReleaseUnderflow {
        /// The charge available to this release in bytes.
        available_bytes: usize,
        /// The release requested in bytes.
        requested_bytes: usize,
    },
    /// A vector index is outside the initialized elements.
    IndexOutOfBounds {
        /// The index that was requested.
        index: usize,
        /// The number of initialized elements.
        len: usize,
    },
    /// An open-addressing table violated its capacity or bucket invariant.
    InvalidTableState,
}

impl fmt::Display for MetadataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CapacityExceeded {
                max_bytes,
                used_bytes,
                requested_bytes,
            } => write!(
                formatter,
                "metadata capacity limit of {max_bytes} bytes would be exceeded by {used_bytes} + {requested_bytes} bytes"
            ),
            Self::CapacityOverflow => formatter.write_str("metadata capacity overflow"),
            Self::AllocationFailed => formatter.write_str("metadata allocation failed"),
            Self::ReleaseUnderflow {
                available_bytes,
                requested_bytes,
            } => write!(
                formatter,
                "metadata charge underflow: cannot release {requested_bytes} bytes from {available_bytes} bytes"
            ),
            Self::IndexOutOfBounds { index, len } => {
                write!(
                    formatter,
                    "metadata index {index} is out of bounds for length {len}"
                )
            }
            Self::InvalidTableState => formatter.write_str("invalid metadata table state"),
        }
    }
}

impl std::error::Error for MetadataError {}

#[derive(Debug)]
struct BudgetState {
    used_bytes: AtomicUsize,
    max_bytes: Option<usize>,
}

/// A thread-safe byte budget shared by metadata containers.
///
/// Thread-owned root storage is outside this budget because its size follows live native-stack depth,
/// not heap object count.
#[derive(Clone, Debug)]
pub struct MetadataBudget {
    state: Arc<BudgetState>,
}

impl MetadataBudget {
    /// Create a shared budget with an optional maximum charge.
    #[must_use]
    pub fn new(max_bytes: Option<usize>) -> Self {
        Self {
            state: Arc::new(BudgetState {
                used_bytes: AtomicUsize::new(0),
                max_bytes,
            }),
        }
    }

    /// Return the maximum charge, or `None` when the budget is unbounded.
    #[must_use]
    pub fn max_bytes(&self) -> Option<usize> {
        self.state.max_bytes
    }

    /// Return the charge currently held by all owners of this budget.
    #[must_use]
    pub fn used_bytes(&self) -> usize {
        self.state.used_bytes.load(Ordering::Acquire)
    }

    /// Reserve bytes and return the owner-tracked charge.
    pub fn reserve(&self, bytes: usize) -> Result<MetadataCharge, MetadataError> {
        reserve_bytes(&self.state, bytes)?;
        Ok(MetadataCharge {
            state: Arc::clone(&self.state),
            bytes,
        })
    }

    pub(super) fn into_empty_charge(self) -> MetadataCharge {
        MetadataCharge {
            state: self.state,
            bytes: 0,
        }
    }
}

impl Default for MetadataBudget {
    fn default() -> Self {
        Self::new(None)
    }
}

/// An owner-tracked reservation from a [`MetadataBudget`].
#[derive(Debug)]
pub struct MetadataCharge {
    state: Arc<BudgetState>,
    bytes: usize,
}

impl MetadataCharge {
    /// Return the bytes held by this reservation.
    #[must_use]
    pub const fn bytes(&self) -> usize {
        self.bytes
    }

    /// Increase this reservation if the shared budget has room.
    pub fn reserve_more(&mut self, bytes: usize) -> Result<(), MetadataError> {
        let new_bytes = self
            .bytes
            .checked_add(bytes)
            .ok_or(MetadataError::CapacityOverflow)?;
        reserve_bytes(&self.state, bytes)?;
        self.bytes = new_bytes;
        Ok(())
    }

    /// Release bytes from this reservation, checking its owned charge.
    pub fn release(&mut self, bytes: usize) -> Result<(), MetadataError> {
        let new_bytes = self
            .bytes
            .checked_sub(bytes)
            .ok_or(MetadataError::ReleaseUnderflow {
                available_bytes: self.bytes,
                requested_bytes: bytes,
            })?;
        release_bytes(&self.state, bytes)?;
        self.bytes = new_bytes;
        Ok(())
    }
}

impl Drop for MetadataCharge {
    fn drop(&mut self) {
        let released = release_bytes(&self.state, self.bytes);
        debug_assert!(released.is_ok());
    }
}

fn reserve_bytes(state: &BudgetState, bytes: usize) -> Result<(), MetadataError> {
    let mut used = state.used_bytes.load(Ordering::Acquire);
    loop {
        let next = used
            .checked_add(bytes)
            .ok_or(MetadataError::CapacityOverflow)?;
        if let Some(max_bytes) = state.max_bytes
            && next > max_bytes
        {
            return Err(MetadataError::CapacityExceeded {
                max_bytes,
                used_bytes: used,
                requested_bytes: bytes,
            });
        }
        match state.used_bytes.compare_exchange_weak(
            used,
            next,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => return Ok(()),
            Err(actual) => used = actual,
        }
    }
}

fn release_bytes(state: &BudgetState, bytes: usize) -> Result<(), MetadataError> {
    let mut used = state.used_bytes.load(Ordering::Acquire);
    loop {
        let next = used
            .checked_sub(bytes)
            .ok_or(MetadataError::ReleaseUnderflow {
                available_bytes: used,
                requested_bytes: bytes,
            })?;
        match state.used_bytes.compare_exchange_weak(
            used,
            next,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => return Ok(()),
            Err(actual) => used = actual,
        }
    }
}
