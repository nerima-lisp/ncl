use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, LambdaList, MultipleValues, ObjectError, Parameter, ParameterType,
    Runtime, Sequence, ThreadContext, Word, typed_builtin,
};

use crate::domain;

fn sequence_arg(ctx: &mut ThreadContext, args: &BuiltinArgs<'_>) -> Result<Sequence, ObjectError> {
    domain::sequence_value(ctx, args.required(0)?)
}

fn generic_length(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let sequence = sequence_arg(ctx, args)?;
    Ok(Word::fixnum(domain::sequence_length(ctx, sequence)? as i64))
}
fn generic_elt(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let sequence = sequence_arg(ctx, args)?;
    let index = args.required(1)?;
    domain::sequence_elt(ctx, sequence, index)
}
fn generic_copy(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let sequence = sequence_arg(ctx, args)?;
    domain::sequence_copy(ctx, runtime, sequence)
}
fn generic_reverse(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let sequence = sequence_arg(ctx, args)?;
    domain::sequence_reverse(ctx, runtime, sequence)
}
fn generic_nreverse(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let sequence = sequence_arg(ctx, args)?;
    domain::sequence_nreverse(ctx, sequence)
}
fn generic_fill(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let sequence = sequence_arg(ctx, args)?;
    let item = args.required(1)?;
    domain::sequence_fill(ctx, sequence, item)
}
fn generic_replace(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let destination = sequence_arg(ctx, args)?;
    let source = domain::sequence_value(ctx, args.required(1)?)?;
    domain::sequence_replace(ctx, destination, source)
}
fn generic_subseq(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let sequence = sequence_arg(ctx, args)?;
    let start = args.required(1)?;
    domain::sequence_subseq(ctx, runtime, sequence, start, args.get(2))
}
fn generic_make(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    domain::make_sequence(
        ctx,
        runtime,
        args.required(0)?,
        args.required(1)?,
        args.get(2),
    )
}

const fn parameter(name: &'static str, ty: ParameterType) -> Parameter {
    Parameter {
        name: BuiltinName::new(name),
        ty,
    }
}
const fn descriptor(required: &'static [Parameter]) -> Builtin {
    Builtin {
        lambda_list: LambdaList::fixed(required),
        convention: BuiltinConvention::Direct(Arity::exact(required.len() as u8)),
    }
}
const ONE_ANY: &[Parameter] = &[parameter("value", ParameterType::Any)];
const ONE_LIST: &[Parameter] = &[parameter("list", ParameterType::List)];
const TWO_N_LIST: &[Parameter] = &[
    parameter("n", ParameterType::Fixnum),
    parameter("list", ParameterType::List),
];
const TWO_ITEM_LIST: &[Parameter] = &[
    parameter("item", ParameterType::Any),
    parameter("list", ParameterType::List),
];
const TWO_ITEM_ALIST: &[Parameter] = &[
    parameter("item", ParameterType::Any),
    parameter("alist", ParameterType::List),
];
const TWO_CONS_VALUE: &[Parameter] = &[
    parameter("cons", ParameterType::Any),
    parameter("value", ParameterType::Any),
];
const SEQUENCE_INDEX: &[Parameter] = &[
    parameter("sequence", ParameterType::Sequence),
    parameter("index", ParameterType::Fixnum),
];
const SEQUENCE_ITEM: &[Parameter] = &[
    parameter("sequence", ParameterType::Sequence),
    parameter("item", ParameterType::Any),
];
const SEQUENCE_SOURCE: &[Parameter] = &[
    parameter("sequence", ParameterType::Sequence),
    parameter("source", ParameterType::Sequence),
];
const SUBSEQ_REQUIRED: &[Parameter] = &[
    parameter("sequence", ParameterType::Sequence),
    parameter("start", ParameterType::Fixnum),
];
const SUBSEQ_OPTIONAL: &[Parameter] = &[parameter("end", ParameterType::Fixnum)];
const MAKE_REQUIRED: &[Parameter] = &[
    parameter("type", ParameterType::Any),
    parameter("length", ParameterType::Fixnum),
];
const MAKE_OPTIONAL: &[Parameter] = &[parameter("initial-element", ParameterType::Any)];

