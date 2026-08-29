use std::collections::HashMap;
use std::rc::Rc;

use ncl_compiler::{
    Constant, DestructureLambdaList, DestructurePattern, DestructureSpec, FunctionCode, FunctionId,
    HandlerBindClause, HandlerCaseClause, Instruction, Program, RestartBindClause,
    RestartCaseClause,
};
use ncl_syntax::Span;

use crate::environment::normalize_name;
use crate::error::ThrowTag;
use crate::evaluator::{ConditionHandlerBinding, RestartBinding};
use crate::{Environment, ReturnValue, Runtime, RuntimeError, Value};

#[path = "vm_destructuring.rs"]
mod destructuring;
#[allow(clippy::wildcard_imports)]
use destructuring::*;

pub fn run_entry(
    runtime: &Runtime,
    program: &Rc<Program>,
    function_id: FunctionId,
    environment: &Environment,
    span: Span,
) -> Result<Value, RuntimeError> {
    let Some(function) = program.functions.get(function_id) else {
        return Err(invalid("compiled function id is out of range", span));
    };
    if !function.parameters.is_empty()
        || !function.optional.is_empty()
        || !function.keywords.is_empty()
        || function.has_keyword_section
        || function.rest.is_some()
        || !function.auxiliary.is_empty()
    {
        return Err(RuntimeError::Arity {
            function: function
                .name
                .as_deref()
                .unwrap_or("compiled entry function")
                .to_string(),
            expected: "0".to_string(),
            actual: 0,
        });
    }
    run_code(runtime, program, function, environment.clone(), span)
}

pub fn run(
    runtime: &Runtime,
    program: &Rc<Program>,
    function_id: FunctionId,
    environment: &Environment,
    arguments: &[Value],
    span: Span,
) -> Result<Value, RuntimeError> {
    let Some(function) = program.functions.get(function_id) else {
        return Err(invalid("compiled function id is out of range", span));
    };
    let (optional_supplied_count, key_start) = argument_layout(function, arguments)?;

    let local = environment.child();
    let _dynamic_guard = runtime.dynamic_guard();
    bind_required(runtime, function, arguments, &local);
    bind_optional(
        runtime,
        program,
        function,
        arguments,
        optional_supplied_count,
        &local,
        span,
    )?;
    bind_rest(runtime, function, arguments, key_start, &local);
    bind_keywords(
        runtime, program, function, arguments, key_start, &local, span,
    )?;
    bind_auxiliary(runtime, program, function, &local, span)?;
    run_code(runtime, program, function, local, span)
}

fn argument_layout(
    function: &FunctionCode,
    arguments: &[Value],
) -> Result<(usize, usize), RuntimeError> {
    let required_count = function.parameters.len();
    let optional_count = function.optional.len();
    let maximum_count = required_count + optional_count;
    let function_name = function
        .name
        .as_deref()
        .unwrap_or("compiled function")
        .to_string();
    if arguments.len() < required_count {
        let expected =
            if optional_count > 0 || function.rest.is_some() || function.has_keyword_section {
                format!("at least {required_count}")
            } else {
                required_count.to_string()
            };
        return Err(RuntimeError::Arity {
            function: function_name,
            expected,
            actual: arguments.len(),
        });
    }
    let optional_supplied_count =
        supplied_optional_count(function, arguments, required_count, optional_count);
    let key_start = required_count + optional_supplied_count;
    if !function.has_keyword_section && function.rest.is_none() && arguments.len() > maximum_count {
        let expected = if optional_count > 0 {
            format!("at most {maximum_count}")
        } else {
            maximum_count.to_string()
        };
        return Err(RuntimeError::Arity {
            function: function_name,
            expected,
            actual: arguments.len(),
        });
    }
    Ok((optional_supplied_count, key_start))
}

fn supplied_optional_count(
    function: &FunctionCode,
    arguments: &[Value],
    required_count: usize,
    optional_count: usize,
) -> usize {
    let supplied_count = arguments
        .len()
        .saturating_sub(required_count)
        .min(optional_count);
    if !function.has_keyword_section {
        return supplied_count;
    }
    (0..supplied_count)
        .take_while(|index| {
            !matches!(
                arguments[required_count + *index],
                Value::Keyword(_) | Value::KeywordExact(_)
            )
        })
        .count()
}

