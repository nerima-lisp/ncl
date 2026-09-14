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
#[path = "dual_engine/cons_identity.rs"]
mod cons_identity;
#[path = "dual_engine/control.rs"]
mod control;
#[path = "dual_engine/core.rs"]
mod core;
#[path = "dual_engine/destructive_lists.rs"]
mod destructive_lists;
#[path = "dual_engine/let_bindings.rs"]
mod let_bindings;
#[path = "dual_engine/mapping_identity.rs"]
mod mapping_identity;
#[path = "dual_engine/modify_order.rs"]
mod modify_order;
#[path = "dual_engine/objects.rs"]
mod objects;
#[path = "dual_engine/packages.rs"]
mod packages;
#[path = "dual_engine/primitives.rs"]
mod primitives;
#[path = "dual_engine/psetf.rs"]
mod psetf;
#[path = "dual_engine/random.rs"]
mod random;
#[path = "dual_engine/sequences.rs"]
mod sequences;
#[path = "dual_engine/setf.rs"]
mod setf;
#[path = "dual_engine/setf_order.rs"]
mod setf_order;
#[path = "dual_engine/setf_values.rs"]
mod setf_values;
#[path = "dual_engine/streams.rs"]
mod streams;
#[path = "dual_engine/time.rs"]
mod time;
#[path = "dual_engine/vector_identity.rs"]
mod vector_identity;
