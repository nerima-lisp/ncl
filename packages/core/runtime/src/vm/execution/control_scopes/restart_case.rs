use std::rc::Rc;

use ncl_compiler::{FunctionId, Program, RestartCaseClause};
use ncl_syntax::Span;

use crate::environment::normalize_name;
use crate::evaluator::RestartBinding;
use crate::vm::entry::{run, run_code};
use crate::vm::primitives::invalid;
use crate::{Environment, ReturnValue, Runtime, RuntimeError, Value};

pub(in crate::vm::execution) fn execute_restart_case_instruction(
    runtime: &Runtime,
    program: &Rc<Program>,
    protected: FunctionId,
    clauses: &[RestartCaseClause],
    stack: &mut Vec<Value>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    let protected_function = program.functions.get(protected).ok_or_else(|| {
        invalid(
            "compiled restart-case protected function id is out of range",
            span,
        )
    })?;
    let guard = runtime.restart_guard(
        clauses
            .iter()
            .map(|clause| RestartBinding::new(clause.name.clone(), None))
            .collect(),
    );
    let protected_result = run_code(
        runtime,
        program,
        protected_function,
        environment.clone(),
        span,
    );
    drop(guard);
    match protected_result {
        Ok(value) => stack.push(value),
        Err(error) => {
            let RuntimeError::InvokeRestart {
                name: invoked,
                arguments,
                ..
            } = &error
            else {
                return Err(error);
            };
            let Some(clause) = clauses
                .iter()
                .find(|clause| normalize_name(invoked.as_str()) == clause.name.as_str())
            else {
                return Err(error);
            };
            program
                .functions
                .get(clause.function)
                .ok_or_else(|| invalid("compiled restart-case clause id is out of range", span))?;
            let argument_values = arguments
                .iter()
                .cloned()
                .map(ReturnValue::into_value)
                .collect::<Vec<_>>();
            stack.push(run(
                runtime,
                program,
                clause.function,
                environment,
                &argument_values,
                span,
            )?);
        }
    }
    Ok(())
}

pub(in crate::vm::execution) fn execute_with_condition_restarts_instruction(
    runtime: &Runtime,
    program: &Rc<Program>,
    functions: (FunctionId, FunctionId, FunctionId),
    stack: &mut Vec<Value>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    let (condition, restarts, body) = functions;
    let condition_function = program.functions.get(condition).ok_or_else(|| {
        invalid(
            "compiled with-condition-restarts condition function id is out of range",
            span,
        )
    })?;
    let condition_value = run_code(
        runtime,
        program,
        condition_function,
        environment.clone(),
        span,
    )?
    .primary_value();
    if condition_value.condition_type_name().is_none() {
        return Err(RuntimeError::Type {
            expected: "CONDITION".to_string(),
            actual: condition_value.type_name().to_string(),
            span: Some(span),
        });
    }
    let restarts_function = program.functions.get(restarts).ok_or_else(|| {
        invalid(
            "compiled with-condition-restarts restarts function id is out of range",
            span,
        )
    })?;
    let restarts_value = run_code(
        runtime,
        program,
        restarts_function,
        environment.clone(),
        span,
    )?
    .primary_value();
    let Some(restart_values) = restarts_value.list_items() else {
        return Err(RuntimeError::Type {
            expected: "LIST".to_string(),
            actual: restarts_value.type_name().to_string(),
            span: Some(span),
        });
    };
    if let Some(restart) = restart_values
        .iter()
        .find(|restart| restart.restart_name().is_none())
    {
        return Err(RuntimeError::Type {
            expected: "RESTART".to_string(),
            actual: restart.type_name().to_string(),
            span: Some(span),
        });
    }
    let guard = runtime.condition_restart_guard(condition_value, restart_values);
    let body_function = program.functions.get(body).ok_or_else(|| {
        invalid(
            "compiled with-condition-restarts body id is out of range",
            span,
        )
    })?;
    let body_result = run_code(runtime, program, body_function, environment.clone(), span);
    drop(guard);
    stack.push(body_result?);
    Ok(())
}
