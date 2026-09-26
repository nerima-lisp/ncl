use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, Fixnum, FromLispArg, LambdaList, List, MultipleValues,
    ObjectError, Parameter, ParameterType, Runtime, Sequence, ThreadContext, Word,
};

use crate::domain;

const OBJECT: Parameter = Parameter {
    name: BuiltinName::new("object"),
    ty: ParameterType::Any,
};
const LIST: Parameter = Parameter {
    name: BuiltinName::new("list"),
    ty: ParameterType::List,
};
const SEQUENCE: Parameter = Parameter {
    name: BuiltinName::new("sequence"),
    ty: ParameterType::Sequence,
};
const INDEX: Parameter = Parameter {
    name: BuiltinName::new("index"),
    ty: ParameterType::Fixnum,
};
const REST: Parameter = Parameter {
    name: BuiltinName::new("objects"),
    ty: ParameterType::Any,
};
const KEY: Parameter = Parameter {
    name: BuiltinName::new("key"),
    ty: ParameterType::FunctionDesignator,
};
const ONE_OBJECT: &[Parameter] = &[OBJECT];
const TWO_OBJECTS: &[Parameter] = &[OBJECT, OBJECT];

const fn direct_descriptor(required: &'static [Parameter]) -> Builtin {
    Builtin {
        lambda_list: LambdaList::fixed(required),
        convention: BuiltinConvention::Direct(Arity::exact(required.len() as u8)),
    }
}

fn identity(args: &BuiltinArgs<'_>) -> Result<Vec<Word>, ObjectError> {
    Ok(args.as_slice().to_vec())
}

fn finish<T>(
    ctx: &mut ThreadContext,
    values: &mut MultipleValues,
    result: Result<T, ncl_object::LispError>,
) -> Result<T, ObjectError> {
    result
        .map_err(|error| {
            ctx.set_pending_lisp_error(error);
            ObjectError::TypeError
        })
        .inspect(|_| values.clear())
}

fn list_arg(ctx: &mut ThreadContext, word: Word) -> Result<List, ObjectError> {
    List::from_lisp_arg(ctx, word).map_err(|error| {
        ctx.set_pending_lisp_error(error);
        ObjectError::TypeError
    })
}

fn sequence_arg(ctx: &ThreadContext, word: Word) -> Result<Sequence, ObjectError> {
    domain::sequence_value(ctx, word)
}

macro_rules! typed {
    ($name:ident, $implementation:path, ($a:ident : $at:ty)) => {
        ncl_object::typed_builtin!($name, $implementation, ($a : $at));
    };
    ($name:ident, $implementation:path, ($a:ident : $at:ty, $b:ident : $bt:ty)) => {
        ncl_object::typed_builtin!($name, $implementation, ($a : $at, $b : $bt));
    };
}

typed!(atom_builtin, domain::list::atom, (value: Word));
typed!(cons_p_builtin, domain::list::cons_p, (value: Word));
typed!(list_p_builtin, domain::list::list_p, (value: Word));
typed!(endp_builtin, domain::list::end_p, (value: Word));
typed!(car_builtin, domain::list::car, (value: List));
typed!(cdr_builtin, domain::list::cdr, (value: List));
typed!(cons_builtin, domain::list::cons, (car: Word, cdr: Word));
typed!(rplaca_builtin, domain::list::rplaca, (cons: Word, value: Word));
typed!(rplacd_builtin, domain::list::rplacd, (cons: Word, value: Word));
typed!(copy_list_builtin, domain::list::copy_list, (value: List));
typed!(nth_builtin, domain::list::nth, (index: Fixnum, value: List));
typed!(nthcdr_builtin, domain::list::nthcdr, (index: Fixnum, value: List));
typed!(list_length_builtin, domain::list::list_length, (value: List));
typed!(first_builtin, domain::list::first, (value: List));
typed!(second_builtin, domain::list::second, (value: List));
typed!(third_builtin, domain::list::third, (value: List));
typed!(fourth_builtin, domain::list::fourth, (value: List));
typed!(fifth_builtin, domain::list::fifth, (value: List));
typed!(sixth_builtin, domain::list::sixth, (value: List));
typed!(seventh_builtin, domain::list::seventh, (value: List));
typed!(eighth_builtin, domain::list::eighth, (value: List));
typed!(ninth_builtin, domain::list::ninth, (value: List));
typed!(tenth_builtin, domain::list::tenth, (value: List));

