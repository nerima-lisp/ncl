//! The `MAKE-CONDITION`/restart-protocol builtins and the fixed-slot
//! condition accessors (C2).
//!
//! These are thin Lisp-callable wrappers over the record-chain primitives in
//! [`crate::restart`] and the slot-initarg metadata in [`crate::slots`].

use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, HeapRootedWord, HeapRootedWords, Instance, LambdaList,
    MultipleValues, ObjectError, ObjectRef, Package, Parameter, ParameterType, Runtime,
    ThreadContext, Word, classify_object, make_string, slot_ref,
};

use crate::ConditionError;

const DESIGNATOR: Parameter = Parameter {
    name: BuiltinName::new("DESIGNATOR"),
    ty: ParameterType::Any,
};
const ARGUMENT: Parameter = Parameter {
    name: BuiltinName::new("ARGUMENT"),
    ty: ParameterType::Any,
};
const REST: Parameter = Parameter {
    name: BuiltinName::new("REST"),
    ty: ParameterType::Any,
};
const CONDITION: &[Parameter] = &[Parameter {
    name: BuiltinName::new("CONDITION"),
    ty: ParameterType::Any,
}];

const fn condition_object_error(error: ConditionError) -> ObjectError {
    match error {
        ConditionError::Object(error) => error,
        ConditionError::RestartNotFound => ObjectError::ControlError,
        ConditionError::Unhandled
        | ConditionError::NotACondition
        | ConditionError::ChainCorrupt => ObjectError::Layout,
    }
}

fn symbol_text(ctx: &ThreadContext, symbol: Word) -> Result<String, ObjectError> {
    let name = ncl_object::symbol_name(ctx, symbol)?;
    let length = ncl_object::string_length(ctx, name)?;
    let mut text = String::with_capacity(length);
    for index in 0..length {
        text.push(ncl_object::string_ref(ctx, name, index)?);
    }
    Ok(text)
}

fn rest_arguments(
    ctx: &ThreadContext,
    args: &BuiltinArgs<'_>,
    start: usize,
) -> Result<HeapRootedWords, ObjectError> {
    let values = (start..args.len())
        .map(|index| args.get(index).ok_or(ObjectError::Layout))
        .collect::<Result<Vec<_>, _>>()?;
    HeapRootedWords::new(ctx, &values)
}

fn restart_name_word(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
) -> Result<Word, ObjectError> {
    make_string(ctx, runtime, &name.chars().collect::<Vec<_>>())
}

fn is_symbol(ctx: &ThreadContext, value: Word) -> bool {
    matches!(classify_object(ctx, value), ObjectRef::Symbol(_))
}

/// The identity [`ncl_object::KeywordAdapter`]: every builtin registered here
/// takes a fixed head plus `&rest`, so there is nothing for a keyword
/// adapter to do beyond passing the argument vector through unchanged. The
/// `Result` wrapper is part of the `KeywordAdapter` function-pointer type,
/// not a choice made here.
#[allow(clippy::unnecessary_wraps)]
fn identity_adapter(args: &BuiltinArgs<'_>) -> Result<Vec<Word>, ObjectError> {
    Ok(args.as_slice().to_vec())
}

/// `(make-condition type &rest initargs)`.
fn make_condition_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let designator = HeapRootedWord::new(ctx, args.required(0)?)?;
    let initargs = rest_arguments(ctx, args, 1)?;
    let name = symbol_text(ctx, designator.get())?;
    let class = crate::condition_class(ctx, runtime, &name).ok_or(ObjectError::TypeError)?;
    crate::slots::instantiate(ctx, runtime, class.as_word(), &initargs)
}

/// `(invoke-restart restart-designator &rest arguments)`.
fn invoke_restart_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let designator = HeapRootedWord::new(ctx, args.required(0)?)?;
    let arguments = rest_arguments(ctx, args, 1)?;
    let argument_words = || {
        arguments
            .iter()
            .map(HeapRootedWord::get)
            .collect::<Vec<_>>()
    };
    if is_symbol(ctx, designator.get()) {
        let text = symbol_text(ctx, designator.get())?;
        let name_word = restart_name_word(ctx, runtime, &text)?;
        let name = HeapRootedWord::new(ctx, name_word)?;
        crate::invoke_restart_by_name(ctx, name.get(), &argument_words())
            .map_err(condition_object_error)
    } else {
        crate::invoke_restart(ctx, designator.get(), &argument_words())
            .map_err(condition_object_error)
    }
}

/// `(find-restart name-or-restart &rest ignored)`. The optional
/// condition-association argument (ANSI's second parameter) is accepted but
/// not used to filter restarts.
fn find_restart_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let designator = HeapRootedWord::new(ctx, args.required(0)?)?;
    if !is_symbol(ctx, designator.get()) {
        return Ok(designator.get());
    }
    let text = symbol_text(ctx, designator.get())?;
    let name_word = restart_name_word(ctx, runtime, &text)?;
    let name = HeapRootedWord::new(ctx, name_word)?;
    crate::find_restart(ctx, name.get())
        .map(|found| found.unwrap_or(Word::NIL))
        .map_err(condition_object_error)
}

