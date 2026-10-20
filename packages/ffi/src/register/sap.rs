//! Lisp adapters for SAP arithmetic over the checked pointer wrapper.

mod access;

use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, LambdaList, MultipleValues, ObjectError, Parameter, ParameterType,
    Runtime, ThreadContext, Word,
};

use crate::FfiError;
use crate::sap::{
    SystemAreaPointer, sap_eq, sap_ge, sap_gt, sap_le, sap_lt, signed_integer_to_word,
};

const SAP: Parameter = Parameter {
    name: BuiltinName::new("SAP"),
    ty: ParameterType::Any,
};
const OFFSET: Parameter = Parameter {
    name: BuiltinName::new("OFFSET"),
    ty: ParameterType::Any,
};
const SAP_PARAMETERS: &[Parameter] = &[SAP];
const SAP_OFFSET_PARAMETERS: &[Parameter] = &[SAP, OFFSET];

pub(super) fn install_sap_builtins(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
) -> Result<(), ObjectError> {
    let builtins: &[(&str, u8, &'static [Parameter], ncl_object::RustBuiltin)] = &[
        ("SAP-INT", 1, SAP_PARAMETERS, sap_int),
        ("INT-SAP", 1, SAP_PARAMETERS, int_sap),
        ("SAP+", 2, SAP_OFFSET_PARAMETERS, sap_plus),
        ("SAP-", 2, SAP_OFFSET_PARAMETERS, sap_minus),
        ("SAP=", 2, SAP_OFFSET_PARAMETERS, sap_equal),
        ("SAP<", 2, SAP_OFFSET_PARAMETERS, sap_less),
        ("SAP<=", 2, SAP_OFFSET_PARAMETERS, sap_less_equal),
        ("SAP>", 2, SAP_OFFSET_PARAMETERS, sap_greater),
        ("SAP>=", 2, SAP_OFFSET_PARAMETERS, sap_greater_equal),
    ];
    for &(name, arity, params, function) in builtins {
        register_direct(runtime, ctx, name, arity, params, function)?;
    }
    access::install(runtime, ctx)?;
    Ok(())
}

fn register_direct(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    name: &'static str,
    arity: u8,
    params: &'static [Parameter],
    function: ncl_object::RustBuiltin,
) -> Result<(), ObjectError> {
    runtime.register_builtin(
        ctx,
        BuiltinIdentifier::new(BuiltinPackage::NclFfi, BuiltinName::new(name)),
        BuiltinImplementation::direct(
            Builtin {
                lambda_list: LambdaList::fixed(params),
                convention: BuiltinConvention::Direct(Arity::exact(arity)),
            },
            function,
        ),
    )?;
    Ok(())
}

pub(super) fn sap_value(
    ctx: &ThreadContext,
    value: Word,
) -> Result<SystemAreaPointer, ObjectError> {
    SystemAreaPointer::from_word(ctx, value).map_err(FfiError::into_object_error)
}

pub(super) fn offset_value(value: Word) -> Result<isize, ObjectError> {
    value
        .as_fixnum()
        .and_then(|value| isize::try_from(value).ok())
        .ok_or(ObjectError::TypeError)
}

fn sap_int(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    sap_value(ctx, args.required(0)?)?
        .to_word(ctx, runtime)
        .map_err(FfiError::into_object_error)
}

fn int_sap(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let value = args.required(0)?;
    SystemAreaPointer::from_word(ctx, value).map_err(FfiError::into_object_error)?;
    Ok(value)
}

fn sap_plus(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let sap = sap_value(ctx, args.required(0)?)?;
    let offset = offset_value(args.required(1)?)?;
    sap.sap_plus(offset)
        .to_word(ctx, runtime)
        .map_err(FfiError::into_object_error)
}

fn sap_minus(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let left = sap_value(ctx, args.required(0)?)?;
    let right = sap_value(ctx, args.required(1)?)?;
    let difference = i128::try_from(left.sap_difference(right)).map_err(|_| ObjectError::Layout)?;
    signed_integer_to_word(ctx, runtime, difference).map_err(FfiError::into_object_error)
}

fn sap_equal(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    compare(ctx, args, sap_eq)
}

fn sap_less(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    compare(ctx, args, sap_lt)
}

fn sap_less_equal(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    compare(ctx, args, sap_le)
}

fn sap_greater(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    compare(ctx, args, sap_gt)
}

fn sap_greater_equal(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    compare(ctx, args, sap_ge)
}

fn compare(
    ctx: &ThreadContext,
    args: &BuiltinArgs<'_>,
    comparison: fn(SystemAreaPointer, SystemAreaPointer) -> bool,
) -> Result<Word, ObjectError> {
    let left = sap_value(ctx, args.required(0)?)?;
    let right = sap_value(ctx, args.required(1)?)?;
    Ok(if comparison(left, right) {
        Word::TRUE
    } else {
        Word::NIL
    })
}
