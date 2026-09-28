#[path = "register_builtins.rs"]
mod register_builtins;
#[path = "register_extra.rs"]
mod register_extra;

pub use register_builtins::register;

use crate::domain;
use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinFunctionCaller, BuiltinIdentifier,
    BuiltinImplementation, BuiltinName, BuiltinPackage, Fixnum, LambdaList, List, Local,
    MultipleValues, ObjectError, Parameter, ParameterType, Runtime, Scope, Sequence, ThreadContext,
    Word,
};
const OBJECT: Parameter = Parameter {
    name: BuiltinName::new("object"),
    ty: ParameterType::Any,
};
const DESTINATION_TYPE: Parameter = Parameter {
    name: BuiltinName::new("destination-type"),
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
const ONE_OBJECT: &[Parameter] = &[OBJECT];
const TWO_OBJECTS: &[Parameter] = &[OBJECT, OBJECT];
fn direct_descriptor(required: &'static [Parameter]) -> Builtin {
    Builtin {
        lambda_list: LambdaList::fixed(required),
        convention: BuiltinConvention::Direct(Arity::exact(
            u8::try_from(required.len()).unwrap_or(u8::MAX),
        )),
    }
}
#[allow(clippy::unnecessary_wraps)]
fn identity(args: &BuiltinArgs<'_>) -> Result<Vec<Word>, ObjectError> {
    Ok(args.as_slice().to_vec())
}
fn finish(
    ctx: &mut ThreadContext,
    values: &mut MultipleValues,
    result: Result<Word, ncl_object::LispError>,
) -> Result<Word, ObjectError> {
    let result = result
        .map_err(|error| {
            ctx.set_pending_lisp_error(error);
            ObjectError::TypeError
        })
        .inspect(|_| values.clear())?;
    let mut scope = Scope::new(ctx);
    let result_handle = scope.root::<Word>(Local::from_word(result));
    Ok(scope.get(result_handle).as_word())
}
fn sequence_arg(ctx: &ThreadContext, word: Word) -> Result<Sequence, ObjectError> {
    domain::sequence_value(ctx, word)
}
macro_rules! typed {
    ($name:ident, $implementation:path, ($a:ident : $at:ty)) => { ncl_object::typed_builtin!($name, $implementation, ($a : $at));
    };
    ($name:ident, $implementation:path, ($a:ident : $at:ty, $b:ident : $bt:ty)) => { ncl_object::typed_builtin!($name, $implementation, ($a : $at, $b : $bt));
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
fn concatenate_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let result = domain::list::concatenate(ctx, runtime, args.required(0)?, &args.as_slice()[1..]);
    finish(ctx, values, result)
}
fn append_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let result = domain::list::append(ctx, runtime, *args);
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
fn callback_word(
    ctx: &ThreadContext,
    word: Word,
) -> Result<ncl_object::typed::FunctionDesignator, ObjectError> {
    ncl_object::typed::FunctionDesignator::try_from_word(ctx, word)
}
type SelectionOperation = fn(
    &mut ThreadContext,
    &Runtime,
    &mut BuiltinFunctionCaller,
    &mut [Word], // check-added-lines: allow(index) slice type
    Word,
    domain::selection::SelectionOptions,
) -> Result<Word, ObjectError>;
fn selection_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
    operation: SelectionOperation,
) -> Result<Word, ObjectError> {
    let (positional, options) = domain::selection::parse_options(ctx, args.as_slice())?;
    let sequence = *positional.get(1).ok_or(ObjectError::TypeError)?;
    let mut scope = Scope::new(ctx);
    let sequence_handle = scope.root::<Word>(Local::from_word(sequence));
    let result = (|| {
        let sequence = scope.get(sequence_handle).as_word();
        let sequence_value = domain::selection::object_sequence(scope.context(), sequence)?;
        let mut items = domain::selection::sequence_values(scope.context_mut(), sequence_value)?;
        let object = *positional.first().ok_or(ObjectError::TypeError)?;
        let mut caller = BuiltinFunctionCaller;
        operation(
            scope.context_mut(),
            runtime,
            &mut caller,
            &mut items,
            object,
            options,
        )
    })();
    let result = result?;
    let result_handle = scope.root::<Word>(Local::from_word(result));
    values.clear();
    Ok(scope.get(result_handle).as_word())
}
fn find_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    selection_entry(ctx, runtime, args, values, domain::selection::find)
}
fn position_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    selection_entry(ctx, runtime, args, values, domain::selection::position)
}
fn count_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    selection_entry(ctx, runtime, args, values, domain::selection::count)
}
fn selection_parse(
    ctx: &ThreadContext,
    args: &[Word],
) -> Result<(Vec<Word>, domain::selection::SelectionOptions), ObjectError> {
    domain::selection::parse_options(ctx, args)
}
fn search_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let (positional, options) = selection_parse(ctx, args.as_slice())?;
    let left_word = *positional.first().ok_or(ObjectError::TypeError)?;
    let right_word = *positional.get(1).ok_or(ObjectError::TypeError)?;
    let mut left = domain::selection::sequence_values(
        ctx,
        domain::selection::object_sequence(ctx, left_word)?,
    )?;
    let mut right = domain::selection::sequence_values(
        ctx,
        domain::selection::object_sequence(ctx, right_word)?,
    )?;
    let mut caller = BuiltinFunctionCaller;
    values.clear();
    domain::selection::search(ctx, runtime, &mut caller, &mut left, &mut right, options)
}
fn mismatch_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let (positional, options) = selection_parse(ctx, args.as_slice())?;
    let left_word = *positional.first().ok_or(ObjectError::TypeError)?;
    let right_word = *positional.get(1).ok_or(ObjectError::TypeError)?;
    let mut left = domain::selection::sequence_values(
        ctx,
        domain::selection::object_sequence(ctx, left_word)?,
    )?;
    let mut right = domain::selection::sequence_values(
        ctx,
        domain::selection::object_sequence(ctx, right_word)?,
    )?;
    let mut caller = BuiltinFunctionCaller;
    values.clear();
    domain::selection::mismatch(ctx, runtime, &mut caller, &mut left, &mut right, options)
}
