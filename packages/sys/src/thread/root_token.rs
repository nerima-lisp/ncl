/// Token for a contiguous set of precise root slots.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RootToken {
    pub(crate) index: usize,
    pub(crate) count: usize,
    pub(crate) generation: u128,
    pub(crate) heap_id: u64,
}

impl RootToken {
    /// Construct a token for a thread-owned root span.
    #[must_use]
    pub(crate) const fn spanning(index: usize, count: usize) -> Self {
        Self {
            index,
            count,
            generation: 0,
            heap_id: 0,
        }
    }
}
