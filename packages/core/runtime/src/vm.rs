use std::rc::Rc;

use ncl_compiler::{Constant, FunctionCode, FunctionId, Instruction, Program};
use ncl_syntax::{FormKind, Span};

use crate::builtins::eql_value;
use crate::environment::normalize_name;
use crate::error::ThrowTag;
use crate::evaluator::{ConditionHandlerBinding, RestartBinding};
use crate::{Environment, ReturnValue, Runtime, RuntimeError, Value};

#[path = "vm/destructuring.rs"]
mod destructuring;

pub(crate) fn run_entry(
    runtime: &Runtime,
    program: Rc<Program>,
    function_id: FunctionId,
    environment: Environment,
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
    run_code(runtime, &program, function, environment, span)
}

pub(crate) fn run(
    runtime: &Runtime,
    program: Rc<Program>,
    function_id: FunctionId,
    environment: Environment,
    arguments: &[Value],
    span: Span,
) -> Result<Value, RuntimeError> {
    let Some(function) = program.functions.get(function_id) else {
        return Err(invalid("compiled function id is out of range", span));
    };
    let required_count = function.parameters.len();
    let optional_count = function.optional.len();
    let maximum_count = required_count + optional_count;
    if arguments.len() < required_count {
        let expected =
            if optional_count > 0 || function.rest.is_some() || function.has_keyword_section {
                format!("at least {required_count}")
            } else {
                required_count.to_string()
            };
        return Err(RuntimeError::Arity {
            function: function
                .name
                .as_deref()
                .unwrap_or("compiled function")
                .to_string(),
            expected,
            actual: arguments.len(),
        });
    }
    let optional_supplied_count = if function.has_keyword_section {
        let available = arguments
            .len()
            .saturating_sub(required_count)
            .min(optional_count);
        (0..available)
            .take_while(|index| {
                !matches!(
                    arguments[required_count + *index],
                    Value::Keyword(_) | Value::KeywordExact(_)
                )
            })
            .count()
    } else {
        arguments
            .len()
            .saturating_sub(required_count)
            .min(optional_count)
    };
    let key_start = required_count + optional_supplied_count;
    if !function.has_keyword_section && function.rest.is_none() && arguments.len() > maximum_count {
        let expected = if optional_count > 0 {
            format!("at most {maximum_count}")
        } else {
            maximum_count.to_string()
        };
        return Err(RuntimeError::Arity {
            function: function
                .name
                .as_deref()
                .unwrap_or("compiled function")
                .to_string(),
            expected,
            actual: arguments.len(),
        });
    }

    let local = environment.child();
    let _dynamic_guard = runtime.dynamic_guard();
    let _special_guard =
        runtime.special_declaration_guard(&function.special_names, &function.special_exact_names);
    for (index, (parameter, argument)) in
        function.parameters.iter().zip(arguments.iter()).enumerate()
    {
        if function
            .required_escaped
            .get(index)
            .copied()
            .unwrap_or(false)
        {
            runtime.define_exact_in(parameter, argument.clone(), &local);
        } else {
            runtime.define_in(parameter, argument.clone(), &local);
        }
    }
    for (index, specification) in function.optional.iter().enumerate() {
        let supplied =
            (index < optional_supplied_count).then(|| &arguments[required_count + index]);
        let value = if let Some(argument) = supplied {
            argument.clone()
        } else {
            let Some(default_function) = program.functions.get(specification.default_function)
            else {
                return Err(RuntimeError::InvalidForm {
                    message: "compiled optional default is out of range".to_string(),
                    span: Some(span),
                });
            };
            run_code(runtime, &program, default_function, local.clone(), span)?.primary_value()
        };
        if specification.name_escaped {
            runtime.define_exact_in(&specification.name, value, &local);
        } else {
            runtime.define_in(&specification.name, value, &local);
        }
        if let Some(supplied_p) = &specification.supplied_p {
            if specification.supplied_p_escaped.unwrap_or(false) {
                runtime.define_exact_in(supplied_p, Value::boolean(supplied.is_some()), &local);
            } else {
                runtime.define_in(supplied_p, Value::boolean(supplied.is_some()), &local);
            }
        }
    }
    if let Some(rest) = &function.rest {
        let rest_start = key_start;
        let value = Value::list(arguments[rest_start..].to_vec());
        if function.rest_escaped {
            runtime.define_exact_in(rest, value, &local);
        } else {
            runtime.define_in(rest, value, &local);
        }
    }
    if function.has_keyword_section {
        let keyword_arguments = &arguments[key_start..];
        if keyword_arguments.len() % 2 != 0 {
            return Err(RuntimeError::InvalidForm {
                message: "keyword arguments must be supplied in pairs".to_string(),
                span: Some(span),
            });
        }
        let mut supplied_keywords = Vec::new();
        let mut accepts_unknown_keywords = function.allow_other_keys;
        for pair in keyword_arguments.chunks_exact(2) {
            let (keyword_name, keyword_name_escaped) = match &pair[0] {
                Value::Keyword(keyword) => (keyword.to_string(), false),
                Value::KeywordExact(keyword) => (keyword.to_string(), true),
                _ => {
                    return Err(RuntimeError::InvalidForm {
                        message: "keyword argument name must be a keyword".to_string(),
                        span: Some(span),
                    });
                }
            };
            if compiled_keyword_matches(
                "ALLOW-OTHER-KEYS",
                false,
                &keyword_name,
                keyword_name_escaped,
            ) && pair[1].is_truthy()
            {
                accepts_unknown_keywords = true;
            }
            supplied_keywords.push((keyword_name, keyword_name_escaped, pair[1].clone()));
        }
        if !accepts_unknown_keywords {
            for (keyword_name, keyword_name_escaped, _) in &supplied_keywords {
                if !compiled_keyword_matches(
                    "ALLOW-OTHER-KEYS",
                    false,
                    keyword_name,
                    *keyword_name_escaped,
                ) && !function.keywords.iter().any(|specification| {
                    compiled_keyword_matches(
                        &specification.keyword_name,
                        specification.keyword_name_escaped,
                        keyword_name,
                        *keyword_name_escaped,
                    )
                }) {
                    return Err(RuntimeError::InvalidForm {
                        message: format!("unknown keyword :{keyword_name}"),
                        span: Some(span),
                    });
                }
            }
        }
        for specification in &function.keywords {
            let supplied = supplied_keywords
                .iter()
                .find(|(keyword_name, keyword_name_escaped, _)| {
                    compiled_keyword_matches(
                        &specification.keyword_name,
                        specification.keyword_name_escaped,
                        keyword_name,
                        *keyword_name_escaped,
                    )
                })
                .map(|(_, _, argument)| argument);
            let value = if let Some(argument) = supplied {
                argument.clone()
            } else {
                let Some(default_function) = program.functions.get(specification.default_function)
                else {
                    return Err(RuntimeError::InvalidForm {
                        message: "compiled keyword default is out of range".to_string(),
                        span: Some(span),
                    });
                };
                run_code(runtime, &program, default_function, local.clone(), span)?.primary_value()
            };
            if specification.name_escaped {
                runtime.define_exact_in(&specification.name, value, &local);
            } else {
                runtime.define_in(&specification.name, value, &local);
            }
            if let Some(supplied_p) = &specification.supplied_p {
                if specification.supplied_p_escaped.unwrap_or(false) {
                    runtime.define_exact_in(supplied_p, Value::boolean(supplied.is_some()), &local);
                } else {
                    runtime.define_in(supplied_p, Value::boolean(supplied.is_some()), &local);
                }
            }
        }
    }
    for specification in &function.auxiliary {
        let Some(default_function) = program.functions.get(specification.default_function) else {
            return Err(RuntimeError::InvalidForm {
                message: "compiled auxiliary default is out of range".to_string(),
                span: Some(span),
            });
        };
        let value =
            run_code(runtime, &program, default_function, local.clone(), span)?.primary_value();
        if specification.name_escaped {
            runtime.define_exact_in(&specification.name, value, &local);
        } else {
            runtime.define_in(&specification.name, value, &local);
        }
    }
    run_code(runtime, &program, function, local, span)
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

fn run_code_from(
    runtime: &Runtime,
    program: &Rc<Program>,
    function: &FunctionCode,
    mut environment: Environment,
    span: Span,
    start_program_counter: usize,
) -> Result<Value, RuntimeError> {
    let mut stack = Vec::new();
    let mut scopes: Vec<(Environment, usize, usize)> = Vec::new();
    let mut special_guards = Vec::new();
    let _dynamic_guard = runtime.dynamic_guard();
    let mut program_counter = start_program_counter;

    loop {
        let Some(instruction) = function.instructions.get(program_counter) else {
            return Err(invalid(
                "compiled function reached an invalid instruction pointer",
                span,
            ));
        };

        match instruction {
            Instruction::Constant(constant) => {
                stack.push(constant_value(constant));
                program_counter += 1;
            }
            Instruction::Quote(form) => {
                stack.push(runtime.quoted_value(form)?);
                program_counter += 1;
            }
            Instruction::QuasiQuote(form) => {
                stack.push(runtime.quasiquote_value(form, &environment)?);
                program_counter += 1;
            }
            Instruction::Load(name) => {
                let value = runtime.lookup_in(name, &environment).ok_or_else(|| {
                    RuntimeError::UnboundVariable {
                        name: name.clone(),
                        span: Some(span),
                    }
                })?;
                stack.push(value);
                program_counter += 1;
            }
            Instruction::LoadExact(name) => {
                let value = runtime.lookup_exact_in(name, &environment).ok_or_else(|| {
                    RuntimeError::UnboundVariable {
                        name: name.clone(),
                        span: Some(span),
                    }
                })?;
                stack.push(value);
                program_counter += 1;
            }
            Instruction::FunctionLoad(name) => {
                let value = runtime
                    .lookup_function_in(name, &environment)
                    .ok_or_else(|| RuntimeError::UnboundVariable {
                        name: name.clone(),
                        span: Some(span),
                    })?;
                stack.push(value);
                program_counter += 1;
            }
            Instruction::FunctionLoadExact(name) => {
                let value = runtime
                    .lookup_function_exact_in(name, &environment)
                    .ok_or_else(|| RuntimeError::UnboundVariable {
                        name: name.clone(),
                        span: Some(span),
                    })?;
                stack.push(value);
                program_counter += 1;
            }
            Instruction::FunctionCallLoad(name) => {
                let value = runtime
                    .lookup_callable_in(name, &environment)
                    .ok_or_else(|| RuntimeError::UnboundVariable {
                        name: name.clone(),
                        span: Some(span),
                    })?;
                stack.push(value);
                program_counter += 1;
            }
            Instruction::FunctionCallLoadExact(name) => {
                let value = runtime
                    .lookup_callable_exact_in(name, &environment)
                    .ok_or_else(|| RuntimeError::UnboundVariable {
                        name: name.clone(),
                        span: Some(span),
                    })?;
                stack.push(value);
                program_counter += 1;
            }
            Instruction::SetfFunctionLoad(name) => {
                let value = environment.lookup_setf_function(name).ok_or_else(|| {
                    RuntimeError::UnboundVariable {
                        name: format!("(SETF {name})"),
                        span: Some(span),
                    }
                })?;
                stack.push(value);
                program_counter += 1;
            }
            Instruction::IsBound(name) => {
                stack.push(Value::boolean(runtime.is_bound_in(name, &environment)));
                program_counter += 1;
            }
            Instruction::IsBoundExact(name) => {
                stack.push(Value::boolean(
                    runtime.is_bound_exact_in(name, &environment),
                ));
                program_counter += 1;
            }
            Instruction::Define(name) => {
                let value = stack
                    .last()
                    .cloned()
                    .ok_or_else(|| invalid("define has no value on the stack", span))?;
                let value = value.primary_value();
                runtime.define_in(name, value.clone(), &environment);
                *stack
                    .last_mut()
                    .ok_or_else(|| invalid("define has no value on the stack", span))? = value;
                program_counter += 1;
            }
            Instruction::DefineExact(name) => {
                let value = stack
                    .last()
                    .cloned()
                    .ok_or_else(|| invalid("define has no value on the stack", span))?;
                let value = value.primary_value();
                runtime.define_exact_in(name, value.clone(), &environment);
                *stack
                    .last_mut()
                    .ok_or_else(|| invalid("define has no value on the stack", span))? = value;
                program_counter += 1;
            }
            Instruction::DefineDynamic(name) => {
                let value = stack
                    .last()
                    .cloned()
                    .ok_or_else(|| invalid("define-dynamic has no value on the stack", span))?;
                let value = value.primary_value();
                runtime.define_dynamic(name, value.clone());
                *stack
                    .last_mut()
                    .ok_or_else(|| invalid("define-dynamic has no value on the stack", span))? =
                    value;
                program_counter += 1;
            }
            Instruction::DefineDynamicExact(name) => {
                let value = stack.last().cloned().ok_or_else(|| {
                    invalid("define-dynamic-exact has no value on the stack", span)
                })?;
                let value = value.primary_value();
                runtime.define_dynamic_exact(name, value.clone());
                *stack.last_mut().ok_or_else(|| {
                    invalid("define-dynamic-exact has no value on the stack", span)
                })? = value;
                program_counter += 1;
            }
            Instruction::DeclareSpecial { names, exact_names } => {
                for name in names {
                    runtime.declare_special(name, false);
                }
                for name in exact_names {
                    runtime.declare_special(name, true);
                }
                program_counter += 1;
            }
            Instruction::DefineFunction(name) => {
                let value = pop_value(&mut stack, span, "local function definition")?;
                environment.define_function(name, value);
                program_counter += 1;
            }
            Instruction::DefineFunctionExact(name) => {
                let value = pop_value(&mut stack, span, "local function definition")?;
                environment.define_function_exact(name, value);
                program_counter += 1;
            }
            Instruction::DefineSpecial { name, force } => {
                let value = stack
                    .last()
                    .cloned()
                    .ok_or_else(|| invalid("define-special has no value on the stack", span))?;
                if *force && runtime.is_constant_in(name) {
                    return Err(runtime.constant_modification_error(name, span));
                }
                let value = runtime.define_special_value(name, value.primary_value(), *force);
                *stack
                    .last_mut()
                    .ok_or_else(|| invalid("define-special has no value on the stack", span))? =
                    value;
                program_counter += 1;
            }
            Instruction::DefineSpecialExact { name, force } => {
                let value = stack
                    .last()
                    .cloned()
                    .ok_or_else(|| invalid("define-special has no value on the stack", span))?;
                if *force && runtime.is_constant_exact_in(name) {
                    return Err(runtime.constant_modification_error(name, span));
                }
                let value = runtime.define_special_value_exact(name, value.primary_value(), *force);
                *stack
                    .last_mut()
                    .ok_or_else(|| invalid("define-special has no value on the stack", span))? =
                    value;
                program_counter += 1;
            }
            Instruction::DefineValues(name) => {
                let value = stack
                    .last()
                    .cloned()
                    .ok_or_else(|| invalid("define-values has no value on the stack", span))?;
                runtime.define_in(name, value, &environment);
                program_counter += 1;
            }
            Instruction::DefineValuesExact(name) => {
                let value = stack
                    .last()
                    .cloned()
                    .ok_or_else(|| invalid("define-values has no value on the stack", span))?;
                runtime.define_exact_in(name, value, &environment);
                program_counter += 1;
            }
            Instruction::Set(name) => {
                let value = stack
                    .last()
                    .cloned()
                    .ok_or_else(|| invalid("setq has no value on the stack", span))?;
                let value = value.primary_value();
                runtime.set_or_define_in(name, value.clone(), &environment, span)?;
                *stack
                    .last_mut()
                    .ok_or_else(|| invalid("setq has no value on the stack", span))? = value;
                program_counter += 1;
            }
            Instruction::SetExact(name) => {
                let value = stack
                    .last()
                    .cloned()
                    .ok_or_else(|| invalid("setq has no value on the stack", span))?;
                let value = value.primary_value();
                runtime.set_or_define_exact_in(name, value.clone(), &environment, span)?;
                *stack
                    .last_mut()
                    .ok_or_else(|| invalid("setq has no value on the stack", span))? = value;
                program_counter += 1;
            }
            Instruction::Setf(place) => {
                let value = stack
                    .last()
                    .cloned()
                    .ok_or_else(|| invalid("setf has no value on the stack", span))?;
                runtime.set_place(place, value.clone(), &environment)?;
                let value = value.primary_value();
                *stack
                    .last_mut()
                    .ok_or_else(|| invalid("setf has no value on the stack", span))? = value;
                program_counter += 1;
            }
            Instruction::MapIntoSetf(place) => {
                let value = stack
                    .last()
                    .cloned()
                    .ok_or_else(|| invalid("map-into has no value on the stack", span))?;
                let value = value.primary_value();
                runtime.set_map_into_destination(place, value.clone(), &environment)?;
                *stack
                    .last_mut()
                    .ok_or_else(|| invalid("map-into has no value on the stack", span))? = value;
                program_counter += 1;
            }
            Instruction::Psetq(names) => {
                if stack.len() < names.len() {
                    return Err(invalid("psetq has fewer values than targets", span));
                }
                let values = stack.split_off(stack.len() - names.len());
                for (name, value) in names.iter().zip(values) {
                    let value = value.primary_value();
                    runtime.set_or_define_in(name, value, &environment, span)?;
                }
                stack.push(Value::Nil);
                program_counter += 1;
            }
            Instruction::PsetqExact(names) => {
                if stack.len() < names.len() {
                    return Err(invalid("psetq has fewer values than targets", span));
                }
                let values = stack.split_off(stack.len() - names.len());
                for ((name, escaped), value) in names.iter().zip(values) {
                    let value = value.primary_value();
                    if *escaped {
                        runtime.set_or_define_exact_in(name, value, &environment, span)?;
                    } else {
                        runtime.set_or_define_in(name, value, &environment, span)?;
                    }
                }
                stack.push(Value::Nil);
                program_counter += 1;
            }
            Instruction::Push(name) => {
                let value = pop_value(&mut stack, span, "push")?.primary_value();
                let current = runtime.lookup_in(name, &environment).ok_or_else(|| {
                    RuntimeError::UnboundVariable {
                        name: name.clone(),
                        span: Some(span),
                    }
                })?;
                let mut elements = current
                    .list_items()
                    .ok_or_else(|| invalid("PUSH place must contain a proper list", span))?;
                elements.insert(0, value);
                let result = Value::list(elements);
                runtime.set_or_define_in(name, result.clone(), &environment, span)?;
                stack.push(result);
                program_counter += 1;
            }
            Instruction::PushExact(name) => {
                let value = pop_value(&mut stack, span, "push")?.primary_value();
                let current = runtime.lookup_exact_in(name, &environment).ok_or_else(|| {
                    RuntimeError::UnboundVariable {
                        name: name.clone(),
                        span: Some(span),
                    }
                })?;
                let mut elements = current
                    .list_items()
                    .ok_or_else(|| invalid("PUSH place must contain a proper list", span))?;
                elements.insert(0, value);
                let result = Value::list(elements);
                runtime.set_or_define_exact_in(name, result.clone(), &environment, span)?;
                stack.push(result);
                program_counter += 1;
            }
            Instruction::PopPlace(name) => {
                let current = runtime.lookup_in(name, &environment).ok_or_else(|| {
                    RuntimeError::UnboundVariable {
                        name: name.clone(),
                        span: Some(span),
                    }
                })?;
                let mut elements = current
                    .list_items()
                    .ok_or_else(|| invalid("POP place must contain a proper list", span))?;
                let popped = if elements.is_empty() {
                    Value::Nil
                } else {
                    elements.remove(0)
                };
                let result = Value::list(elements);
                runtime.set_or_define_in(name, result, &environment, span)?;
                stack.push(popped);
                program_counter += 1;
            }
            Instruction::PopPlaceExact(name) => {
                let current = runtime.lookup_exact_in(name, &environment).ok_or_else(|| {
                    RuntimeError::UnboundVariable {
                        name: name.clone(),
                        span: Some(span),
                    }
                })?;
                let mut elements = current
                    .list_items()
                    .ok_or_else(|| invalid("POP place must contain a proper list", span))?;
                let popped = if elements.is_empty() {
                    Value::Nil
                } else {
                    elements.remove(0)
                };
                let result = Value::list(elements);
                runtime.set_or_define_exact_in(name, result, &environment, span)?;
                stack.push(popped);
                program_counter += 1;
            }
            Instruction::PushNew(name) => {
                let value = pop_value(&mut stack, span, "pushnew")?.primary_value();
                let current = runtime.lookup_in(name, &environment).ok_or_else(|| {
                    RuntimeError::UnboundVariable {
                        name: name.clone(),
                        span: Some(span),
                    }
                })?;
                let mut elements = current
                    .list_items()
                    .ok_or_else(|| invalid("PUSHNEW place must contain a proper list", span))?;
                if elements.iter().any(|candidate| eql_value(&value, candidate)) {
                    stack.push(current);
                } else {
                    elements.insert(0, value);
                    let result = Value::list(elements);
                    runtime.set_or_define_in(name, result.clone(), &environment, span)?;
                    stack.push(result);
                }
                program_counter += 1;
            }
            Instruction::PushNewExact(name) => {
                let value = pop_value(&mut stack, span, "pushnew")?.primary_value();
                let current = runtime.lookup_exact_in(name, &environment).ok_or_else(|| {
                    RuntimeError::UnboundVariable {
                        name: name.clone(),
                        span: Some(span),
                    }
                })?;
                let mut elements = current
                    .list_items()
                    .ok_or_else(|| invalid("PUSHNEW place must contain a proper list", span))?;
                if elements.iter().any(|candidate| eql_value(&value, candidate)) {
                    stack.push(current);
                } else {
                    elements.insert(0, value);
                    let result = Value::list(elements);
                    runtime.set_or_define_exact_in(name, result.clone(), &environment, span)?;
                    stack.push(result);
                }
                program_counter += 1;
            }
            Instruction::Rotatef(names) => {
                if stack.len() < names.len() {
                    return Err(invalid("rotatef has fewer values than targets", span));
                }
                let values = stack.split_off(stack.len() - names.len());
                for (index, name) in names.iter().enumerate() {
                    let source_index = if index + 1 == values.len() { 0 } else { index + 1 };
                    let value = values[source_index].primary_value();
                    runtime.set_or_define_in(name, value, &environment, span)?;
                }
                stack.push(Value::Nil);
                program_counter += 1;
            }
            Instruction::RotatefExact(names) => {
                if stack.len() < names.len() {
                    return Err(invalid("rotatef has fewer values than targets", span));
                }
                let values = stack.split_off(stack.len() - names.len());
                for (index, (name, escaped)) in names.iter().enumerate() {
                    let source_index = if index + 1 == values.len() { 0 } else { index + 1 };
                    let value = values[source_index].primary_value();
                    if *escaped {
                        runtime.set_or_define_exact_in(name, value, &environment, span)?;
                    } else {
                        runtime.set_or_define_in(name, value, &environment, span)?;
                    }
                }
                stack.push(Value::Nil);
                program_counter += 1;
            }
            Instruction::Shiftf(names) => {
                let required = names.len() + 1;
                if stack.len() < required {
                    return Err(invalid("shiftf has fewer values than targets", span));
                }
                let values = stack.split_off(stack.len() - required);
                let result = values[0].primary_value();
                for (index, name) in names.iter().enumerate() {
                    let value = values[index + 1].primary_value();
                    runtime.set_or_define_in(name, value, &environment, span)?;
                }
                stack.push(result);
                program_counter += 1;
            }
            Instruction::ShiftfExact(names) => {
                let required = names.len() + 1;
                if stack.len() < required {
                    return Err(invalid("shiftf has fewer values than targets", span));
                }
                let values = stack.split_off(stack.len() - required);
                let result = values[0].primary_value();
                for (index, (name, escaped)) in names.iter().enumerate() {
                    let value = values[index + 1].primary_value();
                    if *escaped {
                        runtime.set_or_define_exact_in(name, value, &environment, span)?;
                    } else {
                        runtime.set_or_define_in(name, value, &environment, span)?;
                    }
                }
                stack.push(result);
                program_counter += 1;
            }
            Instruction::MultipleValueSetq(names) => {
                let source = pop_value(&mut stack, span, "multiple-value-setq")?;
                let values = source.multiple_values();
                for (index, name) in names.iter().enumerate() {
                    let value = values.get(index).cloned().unwrap_or(Value::Nil);
                    runtime.set_or_define_in(name, value, &environment, span)?;
                }
                stack.push(source.primary_value());
                program_counter += 1;
            }
            Instruction::MultipleValueSetqExact(names) => {
                let source = pop_value(&mut stack, span, "multiple-value-setq")?;
                let values = source.multiple_values();
                for (index, (name, escaped)) in names.iter().enumerate() {
                    let value = values.get(index).cloned().unwrap_or(Value::Nil);
                    if *escaped {
                        runtime.set_or_define_exact_in(name, value, &environment, span)?;
                    } else {
                        runtime.set_or_define_in(name, value, &environment, span)?;
                    }
                }
                stack.push(source.primary_value());
                program_counter += 1;
            }
            Instruction::EnterScope => {
                scopes.push((
                    environment.clone(),
                    runtime.dynamic_depth(),
                    runtime.exact_dynamic_depth(),
                ));
                environment = environment.child();
                program_counter += 1;
            }
            Instruction::EnterMacroletEnvironment(binding_form) => {
                scopes.push((
                    environment.clone(),
                    runtime.dynamic_depth(),
                    runtime.exact_dynamic_depth(),
                ));
                let FormKind::List(binding_forms) = &binding_form.kind else {
                    return Err(invalid(
                        "macrolet environment bindings must be a list",
                        binding_form.span,
                    ));
                };
                environment = runtime.make_macrolet_environment(binding_forms, &environment)?;
                program_counter += 1;
            }
            Instruction::EnterSpecialScope { names, exact_names } => {
                special_guards.push(runtime.special_declaration_guard(names, exact_names));
                program_counter += 1;
            }
            Instruction::ExitSpecialScope => {
                special_guards
                    .pop()
                    .ok_or_else(|| invalid("special scope exit has no matching scope", span))?;
                program_counter += 1;
            }
            Instruction::ExitScope => {
                let (parent, depth, exact_depth) = scopes
                    .pop()
                    .ok_or_else(|| invalid("scope exit has no matching scope", span))?;
                runtime.truncate_dynamic(depth);
                runtime.truncate_exact_dynamic(exact_depth);
                environment = parent;
                program_counter += 1;
            }
            Instruction::Pop => {
                pop_value(&mut stack, span, "pop")?;
                program_counter += 1;
            }
            Instruction::Dup => {
                let value = stack
                    .last()
                    .cloned()
                    .ok_or_else(|| invalid("dup has no value on the stack", span))?;
                stack.push(value);
                program_counter += 1;
            }
            Instruction::Primary => {
                let value = pop_value(&mut stack, span, "primary value")?;
                stack.push(value.primary_value());
                program_counter += 1;
            }
            Instruction::Values(value_count) => {
                if stack.len() < *value_count {
                    return Err(invalid("values has too few stack values", span));
                }
                let values = stack.split_off(stack.len() - *value_count);
                stack.push(Value::values(values));
                program_counter += 1;
            }
            Instruction::NthValue(index_span) => {
                let values = pop_value(&mut stack, span, "nth-value value")?;
                let index_value = pop_value(&mut stack, span, "nth-value index")?;
                let index = match index_value.primary_value() {
                    Value::Integer(index) if index >= 0 => {
                        usize::try_from(index).map_err(|_| RuntimeError::NumericOverflow)?
                    }
                    Value::Integer(_) => {
                        return Err(RuntimeError::InvalidForm {
                            message: "nth-value index must be non-negative".to_string(),
                            span: Some(*index_span),
                        });
                    }
                    value => {
                        return Err(RuntimeError::Type {
                            expected: "INTEGER".to_string(),
                            actual: value.type_name().to_string(),
                            span: Some(*index_span),
                        });
                    }
                };
                let values = values.multiple_values();
                stack.push(values.get(index).cloned().unwrap_or(Value::Nil));
                program_counter += 1;
            }
            Instruction::MultipleValueList => {
                let value = pop_value(&mut stack, span, "multiple-value-list")?;
                stack.push(Value::list(value.multiple_values()));
                program_counter += 1;
            }
            Instruction::BindValues(names) => {
                let value = pop_value(&mut stack, span, "multiple-value-bind")?;
                let values = value.multiple_values();
                for (index, name) in names.iter().enumerate() {
                    runtime.define_in(
                        name,
                        values.get(index).cloned().unwrap_or(Value::Nil),
                        &environment,
                    );
                }
                program_counter += 1;
            }
            Instruction::BindValuesExact(names) => {
                let value = pop_value(&mut stack, span, "multiple-value-bind")?;
                let values = value.multiple_values();
                for (index, (name, escaped)) in names.iter().enumerate() {
                    let value = values.get(index).cloned().unwrap_or(Value::Nil);
                    if *escaped {
                        runtime.define_exact_in(name, value, &environment);
                    } else {
                        runtime.define_in(name, value, &environment);
                    }
                }
                program_counter += 1;
            }
            Instruction::Destructure(specification) => {
                let value = pop_value(&mut stack, span, "destructuring-bind")?;
                destructuring::destructure_specification(
                    specification,
                    value.primary_value(),
                    runtime,
                    program,
                    &environment,
                    span,
                )?;
                program_counter += 1;
            }
            Instruction::JumpIfFalse(target) => {
                let condition = pop_value(&mut stack, span, "conditional jump")?;
                if condition.is_truthy() {
                    program_counter += 1;
                } else {
                    program_counter = jump_target(function, *target, span)?;
                }
            }
            Instruction::Jump(target) => {
                program_counter = jump_target(function, *target, span)?;
            }
            Instruction::MakeClosure(function_id) => {
                if *function_id >= program.functions.len() {
                    return Err(invalid("compiled closure id is out of range", span));
                }
                stack.push(Value::compiled(
                    program.clone(),
                    *function_id,
                    environment.clone(),
                ));
                program_counter += 1;
            }
            Instruction::IgnoreErrors(function_id) => {
                let function = program.functions.get(*function_id).ok_or_else(|| {
                    invalid("compiled ignore-errors function id is out of range", span)
                })?;
                match run_code(runtime, program, function, environment.clone(), span) {
                    Ok(value) => stack.push(value),
                    Err(error @ RuntimeError::ReturnFrom { .. }) => return Err(error),
                    Err(error @ RuntimeError::Go { .. }) => return Err(error),
                    Err(error @ RuntimeError::InvokeRestart { .. }) => return Err(error),
                    Err(error) => {
                        stack.push(Value::values(vec![Value::Nil, Value::condition(&error)]));
                    }
                }
                program_counter += 1;
            }
            Instruction::HandlerCase { protected, clauses } => {
                let protected_function = program.functions.get(*protected).ok_or_else(|| {
                    invalid("compiled handler-case function id is out of range", span)
                })?;
                let guard = runtime.condition_handler_guard(
                    clauses
                        .iter()
                        .filter(|clause| !clause.no_error)
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
                    Ok(value) => {
                        if let Some(clause) = clauses.iter().find(|clause| clause.no_error) {
                            program.functions.get(clause.function).ok_or_else(|| {
                                invalid("compiled handler-case clause id is out of range", span)
                            })?;
                            let mut arguments = value.multiple_values();
                            arguments.truncate(clause.variable_count);
                            arguments.resize(clause.variable_count, Value::Nil);
                            stack.push(run(
                                runtime,
                                program.clone(),
                                clause.function,
                                environment.clone(),
                                &arguments,
                                span,
                            )?);
                        } else {
                            stack.push(value);
                        }
                    }
                    Err(error @ RuntimeError::ReturnFrom { .. }) => return Err(error),
                    Err(error @ RuntimeError::Go { .. }) => return Err(error),
                    Err(error @ RuntimeError::InvokeRestart { .. }) => return Err(error),
                    Err(error) => {
                        let Some(clause) = clauses.iter().find(|clause| {
                            !clause.no_error && error.matches_condition(&clause.condition)
                        }) else {
                            return Err(error);
                        };
                        program.functions.get(clause.function).ok_or_else(|| {
                            invalid("compiled handler-case clause id is out of range", span)
                        })?;
                        let arguments = if clause.variable.is_some() {
                            vec![Value::condition(&error)]
                        } else {
                            Vec::new()
                        };
                        stack.push(run(
                            runtime,
                            program.clone(),
                            clause.function,
                            environment.clone(),
                            &arguments,
                            span,
                        )?);
                    }
                }
                program_counter += 1;
            }
            Instruction::HandlerBind { body, handlers } => {
                let handler_bindings = handlers
                    .iter()
                    .map(|handler| {
                        program.functions.get(handler.function).ok_or_else(|| {
                            invalid("compiled handler-bind clause id is out of range", span)
                        })?;
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
                let body_function = program.functions.get(*body).ok_or_else(|| {
                    invalid("compiled handler-bind body id is out of range", span)
                })?;
                let guard = runtime.condition_handler_guard(handler_bindings);
                let body_result =
                    run_code(runtime, program, body_function, environment.clone(), span);
                drop(guard);
                match body_result {
                    Ok(value) => stack.push(value),
                    Err(error @ RuntimeError::ReturnFrom { .. }) => return Err(error),
                    Err(error @ RuntimeError::Go { .. }) => return Err(error),
                    Err(error @ RuntimeError::InvokeRestart { .. }) => return Err(error),
                    Err(error @ RuntimeError::Signaled { .. }) => return Err(error),
                    Err(error) => {
                        let Some(handler) = handlers
                            .iter()
                            .find(|handler| error.matches_condition(&handler.condition))
                        else {
                            return Err(error);
                        };
                        program.functions.get(handler.function).ok_or_else(|| {
                            invalid("compiled handler-bind clause id is out of range", span)
                        })?;
                        stack.push(run(
                            runtime,
                            program.clone(),
                            handler.function,
                            environment.clone(),
                            &[Value::condition(&error)],
                            span,
                        )?);
                    }
                }
                program_counter += 1;
            }
            Instruction::RestartBind { body, bindings } => {
                let mut restarts = Vec::with_capacity(bindings.len());
                for binding in bindings {
                    let binding_function =
                        program.functions.get(binding.function).ok_or_else(|| {
                            invalid("compiled restart-bind clause id is out of range", span)
                        })?;
                    let function = run_code(
                        runtime,
                        program,
                        binding_function,
                        environment.clone(),
                        span,
                    )?
                    .primary_value();
                    restarts.push((binding.name.as_str(), function));
                }
                let guard = runtime.restart_guard(
                    restarts
                        .iter()
                        .map(|(name, function)| {
                            RestartBinding::new((*name).to_string(), Some(function.clone()))
                        })
                        .collect(),
                );
                let body_function = program.functions.get(*body).ok_or_else(|| {
                    invalid("compiled restart-bind body id is out of range", span)
                })?;
                let body_result =
                    run_code(runtime, program, body_function, environment.clone(), span);
                drop(guard);
                match body_result {
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
                        let Some((_, function)) = restarts
                            .iter()
                            .find(|(name, _)| normalize_name(invoked.as_str()) == *name)
                        else {
                            return Err(error);
                        };
                        let argument_values = arguments
                            .iter()
                            .cloned()
                            .map(ReturnValue::into_value)
                            .collect::<Vec<_>>();
                        stack.push(runtime.apply_in(
                            function,
                            &argument_values,
                            span,
                            &environment,
                        )?);
                    }
                }
                program_counter += 1;
            }
            Instruction::Catch { tag, body } => {
                let tag_function = program.functions.get(*tag).ok_or_else(|| {
                    invalid("compiled catch tag function id is out of range", span)
                })?;
                let tag = run_code(runtime, program, tag_function, environment.clone(), span)?
                    .primary_value();
                let body_function = program.functions.get(*body).ok_or_else(|| {
                    invalid("compiled catch body function id is out of range", span)
                })?;
                match run_code(runtime, program, body_function, environment.clone(), span) {
                    Ok(value) => stack.push(value),
                    Err(RuntimeError::Throw {
                        tag: thrown_tag,
                        value,
                        ..
                    }) if thrown_tag.matches(&tag) => stack.push(value.into_value()),
                    Err(error) => return Err(error),
                }
                program_counter += 1;
            }
            Instruction::WithSimpleRestart { name, body } => {
                let body_function = program.functions.get(*body).ok_or_else(|| {
                    invalid("compiled with-simple-restart body id is out of range", span)
                })?;
                let guard = runtime.restart_guard(vec![RestartBinding::new(name.clone(), None)]);
                let body_result =
                    run_code(runtime, program, body_function, environment.clone(), span);
                drop(guard);
                match body_result {
                    Ok(value) => stack.push(value),
                    Err(RuntimeError::InvokeRestart {
                        name: invoked,
                        value,
                        ..
                    }) if normalize_name(invoked.as_str()) == *name => {
                        stack.push(value.into_value());
                    }
                    Err(error) => return Err(error),
                }
                program_counter += 1;
            }
            Instruction::RestartCase { protected, clauses } => {
                let protected_function = program.functions.get(*protected).ok_or_else(|| {
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
                        let Some(clause) = clauses.iter().find(|clause| {
                            normalize_name(invoked.as_str()) == clause.name.as_str()
                        }) else {
                            return Err(error);
                        };
                        program.functions.get(clause.function).ok_or_else(|| {
                            invalid("compiled restart-case clause id is out of range", span)
                        })?;
                        let argument_values = arguments
                            .iter()
                            .cloned()
                            .map(ReturnValue::into_value)
                            .collect::<Vec<_>>();
                        stack.push(run(
                            runtime,
                            program.clone(),
                            clause.function,
                            environment.clone(),
                            &argument_values,
                            span,
                        )?);
                    }
                }
                program_counter += 1;
            }
            Instruction::WithConditionRestarts {
                condition,
                restarts,
                body,
            } => {
                let condition_function = program.functions.get(*condition).ok_or_else(|| {
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

                let restarts_function = program.functions.get(*restarts).ok_or_else(|| {
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
                let body_function = program.functions.get(*body).ok_or_else(|| {
                    invalid(
                        "compiled with-condition-restarts body id is out of range",
                        span,
                    )
                })?;
                let body_result =
                    run_code(runtime, program, body_function, environment.clone(), span);
                drop(guard);
                stack.push(body_result?);
                program_counter += 1;
            }
            Instruction::Progv {
                symbols,
                values,
                body,
            } => {
                let symbols_function = program.functions.get(*symbols).ok_or_else(|| {
                    invalid("compiled progv symbol function id is out of range", span)
                })?;
                let symbols_value = run_code(
                    runtime,
                    program,
                    symbols_function,
                    environment.clone(),
                    span,
                )?
                .primary_value();
                let symbol_items =
                    symbols_value
                        .list_items()
                        .ok_or_else(|| RuntimeError::Type {
                            expected: "LIST".to_string(),
                            actual: symbols_value.type_name().to_string(),
                            span: Some(span),
                        })?;

                let values_function = program.functions.get(*values).ok_or_else(|| {
                    invalid("compiled progv value function id is out of range", span)
                })?;
                let values_value =
                    run_code(runtime, program, values_function, environment.clone(), span)?
                        .primary_value();
                let value_items = values_value
                    .list_items()
                    .ok_or_else(|| RuntimeError::Type {
                        expected: "LIST".to_string(),
                        actual: values_value.type_name().to_string(),
                        span: Some(span),
                    })?;

                let _dynamic_guard = runtime.dynamic_guard();
                for (index, symbol) in symbol_items.iter().enumerate() {
                    let name = symbol.symbol_name().ok_or_else(|| {
                        invalid("progv symbol list must contain only symbols", span)
                    })?;
                    runtime.define_dynamic(
                        name,
                        value_items.get(index).cloned().unwrap_or(Value::Nil),
                    );
                }
                let body_function = program.functions.get(*body).ok_or_else(|| {
                    invalid("compiled progv body function id is out of range", span)
                })?;
                stack.push(run_code(
                    runtime,
                    program,
                    body_function,
                    environment.clone(),
                    span,
                )?);
                program_counter += 1;
            }
            Instruction::Throw => {
                let value = pop_value(&mut stack, span, "throw")?;
                let tag = pop_value(&mut stack, span, "throw")?.primary_value();
                return Err(RuntimeError::Throw {
                    tag: ThrowTag::new(tag),
                    value: ReturnValue::new(value),
                    span: Some(span),
                });
            }
            Instruction::Block {
                function: function_id,
                name,
            } => {
                let function = program
                    .functions
                    .get(*function_id)
                    .ok_or_else(|| invalid("compiled block function id is out of range", span))?;
                let target = runtime.fresh_block_target();
                let block_environment = environment.child();
                block_environment.define_block(name, target);
                match run_code(runtime, program, function, block_environment, span) {
                    Ok(value) => stack.push(value),
                    Err(RuntimeError::ReturnFrom {
                        target: Some(return_target),
                        value,
                        ..
                    }) if return_target == target => {
                        stack.push(value.into_value());
                    }
                    Err(error) => return Err(error),
                }
                program_counter += 1;
            }
            Instruction::TagBody {
                function: function_id,
                tags,
            } => {
                let tagbody_function = program
                    .functions
                    .get(*function_id)
                    .ok_or_else(|| invalid("compiled tagbody function id is out of range", span))?;
                let target = runtime.fresh_block_target();
                let tag_environment = environment.child();
                for (tag, _) in tags {
                    tag_environment.define_tag(tag, target);
                }

                let mut tagbody_program_counter = 0;
                loop {
                    match run_code_from(
                        runtime,
                        program,
                        tagbody_function,
                        tag_environment.clone(),
                        span,
                        tagbody_program_counter,
                    ) {
                        Ok(_) => {
                            stack.push(Value::Nil);
                            break;
                        }
                        Err(RuntimeError::Go {
                            tag,
                            target: Some(go_target),
                            ..
                        }) if go_target == target => {
                            tagbody_program_counter = tags
                                .iter()
                                .find(|(known_tag, _)| known_tag == &tag)
                                .map(|(_, position)| *position)
                                .ok_or_else(|| {
                                    invalid("compiled GO target is missing from TAGBODY", span)
                                })?;
                        }
                        Err(error) => return Err(error),
                    }
                }
                program_counter += 1;
            }
            Instruction::Go { tag } => {
                return Err(RuntimeError::Go {
                    tag: tag.clone(),
                    target: environment.lookup_tag(tag),
                    span: Some(span),
                });
            }
            Instruction::UnwindProtect {
                protected: protected_id,
                cleanup: cleanup_id,
            } => {
                let protected_function = program.functions.get(*protected_id).ok_or_else(|| {
                    invalid(
                        "compiled unwind-protect protected function id is out of range",
                        span,
                    )
                })?;
                let cleanup_function = program.functions.get(*cleanup_id).ok_or_else(|| {
                    invalid(
                        "compiled unwind-protect cleanup function id is out of range",
                        span,
                    )
                })?;
                let protected_result = run_code(
                    runtime,
                    program,
                    protected_function,
                    environment.clone(),
                    span,
                );
                let cleanup_result = run_code(
                    runtime,
                    program,
                    cleanup_function,
                    environment.clone(),
                    span,
                );
                match cleanup_result {
                    Ok(_) => stack.push(protected_result?),
                    Err(error) => return Err(error),
                }
                program_counter += 1;
            }
            Instruction::ReturnFrom { name } => {
                let value = pop_value(&mut stack, span, "return-from")?;
                return Err(RuntimeError::ReturnFrom {
                    block: name.clone(),
                    target: environment.lookup_block(name),
                    value: ReturnValue::new(value),
                    span: Some(span),
                });
            }
            Instruction::Eval(form_span) => {
                let value = pop_value(&mut stack, span, "eval")?.primary_value();
                let form = runtime.form_from_value(&value, *form_span)?;
                stack.push(runtime.eval_values_in(&form, &environment)?);
                program_counter += 1;
            }
            Instruction::EvalWithEnvironment(form_span) => {
                let target_environment = pop_value(&mut stack, span, "eval environment")?;
                let target_environment = match target_environment.primary_value() {
                    Value::Environment(environment) => environment,
                    value => {
                        return Err(RuntimeError::Type {
                            expected: "ENVIRONMENT".to_string(),
                            actual: value.type_name().to_string(),
                            span: Some(*form_span),
                        });
                    }
                };
                let value = pop_value(&mut stack, span, "eval")?.primary_value();
                let form = runtime.form_from_value(&value, *form_span)?;
                stack.push(runtime.eval_values_in(&form, &target_environment)?);
                program_counter += 1;
            }
            Instruction::Call(argument_count) => {
                if stack.len() < argument_count.saturating_add(1) {
                    return Err(invalid("call has too few stack values", span));
                }
                let arguments_start = stack.len() - argument_count;
                let arguments = stack.split_off(arguments_start);
                let function_value = stack
                    .pop()
                    .ok_or_else(|| invalid("call has no function value", span))?;
                let arguments = arguments
                    .into_iter()
                    .map(|value| value.primary_value())
                    .collect::<Vec<_>>();
                stack.push(runtime.apply_in(
                    &function_value.primary_value(),
                    &arguments,
                    span,
                    &environment,
                )?);
                program_counter += 1;
            }
            Instruction::Apply(argument_count) => {
                if *argument_count == 0 || stack.len() < argument_count.saturating_add(1) {
                    return Err(invalid("apply has too few stack values", span));
                }
                let arguments_start = stack.len() - argument_count;
                let mut evaluated = stack.split_off(arguments_start);
                let function_value = stack
                    .pop()
                    .ok_or_else(|| invalid("apply has no function value", span))?;
                let final_value = evaluated
                    .pop()
                    .ok_or_else(|| invalid("apply has no final list", span))?;
                let mut arguments = evaluated
                    .into_iter()
                    .map(|value| value.primary_value())
                    .collect::<Vec<_>>();
                let mut final_arguments = final_value
                    .primary_value()
                    .list_items()
                    .ok_or_else(|| invalid("apply's final argument must be a proper list", span))?;
                arguments.append(&mut final_arguments);
                stack.push(runtime.apply_in(
                    &function_value.primary_value(),
                    &arguments,
                    span,
                    &environment,
                )?);
                program_counter += 1;
            }
            Instruction::MapCar(sequence_count) => {
                if *sequence_count == 0 || stack.len() < sequence_count.saturating_add(1) {
                    return Err(invalid("mapcar has too few stack values", span));
                }
                let sequences_start = stack.len() - *sequence_count;
                let sequences = stack.split_off(sequences_start);
                let function_value = stack
                    .pop()
                    .ok_or_else(|| invalid("mapcar has no function value", span))?;
                let lists = sequences
                    .iter()
                    .map(|value| {
                        value
                            .primary_value()
                            .list_items()
                            .ok_or_else(|| invalid("mapcar arguments must be proper lists", span))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let length = lists.iter().map(Vec::len).min().unwrap_or(0);
                let mut results = Vec::with_capacity(length);
                for index in 0..length {
                    let arguments = lists
                        .iter()
                        .map(|items| items[index].clone())
                        .collect::<Vec<_>>();
                    results.push(
                        runtime
                            .apply_in(
                                &function_value.primary_value(),
                                &arguments,
                                span,
                                &environment,
                            )?
                            .primary_value(),
                    );
                }
                stack.push(Value::list(results));
                program_counter += 1;
            }
            Instruction::MultipleValueCall(value_form_count) => {
                if stack.len() < value_form_count.saturating_add(1) {
                    return Err(invalid(
                        "multiple-value-call has too few stack values",
                        span,
                    ));
                }
                let start = stack.len() - value_form_count.saturating_add(1);
                let mut operands = stack.split_off(start);
                let function_value = operands
                    .first()
                    .cloned()
                    .ok_or_else(|| invalid("multiple-value-call has no function value", span))?;
                let mut arguments = Vec::new();
                for value in operands.drain(1..) {
                    arguments.extend(value.multiple_values());
                }
                stack.push(runtime.apply_in(
                    &function_value.primary_value(),
                    &arguments,
                    span,
                    &environment,
                )?);
                program_counter += 1;
            }
            Instruction::Return => {
                if !scopes.is_empty() {
                    return Err(invalid(
                        "compiled function returned with an open scope",
                        span,
                    ));
                }
                return pop_value(&mut stack, span, "return");
            }
        }
    }
}

fn constant_value(constant: &Constant) -> Value {
    match constant {
        Constant::Nil => Value::Nil,
        Constant::Boolean(value) => Value::boolean(*value),
        Constant::Integer(value) => Value::Integer(*value),
        Constant::Rational {
            numerator,
            denominator,
        } => Value::rational(i128::from(*numerator), i128::from(*denominator))
            .expect("compiler emitted an invalid rational constant"),
        Constant::Float(value) => Value::Float(*value),
        Constant::String(value) => Value::string(value.clone()),
        Constant::Character(value) => Value::Character(*value),
        Constant::Symbol(value) => Value::symbol(value),
        Constant::SymbolExact(value) => Value::symbol_exact(value),
        Constant::Keyword(value) => Value::keyword(value),
        Constant::KeywordExact(value) => Value::keyword_exact(value),
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

fn compiled_keyword_matches(
    specification_name: &str,
    specification_escaped: bool,
    actual_name: &str,
    _actual_escaped: bool,
) -> bool {
    if specification_escaped {
        specification_name == actual_name
    } else {
        normalize_name(specification_name) == actual_name
    }
}

fn invalid(message: &str, span: Span) -> RuntimeError {
    RuntimeError::InvalidForm {
        message: message.to_string(),
        span: Some(span),
    }
}
