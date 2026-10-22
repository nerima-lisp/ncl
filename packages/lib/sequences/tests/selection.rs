#![allow(missing_docs)]

use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, FunctionObject, LambdaList, MultipleValues, ObjectError, Package,
    Parameter, ParameterType, Runtime, ThreadContext, Word,
};

const SELECTION_TEST_PARAMETERS: &[Parameter] = &[
    Parameter {
        name: BuiltinName::new("OBJECT"),
        ty: ParameterType::Fixnum,
    },
    Parameter {
        name: BuiltinName::new("ELEMENT"),
        ty: ParameterType::Fixnum,
    },
];
const SELECTION_TEST_IDENTIFIER: BuiltinIdentifier = BuiltinIdentifier::new(
    BuiltinPackage::NclTest,
    BuiltinName::new("SEQUENCE-TEST-GREATER"),
);

fn greater_predicate(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let object = args
        .required(0)?
        .as_fixnum()
        .ok_or(ObjectError::TypeError)?;
    let element = args
        .required(1)?
        .as_fixnum()
        .ok_or(ObjectError::TypeError)?;
    values.clear();
    Ok(if object > element {
        Word::TRUE
    } else {
        Word::NIL
    })
}

fn call_result(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    name: &str,
    args: &[Word],
) -> Result<Word, ObjectError> {
    let function = runtime
        .function(ctx, "COMMON-LISP", name)
        .unwrap_or_else(|| panic!("missing builtin {name}"));
    let function =
        FunctionObject::try_from(function).unwrap_or_else(|_| panic!("{name} is not a function"));
    ncl_object::with_roots(ctx, args, |ctx, roots| {
        let rooted_args = roots.iter().map(|root| **root).collect::<Vec<_>>();
        runtime.call_builtin(ctx, function, &rooted_args)
    })
}

fn call(runtime: &Runtime, ctx: &mut ThreadContext, name: &str, args: &[Word]) -> Word {
    call_result(runtime, ctx, name, args).unwrap_or_else(|error| panic!("{name} failed: {error:?}"))
}

fn keyword(ctx: &mut ThreadContext, runtime: &Runtime, name: &str) -> Word {
    let package = runtime
        .find_package(ctx, "KEYWORD")
        .unwrap_or_else(|| panic!("missing KEYWORD package"));
    let (symbol, _) = Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap_or_else(|error| panic!("intern {name}: {error:?}"));
    symbol.into()
}

fn list(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Word {
    let mut result = Word::NIL;
    for &value in values.iter().rev() {
        result = ncl_object::make_cons(ctx, runtime, value, result)
            .unwrap_or_else(|error| panic!("make list: {error:?}"));
    }
    result
}

fn assert_list(ctx: &ThreadContext, mut actual: Word, expected: &[Word]) {
    for &value in expected {
        assert!(actual.is_cons());
        assert_eq!(ncl_object::car(ctx, actual), Ok(value));
        actual = ncl_object::cdr(ctx, actual).unwrap_or_else(|error| panic!("cdr: {error:?}"));
    }
    assert_eq!(actual, Word::NIL);
}

#[test]
fn registered_remove_obeys_nil_and_negative_count() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_lib_sequences::register(&runtime).unwrap();

    let count = keyword(&mut ctx, &runtime, "COUNT");
    let values = [2, 1, 2].map(Word::fixnum);
    let sequence = list(&mut ctx, &runtime, &values);
    let removed = call(
        &runtime,
        &mut ctx,
        "REMOVE",
        &[Word::fixnum(2), sequence, count, Word::NIL],
    );
    assert_list(&ctx, removed, &[Word::fixnum(1)]);

    let sequence = list(&mut ctx, &runtime, &values);
    let unchanged = call(
        &runtime,
        &mut ctx,
        "REMOVE",
        &[Word::fixnum(2), sequence, count, Word::fixnum(-1)],
    );
    assert_list(&ctx, unchanged, &values);
}

