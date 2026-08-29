//! Tests whose bodies are identical under the tree-walking evaluator and
//! the bytecode compiler run once here, via rstest, instead of once per
//! engine in evaluator.rs and compiled.rs.

// support.rs is shared with evaluator.rs and compiled.rs, each of which
// exercises a different subset of it; only evaluate_with is needed here.
#[allow(dead_code)]
mod support;

use ncl_runtime::{Runtime, RuntimeError, Value};

type EvalFn = fn(&Runtime, &str) -> Result<Vec<Value>, RuntimeError>;

#[path = "dual_engine/conditions.rs"]
mod conditions;
#[path = "dual_engine/control.rs"]
mod control;
#[path = "dual_engine/core.rs"]
mod core;
#[path = "dual_engine/objects.rs"]
mod objects;
#[path = "dual_engine/primitives.rs"]
mod primitives;
#[path = "dual_engine/sequences.rs"]
mod sequences;
#[path = "dual_engine/setf.rs"]
mod setf;
