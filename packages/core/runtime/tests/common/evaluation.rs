use std::marker::PhantomData;

use ncl_runtime::{Runtime, RuntimeError, Value};

pub trait EvaluationMode {
    fn evaluate(runtime: &Runtime, source: &str) -> Result<Vec<Value>, RuntimeError>;
}

pub struct TestRuntime<M> {
    runtime: Runtime,
    mode: PhantomData<M>,
}

impl<M: EvaluationMode> TestRuntime<M> {
    pub fn new() -> Self {
        Self {
            runtime: Runtime::new(),
            mode: PhantomData,
        }
    }

    pub fn evaluate(&self, source: &str) -> Result<Vec<Value>, RuntimeError> {
        M::evaluate(&self.runtime, source)
    }
}

impl<M: EvaluationMode> Default for TestRuntime<M> {
    fn default() -> Self {
        Self::new()
    }
}