#[test]
fn registered_position_obeys_nil_end_and_explicit_from_end_range() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_lib_sequences::register(&runtime).unwrap();

    let end = keyword(&mut ctx, &runtime, "END");
    let sequence = list(&mut ctx, &runtime, &[1, 2, 3].map(Word::fixnum));
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "POSITION",
            &[Word::fixnum(3), sequence, end, Word::NIL],
        ),
        Word::fixnum(2)
    );

    let start = keyword(&mut ctx, &runtime, "START");
    let from_end = keyword(&mut ctx, &runtime, "FROM-END");
    let sequence = list(&mut ctx, &runtime, &[1, 2, 2, 3].map(Word::fixnum));
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "POSITION",
            &[
                Word::fixnum(2),
                sequence,
                start,
                Word::fixnum(1),
                end,
                Word::fixnum(3),
                from_end,
                Word::TRUE,
            ],
        ),
        Word::fixnum(2)
    );
}

#[test]
fn registered_remove_passes_object_before_sequence_element() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_lib_sequences::register(&runtime).unwrap();
    let predicate = runtime
        .register_builtin(
            &mut ctx,
            SELECTION_TEST_IDENTIFIER,
            BuiltinImplementation::direct(
                Builtin {
                    lambda_list: LambdaList::fixed(SELECTION_TEST_PARAMETERS),
                    convention: BuiltinConvention::Direct(Arity::exact(2)),
                },
                greater_predicate,
            ),
        )
        .unwrap();
    let values = [1, 2, 4, 1, 3, 4, 5].map(Word::fixnum);
    let test = keyword(&mut ctx, &runtime, "TEST");
    let sequence = list(&mut ctx, &runtime, &values);
    let removed = call(
        &runtime,
        &mut ctx,
        "REMOVE",
        &[Word::fixnum(3), sequence, test, predicate.as_word()],
    );
    assert_list(&ctx, removed, &[4, 3, 4, 5].map(Word::fixnum));

    let test_not = keyword(&mut ctx, &runtime, "TEST-NOT");
    let sequence = list(&mut ctx, &runtime, &values);
    let removed = call(
        &runtime,
        &mut ctx,
        "REMOVE",
        &[Word::fixnum(3), sequence, test_not, predicate.as_word()],
    );
    assert_list(&ctx, removed, &[1, 2, 1].map(Word::fixnum));
}

#[test]
fn registered_zero_count_preserves_substitution_and_validates_bounds() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_lib_sequences::register(&runtime).unwrap();

    let count = keyword(&mut ctx, &runtime, "COUNT");
    let values = [1, 2, 2].map(Word::fixnum);
    let sequence = list(&mut ctx, &runtime, &values);
    let substituted = call(
        &runtime,
        &mut ctx,
        "SUBSTITUTE",
        &[
            Word::fixnum(9),
            Word::fixnum(2),
            sequence,
            count,
            Word::fixnum(0),
        ],
    );
    assert_list(&ctx, substituted, &values);

    let start = keyword(&mut ctx, &runtime, "START");
    let sequence = list(&mut ctx, &runtime, &values);
    let error = call_result(
        &runtime,
        &mut ctx,
        "REMOVE",
        &[
            Word::fixnum(2),
            sequence,
            count,
            Word::fixnum(0),
            start,
            Word::fixnum(4),
        ],
    );
    assert!(matches!(error, Err(ObjectError::TypeError)));
}

#[test]
fn registered_from_end_count_removes_the_last_match() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_lib_sequences::register(&runtime).unwrap();

    let from_end = keyword(&mut ctx, &runtime, "FROM-END");
    let count = keyword(&mut ctx, &runtime, "COUNT");
    let values = [1, 2, 3, 2, 4].map(Word::fixnum);
    let sequence = list(&mut ctx, &runtime, &values);
    let removed = call(
        &runtime,
        &mut ctx,
        "REMOVE",
        &[
            Word::fixnum(2),
            sequence,
            from_end,
            Word::TRUE,
            count,
            Word::fixnum(1),
        ],
    );
    assert_list(&ctx, removed, &[1, 2, 3, 4].map(Word::fixnum));
}

