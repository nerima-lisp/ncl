#[path = "common/evaluate.rs"]
mod common;
#[path = "common/evaluation.rs"]
mod evaluation;

use common::evaluate;
use ncl_runtime::{Runtime, RuntimeError, Value};

struct InterpretedMode;

impl evaluation::EvaluationMode for InterpretedMode {
    fn evaluate(runtime: &Runtime, source: &str) -> Result<Vec<Value>, RuntimeError> {
        runtime.eval_source(source)
    }
}

type TestRuntime = evaluation::TestRuntime<InterpretedMode>;

fn evaluation_fails(source: &str) -> bool {
    Runtime::new().eval_source(source).is_err()
}

#[path = "evaluator/clos.rs"]
mod clos;
#[path = "common/conditions.rs"]
mod conditions;
#[path = "evaluator/control.rs"]
mod control;
#[path = "common/format_streams.rs"]
mod format_streams;
#[path = "evaluator/iteration.rs"]
mod iteration;
#[path = "evaluator/language.rs"]
mod language;
#[path = "evaluator/packages.rs"]
mod packages;
#[path = "evaluator/sequences.rs"]
mod sequences;
#[path = "evaluator/setf.rs"]
mod setf;
#[path = "evaluator/types.rs"]
mod types;
