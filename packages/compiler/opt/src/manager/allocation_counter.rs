pub(super) trait AllocationCounter {
    fn before_pass(&mut self) -> Option<u128>;
    fn after_pass(&mut self, before: Option<u128>) -> Option<u128>;
}

#[derive(Debug, Default)]
pub(super) struct DisabledAllocationCounter;

impl AllocationCounter for DisabledAllocationCounter {
    fn before_pass(&mut self) -> Option<u128> {
        None
    }

    fn after_pass(&mut self, _before: Option<u128>) -> Option<u128> {
        None
    }
}

pub(super) struct SnapshotAllocationCounter<F> {
    snapshot: F,
}

impl<F> SnapshotAllocationCounter<F> {
    pub(super) const fn new(snapshot: F) -> Self {
        Self { snapshot }
    }
}

impl<F> AllocationCounter for SnapshotAllocationCounter<F>
where
    F: FnMut() -> Option<u128>,
{
    fn before_pass(&mut self) -> Option<u128> {
        (self.snapshot)()
    }

    fn after_pass(&mut self, before: Option<u128>) -> Option<u128> {
        before.and_then(|before| (self.snapshot)()?.checked_sub(before))
    }
}
