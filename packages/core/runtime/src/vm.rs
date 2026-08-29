use std::collections::HashMap;
use std::rc::Rc;

use ncl_compiler::{
    DestructureLambdaList, DestructurePattern, DestructureSpec, FunctionCode, FunctionId,
    HandlerBindClause, HandlerCaseClause, Instruction, Program, RestartBindClause,
    RestartCaseClause,
};
use ncl_syntax::Span;

use crate::environment::normalize_name;
use crate::error::ThrowTag;
use crate::evaluator::{ConditionHandlerBinding, RestartBinding};
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
mod destructuring_tests;
#[cfg(test)]
mod execution_call_tests;
#[cfg(test)]
mod execution_stack_tests;
