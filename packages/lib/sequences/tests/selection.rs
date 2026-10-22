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
type TestResult = Result<(), Box<dyn std::error::Error>>;

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

#[test]
fn registered_remove_obeys_nil_and_negative_count() -> TestResult {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_sequences::register(&runtime)?;

    let count = keyword(&mut ctx, &runtime, "COUNT")?;
    let values = [2, 1, 2].map(Word::fixnum);
    let sequence = list(&mut ctx, &runtime, &values)?;
    let removed = call(
        &runtime,
        &mut ctx,
        "REMOVE",
        &[Word::fixnum(2), sequence, count, Word::NIL],
    )?;
    assert_list(&ctx, removed, &[Word::fixnum(1)])?;

    let sequence = list(&mut ctx, &runtime, &values)?;
    let unchanged = call(
        &runtime,
        &mut ctx,
        "REMOVE",
        &[Word::fixnum(2), sequence, count, Word::fixnum(-1)],
    )?;
    assert_list(&ctx, unchanged, &values)?;
    Ok(())
}

#[test]
fn registered_position_obeys_nil_end_and_explicit_from_end_range() -> TestResult {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_sequences::register(&runtime)?;

    let end = keyword(&mut ctx, &runtime, "END")?;
    let sequence = list(&mut ctx, &runtime, &[1, 2, 3].map(Word::fixnum))?;
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "POSITION",
            &[Word::fixnum(3), sequence, end, Word::NIL],
        )?,
        Word::fixnum(2)
    );

    let start = keyword(&mut ctx, &runtime, "START")?;
    let from_end = keyword(&mut ctx, &runtime, "FROM-END")?;
    let sequence = list(&mut ctx, &runtime, &[1, 2, 2, 3].map(Word::fixnum))?;
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
        )?,
        Word::fixnum(2)
    );
    Ok(())
}

#[test]
fn registered_remove_passes_object_before_sequence_element() -> TestResult {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_sequences::register(&runtime)?;
    let predicate = runtime.register_builtin(
        &mut ctx,
        SELECTION_TEST_IDENTIFIER,
        BuiltinImplementation::direct(
            Builtin {
                lambda_list: LambdaList::fixed(SELECTION_TEST_PARAMETERS),
                convention: BuiltinConvention::Direct(Arity::exact(2)),
            },
            greater_predicate,
        ),
    )?;
    let values = [1, 2, 4, 1, 3, 4, 5].map(Word::fixnum);
    let test = keyword(&mut ctx, &runtime, "TEST")?;
    let sequence = list(&mut ctx, &runtime, &values)?;
    let removed = call(
        &runtime,
        &mut ctx,
        "REMOVE",
        &[Word::fixnum(3), sequence, test, predicate.as_word()],
    )?;
    assert_list(&ctx, removed, &[4, 3, 4, 5].map(Word::fixnum))?;

    let test_not = keyword(&mut ctx, &runtime, "TEST-NOT")?;
    let sequence = list(&mut ctx, &runtime, &values)?;
    let removed = call(
        &runtime,
        &mut ctx,
        "REMOVE",
        &[Word::fixnum(3), sequence, test_not, predicate.as_word()],
    )?;
    assert_list(&ctx, removed, &[1, 2, 1].map(Word::fixnum))?;
    Ok(())
}

#[test]
fn registered_zero_count_preserves_substitution_and_validates_bounds() -> TestResult {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_sequences::register(&runtime)?;

    let count = keyword(&mut ctx, &runtime, "COUNT")?;
    let values = [1, 2, 2].map(Word::fixnum);
    let sequence = list(&mut ctx, &runtime, &values)?;
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
    )?;
    assert_list(&ctx, substituted, &values)?;

    let start = keyword(&mut ctx, &runtime, "START")?;
    let sequence = list(&mut ctx, &runtime, &values)?;
    let error = call(
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
    Ok(())
}