fn list_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let result = domain::list::list(ctx, runtime, args.as_slice());
    finish(ctx, values, result)
}
fn list_star_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let result = domain::list::list_star(ctx, runtime, args.as_slice());
    finish(ctx, values, result)
}
fn append_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let result = domain::list::append(ctx, runtime, args.as_slice());
    finish(ctx, values, result)
}
fn nconc_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let result = domain::list::nconc(ctx, runtime, args.as_slice());
    finish(ctx, values, result)
}

fn sequence_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
    call: fn(&mut ThreadContext, &Runtime, Sequence) -> Result<Word, ncl_object::LispError>,
) -> Result<Word, ObjectError> {
    let sequence = sequence_arg(ctx, args.required(0)?)?;
    let result = call(ctx, runtime, sequence);
    finish(ctx, values, result)
}
fn length_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    sequence_builtin(ctx, runtime, args, values, domain::list::length)
}
fn copy_seq_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    sequence_builtin(ctx, runtime, args, values, domain::list::copy_seq)
}
fn reverse_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    sequence_builtin(ctx, runtime, args, values, domain::list::reverse)
}
fn nreverse_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    sequence_builtin(ctx, runtime, args, values, domain::list::nreverse)
}

fn elt_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let sequence = sequence_arg(ctx, args.required(0)?)?;
    let index = Fixnum::try_from_word(args.required(1)?).map_err(|_| ObjectError::TypeError)?;
    let result = domain::list::elt(ctx, runtime, sequence, index);
    finish(ctx, values, result)
}
fn subseq_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let sequence = sequence_arg(ctx, args.required(0)?)?;
    let start = Fixnum::try_from_word(args.required(1)?).map_err(|_| ObjectError::TypeError)?;
    let end = args
        .get(2)
        .map(Fixnum::try_from_word)
        .transpose()
        .map_err(|_| ObjectError::TypeError)?;
    let result = domain::list::subseq(ctx, runtime, sequence, start, end);
    finish(ctx, values, result)
}

fn acons_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let alist = list_arg(ctx, args.required(2)?)?;
    let result = domain::list::acons(ctx, runtime, args.required(0)?, args.required(1)?, alist);
    finish(ctx, values, result)
}
fn getf_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let plist = list_arg(ctx, args.required(0)?)?;
    let result = domain::list::getf(ctx, plist, args.required(1)?, args.required(2)?);
    finish(ctx, values, result)
}
fn assoc_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let alist = list_arg(ctx, args.required(1)?)?;
    let result = domain::list::assoc(ctx, runtime, args.required(0)?, alist);
    finish(ctx, values, result)
}
fn rassoc_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let alist = list_arg(ctx, args.required(1)?)?;
    let result = domain::list::rassoc(ctx, runtime, args.required(0)?, alist);
    finish(ctx, values, result)
}
fn merge_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let first = sequence_arg(ctx, args.required(1)?)?;
    let second = sequence_arg(ctx, args.required(2)?)?;
    domain::sort::merge(
        ctx,
        runtime,
        args.required(0)?,
        first,
        second,
        args.required(3)?,
        args.get(4),
    )
    .inspect(|_| values.clear())
}

fn register_adapted(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    name: &'static str,
    descriptor: Builtin,
    function: ncl_object::RustBuiltin,
) -> Result<(), ObjectError> {
    runtime.register_builtin(
        ctx,
        BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new(name)),
        BuiltinImplementation::adapted(descriptor, function, identity),
    )?;
    Ok(())
}

fn register_direct(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    name: &'static str,
    descriptor: Builtin,
    function: ncl_object::RustBuiltin,
) -> Result<(), ObjectError> {
    runtime.register_builtin(
        ctx,
        BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new(name)),
        BuiltinImplementation::direct(descriptor, function),
    )?;
    Ok(())
}