#[test]
fn registered_selection_handles_empty_vectors_and_list_vector_string_inputs() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_lib_sequences::register(&runtime).unwrap();

    let values = [1, 2, 3].map(Word::fixnum);
    let sequence = list(&mut ctx, &runtime, &values);
    assert_eq!(
        call(&runtime, &mut ctx, "POSITION", &[Word::fixnum(9), sequence],),
        Word::NIL
    );
    assert_eq!(
        call(&runtime, &mut ctx, "COUNT", &[Word::fixnum(2), sequence]),
        Word::fixnum(1)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "FIND", &[Word::fixnum(9), sequence]),
        Word::NIL
    );
    let empty_vector = ncl_object::make_simple_vector(&mut ctx, &runtime, &[]).unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, "FIND", &[Word::fixnum(9), empty_vector]),
        Word::NIL
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "POSITION",
            &[Word::fixnum(9), empty_vector],
        ),
        Word::NIL
    );

    let vector = ncl_object::make_simple_vector(&mut ctx, &runtime, &values).unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, "FIND", &[Word::fixnum(2), vector]),
        Word::fixnum(2)
    );

    let string = ncl_object::make_string(&mut ctx, &runtime, &['a', 'b', 'c']).unwrap();
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "POSITION",
            &[Word::character(u32::from('b')), string],
        ),
        Word::fixnum(1)
    );
}

#[test]
fn registered_key_maps_list_elements_before_matching() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_lib_sequences::register(&runtime).unwrap();
    let predicate = runtime
        .register_builtin(
            &mut ctx,
            SELECTION_TEST_IDENTIFIER,
            BuiltinImplementation::direct(
                Builtin {
                    lambda_list: LambdaList::fixed(SELECTION_TEST_PARAMETERS),
                    convention: BuiltinConvention::Direct(Arity::exact(2)),
                },
                greater_predicate,
            ),
        )
        .unwrap();

    let values = [1, 2, 3]
        .map(Word::fixnum)
        .map(|value| ncl_object::make_cons(&mut ctx, &runtime, value, Word::NIL).unwrap());
    let sequence = list(&mut ctx, &runtime, &values);
    let key = runtime
        .function(&mut ctx, "COMMON-LISP", "CAR")
        .unwrap_or_else(|| panic!("missing builtin CAR"));
    let key = FunctionObject::try_from(key)
        .unwrap_or_else(|_| panic!("CAR is not a function"))
        .as_word();
    let key_option = keyword(&mut ctx, &runtime, "KEY");
    let found = call(
        &runtime,
        &mut ctx,
        "FIND",
        &[Word::fixnum(2), sequence, key_option, key],
    );
    assert_eq!(ncl_object::car(&ctx, found), Ok(Word::fixnum(2)));

    let test_option = keyword(&mut ctx, &runtime, "TEST");
    let found = call(
        &runtime,
        &mut ctx,
        "FIND",
        &[
            Word::fixnum(2),
            sequence,
            key_option,
            key,
            test_option,
            predicate.as_word(),
        ],
    );
    assert_eq!(ncl_object::car(&ctx, found), Ok(Word::fixnum(1)));
}

#[test]
fn registered_remove_duplicates_respects_direction_and_range() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("test operation failed: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("test operation failed: {error:?}"));
    ncl_lib_sequences::register(&runtime)
        .unwrap_or_else(|error| panic!("test operation failed: {error:?}"));

    let start = keyword(&mut ctx, &runtime, "START");
    let end = keyword(&mut ctx, &runtime, "END");
    let from_end = keyword(&mut ctx, &runtime, "FROM-END");
    let sequence = list(&mut ctx, &runtime, &[9, 1, 1, 2, 1, 8].map(Word::fixnum));

    let result = call(
        &runtime,
        &mut ctx,
        "REMOVE-DUPLICATES",
        &[sequence, start, Word::fixnum(1), end, Word::fixnum(5)],
    );
    assert_list(&ctx, result, &[9, 2, 1, 8].map(Word::fixnum));

    let result = call(
        &runtime,
        &mut ctx,
        "REMOVE-DUPLICATES",
        &[
            sequence,
            start,
            Word::fixnum(1),
            end,
            Word::fixnum(5),
            from_end,
            Word::TRUE,
        ],
    );
    assert_list(&ctx, result, &[9, 1, 2, 8].map(Word::fixnum));
}
