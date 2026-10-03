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

    pub(crate) fn belongs_to(self, runtime: &Runtime) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn expansion(
        _: &mut ThreadContext,
        _: &Runtime,
        args: &[Word],
    ) -> Result<SetfExpansion, ObjectError> {
        if args.is_empty() {
            return Err(ObjectError::TypeError);
        }
        Ok(SetfExpansion {
            temporary_variables: vec![],
            value_forms: vec![],
            store_variables: vec![Word::fixnum(7)],
            store_form: Word::fixnum(8),
            access_form: Word::fixnum(9),
        })
    }

    #[test]
    fn registry_round_trip_and_runtime_identity_are_value_checked() -> Result<(), ObjectError> {
        let runtime = Runtime::new()?;
        let other = Runtime::new()?;
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)?;
        let operator = crate::symbol(&mut ctx, &runtime, "TEST-PLACE")?;
        let registry = PlaceRegistry::new(&runtime);
        assert!(registry.belongs_to(&runtime));
        assert!(!registry.belongs_to(&other));
        assert!(registry.get(&ctx, operator)?.is_none());
        registry.define(&ctx, operator, expansion)?;
        assert!(registry.get(&ctx, operator)?.is_some());
        assert_eq!(register_place(&ctx, &runtime, operator, expansion), Ok(()));
        Ok(())
    }
}
