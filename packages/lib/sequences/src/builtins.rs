use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, FromLispArg, LambdaList, MultipleValues, ObjectError, Parameter,
    ParameterType, Runtime, Sequence, ThreadContext, Word, typed_builtin,
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
const THREE_LISTS: &[Parameter] = &[
    parameter("keys", ParameterType::List),
    parameter("data", ParameterType::List),
    parameter("alist", ParameterType::List),
];
const THREE_ANY: &[Parameter] = &[
    parameter("first", ParameterType::Any),
    parameter("second", ParameterType::Any),
    parameter("third", ParameterType::Any),
];
const TWO_FUNCTION_LIST: &[Parameter] = &[
    parameter("predicate", ParameterType::FunctionDesignator),
    parameter("alist", ParameterType::List),
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
typed_builtin!(cons_builtin, domain::cons, (car: Word, cdr: Word));
typed_builtin!(copy_list_builtin, domain::copy_list, (value: ncl_object::List));
typed_builtin!(copy_tree_builtin, domain::copy_tree, (value: Word));
typed_builtin!(rassoc_builtin, domain::rassoc, (item: Word, alist: ncl_object::List));
typed_builtin!(caar_builtin, domain::caar, (value: Word));
typed_builtin!(cadr_builtin, domain::cadr, (value: Word));
typed_builtin!(cdar_builtin, domain::cdar, (value: Word));
typed_builtin!(cddr_builtin, domain::cddr, (value: Word));
typed_builtin!(caaar_builtin, domain::caaar, (value: Word));
typed_builtin!(caadr_builtin, domain::caadr, (value: Word));
typed_builtin!(cadar_builtin, domain::cadar, (value: Word));
typed_builtin!(caddr_builtin, domain::caddr, (value: Word));
typed_builtin!(cdaar_builtin, domain::cdaar, (value: Word));
typed_builtin!(cdadr_builtin, domain::cdadr, (value: Word));
typed_builtin!(cddar_builtin, domain::cddar, (value: Word));
typed_builtin!(cdddr_builtin, domain::cdddr, (value: Word));
typed_builtin!(caaaar_builtin, domain::caaaar, (value: Word));
typed_builtin!(caaadr_builtin, domain::caaadr, (value: Word));
typed_builtin!(caadar_builtin, domain::caadar, (value: Word));
typed_builtin!(caaddr_builtin, domain::caaddr, (value: Word));
typed_builtin!(cadaar_builtin, domain::cadaar, (value: Word));
typed_builtin!(cadadr_builtin, domain::cadadr, (value: Word));
typed_builtin!(caddar_builtin, domain::caddar, (value: Word));
typed_builtin!(cadddr_builtin, domain::cadddr, (value: Word));
typed_builtin!(cdaaar_builtin, domain::cdaaar, (value: Word));
typed_builtin!(cdaadr_builtin, domain::cdaadr, (value: Word));
typed_builtin!(cdadar_builtin, domain::cdadar, (value: Word));
typed_builtin!(cdaddr_builtin, domain::cdaddr, (value: Word));
typed_builtin!(cddaar_builtin, domain::cddaar, (value: Word));
typed_builtin!(cddadr_builtin, domain::cddadr, (value: Word));
typed_builtin!(cdddar_builtin, domain::cdddar, (value: Word));
typed_builtin!(cddddr_builtin, domain::cddddr, (value: Word));

fn adapted<F>(args: &BuiltinArgs<'_>, f: F) -> Result<Word, ncl_object::LispError>
where
    F: FnOnce(&[Word]) -> Result<Word, ncl_object::LispError>,
{
    let values = (0..args.len())
        .map(|i| args.get(i))
        .collect::<Option<Vec<_>>>()
        .ok_or(ncl_object::LispError::ProgramError(
            ncl_object::ProgramError::WrongNumberOfArguments {
                minimum: args.len(),
                maximum: Some(args.len()),
            },
        ))?;
    f(&values)
}
fn list_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let result = adapted(args, |v| domain::list(ctx, runtime, v));
    map_lisp(ctx, result)
}
fn list_star_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let result = adapted(args, |v| domain::list_star(ctx, runtime, v));
    map_lisp(ctx, result)
}
fn append_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let result = adapted(args, |v| domain::append(ctx, runtime, v));
    map_lisp(ctx, result)
}
fn map_lisp(
    ctx: &mut ThreadContext,
    result: Result<Word, ncl_object::LispError>,
) -> Result<Word, ObjectError> {
    result.map_err(|error| {
        ctx.set_pending_lisp_error(error);
        ObjectError::TypeError
    })
}