fn bind_required(
    runtime: &Runtime,
    function: &FunctionCode,
    arguments: &[Value],
    local: &Environment,
) {
    for (index, (parameter, argument)) in function.parameters.iter().zip(arguments).enumerate() {
        if function
            .required_escaped
            .get(index)
            .copied()
            .unwrap_or(false)
        {
            runtime.define_exact_in(parameter, argument.clone(), local);
        } else {
            runtime.define_in(parameter, argument.clone(), local);
        }
    }
}

fn bind_optional(
    runtime: &Runtime,
    program: &Rc<Program>,
    function: &FunctionCode,
    arguments: &[Value],
    supplied_count: usize,
    local: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    for (index, specification) in function.optional.iter().enumerate() {
        let supplied =
            (index < supplied_count).then(|| &arguments[function.parameters.len() + index]);
        let value = match supplied {
            Some(argument) => argument.clone(),
            None => default_value(
                runtime,
                program,
                specification.default_function,
                local,
                span,
                "compiled optional default is out of range",
            )?,
        };
        define_binding(
            runtime,
            &specification.name,
            value,
            specification.name_escaped,
            local,
        );
        if let Some(name) = &specification.supplied_p {
            define_binding(
                runtime,
                name,
                Value::boolean(supplied.is_some()),
                specification.supplied_p_escaped.unwrap_or(false),
                local,
            );
        }
    }
    Ok(())
}

fn bind_rest(
    runtime: &Runtime,
    function: &FunctionCode,
    arguments: &[Value],
    key_start: usize,
    local: &Environment,
) {
    if let Some(name) = &function.rest {
        define_binding(
            runtime,
            name,
            Value::list(arguments[key_start..].to_vec()),
            function.rest_escaped,
            local,
        );
    }
}

fn bind_keywords(
    runtime: &Runtime,
    program: &Rc<Program>,
    function: &FunctionCode,
    arguments: &[Value],
    key_start: usize,
    local: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    if !function.has_keyword_section {
        return Ok(());
    }
    let keyword_arguments = &arguments[key_start..];
    if !keyword_arguments.len().is_multiple_of(2) {
        return Err(RuntimeError::InvalidForm {
            message: "keyword arguments must be supplied in pairs".to_string(),
            span: Some(span),
        });
    }
    let mut supplied = HashMap::new();
    let mut accepts_unknown = function.allow_other_keys;
    for pair in keyword_arguments.as_chunks::<2>().0 {
        let (Value::Keyword(keyword) | Value::KeywordExact(keyword)) = &pair[0] else {
            return Err(RuntimeError::InvalidForm {
                message: "keyword argument name must be a keyword".to_string(),
                span: Some(span),
            });
        };
        let name = keyword.to_string();
        if name == "ALLOW-OTHER-KEYS" && pair[1].is_truthy() {
            accepts_unknown = true;
        }
        supplied.insert(name, pair[1].clone());
    }
    if !accepts_unknown
        && let Some(name) = supplied.keys().find(|name| {
            *name != "ALLOW-OTHER-KEYS"
                && !function
                    .keywords
                    .iter()
                    .any(|specification| specification.keyword_name == **name)
        })
    {
        return Err(RuntimeError::InvalidForm {
            message: format!("unknown keyword :{name}"),
            span: Some(span),
        });
    }
    for specification in &function.keywords {
        let value = match supplied.get(&specification.keyword_name) {
            Some(argument) => argument.clone(),
            None => default_value(
                runtime,
                program,
                specification.default_function,
                local,
                span,
                "compiled keyword default is out of range",
            )?,
        };
        define_binding(
            runtime,
            &specification.name,
            value,
            specification.name_escaped,
            local,
        );
        if let Some(name) = &specification.supplied_p {
            define_binding(
                runtime,
                name,
                Value::boolean(supplied.contains_key(&specification.keyword_name)),
                specification.supplied_p_escaped.unwrap_or(false),
                local,
            );
        }
    }
    Ok(())
}

fn bind_auxiliary(
    runtime: &Runtime,
    program: &Rc<Program>,
    function: &FunctionCode,
    local: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    for specification in &function.auxiliary {
        let value = default_value(
            runtime,
            program,
            specification.default_function,
            local,
            span,
            "compiled auxiliary default is out of range",
        )?;
        define_binding(
            runtime,
            &specification.name,
            value,
            specification.name_escaped,
            local,
        );
    }
    Ok(())
}

