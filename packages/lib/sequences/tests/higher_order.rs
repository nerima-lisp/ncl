#![allow(missing_docs)]

use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, FunctionObject, LambdaList, MultipleValues, ObjectError,
    Parameter, ParameterType, Runtime, ThreadContext, Word,
};
use std::sync::atomic::{AtomicUsize, Ordering};

static MAP_CALL_COUNT: AtomicUsize = AtomicUsize::new(0);
static MAP_CHARACTER_CALL_COUNT: AtomicUsize = AtomicUsize::new(0);

type DirectBuiltin = for<'a> fn(
    &mut ThreadContext,
    &Runtime,
    &BuiltinArgs<'a>,
    &mut MultipleValues,
) -> Result<Word, ObjectError>;

const ONE_ANY: &[Parameter] = &[Parameter {
    name: BuiltinName::new("VALUE"),
    ty: ParameterType::Any,
}];
const TWO_FIXNUMS: &[Parameter] = &[
    Parameter {
        name: BuiltinName::new("LEFT"),
        ty: ParameterType::Fixnum,
    },
    Parameter {
        name: BuiltinName::new("RIGHT"),
        ty: ParameterType::Fixnum,
    },
];

const SUM_IDENTIFIER: BuiltinIdentifier =
    BuiltinIdentifier::new(BuiltinPackage::NclTest, BuiltinName::new("HOF-SUM"));
const POSITIVE_IDENTIFIER: BuiltinIdentifier =
    BuiltinIdentifier::new(BuiltinPackage::NclTest, BuiltinName::new("HOF-POSITIVE"));
const LESS_IDENTIFIER: BuiltinIdentifier =
    BuiltinIdentifier::new(BuiltinPackage::NclTest, BuiltinName::new("HOF-LESS"));
const ZERO_IDENTIFIER: BuiltinIdentifier =
    BuiltinIdentifier::new(BuiltinPackage::NclTest, BuiltinName::new("HOF-ZERO"));
const IDENTITY_IDENTIFIER: BuiltinIdentifier =
    BuiltinIdentifier::new(BuiltinPackage::NclTest, BuiltinName::new("HOF-IDENTITY"));
const UPPERCASE_IDENTIFIER: BuiltinIdentifier =
    BuiltinIdentifier::new(BuiltinPackage::NclTest, BuiltinName::new("HOF-UPPERCASE"));
const MAP_CALL_IDENTIFIER: BuiltinIdentifier =
    BuiltinIdentifier::new(BuiltinPackage::NclTest, BuiltinName::new("HOF-MAP-CALL"));
const MAP_CHARACTER_IDENTIFIER: BuiltinIdentifier = BuiltinIdentifier::new(
    BuiltinPackage::NclTest,
    BuiltinName::new("HOF-MAP-CHARACTER"),
);

#[derive(Clone, Copy)]
struct Functions {
    sum: Word,
    positive: Word,
    less: Word,
    zero: Word,
    identity: Word,
    uppercase: Word,
    map_call: Word,
    map_character: Word,
}

fn register_function(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    identifier: BuiltinIdentifier,
    parameters: &'static [Parameter],
    arity: u8,
    implementation: DirectBuiltin,
) -> Result<Word, ObjectError> {
    runtime
        .register_builtin(
            ctx,
            identifier,
            BuiltinImplementation::direct(
                Builtin {
                    lambda_list: LambdaList::fixed(parameters),
                    convention: BuiltinConvention::Direct(Arity::exact(arity)),
                },
                implementation,
            ),
        )
        .map(ncl_object::FunctionObject::as_word)
}

