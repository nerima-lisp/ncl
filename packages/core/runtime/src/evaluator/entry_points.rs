#![allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
    /// Evaluates one parsed form using the tree-walking evaluator.
    ///
    /// # Errors
    ///
    /// Returns a [`RuntimeError`] when resolving or evaluating the form fails.
    pub fn eval(&self, form: &Form) -> Result<Value, RuntimeError> {
        let resolved = self.resolve_form(form)?;
        self.eval_in(&resolved, &self.global)
    }

    /// Reads and evaluates every top-level form in source text.
    ///
    /// # Errors
    ///
    /// Returns a [`RuntimeError`] when reading or evaluating any form fails.
    pub fn eval_source(&self, source: &str) -> Result<Vec<Value>, RuntimeError> {
        self.load_time_values.borrow_mut().clear();
        read(source)?.iter().map(|form| self.eval(form)).collect()
    }

    /// Compiles and evaluates one parsed form with the bytecode VM.
    ///
    /// # Errors
    ///
    /// Returns a [`RuntimeError`] when resolving, compiling, or evaluating the form fails.
    pub fn eval_compiled(&self, form: &Form) -> Result<Value, RuntimeError> {
        let resolved = self.resolve_form(form)?;
        let expanded = self.prepare_compiled_form(&resolved, &self.global)?;
        let program = Rc::new(Compiler::compile_form(&expanded)?);
        crate::vm::run_entry(self, &program, 0, &self.global, expanded.span)
            .map(|value| value.primary_value())
    }

    /// Compiles and evaluates every top-level form in source text.
    ///
    /// # Errors
    ///
    /// Returns a [`RuntimeError`] when reading, compiling, or evaluating any form fails.
    pub fn eval_compiled_source(&self, source: &str) -> Result<Vec<Value>, RuntimeError> {
        self.load_time_values.borrow_mut().clear();
        read(source)?
            .iter()
            .map(|form| self.eval_compiled(form))
            .collect()
    }
}
