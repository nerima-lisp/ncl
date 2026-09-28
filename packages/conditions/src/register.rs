//! Registration of the owned symbols and the standard condition hierarchy.

use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, Instance, LambdaList, MultipleValues, ObjectError, Package,
    Parameter, ParameterType, Runtime, ThreadContext, Word, car, cdr, make_cons,
    set_symbol_special, slot_ref, string_length, string_ref, symbol_name, with_roots,
};

use crate::class::{HIERARCHY, install_class, wire_superclass};
use crate::symbols::{SymbolKind, SymbolRow, symbols};

const ONE_ANY: &[Parameter] = &[Parameter {
    name: BuiltinName::new("VALUE"),
    ty: ParameterType::Any,
}];
const TWO_ANY: &[Parameter] = &[
    Parameter {
        name: BuiltinName::new("CLASS"),
        ty: ParameterType::Any,
    },
    Parameter {
        name: BuiltinName::new("HANDLER"),
        ty: ParameterType::Any,
    },
];
const CONDITION_ARGUMENT: Parameter = Parameter {
    name: BuiltinName::new("CONDITION"),
    ty: ParameterType::Any,
};
const CONTINUE_FORMAT_CONTROL: Parameter = Parameter {
    name: BuiltinName::new("CONTINUE-FORMAT-CONTROL"),
    ty: ParameterType::Any,
};
const FORMAT_ARGUMENT: Parameter = Parameter {
    name: BuiltinName::new("FORMAT-ARGUMENT"),
    ty: ParameterType::Any,
};

/// Register every owned symbol and the standard condition hierarchy.
///
/// This is the per-crate registration entry point that `ncl-stdlib` calls in
/// dependency order. It creates an internal context, so callers pass only the
/// shared runtime.
///
/// # Errors
/// Returns an object-layer error when allocation, interning, or registration
/// fails.
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    let mut ctx = ThreadContext::new();
    ctx.register(runtime)?;
    for row in symbols() {
        register_symbol(runtime, &mut ctx, row)?;
    }
    install_hierarchy(runtime, &mut ctx)?;
    runtime.register_lisp_error_converter(crate::condition_from_lisp_error);
    register_condition_builtins(runtime, &mut ctx)?;
    Ok(())
}

fn register_condition_builtins(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
) -> Result<(), ObjectError> {
    let one = Builtin {
        lambda_list: LambdaList::fixed(ONE_ANY),
        convention: BuiltinConvention::Direct(Arity::exact(1)),
    };
    let condition_and_rest = Builtin {
        lambda_list: LambdaList::with_rest(&[CONDITION_ARGUMENT], FORMAT_ARGUMENT),
        convention: BuiltinConvention::Adapted,
    };
    let cerror = Builtin {
        lambda_list: LambdaList::with_rest(
            &[CONTINUE_FORMAT_CONTROL, CONDITION_ARGUMENT],
            FORMAT_ARGUMENT,
        ),
        convention: BuiltinConvention::Adapted,
    };
    for (name, implementation) in [
        (
            "SIGNAL",
            BuiltinImplementation::adapted(condition_and_rest, signal_builtin, |args| {
                Ok(args.as_slice().to_vec())
            }),
        ),
        (
            "ERROR",
            BuiltinImplementation::adapted(condition_and_rest, error_builtin, |args| {
                Ok(args.as_slice().to_vec())
            }),
        ),
        (
            "WARN",
            BuiltinImplementation::adapted(condition_and_rest, warn_builtin, |args| {
                Ok(args.as_slice().to_vec())
            }),
        ),
        (
            "CELL-ERROR-NAME",
            BuiltinImplementation::direct(one, cell_error_name_builtin),
        ),
        (
            "CERROR",
            BuiltinImplementation::adapted(cerror, cerror_builtin, |args| {
                Ok(args.as_slice().to_vec())
            }),
        ),
    ] {
        runtime.register_builtin(
            ctx,
            BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new(name)),
            implementation,
        )?;
    }
    let push = Builtin {
        lambda_list: LambdaList::fixed(TWO_ANY),
        convention: BuiltinConvention::Direct(Arity::exact(2)),
    };
    runtime.register_builtin(
        ctx,
        BuiltinIdentifier::new(BuiltinPackage::NclExt, BuiltinName::new("PUSH-HANDLER")),
        BuiltinImplementation::direct(push, push_handler_builtin),
    )?;
    runtime.register_builtin(
        ctx,
        BuiltinIdentifier::new(BuiltinPackage::NclExt, BuiltinName::new("POP-HANDLER")),
        BuiltinImplementation::direct(one, pop_handler_builtin),
    )?;
    Ok(())
}

fn cell_error_name_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    slot_ref(ctx, Instance::from_word(args.required(0)?), 0)
}

const fn condition_object_error(error: crate::ConditionError) -> ObjectError {
    match error {
        crate::ConditionError::Object(error) => error,
        crate::ConditionError::Unhandled
        | crate::ConditionError::NotACondition
        | crate::ConditionError::RestartNotFound
        | crate::ConditionError::ChainCorrupt => ObjectError::Layout,
    }
}