#[test]
fn registered_from_end_count_removes_the_last_match() -> TestResult {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_sequences::register(&runtime)?;

    let from_end = keyword(&mut ctx, &runtime, "FROM-END")?;
    let count = keyword(&mut ctx, &runtime, "COUNT")?;
    let values = [1, 2, 3, 2, 4].map(Word::fixnum);
    let sequence = list(&mut ctx, &runtime, &values)?;
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
    )?;
    assert_list(&ctx, removed, &[1, 2, 3, 4].map(Word::fixnum))?;
    Ok(())
}

#[test]
fn registered_selection_handles_empty_vectors_and_list_vector_string_inputs() -> TestResult {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_sequences::register(&runtime)?;

    let values = [1, 2, 3].map(Word::fixnum);
    let sequence = list(&mut ctx, &runtime, &values)?;
    assert_eq!(
        call(&runtime, &mut ctx, "POSITION", &[Word::fixnum(9), sequence],)?,
        Word::NIL
    );
    assert_eq!(
        call(&runtime, &mut ctx, "COUNT", &[Word::fixnum(2), sequence])?,
        Word::fixnum(1)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "FIND", &[Word::fixnum(9), sequence])?,
        Word::NIL
    );
    let empty_vector = ncl_object::make_simple_vector(&mut ctx, &runtime, &[])?;
    assert_eq!(
        call(&runtime, &mut ctx, "FIND", &[Word::fixnum(9), empty_vector])?,
        Word::NIL
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "POSITION",
            &[Word::fixnum(9), empty_vector],
        )?,
        Word::NIL
    );

    let vector = ncl_object::make_simple_vector(&mut ctx, &runtime, &values)?;
    assert_eq!(
        call(&runtime, &mut ctx, "FIND", &[Word::fixnum(2), vector])?,
        Word::fixnum(2)
    );

    let string = ncl_object::make_string(&mut ctx, &runtime, &['a', 'b', 'c'])?;
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "POSITION",
            &[Word::character(u32::from('b')), string],
        )?,
        Word::fixnum(1)
    );
    Ok(())
}

#[test]
fn registered_key_maps_list_elements_before_matching() -> TestResult {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_sequences::register(&runtime)?;
    let predicate = runtime.register_builtin(
        &mut ctx,
        SELECTION_TEST_IDENTIFIER,
        BuiltinImplementation::direct(
            Builtin {
                lambda_list: LambdaList::fixed(SELECTION_TEST_PARAMETERS),
                convention: BuiltinConvention::Direct(Arity::exact(2)),
            },
            greater_predicate,
        ),
    )?;

    let values = [1, 2, 3]
        .map(Word::fixnum)
        .map(|value| ncl_object::make_cons(&mut ctx, &runtime, value, Word::NIL))
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    let sequence = list(&mut ctx, &runtime, &values)?;
    let key = runtime
        .function(&mut ctx, "COMMON-LISP", "CAR")
        .ok_or(ObjectError::UndefinedFunction)?;
    let key = FunctionObject::try_from(key)?.as_word();
    let key_option = keyword(&mut ctx, &runtime, "KEY")?;
    let found = call(
        &runtime,
        &mut ctx,
        "FIND",
        &[Word::fixnum(2), sequence, key_option, key],
    )?;
    assert_eq!(ncl_object::car(&ctx, found), Ok(Word::fixnum(2)));

    let test_option = keyword(&mut ctx, &runtime, "TEST")?;
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
    )?;
    assert_eq!(ncl_object::car(&ctx, found), Ok(Word::fixnum(1)));
    Ok(())
}

#[test]
fn registered_remove_duplicates_respects_direction_and_range() -> TestResult {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_sequences::register(&runtime)?;

    let start = keyword(&mut ctx, &runtime, "START")?;
    let end = keyword(&mut ctx, &runtime, "END")?;
    let from_end = keyword(&mut ctx, &runtime, "FROM-END")?;
    let sequence = list(&mut ctx, &runtime, &[9, 1, 1, 2, 1, 8].map(Word::fixnum))?;

    let result = call(
        &runtime,
        &mut ctx,
        "REMOVE-DUPLICATES",
        &[sequence, start, Word::fixnum(1), end, Word::fixnum(5)],
    )?;
    assert_list(&ctx, result, &[9, 2, 1, 8].map(Word::fixnum))?;

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
    )?;
    assert_list(&ctx, result, &[9, 1, 2, 8].map(Word::fixnum))?;
    Ok(())
}