fn setup() -> Result<(Runtime, ThreadContext, Functions), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_sequences::register(&runtime)?;
    let functions = Functions {
        sum: register_function(&runtime, &mut ctx, SUM_IDENTIFIER, TWO_FIXNUMS, 2, sum)?,
        positive: register_function(
            &runtime,
            &mut ctx,
            POSITIVE_IDENTIFIER,
            ONE_ANY,
            1,
            positive,
        )?,
        less: register_function(&runtime, &mut ctx, LESS_IDENTIFIER, TWO_FIXNUMS, 2, less)?,
        zero: register_function(&runtime, &mut ctx, ZERO_IDENTIFIER, &[], 0, zero)?,
        identity: register_function(
            &runtime,
            &mut ctx,
            IDENTITY_IDENTIFIER,
            ONE_ANY,
            1,
            identity,
        )?,
        uppercase: register_function(
            &runtime,
            &mut ctx,
            UPPERCASE_IDENTIFIER,
            ONE_ANY,
            1,
            uppercase,
        )?,
        map_call: register_function(
            &runtime,
            &mut ctx,
            MAP_CALL_IDENTIFIER,
            ONE_ANY,
            1,
            map_call,
        )?,
        map_character: register_function(
            &runtime,
            &mut ctx,
            MAP_CHARACTER_IDENTIFIER,
            ONE_ANY,
            1,
            map_character_call,
        )?,
    };
    Ok((runtime, ctx, functions))
}

fn sum(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let left = args
        .required(0)?
        .as_fixnum()
        .ok_or(ObjectError::TypeError)?;
    let right = args
        .required(1)?
        .as_fixnum()
        .ok_or(ObjectError::TypeError)?;
    values.clear();
    Ok(Word::fixnum(
        left.checked_add(right).ok_or(ObjectError::TypeError)?,
    ))
}

fn positive(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let value = args.required(0)?;
    let number = value.as_fixnum().ok_or(ObjectError::TypeError)?;
    values.clear();
    Ok(if number > 0 { value } else { Word::NIL })
}

fn less(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let left = args
        .required(0)?
        .as_fixnum()
        .ok_or(ObjectError::TypeError)?;
    let right = args
        .required(1)?
        .as_fixnum()
        .ok_or(ObjectError::TypeError)?;
    values.clear();
    Ok(if left < right { Word::TRUE } else { Word::NIL })
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "DirectBuiltin implementations must return Result"
)]
fn zero(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    _args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    values.clear();
    Ok(Word::fixnum(7))
}

fn identity(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    values.clear();
    args.required(0)
}

fn uppercase(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let character = args
        .required(0)?
        .as_character()
        .and_then(char::from_u32)
        .ok_or(ObjectError::TypeError)?;
    values.clear();
    Ok(Word::character(u32::from(character.to_ascii_uppercase())))
}

fn map_call(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    MAP_CALL_COUNT.fetch_add(1, Ordering::SeqCst);
    values.clear();
    args.required(0)
}

fn map_character_call(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let index = MAP_CHARACTER_CALL_COUNT.fetch_add(1, Ordering::SeqCst);
    let expected = ['a', 'b', 'c']
        .get(index)
        .copied()
        .map(|character| Word::character(u32::from(character)))
        .ok_or(ObjectError::TypeError)?;
    let value = args.required(0)?;
    if value != expected {
        return Err(ObjectError::TypeError);
    }
    values.clear();
    Ok(value)
}

fn call_result(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    name: &str,
    args: &[Word],
) -> Result<Word, ObjectError> {
    let function = runtime
        .function(ctx, "COMMON-LISP", name)
        .ok_or(ObjectError::UndefinedFunction)?;
    let function = FunctionObject::try_from(function)?;
    ncl_object::with_roots(ctx, args, |ctx, roots| {
        let rooted_args = roots.iter().map(|root| **root).collect::<Vec<_>>();
        runtime.call_builtin(ctx, function, &rooted_args)
    })
}

fn call(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    name: &str,
    args: &[Word],
) -> Result<Word, ObjectError> {
    call_result(runtime, ctx, name, args)
}

