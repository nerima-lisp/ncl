#![allow(missing_docs)]

use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, FunctionObject, LambdaList, MultipleValues, ObjectError, Package,
    Parameter, ParameterType, Runtime, ThreadContext, Word,
};

const ORDER_SET_TEST_PARAMETERS: &[Parameter] = &[
    Parameter {
        name: BuiltinName::new("LEFT"),
        ty: ParameterType::Fixnum,
    },
    Parameter {
        name: BuiltinName::new("RIGHT"),
        ty: ParameterType::Fixnum,
    },
];

const GREATER_IDENTIFIER: BuiltinIdentifier = BuiltinIdentifier::new(
    BuiltinPackage::NclTest,
    BuiltinName::new("ORDER-SET-GREATER"),
);
const GREATER_EQUAL_IDENTIFIER: BuiltinIdentifier = BuiltinIdentifier::new(
    BuiltinPackage::NclTest,
    BuiltinName::new("ORDER-SET-GREATER-EQUAL"),
);
const LESS_IDENTIFIER: BuiltinIdentifier =
    BuiltinIdentifier::new(BuiltinPackage::NclTest, BuiltinName::new("ORDER-SET-LESS"));
const NOT_EQUAL_IDENTIFIER: BuiltinIdentifier = BuiltinIdentifier::new(
    BuiltinPackage::NclTest,
    BuiltinName::new("ORDER-SET-NOT-EQUAL"),
);

type Predicate = for<'a> fn(
    &mut ThreadContext,
    &Runtime,
    &BuiltinArgs<'a>,
    &mut MultipleValues,
) -> Result<Word, ObjectError>;
type TestResult = Result<(), Box<dyn std::error::Error>>;

fn compare_fixnums(
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
    predicate: impl FnOnce(i64, i64) -> bool,
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
    Ok(if predicate(left, right) {
        Word::TRUE
    } else {
        Word::NIL
    })
}

fn greater_predicate(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    compare_fixnums(args, values, |left, right| left > right)
}

fn less_predicate(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    compare_fixnums(args, values, |left, right| left < right)
}

fn greater_equal_predicate(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    compare_fixnums(args, values, |left, right| left >= right)
}

fn not_equal_predicate(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    compare_fixnums(args, values, |left, right| left != right)
}

fn register_predicate(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    identifier: BuiltinIdentifier,
    implementation: Predicate,
) -> Result<Word, ObjectError> {
    Ok(runtime
        .register_builtin(
            ctx,
            identifier,
            BuiltinImplementation::direct(
                Builtin {
                    lambda_list: LambdaList::fixed(ORDER_SET_TEST_PARAMETERS),
                    convention: BuiltinConvention::Direct(Arity::exact(2)),
                },
                implementation,
            ),
        )?
        .as_word())
}

