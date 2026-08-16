#[path = "common/evaluate_compiled.rs"]
mod common;

use common::evaluate_compiled as evaluate;
use ncl_compiler::Instruction;
use ncl_runtime::{Runtime, RuntimeError};

#[path = "compiled/clos.rs"]
mod clos;
#[path = "compiled/conditions.rs"]
mod conditions;
#[path = "compiled/control.rs"]
mod control;
#[path = "compiled/format_streams.rs"]
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
