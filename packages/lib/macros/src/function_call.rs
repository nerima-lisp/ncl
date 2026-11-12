use ncl_object::{
    Builtin, BuiltinArgs, BuiltinConvention, BuiltinFunctionCaller, BuiltinIdentifier,
    BuiltinImplementation, BuiltinName, BuiltinPackage, FunctionArguments, FunctionCaller,
    FunctionDesignator, FunctionObject, LambdaList, LispError, MultipleValues, ObjectError,
    ObjectType, Parameter, ParameterType, Runtime, ThreadContext, Word, car, cdr, function_name,
    symbol_function,
};

mod multiple_values;

const FUNCTION: Parameter = Parameter {
    name: BuiltinName::new("FUNCTION"),
    ty: ParameterType::FunctionDesignator,
};
const ARGUMENT: Parameter = Parameter {
    name: BuiltinName::new("ARGUMENT"),
    ty: ParameterType::Any,
};
const REST_ARGUMENT: Parameter = Parameter {
    name: BuiltinName::new("ARGUMENTS"),
    ty: ParameterType::Any,
};

fn function_designator(ctx: &ThreadContext, word: Word) -> Result<FunctionDesignator, ObjectError> {
    FunctionDesignator::try_from_word(ctx, word).map_err(|_| ObjectError::TypeError)
}

fn call_designator(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    designator: Word,
    arguments: &[Word],
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let mut rooted = Vec::with_capacity(arguments.len() + 1);
    rooted.push(designator);
    rooted.extend_from_slice(arguments);
    let resolved = ncl_object::with_roots(ctx, &rooted, |ctx, rooted| {
        let designator_word = **rooted.first().ok_or(ObjectError::Layout)?;
        ctx.validate_forwarding(designator_word)?;
        let designator = function_designator(ctx, designator_word).inspect_err(|_| {
            ctx.set_pending_lisp_error(LispError::TypeError {
                datum: designator_word,
                expected: ObjectType::Function,
            });
        })?;
        match designator {
            FunctionDesignator::Function(function) => {
                let name = function_name(ctx, ncl_object::Function::from_word(function.as_word()))?;
                FunctionObject::try_from(symbol_function(ctx, name)?)
                    .or(Ok(function))
                    .map(Some)
            }
            FunctionDesignator::Symbol(_) => Ok(None),
        }
    })?;
    if let Some(resolved) = resolved
        && runtime.builtin_allows_nested_evaluation(resolved)
    {
        return runtime
            .call_builtin(ctx, resolved, arguments)
            .map_err(|error| {
                if matches!(error, ObjectError::Unbound | ObjectError::UndefinedFunction) {
                    ctx.set_pending_lisp_error(LispError::CellError(
                        ncl_object::CellError::UndefinedFunction { name: designator },
                    ));
                    ObjectError::UndefinedFunction
                } else {
                    error
                }
            });
    }
    ncl_object::with_heap_rooted_mut_slice(runtime, &rooted, |rooted| {
        let designator_word = *rooted.first().ok_or(ObjectError::Layout)?;
        ctx.validate_forwarding(designator_word)?;
        let designator = match function_designator(ctx, designator_word) {
            Ok(designator) => designator,
            Err(error) => {
                ctx.set_pending_lisp_error(LispError::TypeError {
                    datum: designator_word,
                    expected: ObjectType::Function,
                });
                return Err(error);
            }
        };
        let arguments = rooted.get(1..).ok_or(ObjectError::Layout)?;
        let mut caller = BuiltinFunctionCaller;
        caller
            .call_function(
                ctx,
                runtime,
                designator,
                FunctionArguments::new(arguments),
                values,
            )
            .map_err(|error| {
                if matches!(error, ObjectError::Unbound | ObjectError::UndefinedFunction) {
                    ctx.set_pending_lisp_error(LispError::CellError(
                        ncl_object::CellError::UndefinedFunction {
                            name: designator_word,
                        },
                    ));
                    ObjectError::UndefinedFunction
                } else {
                    error
                }
            })
    })
}

