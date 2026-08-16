#[path = "common/evaluate.rs"]
mod common;

use common::evaluate;
use ncl_runtime::{Runtime, RuntimeError};

#[path = "evaluator/clos.rs"]
mod clos;
#[path = "evaluator/conditions.rs"]
mod conditions;
#[path = "evaluator/control.rs"]
mod control;
#[path = "evaluator/format_streams.rs"]
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
