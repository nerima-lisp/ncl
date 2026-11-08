use super::ThreadContext;

impl ThreadContext {
    /// Associate this context with its owning evaluator for the duration of an evaluation.
    pub const fn set_evaluator_runtime(&mut self, runtime: *mut ()) {
        self.evaluator_runtime = std::ptr::NonNull::new(runtime);
    }

    /// Replace the active evaluator and return its previous association.
    #[must_use]
    pub const fn replace_evaluator_runtime(
        &mut self,
        runtime: *mut (),
    ) -> Option<std::ptr::NonNull<()>> {
        let previous = self.evaluator_runtime;
        self.evaluator_runtime = std::ptr::NonNull::new(runtime);
        previous
    }

    /// Clear the evaluator association after an outer evaluation returns.
    pub const fn clear_evaluator_runtime(&mut self) {
        self.evaluator_runtime = None;
    }

    /// Restore an evaluator association saved before nested evaluation.
    pub const fn restore_evaluator_runtime(&mut self, evaluator: Option<std::ptr::NonNull<()>>) {
        self.evaluator_runtime = evaluator;
    }

    #[must_use]
    pub const fn evaluator_runtime(&self) -> Option<std::ptr::NonNull<()>> {
        self.evaluator_runtime
    }
}