/// `(compute-restarts &rest ignored)`.
fn compute_restarts_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    _args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    crate::compute_restarts(ctx, runtime).map_err(condition_object_error)
}

/// `(restart-name restart)`, returning a `COMMON-LISP`-interned symbol.
fn restart_name_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let restart = HeapRootedWord::new(ctx, args.required(0)?)?;
    let name_word = crate::restart_name(ctx, restart.get()).map_err(condition_object_error)?;
    let name = HeapRootedWord::new(ctx, name_word)?;
    if name.get() == Word::NIL {
        return Ok(Word::NIL);
    }
    let length = ncl_object::string_length(ctx, name.get())?;
    let mut text = String::with_capacity(length);
    for index in 0..length {
        text.push(ncl_object::string_ref(ctx, name.get(), index)?);
    }
    let package = runtime.ensure_package(ctx, "COMMON-LISP")?;
    Package::from_word(package)
        .intern(ctx, runtime, &text)
        .map(|(symbol, _)| symbol)
}

/// Invoke the restart named `name`, or return `NIL` when it is not active,
/// backing `USE-VALUE`/`STORE-VALUE`/`CONTINUE`/`ABORT`/`MUFFLE-WARNING`.
fn invoke_named_restart(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
    arguments: &HeapRootedWords,
) -> Result<Word, ObjectError> {
    let name_value = restart_name_word(ctx, runtime, name)?;
    let name_word = HeapRootedWord::new(ctx, name_value)?;
    let arguments = arguments
        .iter()
        .map(HeapRootedWord::get)
        .collect::<Vec<_>>();
    match crate::invoke_restart_by_name(ctx, name_word.get(), &arguments) {
        Ok(value) => Ok(value),
        Err(ConditionError::RestartNotFound) => Ok(Word::NIL),
        Err(error) => Err(condition_object_error(error)),
    }
}

macro_rules! named_restart_shorthand {
    ($fn_name:ident, $restart_name:literal, forward_value) => {
        fn $fn_name(
            ctx: &mut ThreadContext,
            runtime: &Runtime,
            args: &BuiltinArgs<'_>,
            _values: &mut MultipleValues,
        ) -> Result<Word, ObjectError> {
            let arguments = HeapRootedWords::new(ctx, &[args.required(0)?])?;
            invoke_named_restart(ctx, runtime, $restart_name, &arguments)
        }
    };
    ($fn_name:ident, $restart_name:literal) => {
        fn $fn_name(
            ctx: &mut ThreadContext,
            runtime: &Runtime,
            _args: &BuiltinArgs<'_>,
            _values: &mut MultipleValues,
        ) -> Result<Word, ObjectError> {
            invoke_named_restart(ctx, runtime, $restart_name, &HeapRootedWords::default())
        }
    };
}

named_restart_shorthand!(use_value_builtin, "USE-VALUE", forward_value);
named_restart_shorthand!(store_value_builtin, "STORE-VALUE", forward_value);
named_restart_shorthand!(continue_builtin, "CONTINUE");
named_restart_shorthand!(abort_builtin, "ABORT");
named_restart_shorthand!(muffle_warning_builtin, "MUFFLE-WARNING");

/// `(ncl-ext::push-restart name function report interactive test)`, backing
/// `RESTART-BIND`/`RESTART-CASE`/`WITH-SIMPLE-RESTART`'s expansion exactly as
/// `NCL-EXT::PUSH-HANDLER` backs `HANDLER-BIND`'s.
fn push_restart_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let name = args.required(0)?;
    let function = args.required(1)?;
    let report = args.required(2)?;
    let interactive = args.required(3)?;
    let test = args.required(4)?;
    crate::push_restart(ctx, runtime, name, function, report, interactive, test)
        .map(crate::RestartRecord::as_word)
        .map_err(condition_object_error)
}

/// `(ncl-ext::pop-restart token)`.
fn pop_restart_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    crate::pop_restart(ctx, crate::RestartRecord::from_word(args.required(0)?));
    Ok(Word::NIL)
}

fn slot0_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    slot_ref(ctx, Instance::from_word(args.required(0)?), 0)
}

fn slot1_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    slot_ref(ctx, Instance::from_word(args.required(0)?), 1)
}