fn list(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Result<Word, ObjectError> {
    let mut result = Word::NIL;
    for value in values.iter().rev().copied() {
        result = ncl_object::make_cons(ctx, runtime, value, result)?;
    }
    Ok(result)
}

fn builtin(ctx: &mut ThreadContext, runtime: &Runtime, name: &str) -> Result<Word, ObjectError> {
    let function = runtime
        .function(ctx, "COMMON-LISP", name)
        .ok_or(ObjectError::UndefinedFunction)?;
    Ok(FunctionObject::try_from(function)?.as_word())
}

fn assert_list(
    ctx: &ThreadContext,
    mut actual: Word,
    expected: &[Word],
) -> Result<(), ObjectError> {
    for expected in expected {
        assert!(actual.is_cons());
        assert_eq!(ncl_object::car(ctx, actual)?, *expected);
        actual = ncl_object::cdr(ctx, actual)?;
    }
    assert_eq!(actual, Word::NIL);
    Ok(())
}

#[test]
fn registered_map_into_handles_lists_vectors_strings_and_zero_sources() -> Result<(), ObjectError> {
    let (runtime, mut ctx, functions) = setup()?;

    let destination = list(&mut ctx, &runtime, &[0, 0, 99].map(Word::fixnum))?;
    let first = list(&mut ctx, &runtime, &[1, 2, 3].map(Word::fixnum))?;
    let second = list(&mut ctx, &runtime, &[10].map(Word::fixnum))?;
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "MAP-INTO",
            &[destination, functions.sum, first, second],
        )?,
        destination
    );
    assert_list(&ctx, destination, &[11, 0, 99].map(Word::fixnum))?;

    let destination_values = [0, 0].map(Word::fixnum);
    let destination = ncl_object::make_simple_vector(&mut ctx, &runtime, &destination_values)?;
    let first = list(&mut ctx, &runtime, &[5, 6].map(Word::fixnum))?;
    let second_values = [7, 8].map(Word::fixnum);
    let second = ncl_object::make_simple_vector(&mut ctx, &runtime, &second_values)?;
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "MAP-INTO",
            &[destination, functions.sum, first, second],
        )?,
        destination
    );
    assert_eq!(
        ncl_object::simple_vector_ref(&ctx, destination, 0)?,
        Word::fixnum(12)
    );
    assert_eq!(
        ncl_object::simple_vector_ref(&ctx, destination, 1)?,
        Word::fixnum(14)
    );

    let empty = ncl_object::make_simple_vector(&mut ctx, &runtime, &[])?;
    let source = list(&mut ctx, &runtime, &[1].map(Word::fixnum))?;
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "MAP-INTO",
            &[empty, functions.identity, source],
        )?,
        empty
    );

    let destination_values = [0, 0].map(Word::fixnum);
    let destination = ncl_object::make_simple_vector(&mut ctx, &runtime, &destination_values)?;
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "MAP-INTO",
            &[destination, functions.zero],
        )?,
        destination
    );
    assert_eq!(
        ncl_object::simple_vector_ref(&ctx, destination, 0)?,
        Word::fixnum(7)
    );
    assert_eq!(
        ncl_object::simple_vector_ref(&ctx, destination, 1)?,
        Word::fixnum(7)
    );
    Ok(())
}