fn condition_argument(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    value: Word,
    default_class: &str,
    format_arguments: &[Word],
) -> Result<Word, ObjectError> {
    if crate::condition_class_of(ctx, value).is_ok() {
        return Ok(value);
    }
    if string_text(ctx, value).is_some() {
        let class =
            crate::condition_class(ctx, runtime, default_class).ok_or(ObjectError::Layout)?;
        let mut class = class.as_word();
        let class_token = ncl_object::push_root(ctx, &mut class);
        let mut value = value;
        let value_token = ncl_object::push_root(ctx, &mut value);
        let result = with_roots(ctx, format_arguments, |ctx, roots| {
            let format_arguments = roots.iter().map(|root| **root).collect::<Vec<_>>();
            let arguments = argument_list(ctx, runtime, &format_arguments)?;
            with_roots(ctx, &[arguments], |ctx, roots| {
                let arguments = **roots.first().ok_or(ObjectError::Layout)?;
                crate::make_condition(
                    ctx,
                    runtime,
                    crate::ConditionClass::from_word(class),
                    &[value, arguments],
                )
                .map_err(condition_object_error)
            })
        });
        ncl_object::pop_root(ctx, value_token);
        ncl_object::pop_root(ctx, class_token);
        return result;
    }
    let class_name = symbol_text(ctx, value)?;
    let class = crate::condition_class(ctx, runtime, &class_name).ok_or(ObjectError::TypeError)?;
    crate::make_condition(ctx, runtime, class, &[]).map_err(condition_object_error)
}

fn argument_list(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
) -> Result<Word, ObjectError> {
    with_roots(ctx, values, |ctx, roots| {
        let mut list = Word::NIL;
        let token = ncl_object::push_root(ctx, &mut list);
        let result = (|| {
            for value in roots.iter().rev() {
                list = make_cons(ctx, runtime, **value, list)?;
            }
            Ok(list)
        })();
        ncl_object::pop_root(ctx, token);
        result
    })
}

fn rest_arguments(args: &BuiltinArgs<'_>, start: usize) -> Result<Vec<Word>, ObjectError> {
    (start..args.len())
        .map(|index| args.get(index).ok_or(ObjectError::Layout))
        .collect()
}

fn signal_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let value = args.required(0)?;
    let format_arguments = rest_arguments(args, 1)?;
    let condition = condition_argument(ctx, runtime, value, "SIMPLE-CONDITION", &format_arguments)?;
    let result = with_roots(ctx, &[condition], |ctx, roots| {
        let condition = **roots.first().ok_or(ObjectError::Layout)?;
        crate::signal(ctx, condition).map_err(condition_object_error)
    });
    result.map(|()| Word::NIL)
}

fn error_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let value = args.required(0)?;
    let format_arguments = rest_arguments(args, 1)?;
    let condition = condition_argument(ctx, runtime, value, "SIMPLE-ERROR", &format_arguments)?;
    let report = condition_report(ctx, condition);
    let result = with_roots(ctx, &[condition], |ctx, roots| {
        let condition = **roots.first().ok_or(ObjectError::Layout)?;
        crate::error(ctx, condition).map_err(|error| match error {
            crate::ConditionError::Unhandled => {
                // check-added-lines: allow(unsupported) preserve unhandled condition propagation
                ObjectError::Unsupported
            }
            error @ (crate::ConditionError::NotACondition
            | crate::ConditionError::RestartNotFound
            | crate::ConditionError::ChainCorrupt
            | crate::ConditionError::Object(_)) => condition_object_error(error),
        })
    });
    match result {
        Ok(()) => Ok(Word::NIL),
        // check-added-lines: allow(unsupported) report unhandled condition
        Err(ObjectError::Unsupported) => {
            // check-added-lines: allow(unsupported) report unhandled condition
            // check-added-lines: allow(unsupported) report unhandled condition
            if let Some(message) = report.or_else(|| string_text(ctx, value)) {
                eprintln!("{message}");
            }
            // check-added-lines: allow(unsupported) unhandled condition propagation
            Err(ObjectError::Unsupported) // check-added-lines: allow(unsupported) propagate unhandled condition
        }
        Err(error) => Err(error),
    }
}

fn warn_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let value = args.required(0)?;
    let format_arguments = rest_arguments(args, 1)?;
    let condition = condition_argument(ctx, runtime, value, "SIMPLE-WARNING", &format_arguments)?;
    let result = with_roots(ctx, &[condition], |ctx, roots| {
        let condition = **roots.first().ok_or(ObjectError::Layout)?;
        crate::warn(ctx, condition).map_err(condition_object_error)
    });
    result.map(|()| Word::NIL)
}