fn default_value(
    runtime: &Runtime,
    program: &Rc<Program>,
    function_id: FunctionId,
    local: &Environment,
    span: Span,
    message: &str,
) -> Result<Value, RuntimeError> {
    let Some(function) = program.functions.get(function_id) else {
        return Err(RuntimeError::InvalidForm {
            message: message.to_string(),
            span: Some(span),
        });
    };
    Ok(run_code(runtime, program, function, local.clone(), span)?.primary_value())
}

fn define_binding(runtime: &Runtime, name: &str, value: Value, escaped: bool, local: &Environment) {
    if escaped {
        runtime.define_exact_in(name, value, local);
    } else {
        runtime.define_in(name, value, local);
    }
}

fn run_code(
    runtime: &Runtime,
    program: &Rc<Program>,
    function: &FunctionCode,
    environment: Environment,
    span: Span,
) -> Result<Value, RuntimeError> {
    run_code_from(runtime, program, function, environment, span, 0)
}

#[path = "vm_execution.rs"]
mod execution;
#[allow(clippy::wildcard_imports)]
use execution::*;

fn constant_value(constant: &Constant, span: Span) -> Result<Value, RuntimeError> {
    match constant {
        Constant::Nil => Ok(Value::Nil),
        Constant::Boolean(value) => Ok(Value::boolean(*value)),
        Constant::Integer(value) => Ok(Value::Integer(*value)),
        Constant::Rational {
            numerator,
            denominator,
        } => Value::rational(i128::from(*numerator), i128::from(*denominator)).map_err(|_| {
            RuntimeError::InvalidForm {
                message: "compiled rational constant is invalid".to_owned(),
                span: Some(span),
            }
        }),
        Constant::Float(value) => Ok(Value::Float(*value)),
        Constant::String(value) => Ok(Value::string(value.clone())),
        Constant::Character(value) => Ok(Value::Character(*value)),
        Constant::Symbol(value) => Ok(Value::symbol(value)),
        Constant::SymbolExact(value) => Ok(Value::symbol_exact(value)),
        Constant::Keyword(value) => Ok(Value::keyword(value)),
        Constant::KeywordExact(value) => Ok(Value::keyword_exact(value)),
    }
}

fn pop_value(stack: &mut Vec<Value>, span: Span, operation: &str) -> Result<Value, RuntimeError> {
    stack
        .pop()
        .ok_or_else(|| invalid(&format!("{operation} has no value on the stack"), span))
}

fn jump_target(function: &FunctionCode, target: usize, span: Span) -> Result<usize, RuntimeError> {
    if target >= function.instructions.len() {
        return Err(invalid("compiled jump target is out of range", span));
    }
    Ok(target)
}

