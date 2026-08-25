use std::rc::Rc;

use ncl_compiler::Compiler;
use ncl_syntax::{Form, read};

use super::Runtime;
use crate::{RuntimeError, Value};

impl Runtime {
    pub fn eval(&self, form: &Form) -> Result<Value, RuntimeError> {
        let resolved = self.resolve_form(form)?;
        self.eval_in(&resolved, &self.global)
    }

    pub fn eval_source(&self, source: &str) -> Result<Vec<Value>, RuntimeError> {
        read(source)?.iter().map(|form| self.eval(form)).collect()
    }

    pub fn eval_compiled(&self, form: &Form) -> Result<Value, RuntimeError> {
        let resolved = self.resolve_form(form)?;
        let expanded = self.prepare_compiled_form(&resolved, &self.global)?;
        let program = Rc::new(Compiler::compile_form(&expanded)?);
        crate::vm::run_entry(self, program, 0, self.global.clone(), expanded.span)
            .map(|value| value.primary_value())
    }

    pub fn eval_compiled_source(&self, source: &str) -> Result<Vec<Value>, RuntimeError> {
        read(source)?
            .iter()
            .map(|form| self.eval_compiled(form))
            .collect()
    }
}
