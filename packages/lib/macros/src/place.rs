#![allow(missing_docs, clippy::missing_errors_doc)]

use ncl_object::{ObjectError, Runtime, ThreadContext, Word};
pub use ncl_object::{PlaceExpander, SetfExpansion};

/// Borrowed handle for the place registry owned by one runtime.
#[derive(Clone, Copy, Debug)]
pub struct PlaceRegistry<'runtime> {
    runtime: &'runtime Runtime,
}

impl<'runtime> PlaceRegistry<'runtime> {
    #[must_use]
    pub const fn new(runtime: &'runtime Runtime) -> Self {
        Self { runtime }
    }

    /// Register a compound-place expander in `runtime`.
    pub fn define(
        &self,
        ctx: &ThreadContext,
        operator: Word,
        expander: PlaceExpander,
    ) -> Result<(), ObjectError> {
        self.runtime
            .register_place_expander(ctx, operator, expander)
    }

    /// Read an expander. The runtime lock is released before callers invoke it.
    pub fn get(
        &self,
        ctx: &ThreadContext,
        operator: Word,
    ) -> Result<Option<PlaceExpander>, ObjectError> {
        self.runtime.place_expander(ctx, operator)
    }

    pub(crate) fn belongs_to(&self, runtime: &Runtime) -> bool {
        std::ptr::eq(self.runtime, runtime)
    }
}

/// Register a compound-place expander for one runtime.
pub fn register_place(
    ctx: &ThreadContext,
    runtime: &Runtime,
    operator: Word,
    expander: PlaceExpander,
) -> Result<(), ObjectError> {
    PlaceRegistry::new(runtime).define(ctx, operator, expander)
}