fn invalid(message: &str, span: Span) -> RuntimeError {
    RuntimeError::InvalidForm {
        message: message.to_string(),
        span: Some(span),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn function(instructions: Vec<Instruction>) -> FunctionCode {
        FunctionCode {
            name: Some("test-function".to_string()),
            parameters: Vec::new(),
            required_escaped: Vec::new(),
            optional: Vec::new(),
            keywords: Vec::new(),
            has_keyword_section: false,
            allow_other_keys: false,
            rest: None,
            rest_escaped: false,
            auxiliary: Vec::new(),
            instructions,
        }
    }

    #[test]
    fn rejects_an_entry_function_id_out_of_range() {
        let runtime = Runtime::new();
        let program = Rc::new(Program {
            functions: Vec::new(),
            entry: 0,
        });
        let Err(error) = run_entry(&runtime, &program, 0, &Environment::new(), Span::new(0, 1))
        else {
            panic!("an invalid function id must be rejected");
        };

        assert!(matches!(
            error,
            RuntimeError::InvalidForm { message, .. }
                if message == "compiled function id is out of range"
        ));
    }

    #[test]
    fn rejects_invalid_jump_targets_before_indexing_the_instruction_stream() {
        let runtime = Runtime::new();
        let program = Rc::new(Program {
            functions: vec![function(vec![Instruction::Jump(1)])],
            entry: 0,
        });
        let Err(error) = run_entry(&runtime, &program, 0, &Environment::new(), Span::new(0, 1))
        else {
            panic!("an invalid jump target must be rejected");
        };

        assert!(matches!(
            error,
            RuntimeError::InvalidForm { message, .. }
                if message == "compiled jump target is out of range"
        ));
    }

    #[test]
    fn rejects_invalid_compiled_rational_constants() {
        let runtime = Runtime::new();
        let program = Rc::new(Program {
            functions: vec![function(vec![Instruction::Constant(Constant::Rational {
                numerator: 1,
                denominator: 0,
            })])],
            entry: 0,
        });

        let result = run_entry(&runtime, &program, 0, &Environment::new(), Span::new(0, 1));

        assert!(matches!(
            result,
            Err(RuntimeError::InvalidForm { message, .. })
                if message == "compiled rational constant is invalid"
        ));
    }

    #[test]
    fn rejects_too_few_required_arguments() {
        let runtime = Runtime::new();
        let mut compiled = function(vec![Instruction::Return]);
        compiled.parameters.push("value".to_string());
        let program = Rc::new(Program {
            functions: vec![compiled],
            entry: 0,
        });

        let result = run(
            &runtime,
            &program,
            0,
            &Environment::new(),
            &[],
            Span::new(0, 1),
        );

        assert!(
            matches!(result, Err(RuntimeError::Arity { expected, actual, .. }) if expected == "1" && actual == 0)
        );
    }

    #[test]
    fn rejects_too_many_arguments_without_a_rest_parameter() {
        let runtime = Runtime::new();
        let mut compiled = function(vec![Instruction::Return]);
        compiled.parameters.push("value".to_string());
        let program = Rc::new(Program {
            functions: vec![compiled],
            entry: 0,
        });

        let result = run(
            &runtime,
            &program,
            0,
            &Environment::new(),
            &[Value::Integer(1), Value::Integer(2)],
            Span::new(0, 1),
        );

        assert!(
            matches!(result, Err(RuntimeError::Arity { expected, actual, .. }) if expected == "1" && actual == 2)
        );
    }

    #[test]
    fn accepts_extra_arguments_when_a_rest_parameter_is_declared() {
        let runtime = Runtime::new();
        let mut compiled = function(vec![
            Instruction::Load("rest".to_string()),
            Instruction::Return,
        ]);
        compiled.parameters.push("value".to_string());
        compiled.rest = Some("rest".to_string());
        let program = Rc::new(Program {
            functions: vec![compiled],
            entry: 0,
        });

        let result = match run(
            &runtime,
            &program,
            0,
            &Environment::new(),
            &[Value::Integer(1), Value::Integer(2)],
            Span::new(0, 1),
        ) {
            Ok(value) => value,
            Err(error) => panic!("a rest parameter must accept additional arguments: {error}"),
        };

        assert_eq!(result.to_string(), "(2)");
    }

    #[test]
    fn rejects_entry_functions_with_parameters_or_dynamic_bindings() {
        let runtime = Runtime::new();
        let mut compiled = function(vec![Instruction::Return]);
        compiled.parameters.push("value".to_string());
        let program = Rc::new(Program {
            functions: vec![compiled],
            entry: 0,
        });

        let result = run_entry(&runtime, &program, 0, &Environment::new(), Span::new(0, 1));

        assert!(matches!(
            result,
            Err(RuntimeError::Arity { expected, actual, .. }) if expected == "0" && actual == 0
        ));
    }

    #[test]
    fn argument_layout_handles_optional_keyword_and_rest_shapes() {
        let mut optional = function(vec![]);
        optional.parameters.push("required".to_string());
        optional.optional.push(ncl_compiler::OptionalParameter {
            name: "optional".to_string(),
            name_escaped: false,
            default_function: 0,
            supplied_p: None,
            supplied_p_escaped: None,
        });
        optional.has_keyword_section = true;

        let layouts = [
            (&optional, vec![Value::Integer(1)], (0, 1)),
            (
                &optional,
                vec![Value::Integer(1), Value::Keyword("key".to_string().into())],
                (0, 1),
            ),
            (
                &optional,
                vec![Value::Integer(1), Value::Integer(2)],
                (1, 2),
            ),
        ];

        for (function, arguments, expected) in layouts {
            assert!(matches!(
                argument_layout(function, &arguments),
                Ok(layout) if layout == expected
            ));
        }
    }

    #[test]
    fn rejects_call_without_enough_stack_values() {
        let result = execute_call_instruction(
            &Runtime::new(),
            0,
            &mut Vec::new(),
            &Environment::new(),
            Span::new(0, 1),
        );

        assert!(matches!(
            result,
            Err(RuntimeError::InvalidForm { message, .. })
                if message == "call has too few stack values"
        ));
    }

    #[test]
    fn rejects_apply_without_enough_stack_values_or_a_final_list() {
        let mut stack = Vec::new();
        let result = execute_apply_instruction(
            &Runtime::new(),
            0,
            &mut stack,
            &Environment::new(),
            Span::new(0, 1),
        );
        assert!(matches!(
            result,
            Err(RuntimeError::InvalidForm { message, .. })
                if message == "apply has too few stack values"
        ));

        stack = vec![Value::Integer(1), Value::Integer(2)];
        let result = execute_apply_instruction(
            &Runtime::new(),
            1,
            &mut stack,
            &Environment::new(),
            Span::new(0, 1),
        );
        assert!(matches!(
            result,
            Err(RuntimeError::InvalidForm { message, .. })
                if message == "apply's final argument must be a proper list"
        ));
    }

    #[test]
    fn rejects_mapcar_without_enough_stack_values_or_proper_lists() {
        let result = execute_mapcar_instruction(
            &Runtime::new(),
            0,
            &mut Vec::new(),
            &Environment::new(),
            Span::new(0, 1),
        );
        assert!(matches!(
            result,
            Err(RuntimeError::InvalidForm { message, .. })
                if message == "mapcar has too few stack values"
        ));

        let mut stack = vec![Value::Integer(1), Value::Integer(2)];
        let result = execute_mapcar_instruction(
            &Runtime::new(),
            1,
            &mut stack,
            &Environment::new(),
            Span::new(0, 1),
        );
        assert!(matches!(
            result,
            Err(RuntimeError::InvalidForm { message, .. })
                if message == "mapcar arguments must be proper lists"
        ));
    }

    #[test]
    fn rejects_multiple_value_call_without_enough_stack_values() {
        let result = execute_multiple_value_call_instruction(
            &Runtime::new(),
            0,
            &mut Vec::new(),
            &Environment::new(),
            Span::new(0, 1),
        );

        assert!(matches!(
            result,
            Err(RuntimeError::InvalidForm { message, .. })
                if message == "multiple-value-call has too few stack values"
        ));
    }

    #[test]
    fn destructure_dotted_parts_normalizes_list_shapes() {
        let nested = Value::dotted_list(
            vec![Value::Integer(1)],
            Value::dotted_list(vec![Value::Integer(2)], Value::Integer(3)),
        );
        let cases = [
            (Value::Nil, vec![], Value::Nil),
            (
                Value::list(vec![Value::Integer(1)]),
                vec![Value::Integer(1)],
                Value::Nil,
            ),
            (
                Value::dotted_list(vec![Value::Integer(1)], Value::Nil),
                vec![Value::Integer(1)],
                Value::Nil,
            ),
            (
                Value::dotted_list(
                    vec![Value::Integer(1)],
                    Value::list(vec![Value::Integer(2)]),
                ),
                vec![Value::Integer(1), Value::Integer(2)],
                Value::Nil,
            ),
            (
                nested,
                vec![Value::Integer(1), Value::Integer(2)],
                Value::Integer(3),
            ),
            (
                Value::dotted_list(vec![Value::Integer(1)], Value::Integer(2)),
                vec![Value::Integer(1)],
                Value::Integer(2),
            ),
        ];

        for (value, expected_items, expected_tail) in cases {
            let Some((items, tail)) = destructure_dotted_parts(&value) else {
                panic!("a list-shaped value must be decomposed");
            };
            assert_eq!(
                items.iter().map(Value::to_string).collect::<Vec<_>>(),
                expected_items
                    .iter()
                    .map(Value::to_string)
                    .collect::<Vec<_>>()
            );
            assert_eq!(tail.to_string(), expected_tail.to_string());
        }

        assert!(destructure_dotted_parts(&Value::Integer(1)).is_none());
    }

    #[test]
    fn rejects_invalid_destructure_shapes() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        let span = Span::new(0, 1);

        let cases = [
            (
                DestructurePattern::List(vec![]),
                Value::Integer(1),
                "destructuring-bind pattern requires a proper list",
            ),
            (
                DestructurePattern::List(vec![DestructurePattern::Name("x".to_string())]),
                Value::Nil,
                "destructuring-bind pattern has the wrong number of elements",
            ),
            (
                DestructurePattern::Dotted {
                    items: vec![DestructurePattern::Name("x".to_string())],
                    tail: Box::new(DestructurePattern::Name("rest".to_string())),
                },
                Value::Nil,
                "destructuring-bind pattern has too few elements",
            ),
        ];

        for (pattern, value, message) in cases {
            let result = destructure_value(&pattern, value, &runtime, &environment, span);
            assert!(
                matches!(result, Err(RuntimeError::InvalidForm { message: actual, .. }) if actual == message)
            );
        }
    }

    #[test]
    fn binds_nested_destructure_patterns() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        let span = Span::new(0, 1);
        let cases = [
            (
                DestructurePattern::Name("value".to_string()),
                Value::Integer(1),
                vec![("value", "1")],
            ),
            (
                DestructurePattern::List(vec![
                    DestructurePattern::Name("first".to_string()),
                    DestructurePattern::Name("second".to_string()),
                ]),
                Value::list(vec![Value::Integer(1), Value::Integer(2)]),
                vec![("first", "1"), ("second", "2")],
            ),
            (
                DestructurePattern::Dotted {
                    items: vec![DestructurePattern::Name("first".to_string())],
                    tail: Box::new(DestructurePattern::Name("rest".to_string())),
                },
                Value::list(vec![
                    Value::Integer(1),
                    Value::Integer(2),
                    Value::Integer(3),
                ]),
                vec![("first", "1"), ("rest", "(2 3)")],
            ),
        ];

        for (pattern, value, expected_bindings) in cases {
            assert!(destructure_value(&pattern, value, &runtime, &environment, span).is_ok());
            for (name, expected) in expected_bindings {
                let Some(actual) = environment.lookup(name) else {
                    panic!("binding {name} was not created");
                };
                assert_eq!(actual.to_string(), expected);
            }
        }
    }

    #[test]
    fn executes_stack_transformations_as_a_table() {
        let runtime = Runtime::new();
        let span = Span::new(0, 1);
        let cases = [
            (Instruction::Pop, vec![Value::Integer(1)], vec![]),
            (
                Instruction::Dup,
                vec![Value::Integer(1)],
                vec![Value::Integer(1), Value::Integer(1)],
            ),
            (
                Instruction::Primary,
                vec![Value::values(vec![Value::Integer(1), Value::Integer(2)])],
                vec![Value::Integer(1)],
            ),
            (
                Instruction::Values(2),
                vec![Value::Integer(1), Value::Integer(2)],
                vec![Value::values(vec![Value::Integer(1), Value::Integer(2)])],
            ),
            (
                Instruction::MultipleValueList,
                vec![Value::values(vec![Value::Integer(1), Value::Integer(2)])],
                vec![Value::list(vec![Value::Integer(1), Value::Integer(2)])],
            ),
        ];

        for (instruction, mut stack, expected_stack) in cases {
            let mut scopes = Vec::new();
            let mut environment = Environment::new();
            let mut program_counter = 0;
            let result = execute_stack_instruction(
                &runtime,
                &instruction,
                &mut stack,
                &mut scopes,
                &mut environment,
                &mut program_counter,
                span,
            );
            assert!(matches!(result, Ok(true)));
            assert_eq!(
                stack.iter().map(Value::to_string).collect::<Vec<_>>(),
                expected_stack
                    .iter()
                    .map(Value::to_string)
                    .collect::<Vec<_>>()
            );
            assert_eq!(program_counter, 1);
        }
    }

    #[test]
    fn rejects_invalid_stack_transformations_as_a_table() {
        let runtime = Runtime::new();
        let span = Span::new(0, 1);
        let cases = [
            (Instruction::Pop, "pop has no value on the stack"),
            (Instruction::Dup, "dup has no value on the stack"),
            (
                Instruction::Primary,
                "primary value has no value on the stack",
            ),
            (Instruction::Values(1), "values has too few stack values"),
            (
                Instruction::MultipleValueList,
                "multiple-value-list has no value on the stack",
            ),
            (Instruction::ExitScope, "scope exit has no matching scope"),
        ];

        for (instruction, message) in cases {
            let mut stack = Vec::new();
            let mut scopes = Vec::new();
            let mut environment = Environment::new();
            let mut program_counter = 0;
            let result = execute_stack_instruction(
                &runtime,
                &instruction,
                &mut stack,
                &mut scopes,
                &mut environment,
                &mut program_counter,
                span,
            );
            let result_debug = format!("{result:?}");
            assert!(
                matches!(result, Err(RuntimeError::InvalidForm { message: actual, .. }) if actual == message),
                "{instruction:?}: {result_debug}"
            );
            assert_eq!(program_counter, 0);
        }
    }
}