fn pass_arguments(args: &BuiltinArgs<'_>) -> Result<Vec<Word>, ObjectError> {
    (0..args.len())
        .map(|index| args.get(index).ok_or(ObjectError::TypeError))
        .collect()
}
fn acons_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let result = domain::acons(
        ctx,
        runtime,
        args.required(0)?,
        args.required(1)?,
        ncl_object::List::from_lisp_arg(ctx, args.required(2)?)
            .map_err(|_| ObjectError::TypeError)?,
    );
    map_lisp(ctx, result)
}
fn pairlis_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let result = domain::pairlis(
        ctx,
        runtime,
        ncl_object::List::from_lisp_arg(ctx, args.required(0)?)
            .map_err(|_| ObjectError::TypeError)?,
        ncl_object::List::from_lisp_arg(ctx, args.required(1)?)
            .map_err(|_| ObjectError::TypeError)?,
        ncl_object::List::from_lisp_arg(ctx, args.required(2)?)
            .map_err(|_| ObjectError::TypeError)?,
    );
    map_lisp(ctx, result)
}
fn getf_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let result = domain::getf(
        ctx,
        ncl_object::List::from_lisp_arg(ctx, args.required(0)?)
            .map_err(|_| ObjectError::TypeError)?,
        args.required(1)?,
        args.get(2).unwrap_or(Word::NIL),
    );
    map_lisp(ctx, result)
}
fn assoc_if_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let list = ncl_object::List::from_lisp_arg(ctx, args.required(1)?)
        .map_err(|_| ObjectError::TypeError)?;
    let result = domain::assoc_if(ctx, runtime, args.required(0)?, list, false);
    map_lisp(ctx, result)
}
fn assoc_if_not_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let list = ncl_object::List::from_lisp_arg(ctx, args.required(1)?)
        .map_err(|_| ObjectError::TypeError)?;
    let result = domain::assoc_if(ctx, runtime, args.required(0)?, list, true);
    map_lisp(ctx, result)
}
fn rassoc_if_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let list = ncl_object::List::from_lisp_arg(ctx, args.required(1)?)
        .map_err(|_| ObjectError::TypeError)?;
    let result = domain::rassoc_if(ctx, runtime, args.required(0)?, list, false);
    map_lisp(ctx, result)
}
fn rassoc_if_not_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let list = ncl_object::List::from_lisp_arg(ctx, args.required(1)?)
        .map_err(|_| ObjectError::TypeError)?;
    let result = domain::rassoc_if(ctx, runtime, args.required(0)?, list, true);
    map_lisp(ctx, result)
}

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
        "CONS" => (descriptor(TWO_CONS_VALUE), cons_builtin),
        "COPY-LIST" => (descriptor(ONE_LIST), copy_list_builtin),
        "COPY-TREE" => (descriptor(ONE_ANY), copy_tree_builtin),
        "RASSOC" => (descriptor(TWO_ITEM_ALIST), rassoc_builtin),
        "CAAR" => (descriptor(ONE_ANY), caar_builtin),
        "CADR" => (descriptor(ONE_ANY), cadr_builtin),
        "CDAR" => (descriptor(ONE_ANY), cdar_builtin),
        "CDDR" => (descriptor(ONE_ANY), cddr_builtin),
        "CAAAR" => (descriptor(ONE_ANY), caaar_builtin),
        "CAADR" => (descriptor(ONE_ANY), caadr_builtin),
        "CADAR" => (descriptor(ONE_ANY), cadar_builtin),
        "CADDR" => (descriptor(ONE_ANY), caddr_builtin),
        "CDAAR" => (descriptor(ONE_ANY), cdaar_builtin),
        "CDADR" => (descriptor(ONE_ANY), cdadr_builtin),
        "CDDAR" => (descriptor(ONE_ANY), cddar_builtin),
        "CDDDR" => (descriptor(ONE_ANY), cdddr_builtin),
        "CAAAAR" => (descriptor(ONE_ANY), caaaar_builtin),
        "CAAADR" => (descriptor(ONE_ANY), caaadr_builtin),
        "CAADAR" => (descriptor(ONE_ANY), caadar_builtin),
        "CAADDR" => (descriptor(ONE_ANY), caaddr_builtin),
        "CADAAR" => (descriptor(ONE_ANY), cadaar_builtin),
        "CADADR" => (descriptor(ONE_ANY), cadadr_builtin),
        "CADDAR" => (descriptor(ONE_ANY), caddar_builtin),
        "CADDDR" => (descriptor(ONE_ANY), cadddr_builtin),
        "CDAAAR" => (descriptor(ONE_ANY), cdaaar_builtin),
        "CDAADR" => (descriptor(ONE_ANY), cdaadr_builtin),
        "CDADAR" => (descriptor(ONE_ANY), cdadar_builtin),
        "CDADDR" => (descriptor(ONE_ANY), cdaddr_builtin),
        "CDDAAR" => (descriptor(ONE_ANY), cddaar_builtin),
        "CDDADR" => (descriptor(ONE_ANY), cddadr_builtin),
        "CDDDAR" => (descriptor(ONE_ANY), cdddar_builtin),
        "CDDDDR" => (descriptor(ONE_ANY), cddddr_builtin),
        "LIST" => (
            Builtin {
                lambda_list: LambdaList::with_rest(&[], parameter("arguments", ParameterType::Any)),
                convention: BuiltinConvention::Adapted,
            },
            list_builtin,
        ),
        "LIST*" => (
            Builtin {
                lambda_list: LambdaList::with_rest(&[], parameter("arguments", ParameterType::Any)),
                convention: BuiltinConvention::Adapted,
            },
            list_star_builtin,
        ),
        "APPEND" => (
            Builtin {
                lambda_list: LambdaList::with_rest(&[], parameter("arguments", ParameterType::Any)),
                convention: BuiltinConvention::Adapted,
            },
            append_builtin,
        ),
        "ACONS" => (descriptor(THREE_ANY), acons_builtin),
        "PAIRLIS" => (descriptor(THREE_LISTS), pairlis_builtin),
        "GETF" => (descriptor(THREE_ANY), getf_builtin),
        "ASSOC-IF" => (descriptor(TWO_FUNCTION_LIST), assoc_if_builtin),
        "ASSOC-IF-NOT" => (descriptor(TWO_FUNCTION_LIST), assoc_if_not_builtin),
        "RASSOC-IF" => (descriptor(TWO_FUNCTION_LIST), rassoc_if_builtin),
        "RASSOC-IF-NOT" => (descriptor(TWO_FUNCTION_LIST), rassoc_if_not_builtin),
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
    Some(match implementation.0.convention {
        BuiltinConvention::Direct(_) => {
            BuiltinImplementation::direct(implementation.0, implementation.1)
        }
        BuiltinConvention::Adapted => {
            BuiltinImplementation::adapted(implementation.0, implementation.1, pass_arguments)
        }
    })
}

include!("builtins/fallback.rs");