mod search_mismatch_public_api {
    use super::greater_predicate;
    use ncl_object::{
        Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
        BuiltinName, BuiltinPackage, FunctionObject, LambdaList, MultipleValues, ObjectError,
        Parameter, ParameterType, Runtime, ThreadContext, Word,
    };

    const SELECTION_TEST_PARAMETERS: &[Parameter] = super::SELECTION_TEST_PARAMETERS;
    const SELECTION_TEST_IDENTIFIER: BuiltinIdentifier = super::SELECTION_TEST_IDENTIFIER;
    const SELECTION_ODD_PARAMETERS: &[Parameter] = &[Parameter {
        name: BuiltinName::new("OBJECT"),
        ty: ParameterType::Fixnum,
    }];
    const SELECTION_ODD_IDENTIFIER: BuiltinIdentifier = BuiltinIdentifier::new(
        BuiltinPackage::NclTest,
        BuiltinName::new("SEQUENCE-TEST-ODD"),
    );

    fn call(runtime: &Runtime, ctx: &mut ThreadContext, name: &str, args: &[Word]) -> Word {
        super::call(runtime, ctx, name, args)
            .unwrap_or_else(|error| panic!("test operation failed: {error:?}"))
    }

    fn call_result(
        runtime: &Runtime,
        ctx: &mut ThreadContext,
        name: &str,
        args: &[Word],
    ) -> Result<Word, ObjectError> {
        super::call(runtime, ctx, name, args)
    }

    fn keyword(ctx: &mut ThreadContext, runtime: &Runtime, name: &str) -> Word {
        super::keyword(ctx, runtime, name)
            .unwrap_or_else(|error| panic!("test operation failed: {error:?}"))
    }

    fn list(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Word {
        super::list(ctx, runtime, values)
            .unwrap_or_else(|error| panic!("test operation failed: {error:?}"))
    }

    fn odd_predicate(
        _ctx: &mut ThreadContext,
        _runtime: &Runtime,
        args: &BuiltinArgs<'_>,
        values: &mut MultipleValues,
    ) -> Result<Word, ObjectError> {
        let object = args
            .required(0)?
            .as_fixnum()
            .ok_or(ObjectError::TypeError)?;
        values.clear();
        Ok(if object % 2 != 0 {
            Word::TRUE
        } else {
            Word::NIL
        })
    }

    fn registered_selection_runtime() -> (Runtime, ThreadContext) {
        let runtime =
            Runtime::new().unwrap_or_else(|error| panic!("test operation failed: {error:?}"));
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)
            .unwrap_or_else(|error| panic!("test operation failed: {error:?}"));
        ncl_lib_sequences::register(&runtime)
            .unwrap_or_else(|error| panic!("test operation failed: {error:?}"));
        (runtime, ctx)
    }

    #[test]
    fn registered_search_returns_leftmost_rightmost_and_no_match_results() {
        let runtime =
            Runtime::new().unwrap_or_else(|error| panic!("test operation failed: {error:?}"));
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)
            .unwrap_or_else(|error| panic!("test operation failed: {error:?}"));
        ncl_lib_sequences::register(&runtime)
            .unwrap_or_else(|error| panic!("test operation failed: {error:?}"));

        let pattern = list(&mut ctx, &runtime, &[2, 3].map(Word::fixnum));
        let sequence = list(&mut ctx, &runtime, &[1, 2, 3, 2, 3].map(Word::fixnum));
        assert_eq!(
            call(&runtime, &mut ctx, "SEARCH", &[pattern, sequence]),
            Word::fixnum(1)
        );

