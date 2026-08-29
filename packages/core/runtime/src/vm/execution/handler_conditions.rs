#[allow(clippy::wildcard_imports)]
use super::*;

pub(super) fn execute_handler_case_instruction(
    runtime: &Runtime,
    program: &Rc<Program>,
    protected: usize,
    clauses: &[HandlerCaseClause],
    stack: &mut Vec<Value>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    let protected_function = program
        .functions
        .get(protected)
        .ok_or_else(|| invalid("compiled handler-case function id is out of range", span))?;
    let guard = runtime.condition_handler_guard(
        clauses
            .iter()
            .map(|clause| ConditionHandlerBinding {
                condition: clause.condition.clone(),
                function: None,
                catch: true,
            })
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
        Err(
            error @ (RuntimeError::ReturnFrom { .. }
            | RuntimeError::Go { .. }
            | RuntimeError::InvokeRestart { .. }),
        ) => return Err(error),
        Err(error) => {
            let Some(clause) = clauses
                .iter()
                .find(|clause| error.matches_condition(&clause.condition))
            else {
                return Err(error);
            };
            program
                .functions
                .get(clause.function)
                .ok_or_else(|| invalid("compiled handler-case clause id is out of range", span))?;
            let arguments = if clause.variable.is_some() {
                vec![Value::condition(&error)]
            } else {
                Vec::new()
            };
            stack.push(run(
                runtime,
                program,
                clause.function,
                environment,
                &arguments,
                span,
            )?);
        }
    }
    Ok(())
}

pub(super) fn execute_handler_bind_instruction(
    runtime: &Runtime,
    program: &Rc<Program>,
    body: usize,
    handlers: &[HandlerBindClause],
    stack: &mut Vec<Value>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    let handler_bindings = handlers
        .iter()
        .map(|handler| {
            program
                .functions
                .get(handler.function)
                .ok_or_else(|| invalid("compiled handler-bind clause id is out of range", span))?;
            Ok(ConditionHandlerBinding {
                condition: handler.condition.clone(),
                function: Some(Value::compiled(
                    program.clone(),
                    handler.function,
                    environment.clone(),
                )),
                catch: false,
            })
        })
        .collect::<Result<Vec<_>, RuntimeError>>()?;
    let body_function = program
        .functions
        .get(body)
        .ok_or_else(|| invalid("compiled handler-bind body id is out of range", span))?;
    let guard = runtime.condition_handler_guard(handler_bindings);
    let body_result = run_code(runtime, program, body_function, environment.clone(), span);
    drop(guard);
    match body_result {
        Ok(value) => stack.push(value),
        Err(
            error @ (RuntimeError::ReturnFrom { .. }
            | RuntimeError::Go { .. }
            | RuntimeError::InvokeRestart { .. }
            | RuntimeError::Signaled(_)),
        ) => return Err(error),
        Err(error) => {
            let Some(handler) = handlers
                .iter()
                .find(|handler| error.matches_condition(&handler.condition))
            else {
                return Err(error);
            };
            program
                .functions
                .get(handler.function)
                .ok_or_else(|| invalid("compiled handler-bind clause id is out of range", span))?;
            stack.push(run(
                runtime,
                program,
                handler.function,
                environment,
                &[Value::condition(&error)],
                span,
            )?);
        }
    }
    Ok(())
}