typed_builtin!(atom_builtin, domain::atom, (value: Word));
typed_builtin!(cons_p_builtin, domain::cons_p, (value: Word));
typed_builtin!(list_p_builtin, domain::list_p, (value: Word));
typed_builtin!(end_p_builtin, domain::end_p, (value: Word));
typed_builtin!(car_builtin, domain::car, (value: ncl_object::List));
typed_builtin!(cdr_builtin, domain::cdr, (value: ncl_object::List));
typed_builtin!(list_length_builtin, domain::list_length, (value: ncl_object::List));
typed_builtin!(nth_builtin, domain::nth, (index: ncl_object::Fixnum, value: ncl_object::List));
typed_builtin!(nthcdr_builtin, domain::nthcdr, (index: ncl_object::Fixnum, value: ncl_object::List));
typed_builtin!(member_builtin, domain::member, (item: Word, value: ncl_object::List));
typed_builtin!(assoc_builtin, domain::assoc, (item: Word, value: ncl_object::List));
typed_builtin!(rplaca_builtin, domain::rplaca, (cons: Word, value: Word));
typed_builtin!(rplacd_builtin, domain::rplacd, (cons: Word, value: Word));

pub(crate) fn entry(name: &str) -> Option<BuiltinImplementation> {
    let implementation: (Builtin, ncl_object::RustBuiltin) = match name {
        "ATOM" => (descriptor(ONE_ANY), atom_builtin),
        "CONSP" => (descriptor(ONE_ANY), cons_p_builtin),
        "LISTP" => (descriptor(ONE_ANY), list_p_builtin),
        "ENDP" => (descriptor(ONE_ANY), end_p_builtin),
        "CAR" => (descriptor(ONE_LIST), car_builtin),
        "CDR" => (descriptor(ONE_LIST), cdr_builtin),
        "LIST-LENGTH" => (descriptor(ONE_LIST), list_length_builtin),
        "NTH" => (descriptor(TWO_N_LIST), nth_builtin),
        "NTHCDR" => (descriptor(TWO_N_LIST), nthcdr_builtin),
        "MEMBER" => (descriptor(TWO_ITEM_LIST), member_builtin),
        "ASSOC" => (descriptor(TWO_ITEM_ALIST), assoc_builtin),
        "RPLACA" => (descriptor(TWO_CONS_VALUE), rplaca_builtin),
        "RPLACD" => (descriptor(TWO_CONS_VALUE), rplacd_builtin),
        "LENGTH" => (descriptor(ONE_ANY), generic_length),
        "ELT" => (
            Builtin {
                lambda_list: LambdaList::fixed(SEQUENCE_INDEX),
                convention: BuiltinConvention::Direct(Arity::exact(2)),
            },
            generic_elt,
        ),
        "COPY-SEQ" => (descriptor(ONE_ANY), generic_copy),
        "REVERSE" => (descriptor(ONE_ANY), generic_reverse),
        "NREVERSE" => (descriptor(ONE_ANY), generic_nreverse),
        "FILL" => (
            Builtin {
                lambda_list: LambdaList::fixed(SEQUENCE_ITEM),
                convention: BuiltinConvention::Direct(Arity::exact(2)),
            },
            generic_fill,
        ),
        "REPLACE" => (
            Builtin {
                lambda_list: LambdaList::fixed(SEQUENCE_SOURCE),
                convention: BuiltinConvention::Direct(Arity::exact(2)),
            },
            generic_replace,
        ),
        "SUBSEQ" => (
            Builtin {
                lambda_list: LambdaList::with_optional(SUBSEQ_REQUIRED, SUBSEQ_OPTIONAL),
                convention: BuiltinConvention::Adapted,
            },
            generic_subseq,
        ),
        "MAKE-SEQUENCE" => (
            Builtin {
                lambda_list: LambdaList::with_optional(MAKE_REQUIRED, MAKE_OPTIONAL),
                convention: BuiltinConvention::Adapted,
            },
            generic_make,
        ),
        _ => return None,
    };
    Some(BuiltinImplementation::direct(
        implementation.0,
        implementation.1,
    ))
}

pub(crate) fn unsupported(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    _args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Err(ObjectError::Unsupported)
}
pub(crate) fn passthrough(_args: &BuiltinArgs<'_>) -> Result<Vec<Word>, ObjectError> {
    Err(ObjectError::Unsupported)
}
pub(crate) fn unsupported_implementation() -> BuiltinImplementation {
    BuiltinImplementation::adapted(
        Builtin {
            lambda_list: LambdaList::with_rest(&[], parameter("arguments", ParameterType::Any)),
            convention: BuiltinConvention::Adapted,
        },
        unsupported,
        passthrough,
    )
}
pub(crate) fn identifier(name: &'static str) -> BuiltinIdentifier {
    BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new(name))
}