/// Register `MAKE-CONDITION`, the restart protocol, and the fixed-slot
/// condition accessors.
///
/// # Errors
/// Returns an object-layer error when registration fails.
pub fn register(runtime: &Runtime, ctx: &mut ThreadContext) -> Result<(), ObjectError> {
    let one = Builtin {
        lambda_list: LambdaList::fixed(CONDITION),
        convention: BuiltinConvention::Direct(Arity::exact(1)),
    };
    register_common_lisp_builtins(runtime, ctx, one)?;
    register_ncl_ext_builtins(runtime, ctx, one)
}

/// Register the `COMMON-LISP`-package restart-protocol and condition-accessor
/// builtins (`MAKE-CONDITION` through `ARITHMETIC-ERROR-OPERANDS`).
fn register_common_lisp_builtins(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    one: Builtin,
) -> Result<(), ObjectError> {
    let designator_and_rest = Builtin {
        lambda_list: LambdaList::with_rest(&[DESIGNATOR], REST),
        convention: BuiltinConvention::Adapted,
    };
    let value_and_rest = Builtin {
        lambda_list: LambdaList::with_rest(&[ARGUMENT], REST),
        convention: BuiltinConvention::Adapted,
    };
    let rest_only = Builtin {
        lambda_list: LambdaList::with_rest(&[], REST),
        convention: BuiltinConvention::Adapted,
    };
    for (name, implementation) in [
        (
            "MAKE-CONDITION",
            BuiltinImplementation::adapted(
                designator_and_rest,
                make_condition_builtin,
                identity_adapter,
            ),
        ),
        (
            "INVOKE-RESTART",
            BuiltinImplementation::adapted(
                designator_and_rest,
                invoke_restart_builtin,
                identity_adapter,
            ),
        ),
        (
            "FIND-RESTART",
            BuiltinImplementation::adapted(
                designator_and_rest,
                find_restart_builtin,
                identity_adapter,
            ),
        ),
        (
            "COMPUTE-RESTARTS",
            BuiltinImplementation::adapted(rest_only, compute_restarts_builtin, identity_adapter),
        ),
        (
            "RESTART-NAME",
            BuiltinImplementation::direct(one, restart_name_builtin),
        ),
        (
            "USE-VALUE",
            BuiltinImplementation::adapted(value_and_rest, use_value_builtin, identity_adapter),
        ),
        (
            "STORE-VALUE",
            BuiltinImplementation::adapted(value_and_rest, store_value_builtin, identity_adapter),
        ),
        (
            "CONTINUE",
            BuiltinImplementation::adapted(rest_only, continue_builtin, identity_adapter),
        ),
        (
            "ABORT",
            BuiltinImplementation::adapted(rest_only, abort_builtin, identity_adapter),
        ),
        (
            "MUFFLE-WARNING",
            BuiltinImplementation::adapted(rest_only, muffle_warning_builtin, identity_adapter),
        ),
        (
            "TYPE-ERROR-DATUM",
            BuiltinImplementation::direct(one, slot0_builtin),
        ),
        (
            "TYPE-ERROR-EXPECTED-TYPE",
            BuiltinImplementation::direct(one, slot1_builtin),
        ),
        (
            "SIMPLE-CONDITION-FORMAT-CONTROL",
            BuiltinImplementation::direct(one, slot0_builtin),
        ),
        (
            "SIMPLE-CONDITION-FORMAT-ARGUMENTS",
            BuiltinImplementation::direct(one, slot1_builtin),
        ),
        (
            "ARITHMETIC-ERROR-OPERATION",
            BuiltinImplementation::direct(one, slot0_builtin),
        ),
        (
            "ARITHMETIC-ERROR-OPERANDS",
            BuiltinImplementation::direct(one, slot1_builtin),
        ),
    ] {
        runtime.register_builtin(
            ctx,
            BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new(name)),
            implementation,
        )?;
    }
    Ok(())
}

/// Register `NCL-EXT::PUSH-RESTART`/`NCL-EXT::POP-RESTART`, the two
/// primitives `RESTART-BIND`/`RESTART-CASE`/`WITH-SIMPLE-RESTART` expand
/// onto (see `ncl-lib-macros`).
fn register_ncl_ext_builtins(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    one: Builtin,
) -> Result<(), ObjectError> {
    let five = Builtin {
        lambda_list: LambdaList::fixed(&[
            DESIGNATOR, DESIGNATOR, DESIGNATOR, DESIGNATOR, DESIGNATOR,
        ]),
        convention: BuiltinConvention::Direct(Arity::exact(5)),
    };
    runtime.register_builtin(
        ctx,
        BuiltinIdentifier::new(BuiltinPackage::NclExt, BuiltinName::new("PUSH-RESTART")),
        BuiltinImplementation::direct(five, push_restart_builtin),
    )?;
    runtime.register_builtin(
        ctx,
        BuiltinIdentifier::new(BuiltinPackage::NclExt, BuiltinName::new("POP-RESTART")),
        BuiltinImplementation::direct(one, pop_restart_builtin),
    )?;
    Ok(())
}