fn funcall(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let designator = args.required(0)?;
    let arguments: Vec<Word> = (1..args.len())
        .filter_map(|index| args.get(index))
        .collect();
    call_designator(ctx, runtime, designator, &arguments, values)
}

fn append_list(ctx: &ThreadContext, list: Word, output: &mut Vec<Word>) -> Result<(), ObjectError> {
    let mut cursor = list;
    while cursor != Word::NIL {
        if !cursor.is_cons() {
            return Err(ObjectError::TypeError);
        }
        output.push(car(ctx, cursor)?);
        cursor = cdr(ctx, cursor)?;
    }
    Ok(())
}

fn apply(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let designator = args.required(0)?;
    let supplied: Vec<Word> = (0..args.len())
        .filter_map(|index| args.get(index))
        .collect();
    if supplied.len() < 2 {
        return Err(ObjectError::TypeError);
    }
    let last = *supplied.last().ok_or(ObjectError::TypeError)?;
    let mut arguments = supplied[1..supplied.len() - 1].to_vec();
    append_list(ctx, last, &mut arguments)?;
    call_designator(ctx, runtime, designator, &arguments, values)
}

/// Register the function-calling builtins owned by this crate.
///
/// # Errors
/// Returns an object-layer error if either builtin cannot be registered.
pub fn register(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<(), ObjectError> {
    runtime.register_builtin(
        ctx,
        BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new("FUNCALL")),
        BuiltinImplementation::adapted(
            Builtin {
                lambda_list: LambdaList::with_rest(&[FUNCTION], REST_ARGUMENT),
                convention: BuiltinConvention::Adapted,
            },
            funcall,
            |args| Ok(args.as_slice().to_vec()),
        )
        .with_nested_evaluation(),
    )?;
    runtime.register_builtin(
        ctx,
        BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new("APPLY")),
        BuiltinImplementation::adapted(
            Builtin {
                lambda_list: LambdaList::with_rest(&[FUNCTION, ARGUMENT], REST_ARGUMENT),
                convention: BuiltinConvention::Adapted,
            },
            apply,
            |args| Ok(args.as_slice().to_vec()),
        )
        .with_nested_evaluation(),
    )?;
    multiple_values::register(ctx, runtime)?;
    Ok(())
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "unit tests assert on callback and multiple-value results from valid test functions"
)]
mod tests {
    use super::*;
    use ncl_object::{
        Arity, Package, make_code_object, make_cons, make_simple_fun, make_string, make_symbol,
    };
    use std::sync::Arc;

    fn add(
        _ctx: &mut ThreadContext,
        _runtime: &Runtime,
        args: &BuiltinArgs<'_>,
        values: &mut MultipleValues,
    ) -> Result<Word, ObjectError> {
        let sum = (0..args.len())
            .map(|index| {
                args.get(index)
                    .and_then(Word::as_fixnum)
                    .ok_or(ObjectError::TypeError)
            })
            .try_fold(0_i64, |sum, value| value.map(|value| sum + value))?;
        values.set(&[Word::fixnum(sum), Word::fixnum(sum + 1)]);
        Ok(Word::fixnum(sum))
    }