/// Register every implemented sequence builtin.
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    let mut ctx = ThreadContext::new();
    ctx.register(runtime)?;

    for (name, function, required) in [
        ("ATOM", atom_builtin as ncl_object::RustBuiltin, ONE_OBJECT),
        ("CONSP", cons_p_builtin, ONE_OBJECT),
        ("LISTP", list_p_builtin, ONE_OBJECT),
        ("ENDP", endp_builtin, ONE_OBJECT),
        ("CAR", car_builtin, &[LIST][..]),
        ("CDR", cdr_builtin, &[LIST][..]),
        ("CONS", cons_builtin, TWO_OBJECTS),
        ("RPLACA", rplaca_builtin, TWO_OBJECTS),
        ("RPLACD", rplacd_builtin, TWO_OBJECTS),
        ("COPY-LIST", copy_list_builtin, &[LIST][..]),
        ("NTH", nth_builtin, &[INDEX, LIST][..]),
        ("NTHCDR", nthcdr_builtin, &[INDEX, LIST][..]),
        ("LIST-LENGTH", list_length_builtin, &[LIST][..]),
        ("FIRST", first_builtin, &[LIST][..]),
        ("SECOND", second_builtin, &[LIST][..]),
        ("THIRD", third_builtin, &[LIST][..]),
        ("FOURTH", fourth_builtin, &[LIST][..]),
        ("FIFTH", fifth_builtin, &[LIST][..]),
        ("SIXTH", sixth_builtin, &[LIST][..]),
        ("SEVENTH", seventh_builtin, &[LIST][..]),
        ("EIGHTH", eighth_builtin, &[LIST][..]),
        ("NINTH", ninth_builtin, &[LIST][..]),
        ("TENTH", tenth_builtin, &[LIST][..]),
    ] {
        runtime.register_builtin(
            ctx_ref(&mut ctx),
            BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new(name)),
            BuiltinImplementation::direct(direct_descriptor(required), function),
        )?;
    }

    let rest = Builtin {
        lambda_list: LambdaList::with_rest(&[], REST),
        convention: BuiltinConvention::Adapted,
    };
    for (name, function) in [
        ("LIST", list_builtin as ncl_object::RustBuiltin),
        ("APPEND", append_builtin),
        ("NCONC", nconc_builtin),
    ] {
        register_adapted(runtime, &mut ctx, name, rest, function)?;
    }
    register_adapted(
        runtime,
        &mut ctx,
        "LIST*",
        Builtin {
            lambda_list: LambdaList::with_rest(ONE_OBJECT, REST),
            convention: BuiltinConvention::Adapted,
        },
        list_star_builtin,
    )?;

    for (name, function, params) in [
        (
            "LENGTH",
            length_builtin as ncl_object::RustBuiltin,
            &[SEQUENCE][..],
        ),
        ("COPY-SEQ", copy_seq_builtin, &[SEQUENCE][..]),
        ("REVERSE", reverse_builtin, &[SEQUENCE][..]),
        ("NREVERSE", nreverse_builtin, &[SEQUENCE][..]),
        ("ELT", elt_builtin, &[SEQUENCE, INDEX][..]),
    ] {
        register_direct(runtime, &mut ctx, name, direct_descriptor(params), function)?;
    }
    register_adapted(
        runtime,
        &mut ctx,
        "SUBSEQ",
        Builtin {
            lambda_list: LambdaList::with_optional(&[SEQUENCE, INDEX], &[INDEX]),
            convention: BuiltinConvention::Adapted,
        },
        subseq_builtin,
    )?;
    register_direct(
        runtime,
        &mut ctx,
        "ACONS",
        direct_descriptor(&[OBJECT, OBJECT, LIST]),
        acons_builtin,
    )?;
    register_direct(
        runtime,
        &mut ctx,
        "GETF",
        direct_descriptor(&[LIST, OBJECT, OBJECT]),
        getf_builtin,
    )?;
    register_direct(
        runtime,
        &mut ctx,
        "ASSOC",
        direct_descriptor(&[OBJECT, LIST]),
        assoc_builtin,
    )?;
    register_direct(
        runtime,
        &mut ctx,
        "RASSOC",
        direct_descriptor(&[OBJECT, LIST]),
        rassoc_builtin,
    )?;
    register_adapted(
        runtime,
        &mut ctx,
        "MERGE",
        Builtin {
            lambda_list: LambdaList::with_optional(&[OBJECT, SEQUENCE, SEQUENCE, OBJECT], &[KEY]),
            convention: BuiltinConvention::Adapted,
        },
        merge_builtin,
    )?;

    for line in include_str!("../ownership.tsv").lines().skip(1) {
        let fields: Vec<_> = line.split('\t').collect();
        if fields.len() < 3 || fields[0] != "COMMON-LISP" || !fields[2].contains("function") {
            continue;
        }
        let name = fields[1];
        let Some(implementation) = domain::map::map_entry(name)
            .or_else(|| domain::set::set_entry(name))
            .or_else(|| domain::filter::filter_entry(name))
            .or_else(|| domain::sort::sort_entry(name))
        else {
            continue;
        };
        runtime.register_builtin(
            ctx_ref(&mut ctx),
            BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new(name)),
            implementation,
        )?;
    }
    Ok(())
}

fn ctx_ref(ctx: &mut ThreadContext) -> &mut ThreadContext {
    ctx
}