fn cerror_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let continue_control = args.required(0)?;
    let datum = args.required(1)?;
    let format_arguments = rest_arguments(args, 2)?;
    let result = with_roots(ctx, &[continue_control, datum], |outer_ctx, outer| {
        with_roots(outer_ctx, &format_arguments, |ctx, format_roots| {
            let format_values = format_roots.iter().map(|root| **root).collect::<Vec<_>>();
            let condition = condition_argument(
                ctx,
                runtime,
                **outer.get(1).ok_or(ObjectError::Layout)?,
                "SIMPLE-ERROR",
                &format_values,
            )?;
            with_roots(ctx, &[condition], |ctx, condition_roots| {
                let refreshed_format_values =
                    format_roots.iter().map(|root| **root).collect::<Vec<_>>();
                let continue_args = argument_list(ctx, runtime, &refreshed_format_values)?;
                with_roots(ctx, &[continue_args], |ctx, continue_roots| {
                    let continue_control = **outer.first().ok_or(ObjectError::Layout)?;
                    let continue_args = **continue_roots.first().ok_or(ObjectError::Layout)?;
                    let condition = **condition_roots.first().ok_or(ObjectError::Layout)?;
                    crate::cerror(ctx, runtime, continue_control, continue_args, condition)
                        .map_err(condition_object_error)
                })
            })
        })
    });
    result.map(|()| Word::NIL)
}

fn symbol_text(ctx: &ThreadContext, symbol: Word) -> Result<String, ObjectError> {
    let name = symbol_name(ctx, symbol)?;
    let length = string_length(ctx, name)?;
    let mut text = String::with_capacity(length);
    for index in 0..length {
        text.push(string_ref(ctx, name, index)?);
    }
    Ok(text)
}

fn string_text(ctx: &ThreadContext, value: Word) -> Option<String> {
    let length = string_length(ctx, value).ok()?;
    let mut text = String::with_capacity(length);
    for index in 0..length {
        text.push(string_ref(ctx, value, index).ok()?);
    }
    Some(text)
}

fn condition_report(ctx: &ThreadContext, condition: Word) -> Option<String> {
    let instance = Instance::from_word(condition);
    let control = string_text(ctx, slot_ref(ctx, instance, 0).ok()?)?;
    let mut arguments = slot_ref(ctx, instance, 1).ok()?;
    let mut report = String::new();
    let mut chars = control.chars();
    while let Some(character) = chars.next() {
        if character == '~' {
            if let Some(directive) = chars.next() {
                if matches!(directive, 'a' | 'A') {
                    let argument = car(ctx, arguments).ok()?;
                    arguments = cdr(ctx, arguments).ok()?;
                    if let Some(number) = argument.as_fixnum() {
                        report.push_str(&number.to_string());
                    } else if let Some(text) = string_text(ctx, argument) {
                        report.push_str(&text);
                    } else {
                        report.push_str("#<OBJECT>");
                    }
                } else {
                    report.push('~');
                    report.push(directive);
                }
            } else {
                report.push('~');
            }
        } else {
            report.push(character);
        }
    }
    Some(report)
}

fn push_handler_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let class_name = symbol_text(ctx, args.required(0)?)?;
    let class = crate::condition_class(ctx, runtime, &class_name).ok_or(ObjectError::Layout)?;
    let chain = crate::push_handler(ctx, runtime, class, args.required(1)?)
        .map_err(condition_object_error)?;
    Ok(chain.as_word())
}

fn pop_handler_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    crate::pop_handler(
        ctx,
        runtime,
        crate::HandlerChain::from_word(args.required(0)?),
    );
    Ok(Word::NIL)
}

fn register_symbol(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    row: &SymbolRow,
) -> Result<(), ObjectError> {
    let package = runtime.ensure_package(ctx, row.package)?;
    let (symbol, _status) = Package::from_word(package).intern(ctx, runtime, row.name)?;
    match row.kind {
        SymbolKind::Function | SymbolKind::ClassAndFunction => {
            runtime.define_function(ctx, row.package, row.name, Word::UNBOUND)?;
        }
        SymbolKind::Variable => set_symbol_special(ctx, symbol, true)?,
        SymbolKind::Class
        | SymbolKind::Other
        | SymbolKind::Constant
        | SymbolKind::Type
        | SymbolKind::Macro
        | SymbolKind::MacroAndClass
        | SymbolKind::SpecialOperatorAndClass
        | SymbolKind::VariableAndFunction => {}
    }
    Ok(())
}

fn install_hierarchy(runtime: &Runtime, ctx: &mut ThreadContext) -> Result<(), ObjectError> {
    for row in symbols() {
        match row.kind {
            SymbolKind::Class | SymbolKind::ClassAndFunction => {
                install_class(ctx, runtime, row.name)?;
            }
            SymbolKind::Function
            | SymbolKind::Variable
            | SymbolKind::Other
            | SymbolKind::Constant
            | SymbolKind::Type
            | SymbolKind::Macro
            | SymbolKind::MacroAndClass
            | SymbolKind::SpecialOperatorAndClass
            | SymbolKind::VariableAndFunction => {}
        }
    }
    for row in HIERARCHY {
        if let Some(parent) = row.superclass {
            wire_superclass(ctx, runtime, row.name, parent)?;
        }
    }
    Ok(())
}