#[test]
fn registered_map_into_treats_strings_as_source_and_destination_vectors() -> Result<(), ObjectError>
{
    let (runtime, mut ctx, functions) = setup()?;
    let destination = ncl_object::make_string(&mut ctx, &runtime, &['x', 'x', 'x'])?;
    let source = ncl_object::make_string(&mut ctx, &runtime, &['a', 'B', '!'])?;
    assert_eq!(
        call_result(
            &runtime,
            &mut ctx,
            "MAP-INTO",
            &[destination, functions.uppercase, source],
        )?,
        destination
    );
    assert_eq!(ncl_object::string_ref(&ctx, destination, 0)?, 'A');
    assert_eq!(ncl_object::string_ref(&ctx, destination, 1)?, 'B');
    assert_eq!(ncl_object::string_ref(&ctx, destination, 2)?, '!');

    let destination = ncl_object::make_string(&mut ctx, &runtime, &['x'])?;
    let source = list(&mut ctx, &runtime, &[Word::fixnum(1)])?;
    assert_eq!(
        call_result(
            &runtime,
            &mut ctx,
            "MAP-INTO",
            &[destination, functions.identity, source],
        ),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

#[test]
fn registered_map_into_accepts_string_sources_for_vector_destinations() -> Result<(), ObjectError> {
    let (runtime, mut ctx, functions) = setup()?;
    let vector = ncl_object::make_simple_vector(&mut ctx, &runtime, &[Word::NIL; 3])?;
    let source = ncl_object::make_string(&mut ctx, &runtime, &['c', 'a', 't'])?;
    assert_eq!(
        call_result(
            &runtime,
            &mut ctx,
            "MAP-INTO",
            &[vector, functions.identity, source],
        )?,
        vector
    );
    assert_eq!(
        [0, 1, 2]
            .map(|index| ncl_object::simple_vector_ref(&ctx, vector, index))
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?,
        ['c', 'a', 't'].map(|value| Word::character(u32::from(value)))
    );
    Ok(())
}

#[test]
fn registered_map_with_nil_result_calls_function_for_each_sequence_element()
-> Result<(), ObjectError> {
    let (runtime, mut ctx, functions) = setup()?;
    let list_source = list(&mut ctx, &runtime, &[1, 2].map(Word::fixnum))?;
    MAP_CALL_COUNT.store(0, Ordering::SeqCst);
    assert_eq!(
        call_result(
            &runtime,
            &mut ctx,
            "MAP",
            &[Word::NIL, functions.map_call, list_source],
        )?,
        Word::NIL
    );
    assert_eq!(MAP_CALL_COUNT.load(Ordering::SeqCst), 2);

    let string_source = ncl_object::make_string(&mut ctx, &runtime, &['a', 'b', 'c'])?;
    MAP_CHARACTER_CALL_COUNT.store(0, Ordering::SeqCst);
    assert_eq!(
        call_result(
            &runtime,
            &mut ctx,
            "MAP",
            &[Word::NIL, functions.map_character, string_source],
        )?,
        Word::NIL
    );
    assert_eq!(MAP_CHARACTER_CALL_COUNT.load(Ordering::SeqCst), 3);
    Ok(())
}

#[test]
fn registered_predicates_follow_shortest_sequence_and_some_result_rules() -> Result<(), ObjectError>
{
    let (runtime, mut ctx, functions) = setup()?;
    let every_source = list(&mut ctx, &runtime, &[1, 2].map(Word::fixnum))?;
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "EVERY",
            &[functions.positive, every_source],
        )?,
        Word::TRUE
    );

    let some_source = list(
        &mut ctx,
        &runtime,
        &[Word::fixnum(-1), Word::fixnum(7), Word::TRUE],
    )?;
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "SOME",
            &[functions.positive, some_source],
        )?,
        Word::fixnum(7)
    );

    let notany_source = list(
        &mut ctx,
        &runtime,
        &[Word::fixnum(-1), Word::fixnum(2), Word::TRUE],
    )?;
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "NOTANY",
            &[functions.positive, notany_source],
        )?,
        Word::NIL
    );

    let notevery_source = list(&mut ctx, &runtime, &[Word::fixnum(-1), Word::TRUE])?;
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "NOTEVERY",
            &[functions.positive, notevery_source],
        )?,
        Word::TRUE
    );

    let left = list(
        &mut ctx,
        &runtime,
        &[Word::fixnum(1), Word::fixnum(2), Word::TRUE],
    )?;
    let right = list(&mut ctx, &runtime, &[Word::fixnum(2), Word::fixnum(3)])?;
    assert_eq!(
        call(&runtime, &mut ctx, "EVERY", &[functions.less, left, right],)?,
        Word::TRUE
    );

    let empty = ncl_object::make_simple_vector(&mut ctx, &runtime, &[])?;
    assert_eq!(
        call(&runtime, &mut ctx, "EVERY", &[functions.positive, empty])?,
        Word::TRUE
    );
    assert_eq!(
        call(&runtime, &mut ctx, "SOME", &[functions.positive, empty])?,
        Word::NIL
    );
    assert_eq!(
        call(&runtime, &mut ctx, "NOTANY", &[functions.positive, empty])?,
        Word::TRUE
    );
    assert_eq!(
        call(&runtime, &mut ctx, "NOTEVERY", &[functions.positive, empty])?,
        Word::NIL
    );
    Ok(())
}

#[test]
fn registered_predicates_accept_string_sequences() -> Result<(), ObjectError> {
    let (runtime, mut ctx, functions) = setup()?;
    let string = ncl_object::make_string(&mut ctx, &runtime, &['a', 'b'])?;
    assert_eq!(
        call_result(&runtime, &mut ctx, "EVERY", &[functions.identity, string],)?,
        Word::TRUE
    );
    Ok(())
}