fn call(
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

fn keyword(ctx: &mut ThreadContext, runtime: &Runtime, name: &str) -> Result<Word, ObjectError> {
    let package = runtime
        .find_package(ctx, "KEYWORD")
        .ok_or(ObjectError::Layout)?;
    let (symbol, _) = Package::from_word(package).intern(ctx, runtime, name)?;
    Ok(symbol)
}

fn list(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Result<Word, ObjectError> {
    let mut result = Word::NIL;
    for &value in values.iter().rev() {
        result = ncl_object::make_cons(ctx, runtime, value, result)?;
    }
    Ok(result)
}

fn assert_list(
    ctx: &ThreadContext,
    mut actual: Word,
    expected: &[Word],
) -> Result<(), ObjectError> {
    for &value in expected {
        assert!(actual.is_cons());
        assert_eq!(ncl_object::car(ctx, actual)?, value);
        actual = ncl_object::cdr(ctx, actual)?;
    }
    assert_eq!(actual, Word::NIL);
    Ok(())
}

fn assert_number_set(
    ctx: &ThreadContext,
    mut actual: Word,
    expected: &[i64],
) -> Result<(), ObjectError> {
    let mut values = Vec::new();
    while actual.is_cons() {
        let value = ncl_object::car(ctx, actual)?;
        values.push(value.as_fixnum().ok_or(ObjectError::TypeError)?);
        actual = ncl_object::cdr(ctx, actual)?;
    }
    assert_eq!(actual, Word::NIL);
    values.sort_unstable();
    assert_eq!(values, expected);
    Ok(())
}

fn record(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    key: i64,
    tag: i64,
) -> Result<Word, ObjectError> {
    ncl_object::make_cons(ctx, runtime, Word::fixnum(key), Word::fixnum(tag))
}

fn car_function(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<Word, ObjectError> {
    let function = runtime
        .function(ctx, "COMMON-LISP", "CAR")
        .ok_or(ObjectError::UndefinedFunction)?;
    Ok(FunctionObject::try_from(function)?.as_word())
}

#[test]
fn registered_sort_writes_vectors_and_stable_sort_keeps_equal_keys_in_order() -> TestResult {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_sequences::register(&runtime)?;
    let less = register_predicate(&runtime, &mut ctx, LESS_IDENTIFIER, less_predicate)?;

    let values = [3, 1, 2].map(Word::fixnum);
    let vector = ncl_object::make_simple_vector(&mut ctx, &runtime, &values)?;
    let sorted = call(&runtime, &mut ctx, "SORT", &[vector, less])?;
    assert_eq!(sorted, vector);
    assert_eq!(ncl_object::simple_vector_length(&ctx, sorted), Ok(3));
    assert_eq!(
        (0..3)
            .map(|index| ncl_object::simple_vector_ref(&ctx, sorted, index))
            .collect::<Result<Vec<_>, _>>()?,
        [1, 2, 3].map(Word::fixnum)
    );

    let empty_vector = ncl_object::make_simple_vector(&mut ctx, &runtime, &[])?;
    assert_eq!(
        call(&runtime, &mut ctx, "SORT", &[empty_vector, less])?,
        empty_vector
    );

    let records = [
        record(&mut ctx, &runtime, 2, 10)?,
        record(&mut ctx, &runtime, 1, 20)?,
        record(&mut ctx, &runtime, 2, 30)?,
        record(&mut ctx, &runtime, 1, 40)?,
    ];
    let sequence = list(&mut ctx, &runtime, &records)?;
    let key = car_function(&mut ctx, &runtime)?;
    let key_option = keyword(&mut ctx, &runtime, "KEY")?;
    let sorted = call(
        &runtime,
        &mut ctx,
        "STABLE-SORT",
        &[sequence, less, key_option, key],
    )?;
    assert_list(
        &ctx,
        sorted,
        &[records[1], records[3], records[0], records[2]],
    )?;
    Ok(())
}

#[test]
fn registered_set_operations_obey_test_key_and_test_not_options() -> TestResult {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_sequences::register(&runtime)?;
    let greater_equal = register_predicate(
        &runtime,
        &mut ctx,
        GREATER_EQUAL_IDENTIFIER,
        greater_equal_predicate,
    )?;
    let not_equal = register_predicate(
        &runtime,
        &mut ctx,
        NOT_EQUAL_IDENTIFIER,
        not_equal_predicate,
    )?;
    let test = keyword(&mut ctx, &runtime, "TEST")?;
    let test_not = keyword(&mut ctx, &runtime, "TEST-NOT")?;
    let key_option = keyword(&mut ctx, &runtime, "KEY")?;
    let key = car_function(&mut ctx, &runtime)?;

    let left_records = [
        record(&mut ctx, &runtime, 1, 11)?,
        record(&mut ctx, &runtime, 4, 44)?,
    ];
    let right_records = [
        record(&mut ctx, &runtime, 3, 33)?,
        record(&mut ctx, &runtime, 2, 22)?,
    ];
    let left = list(&mut ctx, &runtime, &left_records)?;
    let right = list(&mut ctx, &runtime, &right_records)?;
    let difference = call(
        &runtime,
        &mut ctx,
        "SET-DIFFERENCE",
        &[left, right, key_option, key, test, greater_equal],
    )?;
    assert_list(&ctx, difference, &[left_records[0]])?;

    let left = list(&mut ctx, &runtime, &[Word::fixnum(1), Word::fixnum(2)])?;
    let right = list(&mut ctx, &runtime, &[Word::fixnum(2), Word::fixnum(3)])?;
    let union = call(
        &runtime,
        &mut ctx,
        "UNION",
        &[left, right, test_not, not_equal],
    )?;
    assert_number_set(&ctx, union, &[1, 2, 3])?;

    let left = list(
        &mut ctx,
        &runtime,
        &[Word::fixnum(1), Word::fixnum(2), Word::fixnum(4)],
    )?;
    let right = list(
        &mut ctx,
        &runtime,
        &[Word::fixnum(2), Word::fixnum(3), Word::fixnum(4)],
    )?;
    let exclusive_or = call(&runtime, &mut ctx, "SET-EXCLUSIVE-OR", &[left, right])?;
    assert_number_set(&ctx, exclusive_or, &[1, 3])?;

    let values = list(&mut ctx, &runtime, &[Word::fixnum(1), Word::fixnum(2)])?;
    let union_with_nil = call(&runtime, &mut ctx, "UNION", &[Word::NIL, values])?;
    assert_number_set(&ctx, union_with_nil, &[1, 2])?;
    let values = list(&mut ctx, &runtime, &[Word::fixnum(1), Word::fixnum(2)])?;
    let union_with_nil = call(&runtime, &mut ctx, "UNION", &[values, Word::NIL])?;
    assert_number_set(&ctx, union_with_nil, &[1, 2])?;
    Ok(())
}

#[test]
fn registered_union_passes_list_one_element_before_list_two_element_to_test() -> TestResult {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_sequences::register(&runtime)?;
    let greater = register_predicate(&runtime, &mut ctx, GREATER_IDENTIFIER, greater_predicate)?;
    let test = keyword(&mut ctx, &runtime, "TEST")?;
    let left = list(&mut ctx, &runtime, &[Word::fixnum(3)])?;
    let right = list(&mut ctx, &runtime, &[Word::fixnum(2)])?;

    let union = call(&runtime, &mut ctx, "UNION", &[left, right, test, greater])?;
    assert_number_set(&ctx, union, &[3])?;
    Ok(())
}

#[test]
fn registered_set_membership_does_not_test_a_candidate_against_itself() -> TestResult {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_sequences::register(&runtime)?;
    let greater = register_predicate(&runtime, &mut ctx, GREATER_IDENTIFIER, greater_predicate)?;
    let test = keyword(&mut ctx, &runtime, "TEST")?;

    let left = list(&mut ctx, &runtime, &[Word::fixnum(1)])?;
    let right = list(&mut ctx, &runtime, &[Word::fixnum(2)])?;
    let difference = call(
        &runtime,
        &mut ctx,
        "SET-DIFFERENCE",
        &[left, right, test, greater],
    )?;
    assert_list(&ctx, difference, &[Word::fixnum(1)])?;

    let left = list(&mut ctx, &runtime, &[Word::fixnum(1)])?;
    let right = list(&mut ctx, &runtime, &[Word::fixnum(2)])?;
    let exclusive_or = call(
        &runtime,
        &mut ctx,
        "SET-EXCLUSIVE-OR",
        &[left, right, test, greater],
    )?;
    assert_number_set(&ctx, exclusive_or, &[1, 2])?;
    Ok(())
}

#[test]
fn registered_order_set_options_reject_incomplete_calls() -> TestResult {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_sequences::register(&runtime)?;
    let values = list(&mut ctx, &runtime, &[Word::fixnum(1)])?;
    let key = keyword(&mut ctx, &runtime, "KEY")?;

    assert!(matches!(
        call(&runtime, &mut ctx, "UNION", &[values]),
        Err(ObjectError::TypeError)
    ));
    assert!(matches!(
        call(&runtime, &mut ctx, "UNION", &[values, Word::NIL, key]),
        Err(ObjectError::TypeError)
    ));
    Ok(())
}
