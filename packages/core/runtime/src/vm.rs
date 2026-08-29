use std::rc::Rc;

use ncl_compiler::{FunctionCode, HandlerBindClause, HandlerCaseClause, Instruction, Program};
use ncl_syntax::Span;

use crate::error::ThrowTag;
use crate::evaluator::ConditionHandlerBinding;
use crate::{Environment, ReturnValue, Runtime, RuntimeError, Value};

mod argument_binding;
mod destructuring;
mod entry;
mod execution;
mod primitives;

use entry::run_code;
pub use entry::{run, run_entry};

#[allow(clippy::wildcard_imports)]
use destructuring::*;
#[allow(clippy::wildcard_imports)]
use primitives::*;

#[cfg(test)]
mod destructuring_lambda_list_tests;
#[cfg(test)]
mod destructuring_pattern_dotted_tests;
#[cfg(test)]
mod destructuring_tests;
#[cfg(test)]
mod execution_call_tests;
#[cfg(test)]
mod execution_stack_tests;
