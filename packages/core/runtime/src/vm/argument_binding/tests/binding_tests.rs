use std::rc::Rc;

use ncl_compiler::Program;
use ncl_syntax::Span;

use crate::{Environment, Runtime, RuntimeError, Value};

use super::{bind_optional, bind_required, function};

#[test]
fn bind_required_honors_escaped_parameter_names() {
    let runtime = Runtime::new();
    let local = Environment::new();
    let mut compiled = function(vec![]);
    compiled.parameters.push("Value".to_string());
    compiled.required_escaped.push(true);

    bind_required(&runtime, &compiled, &[Value::Integer(3)], &local);

    assert!(matches!(
        local.lookup_exact("Value"),
        Some(Value::Integer(3))
    ));
    assert!(local.lookup_exact("value").is_none());
    assert!(local.lookup("value").is_none());
}

#[test]
fn bind_optional_rejects_an_out_of_range_default_function_id() {
    let runtime = Runtime::new();
    let local = Environment::new();
    let mut compiled = function(vec![]);
    compiled.optional.push(ncl_compiler::OptionalParameter {
        name: "opt".to_string(),
        name_escaped: false,
        default_function: 0,
        supplied_p: None,
        supplied_p_escaped: None,
    });
    let program = Rc::new(Program {
        functions: Vec::new(),
        entry: 0,
    });

    let result = bind_optional(
        &runtime,
        &program,
        &compiled,
        &[],
        0,
        &local,
        Span::new(0, 1),
    );

    assert!(
        matches!(result, Err(RuntimeError::InvalidForm { message, .. }) if message == "compiled optional default is out of range")
    );
}

#[test]
fn bind_optional_binds_supplied_values_and_supplied_p_with_escaped_names() {
    let runtime = Runtime::new();
    let local = Environment::new();
    let mut compiled = function(vec![]);
    compiled.optional.push(ncl_compiler::OptionalParameter {
        name: "Opt".to_string(),
        name_escaped: true,
        default_function: 0,
        supplied_p: Some("Opt-P".to_string()),
        supplied_p_escaped: Some(true),
    });
    let program = Rc::new(Program {
        functions: vec![function(vec![])],
        entry: 0,
    });

    let result = bind_optional(
        &runtime,
        &program,
        &compiled,
        &[Value::Integer(9)],
        1,
        &local,
        Span::new(0, 1),
    );

    assert!(result.is_ok());
    assert!(matches!(local.lookup_exact("Opt"), Some(Value::Integer(9))));
    assert!(matches!(
        local.lookup_exact("Opt-P"),
        Some(Value::Boolean(true))
    ));
}