    #[expect(
        clippy::unnecessary_wraps,
        reason = "test callbacks match the fallible builtin ABI while setting multiple values"
    )]
    fn returns_zero(
        _ctx: &mut ThreadContext,
        _runtime: &Runtime,
        _args: &BuiltinArgs<'_>,
        values: &mut MultipleValues,
    ) -> Result<Word, ObjectError> {
        values.set(&[]);
        Ok(Word::NIL)
    }

    #[expect(
        clippy::unnecessary_wraps,
        reason = "test callbacks match the fallible builtin ABI while setting multiple values"
    )]
    fn returns_one(
        _ctx: &mut ThreadContext,
        _runtime: &Runtime,
        _args: &BuiltinArgs<'_>,
        values: &mut MultipleValues,
    ) -> Result<Word, ObjectError> {
        let value = Word::fixnum(11);
        values.set(&[value]);
        Ok(value)
    }

    #[expect(
        clippy::unnecessary_wraps,
        reason = "test callbacks match the fallible builtin ABI while setting multiple values"
    )]
    fn returns_two(
        _ctx: &mut ThreadContext,
        _runtime: &Runtime,
        _args: &BuiltinArgs<'_>,
        values: &mut MultipleValues,
    ) -> Result<Word, ObjectError> {
        let values_to_return = [Word::fixnum(21), Word::fixnum(22)];
        values.set(&values_to_return);
        Ok(values_to_return[0])
    }

    fn setup() -> (Runtime, ThreadContext, ncl_object::FunctionObject) {
        let runtime = Runtime::new().unwrap();
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime).unwrap();
        let function = runtime
            .register_builtin(
                &mut ctx,
                BuiltinIdentifier::new(BuiltinPackage::NclTest, BuiltinName::new("ADD")),
                BuiltinImplementation::direct(
                    Builtin {
                        lambda_list: LambdaList::with_rest(&[], ARGUMENT),
                        convention: BuiltinConvention::Direct(Arity::exact(0)),
                    },
                    add,
                ),
            )
            .unwrap();
        register(&mut ctx, &runtime).unwrap();
        (runtime, ctx, function)
    }

    fn register_result_builtin(
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        name: &'static str,
        function: ncl_object::RustBuiltin,
    ) -> ncl_object::FunctionObject {
        runtime
            .register_builtin(
                ctx,
                BuiltinIdentifier::new(BuiltinPackage::NclTest, BuiltinName::new(name)),
                BuiltinImplementation::direct(
                    Builtin {
                        lambda_list: LambdaList::new(&[], &[], None, &[], false),
                        convention: BuiltinConvention::Direct(Arity::exact(0)),
                    },
                    function,
                ),
            )
            .unwrap()
    }

    #[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
    fn encode_word(word: Word) -> u64 {
        !word.bits()
    }

    #[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
    fn decode_word(encoded: u64) -> Word {
        Word::from_bits(!encoded)
    }

    #[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
    fn root_encoded_word(ctx: &ThreadContext, encoded: u64) -> ncl_object::HeapRootedWord {
        ncl_object::HeapRootedWord::new(ctx, decode_word(encoded)).unwrap()
    }

    #[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
    struct StaleFunctionFixture {
        runtime: Arc<Runtime>,
        symbol_bits: u64,
        function_bits: u64,
        _new_function_bits: u64,
        funcall_bits: u64,
        apply_bits: u64,
        _code: [ncl_sys::CodePtr; 2],
    }

    #[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
    fn published_value_code(value: u16) -> ncl_sys::CodePtr {
        #[cfg(target_arch = "x86_64")]
        let bytes = {
            let fixnum = u32::from(value) * 2;
            let mut bytes = [0; 11];
            bytes[0] = 0xb8;
            bytes[1..5].copy_from_slice(&fixnum.to_le_bytes());
            bytes[5] = 0xba;
            bytes[6..10].copy_from_slice(&1_u32.to_le_bytes());
            bytes[10] = 0xc3;
            bytes
        };
        #[cfg(target_arch = "aarch64")]
        let bytes = {
            let mov_value = 0xd280_0000_u32 | ((u32::from(value) * 2) << 5);
            let mut bytes = [0; 12];
            bytes[..4].copy_from_slice(&mov_value.to_le_bytes());
            bytes[4..8].copy_from_slice(&0xd280_0021_u32.to_le_bytes());
            bytes[8..].copy_from_slice(&0xd65f_03c0_u32.to_le_bytes());
            bytes
        };
        let mut code = ncl_sys::alloc_code(bytes.len()).unwrap();
        ncl_sys::write_code(&mut code, 0, &bytes).unwrap();
        ncl_sys::publish_code(&mut code).unwrap();
        code
    }

    #[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
    fn prepare_stale_function_fixture(runtime: Arc<Runtime>) -> StaleFunctionFixture {
        let mut setup = ThreadContext::new();
        setup.register(&runtime).unwrap();
        register(&mut setup, &runtime).unwrap();
        let funcall_word = runtime
            .function(&mut setup, "COMMON-LISP", "FUNCALL")
            .unwrap();
        let apply_word = runtime
            .function(&mut setup, "COMMON-LISP", "APPLY")
            .unwrap();
        let old_code_ptr = published_value_code(11);
        let new_code_ptr = published_value_code(29);
        let old_code = make_code_object(
            &mut setup,
            &runtime,
            old_code_ptr.address(),
            old_code_ptr.len(),
            Word::NIL,
            Word::NIL,
            Word::NIL,
        )
        .unwrap();
        let new_code = make_code_object(
            &mut setup,
            &runtime,
            new_code_ptr.address(),
            new_code_ptr.len(),
            Word::NIL,
            Word::NIL,
            Word::NIL,
        )
        .unwrap();
        let symbol_name = make_string(&mut setup, &runtime, &['f']).unwrap();
        let symbol = make_symbol(&mut setup, &runtime, symbol_name).unwrap();
        for _ in 0..1_024 {
            make_code_object(
                &mut setup,
                &runtime,
                old_code_ptr.address(),
                old_code_ptr.len(),
                Word::NIL,
                Word::NIL,
                Word::NIL,
            )
            .unwrap();
        }
        let old_function = make_simple_fun(
            &mut setup,
            &runtime,
            old_code_ptr.address(),
            Word::NIL,
            Word::NIL,
            old_code,
        )
        .unwrap()
        .as_word();
        let new_function = make_simple_fun(
            &mut setup,
            &runtime,
            new_code_ptr.address(),
            Word::NIL,
            Word::NIL,
            new_code,
        )
        .unwrap()
        .as_word();
        setup
            .write_object_slot(symbol, ncl_object::symbol_offset::FUNCTION, old_function)
            .unwrap();
        let fixture = StaleFunctionFixture {
            runtime,
            symbol_bits: encode_word(symbol),
            function_bits: encode_word(old_function),
            _new_function_bits: encode_word(new_function),
            funcall_bits: encode_word(funcall_word),
            apply_bits: encode_word(apply_word),
            _code: [old_code_ptr, new_code_ptr],
        };
        fixture
    }

    #[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
    #[test]
    fn funcall_and_apply_report_forwarding_violation_for_stale_symbol_functions() {
        let runtime = Arc::new(Runtime::new().unwrap());
        let fixture_runtime = Arc::clone(&runtime);
        let fixture = std::thread::spawn(move || prepare_stale_function_fixture(fixture_runtime))
            .join()
            .unwrap();
        let runtime = &fixture.runtime;

        let mut ctx = ThreadContext::new();
        ctx.register(&runtime).unwrap();
        let symbol = root_encoded_word(&ctx, fixture.symbol_bits);
        let funcall = root_encoded_word(&ctx, fixture.funcall_bits);
        let apply = root_encoded_word(&ctx, fixture.apply_bits);
        ctx.collect(true).unwrap();
        let stale_function = decode_word(fixture.function_bits);
        let current_function = symbol_function(&ctx, symbol.get()).unwrap();
        assert_ne!(current_function, stale_function);
        let funcall = FunctionObject::try_from(funcall.get()).unwrap();
        let apply = FunctionObject::try_from(apply.get()).unwrap();
        ctx.write_object_slot(
            symbol.get(),
            ncl_object::symbol_offset::FUNCTION,
            stale_function,
        )
        .unwrap();
        ctx.set_strict_forwarding(true);
        assert_eq!(
            ctx.validate_forwarding(stale_function),
            Err(ObjectError::ForwardingViolation)
        );
        assert_eq!(
            ctx.validate_forwarding(symbol_function(&ctx, symbol.get()).unwrap()),
            Err(ObjectError::ForwardingViolation)
        );

        assert_eq!(
            ncl_object::classify_object(&ctx, symbol.get()),
            ncl_object::ObjectRef::Symbol(symbol.get())
        );
        assert!(ncl_object::FunctionDesignator::try_from_word(&ctx, symbol.get()).is_ok());
        assert_eq!(
            runtime.call_builtin(&mut ctx, funcall, &[stale_function]),
            Err(ObjectError::ForwardingViolation)
        );
        assert_eq!(
            runtime.call_builtin(&mut ctx, apply, &[stale_function, Word::NIL]),
            Err(ObjectError::ForwardingViolation)
        );
        assert_eq!(
            runtime.call_builtin(&mut ctx, funcall, &[symbol.get()]),
            Err(ObjectError::ForwardingViolation)
        );
        assert_eq!(
            runtime.call_builtin(&mut ctx, apply, &[symbol.get(), Word::NIL]),
            Err(ObjectError::ForwardingViolation)
        );
    }

    #[test]
    fn funcall_and_apply_forward_arguments_and_multiple_values() {
        let (runtime, mut ctx, function) = setup();
        let funcall = runtime
            .function(&mut ctx, "COMMON-LISP", "FUNCALL")
            .unwrap();
        let apply = runtime.function(&mut ctx, "COMMON-LISP", "APPLY").unwrap();
        let funcall_function = ncl_object::FunctionObject::try_from(funcall).unwrap();
        let apply_function = ncl_object::FunctionObject::try_from(apply).unwrap();
        let tail = make_cons(&mut ctx, &runtime, Word::fixnum(3), Word::NIL).unwrap();
        let list = make_cons(&mut ctx, &runtime, Word::fixnum(2), tail).unwrap();
        assert_eq!(
            runtime.call_builtin(
                &mut ctx,
                ncl_object::FunctionObject::try_from(funcall).unwrap(),
                &[function.as_word(), Word::fixnum(1), Word::fixnum(2)]
            ),
            Ok(Word::fixnum(3))
        );
        assert_eq!(ctx.values(), &[Word::fixnum(3), Word::fixnum(4)]);
        assert_eq!(
            runtime.call_builtin(
                &mut ctx,
                ncl_object::FunctionObject::try_from(apply).unwrap(),
                &[function.as_word(), Word::fixnum(1), list]
            ),
            Ok(Word::fixnum(6))
        );
        assert_eq!(ctx.values(), &[Word::fixnum(6), Word::fixnum(7)]);

        for (name, callback, result, values) in [
            (
                "RETURNS-ZERO",
                returns_zero as ncl_object::RustBuiltin,
                Word::NIL,
                &[][..],
            ),
            (
                "RETURNS-ONE",
                returns_one as ncl_object::RustBuiltin,
                Word::fixnum(11),
                &[Word::fixnum(11)][..],
            ),
            (
                "RETURNS-TWO",
                returns_two as ncl_object::RustBuiltin,
                Word::fixnum(21),
                &[Word::fixnum(21), Word::fixnum(22)][..],
            ),
        ] {
            let result_function = register_result_builtin(&mut ctx, &runtime, name, callback);
            let result_function_word = result_function.as_word();
            assert_eq!(
                runtime.call_builtin(&mut ctx, funcall_function, &[result_function_word],),
                Ok(result)
            );
            assert_eq!(ctx.values(), values);
            assert_eq!(
                runtime.call_builtin(&mut ctx, apply_function, &[result_function_word, Word::NIL],),
                Ok(result)
            );
            assert_eq!(ctx.values(), values);
        }
    }

    #[test]
    fn funcall_and_apply_forward_five_arguments_with_rooted_values_under_gc_stress() {
        let runtime = Runtime::new().unwrap();
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime).unwrap();
        let function = runtime
            .register_builtin(
                &mut ctx,
                BuiltinIdentifier::new(BuiltinPackage::NclTest, BuiltinName::new("ADD-STRESS")),
                BuiltinImplementation::direct(
                    Builtin {
                        lambda_list: LambdaList::with_rest(&[], ARGUMENT),
                        convention: BuiltinConvention::Direct(Arity::exact(0)),
                    },
                    add,
                ),
            )
            .unwrap();
        register(&mut ctx, &runtime).unwrap();
        let mut function_word = function.as_word();
        let function_token = ncl_object::push_root(&mut ctx, &mut function_word);
        let mut list = Word::NIL;
        let list_token = ncl_object::push_root(&mut ctx, &mut list);
        for value in (1..=5).rev() {
            list = make_cons(&mut ctx, &runtime, Word::fixnum(value), list).unwrap();
        }
        let mut funcall_word = runtime
            .function(&mut ctx, "COMMON-LISP", "FUNCALL")
            .unwrap();
        let funcall_token = ncl_object::push_root(&mut ctx, &mut funcall_word);
        let mut apply_word = runtime.function(&mut ctx, "COMMON-LISP", "APPLY").unwrap();
        let apply_token = ncl_object::push_root(&mut ctx, &mut apply_word);
        let funcall = ncl_object::FunctionObject::try_from(funcall_word).unwrap();
        let apply = ncl_object::FunctionObject::try_from(apply_word).unwrap();
        let target = ncl_object::FunctionObject::try_from(function_word).unwrap();
        ctx.set_gc_stress(true);
        ctx.set_strict_forwarding(true);
        assert_eq!(
            runtime.call_builtin(
                &mut ctx,
                funcall,
                &[
                    target.as_word(),
                    Word::fixnum(1),
                    Word::fixnum(2),
                    Word::fixnum(3),
                    Word::fixnum(4),
                    Word::fixnum(5),
                ],
            ),
            Ok(Word::fixnum(15))
        );
        assert_eq!(ctx.values(), &[Word::fixnum(15), Word::fixnum(16)]);
        assert_eq!(
            runtime.call_builtin(&mut ctx, apply, &[target.as_word(), Word::fixnum(0), list],),
            Ok(Word::fixnum(15))
        );
        assert_eq!(ctx.values(), &[Word::fixnum(15), Word::fixnum(16)]);

        assert!(ncl_object::pop_root(&mut ctx, apply_token));
        assert!(ncl_object::pop_root(&mut ctx, funcall_token));
        assert!(ncl_object::pop_root(&mut ctx, list_token));
        assert!(ncl_object::pop_root(&mut ctx, function_token));
    }

    #[test]
    fn invalid_and_unbound_designators_report_their_error_kind() {
        let (runtime, mut ctx, _function) = setup();
        let funcall = ncl_object::FunctionObject::try_from(
            runtime
                .function(&mut ctx, "COMMON-LISP", "FUNCALL")
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            runtime.call_builtin(&mut ctx, funcall, &[Word::fixnum(0)]),
            Err(ObjectError::TypeError)
        );
        assert!(!ctx.thread_access().runtime_error_pending());
        let package = runtime.ensure_package(&mut ctx, "NCL-TEST").unwrap();
        let (symbol, _) = Package::from_word(package)
            .intern(&mut ctx, &runtime, "ADD")
            .unwrap();
        assert_eq!(
            runtime.call_builtin(&mut ctx, funcall, &[symbol, Word::fixnum(1)]),
            Ok(Word::fixnum(1))
        );
        assert_eq!(ctx.values(), &[Word::fixnum(1), Word::fixnum(2)]);
        let (symbol, _) = Package::from_word(package)
            .intern(&mut ctx, &runtime, "MISSING")
            .unwrap();
        assert_eq!(
            runtime.call_builtin(&mut ctx, funcall, &[symbol, Word::fixnum(1)]),
            Err(ObjectError::UndefinedFunction)
        );
    }

    #[test]
    fn apply_checks_arity_and_proper_list() {
        let (runtime, mut ctx, function) = setup();
        let apply = ncl_object::FunctionObject::try_from(
            runtime.function(&mut ctx, "COMMON-LISP", "APPLY").unwrap(),
        )
        .unwrap();
        assert_eq!(
            runtime.call_builtin(&mut ctx, apply, &[function.as_word()]),
            Err(ObjectError::TypeError)
        );
        assert_eq!(
            runtime.call_builtin(
                &mut ctx,
                apply,
                &[function.as_word(), Word::fixnum(1), Word::fixnum(2)]
            ),
            Err(ObjectError::TypeError)
        );
    }
}