#[test]
fn registered_predicates_require_a_sequence_argument() -> Result<(), ObjectError> {
    let (runtime, mut ctx, functions) = setup()?;
    assert_eq!(
        call_result(&runtime, &mut ctx, "EVERY", &[functions.positive]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call_result(&runtime, &mut ctx, "SOME", &[functions.positive]),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

#[test]
fn registered_list_mappers_require_lists_and_map_successive_tails() -> Result<(), ObjectError> {
    let (runtime, mut ctx, functions) = setup()?;
    let first = list(&mut ctx, &runtime, &[1, 2, 3].map(Word::fixnum))?;
    let second = list(&mut ctx, &runtime, &[10, 20].map(Word::fixnum))?;
    let mapped = call(
        &runtime,
        &mut ctx,
        "MAPCAR",
        &[functions.sum, first, second],
    )?;
    assert_list(&ctx, mapped, &[11, 22].map(Word::fixnum))?;

    assert_eq!(
        call(&runtime, &mut ctx, "MAPC", &[functions.identity, first],)?,
        first
    );

    let cdr = builtin(&mut ctx, &runtime, "CDR")?;
    let tails = call(&runtime, &mut ctx, "MAPLIST", &[cdr, first])?;
    let second_tail = ncl_object::cdr(&ctx, first)?;
    let last_tail = ncl_object::cdr(&ctx, second_tail)?;
    assert_list(&ctx, tails, &[second_tail, last_tail, Word::NIL])?;
    assert_eq!(call(&runtime, &mut ctx, "MAPL", &[cdr, first])?, first);

    let list_function = builtin(&mut ctx, &runtime, "LIST")?;
    let concatenated = call(&runtime, &mut ctx, "MAPCAN", &[list_function, first])?;
    assert_list(&ctx, concatenated, &[1, 2, 3].map(Word::fixnum))?;
    let concatenated_tails = call(&runtime, &mut ctx, "MAPCON", &[list_function, first])?;
    assert_list(&ctx, concatenated_tails, &[first, second_tail, last_tail])?;

    let string = ncl_object::make_string(&mut ctx, &runtime, &['x'])?;
    let string_result = list(&mut ctx, &runtime, &[string])?;
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "MAPCAN",
            &[functions.identity, string_result]
        )?,
        string
    );

    let dotted_result =
        ncl_object::make_cons(&mut ctx, &runtime, Word::fixnum(1), Word::fixnum(2))?;
    let results = list(&mut ctx, &runtime, &[dotted_result, Word::fixnum(9)])?;
    let concatenated = call(&runtime, &mut ctx, "MAPCAN", &[functions.identity, results])?;
    assert_eq!(concatenated, dotted_result);
    assert_eq!(ncl_object::car(&ctx, concatenated)?, Word::fixnum(1));
    assert_eq!(ncl_object::cdr(&ctx, concatenated)?, Word::fixnum(9));

    let vector =
        ncl_object::make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1), Word::fixnum(2)])?;
    assert_eq!(
        call_result(&runtime, &mut ctx, "MAPCAR", &[functions.identity, vector]),
        Err(ObjectError::TypeError)
    );

    let dotted_list = ncl_object::make_cons(&mut ctx, &runtime, Word::fixnum(1), Word::fixnum(2))?;
    assert_eq!(
        call_result(
            &runtime,
            &mut ctx,
            "MAPCAR",
            &[functions.identity, dotted_list],
        ),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

#[test]
fn registered_list_mappers_require_at_least_one_list() -> Result<(), ObjectError> {
    let (runtime, mut ctx, functions) = setup()?;
    assert_eq!(
        call_result(&runtime, &mut ctx, "MAPCAR", &[functions.identity]),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

#[test]
fn registered_mapping_functions_reject_circular_lists() -> Result<(), ObjectError> {
    let (runtime, mut ctx, functions) = setup()?;
    let circular = ncl_object::make_cons(&mut ctx, &runtime, Word::fixnum(1), Word::NIL)?;
    ncl_object::rplacd(&mut ctx, circular, circular)?;

    assert_eq!(
        call_result(
            &runtime,
            &mut ctx,
            "MAP",
            &[Word::NIL, functions.identity, circular],
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call_result(
            &runtime,
            &mut ctx,
            "MAPCAR",
            &[functions.identity, circular]
        ),
        Err(ObjectError::TypeError)
    );
    Ok(())
}
