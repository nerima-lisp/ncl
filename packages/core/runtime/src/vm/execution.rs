#![allow(clippy::wildcard_imports)]
use super::*;

mod application;
#[cfg(test)]
#[allow(clippy::wildcard_imports)]
pub(super) use application::*;
#[cfg(test)]
mod application_tests;

mod assignment;
#[allow(clippy::wildcard_imports)]
use assignment::*;

mod bindings;
#[allow(clippy::wildcard_imports)]
use bindings::*;

mod branch;
#[allow(clippy::wildcard_imports)]
use branch::*;
#[cfg(test)]
mod branch_tests;

mod control_dispatch;
#[allow(clippy::wildcard_imports)]
use control_dispatch::*;

mod control_scopes;
#[allow(clippy::wildcard_imports)]
use control_scopes::*;
#[cfg(test)]
mod control_scopes_tests;

mod execution_runtime;

mod handler_conditions;
#[allow(clippy::wildcard_imports)]
use handler_conditions::*;

mod handler_restart_dispatch;
#[allow(clippy::wildcard_imports)]
use handler_restart_dispatch::*;

mod pre_control;

mod stack_ops;
#[allow(clippy::wildcard_imports)]
pub(super) use stack_ops::*;

pub(super) fn run_code_from(
    runtime: &Runtime,
    program: &Rc<Program>,
    function: &FunctionCode,
    environment: Environment,
    span: Span,
    start: usize,
) -> Result<Value, RuntimeError> {
    execution_runtime::run_code_from(runtime, program, function, environment, span, start)
}