        let from_end = keyword(&mut ctx, &runtime, "FROM-END");
        assert_eq!(
            call(
                &runtime,
                &mut ctx,
                "SEARCH",
                &[pattern, sequence, from_end, Word::TRUE],
            ),
            Word::fixnum(3)
        );

        let absent_pattern = list(&mut ctx, &runtime, &[3, 1].map(Word::fixnum));
        assert_eq!(
            call(&runtime, &mut ctx, "SEARCH", &[absent_pattern, sequence]),
            Word::NIL
        );

        let longer_pattern = list(&mut ctx, &runtime, &[1, 2, 3, 4, 5, 6].map(Word::fixnum));
        assert_eq!(
            call(&runtime, &mut ctx, "SEARCH", &[longer_pattern, sequence]),
            Word::NIL
        );
    }

    #[test]
    fn registered_search_tests_sequence_one_before_sequence_two() {
        let runtime =
            Runtime::new().unwrap_or_else(|error| panic!("test operation failed: {error:?}"));
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)
            .unwrap_or_else(|error| panic!("test operation failed: {error:?}"));
        ncl_lib_sequences::register(&runtime)
            .unwrap_or_else(|error| panic!("test operation failed: {error:?}"));
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
            .unwrap_or_else(|error| panic!("test operation failed: {error:?}"));

        let pattern = list(&mut ctx, &runtime, &[3].map(Word::fixnum));
        let sequence = list(&mut ctx, &runtime, &[1, 4].map(Word::fixnum));
        let test = keyword(&mut ctx, &runtime, "TEST");
        assert_eq!(
            call(
                &runtime,
                &mut ctx,
                "SEARCH",
                &[pattern, sequence, test, predicate.as_word()],
            ),
            Word::fixnum(0)
        );
    }

    #[test]
    fn registered_search_applies_key_to_both_sequences() {
        let runtime =
            Runtime::new().unwrap_or_else(|error| panic!("test operation failed: {error:?}"));
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)
            .unwrap_or_else(|error| panic!("test operation failed: {error:?}"));
        ncl_lib_sequences::register(&runtime)
            .unwrap_or_else(|error| panic!("test operation failed: {error:?}"));
        let key = runtime
            .register_builtin(
                &mut ctx,
                SELECTION_ODD_IDENTIFIER,
                BuiltinImplementation::direct(
                    Builtin {
                        lambda_list: LambdaList::fixed(SELECTION_ODD_PARAMETERS),
                        convention: BuiltinConvention::Direct(Arity::exact(1)),
                    },
                    odd_predicate,
                ),
            )
            .unwrap_or_else(|error| panic!("test operation failed: {error:?}"))
            .as_word();

        let pattern = list(&mut ctx, &runtime, &[0, 1].map(Word::fixnum));
        let sequence = list(&mut ctx, &runtime, &[2, 4, 6, 1, 3, 5].map(Word::fixnum));
        let key_option = keyword(&mut ctx, &runtime, "KEY");

        assert_eq!(
            call(
                &runtime,
                &mut ctx,
                "SEARCH",
                &[pattern, sequence, key_option, key],
            ),
            Word::fixnum(2)
        );
    }

    #[test]
    fn registered_mismatch_reports_leftmost_and_from_end_difference_positions() {
        let runtime =
            Runtime::new().unwrap_or_else(|error| panic!("test operation failed: {error:?}"));
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)
            .unwrap_or_else(|error| panic!("test operation failed: {error:?}"));
        ncl_lib_sequences::register(&runtime)
            .unwrap_or_else(|error| panic!("test operation failed: {error:?}"));

        let left = list(&mut ctx, &runtime, &[1, 2, 3, 4].map(Word::fixnum));
        let right = list(&mut ctx, &runtime, &[1, 9, 3, 8].map(Word::fixnum));
        assert_eq!(
            call(&runtime, &mut ctx, "MISMATCH", &[left, right]),
            Word::fixnum(1)
        );

        let from_end = keyword(&mut ctx, &runtime, "FROM-END");
        assert_eq!(
            call(
                &runtime,
                &mut ctx,
                "MISMATCH",
                &[left, right, from_end, Word::TRUE],
            ),
            Word::fixnum(4)
        );

        let aligned_left = list(&mut ctx, &runtime, &[3, 2, 1, 1, 2, 3].map(Word::fixnum));
        let aligned_right = list(&mut ctx, &runtime, &[1, 2, 3].map(Word::fixnum));
        assert_eq!(
            call(
                &runtime,
                &mut ctx,
                "MISMATCH",
                &[aligned_left, aligned_right, from_end, Word::TRUE],
            ),
            Word::fixnum(3)
        );

        let equal_left = list(&mut ctx, &runtime, &[1, 2, 3].map(Word::fixnum));
        let equal_right = list(&mut ctx, &runtime, &[1, 2, 3].map(Word::fixnum));
        assert_eq!(
            call(&runtime, &mut ctx, "MISMATCH", &[equal_left, equal_right]),
            Word::NIL
        );
    }

    #[test]
    fn registered_mismatch_tests_sequence_one_before_sequence_two() {
        let runtime =
            Runtime::new().unwrap_or_else(|error| panic!("test operation failed: {error:?}"));
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)
            .unwrap_or_else(|error| panic!("test operation failed: {error:?}"));
        ncl_lib_sequences::register(&runtime)
            .unwrap_or_else(|error| panic!("test operation failed: {error:?}"));
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
            .unwrap_or_else(|error| panic!("test operation failed: {error:?}"));

        let left = list(&mut ctx, &runtime, &[3].map(Word::fixnum));
        let right = list(&mut ctx, &runtime, &[1].map(Word::fixnum));
        let test = keyword(&mut ctx, &runtime, "TEST");
        assert_eq!(
            call(
                &runtime,
                &mut ctx,
                "MISMATCH",
                &[left, right, test, predicate.as_word()],
            ),
            Word::NIL
        );

        let test_not = keyword(&mut ctx, &runtime, "TEST-NOT");
        assert_eq!(
            call(
                &runtime,
                &mut ctx,
                "MISMATCH",
                &[left, right, test_not, predicate.as_word()],
            ),
            Word::fixnum(0)
        );
    }

    #[test]
    fn registered_mismatch_applies_key_to_both_sequences() {
        let runtime =
            Runtime::new().unwrap_or_else(|error| panic!("test operation failed: {error:?}"));
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)
            .unwrap_or_else(|error| panic!("test operation failed: {error:?}"));
        ncl_lib_sequences::register(&runtime)
            .unwrap_or_else(|error| panic!("test operation failed: {error:?}"));

        let left_values = [1, 2].map(|value| {
            ncl_object::make_cons(&mut ctx, &runtime, Word::fixnum(value), Word::NIL)
                .unwrap_or_else(|error| panic!("test operation failed: {error:?}"))
        });
        let left = list(&mut ctx, &runtime, &left_values);
        let right_values = [1, 2].map(|value| {
            ncl_object::make_cons(&mut ctx, &runtime, Word::fixnum(value), Word::NIL)
                .unwrap_or_else(|error| panic!("test operation failed: {error:?}"))
        });
        let right = list(&mut ctx, &runtime, &right_values);
        let key_function = runtime
            .function(&mut ctx, "COMMON-LISP", "CAR")
            .unwrap_or_else(|| panic!("missing builtin CAR"));
        let key = FunctionObject::try_from(key_function)
            .unwrap_or_else(|_| panic!("CAR is not a function"))
            .as_word();
        let key_option = keyword(&mut ctx, &runtime, "KEY");

        assert_eq!(
            call(
                &runtime,
                &mut ctx,
                "MISMATCH",
                &[left, right, key_option, key],
            ),
            Word::NIL
        );
    }

    #[test]
    fn registered_search_honors_start1() {
        let (runtime, mut ctx) = registered_selection_runtime();
        let start1 = keyword(&mut ctx, &runtime, "START1");
        let pattern = list(&mut ctx, &runtime, &[0, 2, 3].map(Word::fixnum));
        let sequence = list(&mut ctx, &runtime, &[2, 3].map(Word::fixnum));
        assert_eq!(
            call(
                &runtime,
                &mut ctx,
                "SEARCH",
                &[pattern, sequence, start1, Word::fixnum(1)],
            ),
            Word::fixnum(0)
        );
    }

    #[test]
    fn registered_search_honors_end1() {
        let (runtime, mut ctx) = registered_selection_runtime();
        let end1 = keyword(&mut ctx, &runtime, "END1");
        let pattern = list(&mut ctx, &runtime, &[2, 3, 0].map(Word::fixnum));
        let sequence = list(&mut ctx, &runtime, &[2, 3].map(Word::fixnum));
        assert_eq!(
            call(
                &runtime,
                &mut ctx,
                "SEARCH",
                &[pattern, sequence, end1, Word::fixnum(1)],
            ),
            Word::fixnum(0)
        );
    }

    #[test]
    fn registered_search_honors_start2() {
        let (runtime, mut ctx) = registered_selection_runtime();
        let start2 = keyword(&mut ctx, &runtime, "START2");
        let pattern = list(&mut ctx, &runtime, &[2].map(Word::fixnum));
        let sequence = list(&mut ctx, &runtime, &[2, 1, 2].map(Word::fixnum));
        assert_eq!(
            call(
                &runtime,
                &mut ctx,
                "SEARCH",
                &[pattern, sequence, start2, Word::fixnum(1)],
            ),
            Word::fixnum(2)
        );
    }

    #[test]
    fn registered_search_honors_end2() {
        let (runtime, mut ctx) = registered_selection_runtime();
        let end2 = keyword(&mut ctx, &runtime, "END2");
        let from_end = keyword(&mut ctx, &runtime, "FROM-END");
        let pattern = list(&mut ctx, &runtime, &[2].map(Word::fixnum));
        let sequence = list(&mut ctx, &runtime, &[2, 9, 2].map(Word::fixnum));
        assert_eq!(
            call(
                &runtime,
                &mut ctx,
                "SEARCH",
                &[
                    pattern,
                    sequence,
                    end2,
                    Word::fixnum(2),
                    from_end,
                    Word::TRUE,
                ],
            ),
            Word::fixnum(0)
        );
    }

    #[test]
    fn registered_mismatch_honors_start1() {
        let (runtime, mut ctx) = registered_selection_runtime();
        let start1 = keyword(&mut ctx, &runtime, "START1");
        let left = list(&mut ctx, &runtime, &[9, 1, 2].map(Word::fixnum));
        let right = list(&mut ctx, &runtime, &[0, 1, 2].map(Word::fixnum));
        assert_eq!(
            call(
                &runtime,
                &mut ctx,
                "MISMATCH",
                &[left, right, start1, Word::fixnum(1)],
            ),
            Word::fixnum(1)
        );
    }

    #[test]
    fn registered_mismatch_honors_end1() {
        let (runtime, mut ctx) = registered_selection_runtime();
        let end1 = keyword(&mut ctx, &runtime, "END1");
        let left = list(&mut ctx, &runtime, &[1, 2, 3].map(Word::fixnum));
        let right = list(&mut ctx, &runtime, &[1, 2, 3].map(Word::fixnum));
        assert_eq!(
            call(
                &runtime,
                &mut ctx,
                "MISMATCH",
                &[left, right, end1, Word::fixnum(2)],
            ),
            Word::fixnum(2)
        );
    }

    #[test]
    fn registered_mismatch_honors_start2() {
        let (runtime, mut ctx) = registered_selection_runtime();
        let start2 = keyword(&mut ctx, &runtime, "START2");
        let left = list(&mut ctx, &runtime, &[1, 2, 3].map(Word::fixnum));
        let right = list(&mut ctx, &runtime, &[9, 1, 2].map(Word::fixnum));
        assert_eq!(
            call(
                &runtime,
                &mut ctx,
                "MISMATCH",
                &[left, right, start2, Word::fixnum(1)],
            ),
            Word::fixnum(2)
        );
    }

    #[test]
    fn registered_mismatch_honors_end2() {
        let (runtime, mut ctx) = registered_selection_runtime();
        let end2 = keyword(&mut ctx, &runtime, "END2");
        let left = list(&mut ctx, &runtime, &[1, 2].map(Word::fixnum));
        let right = list(&mut ctx, &runtime, &[1, 2, 9].map(Word::fixnum));
        assert_eq!(
            call(
                &runtime,
                &mut ctx,
                "MISMATCH",
                &[left, right, end2, Word::fixnum(2)],
            ),
            Word::NIL
        );
    }

    #[test]
    fn registered_mismatch_aligns_bounded_subsequences_from_end() {
        let (runtime, mut ctx) = registered_selection_runtime();
        let left = list(&mut ctx, &runtime, &[9, 1, 2, 3, 8].map(Word::fixnum));
        let right = list(&mut ctx, &runtime, &[0, 1, 9, 1, 2, 3, 7].map(Word::fixnum));
        let start1 = keyword(&mut ctx, &runtime, "START1");
        let end1 = keyword(&mut ctx, &runtime, "END1");
        let start2 = keyword(&mut ctx, &runtime, "START2");
        let end2 = keyword(&mut ctx, &runtime, "END2");
        let from_end = keyword(&mut ctx, &runtime, "FROM-END");
        assert_eq!(
            call(
                &runtime,
                &mut ctx,
                "MISMATCH",
                &[
                    left,
                    right,
                    start1,
                    Word::fixnum(1),
                    end1,
                    Word::fixnum(4),
                    start2,
                    Word::fixnum(1),
                    end2,
                    Word::fixnum(6),
                    from_end,
                    Word::TRUE,
                ],
            ),
            Word::fixnum(1)
        );

        let equal_left = list(&mut ctx, &runtime, &[9, 1, 2, 3, 8].map(Word::fixnum));
        let equal_right = list(&mut ctx, &runtime, &[0, 1, 2, 3, 7].map(Word::fixnum));
        assert_eq!(
            call(
                &runtime,
                &mut ctx,
                "MISMATCH",
                &[
                    equal_left,
                    equal_right,
                    start1,
                    Word::fixnum(1),
                    end1,
                    Word::fixnum(4),
                    start2,
                    Word::fixnum(1),
                    end2,
                    Word::fixnum(4),
                    from_end,
                    Word::TRUE,
                ],
            ),
            Word::NIL
        );
    }

    #[test]
    fn registered_search_treats_nil_end1_as_unbounded() {
        let (runtime, mut ctx) = registered_selection_runtime();
        let end1 = keyword(&mut ctx, &runtime, "END1");
        let pattern = list(&mut ctx, &runtime, &[1, 9].map(Word::fixnum));
        let sequence = list(&mut ctx, &runtime, &[1, 2, 1, 9].map(Word::fixnum));

        assert_eq!(
            call(
                &runtime,
                &mut ctx,
                "SEARCH",
                &[pattern, sequence, end1, Word::NIL],
            ),
            Word::fixnum(2)
        );
        assert_eq!(
            call(
                &runtime,
                &mut ctx,
                "SEARCH",
                &[pattern, sequence, end1, Word::fixnum(1)],
            ),
            Word::fixnum(0)
        );
    }

    #[test]
    fn registered_search_and_mismatch_enforce_declared_keywords() {
        let (runtime, mut ctx) = registered_selection_runtime();
        let left = list(&mut ctx, &runtime, &[1].map(Word::fixnum));
        let right = list(&mut ctx, &runtime, &[1].map(Word::fixnum));
        let start1 = keyword(&mut ctx, &runtime, "START1");
        let allow_other_keys = keyword(&mut ctx, &runtime, "ALLOW-OTHER-KEYS");

        for operation in ["SEARCH", "MISMATCH"] {
            for name in ["START", "END", "COUNT", "UNRECOGNIZED"] {
                let unsupported = keyword(&mut ctx, &runtime, name);
                let result = call_result(
                    &runtime,
                    &mut ctx,
                    operation,
                    &[left, right, unsupported, Word::TRUE],
                );
                assert!(
                    matches!(result, Err(ObjectError::TypeError)),
                    "{operation} accepted :{name}"
                );
            }

            let unrecognized = keyword(&mut ctx, &runtime, "UNRECOGNIZED");
            let accepted = call(
                &runtime,
                &mut ctx,
                operation,
                &[
                    left,
                    right,
                    unrecognized,
                    Word::TRUE,
                    allow_other_keys,
                    Word::TRUE,
                ],
            );
            let expected = if operation == "SEARCH" {
                Word::fixnum(0)
            } else {
                Word::NIL
            };
            assert_eq!(accepted, expected);

            let rejected = call_result(
                &runtime,
                &mut ctx,
                operation,
                &[
                    left,
                    right,
                    unrecognized,
                    Word::TRUE,
                    allow_other_keys,
                    Word::NIL,
                ],
            );
            assert!(
                matches!(rejected, Err(ObjectError::TypeError)),
                "{operation} ignored :UNRECOGNIZED with :ALLOW-OTHER-KEYS NIL"
            );
        }

        let left = list(&mut ctx, &runtime, &[9, 1].map(Word::fixnum));
        let right = list(&mut ctx, &runtime, &[1].map(Word::fixnum));
        assert_eq!(
            call(
                &runtime,
                &mut ctx,
                "SEARCH",
                &[
                    left,
                    right,
                    start1,
                    Word::fixnum(0),
                    start1,
                    Word::fixnum(1),
                ],
            ),
            Word::NIL
        );
        assert_eq!(
            call(
                &runtime,
                &mut ctx,
                "MISMATCH",
                &[
                    left,
                    right,
                    start1,
                    Word::fixnum(0),
                    start1,
                    Word::fixnum(1),
                ],
            ),
            Word::fixnum(0)
        );
    }

    #[test]
    fn registered_position_and_find_reject_search_mismatch_bounds() {
        let runtime =
            Runtime::new().unwrap_or_else(|error| panic!("test operation failed: {error:?}"));
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)
            .unwrap_or_else(|error| panic!("test operation failed: {error:?}"));
        ncl_lib_sequences::register(&runtime)
            .unwrap_or_else(|error| panic!("test operation failed: {error:?}"));

        let values = list(&mut ctx, &runtime, &[1, 2].map(Word::fixnum));
        let object = Word::fixnum(1);
        for operation in ["POSITION", "FIND"] {
            for name in ["START1", "END1", "START2", "END2"] {
                let bound = keyword(&mut ctx, &runtime, name);
                let result = call_result(
                    &runtime,
                    &mut ctx,
                    operation,
                    &[object, values, bound, Word::fixnum(1)],
                );
                assert!(
                    matches!(result, Err(ObjectError::TypeError)),
                    "{operation} accepted :{name}"
                );
            }
        }
    }

    #[test]
    fn registered_mismatch_treats_nil_end1_as_unbounded() {
        let (runtime, mut ctx) = registered_selection_runtime();
        let end1 = keyword(&mut ctx, &runtime, "END1");
        let left = list(&mut ctx, &runtime, &[1, 9].map(Word::fixnum));
        let right = list(&mut ctx, &runtime, &[1].map(Word::fixnum));

        assert_eq!(
            call(
                &runtime,
                &mut ctx,
                "MISMATCH",
                &[left, right, end1, Word::NIL],
            ),
            Word::fixnum(1)
        );
        assert_eq!(
            call(
                &runtime,
                &mut ctx,
                "MISMATCH",
                &[left, right, end1, Word::fixnum(1)],
            ),
            Word::NIL
        );
    }
}
