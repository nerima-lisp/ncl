#![allow(missing_docs, clippy::missing_errors_doc)]

use ncl_object::{ObjectError, Runtime, ThreadContext, Word};
pub use ncl_object::{PlaceExpander, SetfExpansion};

/// Handle for querying the place registry owned by a runtime.
#[derive(Clone, Copy, Debug, Default)]
pub struct PlaceRegistry;

impl PlaceRegistry {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Register a compound-place expander in `runtime`.
    pub fn define(
        &self,
        ctx: &ThreadContext,
        runtime: &Runtime,
        operator: Word,
        expander: PlaceExpander,
    ) -> Result<(), ObjectError> {
        runtime.register_place_expander(ctx, operator, expander)
    }

    /// Read an expander. The runtime lock is released before callers invoke it.
    pub fn get(
        &self,
        ctx: &ThreadContext,
        runtime: &Runtime,
        operator: Word,
    ) -> Result<Option<PlaceExpander>, ObjectError> {
        runtime.place_expander(ctx, operator)
    }
}

/// Register a compound-place expander for one runtime.
pub fn register_place(
    ctx: &ThreadContext,
    runtime: &Runtime,
    operator: Word,
    expander: PlaceExpander,
) -> Result<(), ObjectError> {
    runtime.register_place_expander(ctx, operator, expander)
}
