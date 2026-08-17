#[path = "common/evaluate_compiled.rs"]
mod common;
#[path = "common/evaluation.rs"]
mod evaluation;

use common::evaluate_compiled as evaluate;
use ncl_compiler::Instruction;
use ncl_runtime::{Runtime, RuntimeError, Value};

struct CompiledMode;

impl evaluation::EvaluationMode for CompiledMode {
    fn evaluate(runtime: &Runtime, source: &str) -> Result<Vec<Value>, RuntimeError> {
        runtime.eval_compiled_source(source)
    }
}

type TestRuntime = evaluation::TestRuntime<CompiledMode>;

fn evaluation_fails(source: &str) -> bool {
    Runtime::new().eval_compiled_source(source).is_err()
}

#[path = "compiled/clos.rs"]
mod clos;
#[path = "common/conditions.rs"]
mod conditions;
#[path = "compiled/control.rs"]
mod control;
#[path = "common/format_streams.rs"]
mod format_streams;
#[path = "compiled/iteration.rs"]
mod iteration;
#[path = "compiled/language.rs"]
mod language;
#[path = "compiled/packages.rs"]
mod packages;
#[path = "compiled/sequences.rs"]
mod sequences;
#[path = "compiled/setf.rs"]
mod setf;
#[path = "compiled/types.rs"]
mod types;
