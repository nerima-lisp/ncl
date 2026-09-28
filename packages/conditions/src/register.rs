//! Registration of the owned symbols and the standard condition hierarchy.

use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, Instance, LambdaList, MultipleValues, ObjectError, Package,
    Parameter, ParameterType, Runtime, ThreadContext, Word, set_symbol_special, slot_ref,
    string_length, string_ref, symbol_name,
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
) -> Result<Word, ObjectError> {
    if crate::condition_class_of(ctx, value).is_ok() {
        return Ok(value);
    }
    if string_text(ctx, value).is_some() {
        let class =
            crate::condition_class(ctx, runtime, default_class).ok_or(ObjectError::Layout)?;
        return crate::make_condition(ctx, runtime, class, &[value])
            .map_err(condition_object_error);
    }
    let class_name = symbol_text(ctx, value)?;
    let class = crate::condition_class(ctx, runtime, &class_name).ok_or(ObjectError::TypeError)?;
    crate::make_condition(ctx, runtime, class, &[]).map_err(condition_object_error)
}

fn signal_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let condition = condition_argument(ctx, runtime, args.required(0)?, "SIMPLE-CONDITION")?;
    crate::signal(ctx, condition)
        .map(|()| Word::NIL)
        .map_err(condition_object_error)
}

fn error_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let value = args.required(0)?;
    let condition = condition_argument(ctx, runtime, value, "SIMPLE-ERROR")?;
    match crate::error(ctx, condition) {
        Ok(()) => Ok(Word::NIL),
        Err(crate::ConditionError::Unhandled) => {
            if let Some(message) = string_text(ctx, value) {
                eprintln!("{message}");
            }
            // check-added-lines: allow(unsupported) unhandled condition propagation
            Err(ObjectError::Unsupported)
        }
        Err(error) => Err(condition_object_error(error)),
    }
}

fn warn_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let condition = condition_argument(ctx, runtime, args.required(0)?, "SIMPLE-WARNING")?;
    crate::warn(ctx, condition)
        .map(|()| Word::NIL)
        .map_err(condition_object_error)
}

fn cerror_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let condition = condition_argument(ctx, runtime, args.required(1)?, "SIMPLE-ERROR")?;
    crate::cerror(ctx, runtime, Word::NIL, Word::NIL, condition)
        .map(|()| Word::NIL)
        .map_err(condition_object_error)
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
