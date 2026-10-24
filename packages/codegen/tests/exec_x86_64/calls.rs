use super::*;

fn build_weighted_target() -> ncl_ir::Function {
    let mut target_builder = FunctionBuilder::new(
        ncl_ir::FunctionId(100),
        "weighted-five-argument-callee",
        ["first", "second", "third", "fourth", "fifth"]
            .into_iter()
            .map(|name| Param {
                name: name.into(),
                ty: Ty::Word,
            })
            .collect(),
        vec![Ty::Word],
    );
    let arguments = (0..5)
        .map(|index| {
            target_builder
                .push_op(OpKind::LoadArg { index }, &[Ty::Word])
                .expect("target argument")[0]
        })
        .collect::<Vec<_>>();
    let mut result = word_to_i64(&mut target_builder, arguments[0]);
    for (index, argument) in arguments.iter().copied().enumerate().skip(1) {
        let mut weighted = word_to_i64(&mut target_builder, argument);
        for _ in 0..index {
            weighted = target_builder
                .push_op(
                    OpKind::Prim {
                        op: Prim::FixnumAdd,
                        args: vec![weighted, weighted],
                        condition: None,
                    },
                    &[Ty::I64],
                )
                .expect("double weighted argument")[0];
        }
        result = target_builder
            .push_op(
                OpKind::Prim {
                    op: Prim::FixnumAdd,
                    args: vec![result, weighted],
                    condition: None,
                },
                &[Ty::I64],
            )
            .expect("add weighted argument")[0];
    }
    let result = i64_to_word(&mut target_builder, result);
    target_builder
        .terminate(Terminator::Return {
            values: vec![result],
        })
        .expect("target return");
    target_builder.finish()
}

fn build_five_argument_entry() -> ncl_ir::Function {
    let mut entry_builder = FunctionBuilder::new(
        ncl_ir::FunctionId(101),
        "five-argument-caller",
        vec![
            Param {
                name: "callee".into(),
                ty: Ty::Word,
            },
            Param {
                name: "first".into(),
                ty: Ty::Word,
            },
            Param {
                name: "second".into(),
                ty: Ty::Word,
            },
            Param {
                name: "third".into(),
                ty: Ty::Word,
            },
            Param {
                name: "fourth".into(),
                ty: Ty::Word,
            },
            Param {
                name: "fifth".into(),
                ty: Ty::Word,
            },
        ],
        vec![Ty::Word],
    );
    let callee_value = entry_builder
        .push_op(OpKind::LoadArg { index: 0 }, &[Ty::Word])
        .expect("target address")[0];
    let arguments = (1..=5)
        .map(|index| {
            entry_builder
                .push_op(OpKind::LoadArg { index }, &[Ty::Word])
                .expect("entry argument")[0]
        })
        .collect::<Vec<_>>();
    let argc = fixnum_word(&mut entry_builder, 5);
    entry_builder
        .push_op(OpKind::Safepoint, &[])
        .expect("call safepoint");
    let mut call_args = vec![argc];
    call_args.extend(arguments);
    let result = entry_builder
        .push_op(
            OpKind::Call {
                function: callee_value,
                args: call_args,
            },
            &[Ty::Word],
        )
        .expect("five-argument call")[0];
    entry_builder
        .terminate(Terminator::Return {
            values: vec![result],
        })
        .expect("entry return");
    entry_builder.finish()
}

#[test]
fn executes_generated_call_with_fifth_argument_from_rest_area() {
    let target =
        compile_function_x86_64(&build_weighted_target(), &X86_64Abi).expect("target lowering");
    let mut target_code = alloc_code(target.code.len()).expect("target code allocation");
    write_code(&mut target_code, 0, &target.code).expect("target code write");
    publish_code(&mut target_code).expect("target code publication");

    let entry =
        compile_function_x86_64(&build_five_argument_entry(), &BuiltinAbi).expect("entry lowering");
    let mut entry_code = alloc_code(entry.code.len()).expect("entry code allocation");
    write_code(&mut entry_code, 0, &entry.code).expect("entry code write");
    publish_code(&mut entry_code).expect("entry code publication");

    let values = [1, 2, 3, 4, 5].map(|value| Word::fixnum(value).bits());
    let rest = [values[3], values[4]];
    let mut thread = Thread::new();
    let result = invoke_entry(
        &entry_code,
        entry.entry_offset as usize,
        &mut thread,
        6,
        [
            target_code.address() as u64,
            values[0],
            values[1],
            values[2],
        ],
        rest.as_ptr() as u64,
    );
    assert_eq!(result, (Word::fixnum(129).bits(), 1));
}

#[test]
fn rejects_builtin_missing_from_execution_fixture_abi() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(102),
        "missing-builtin",
        Vec::new(),
        vec![Ty::Word],
    );
    builder
        .push_op(OpKind::Safepoint, &[])
        .expect("builtin safepoint");
    let value = builder
        .push_op(
            OpKind::Builtin {
                name: "unregistered".into(),
                args: Vec::new(),
                specialization: None,
                specialization_disabled: false,
            },
            &[Ty::Word],
        )
        .expect("builtin operation")[0];
    builder
        .terminate(Terminator::Return {
            values: vec![value],
        })
        .expect("return");

    assert!(matches!(
        compile_function_x86_64(&builder.finish(), &BuiltinAbi),
        Err(ncl_codegen::CodegenError::Unsupported(message))
            if message.contains("builtin address is unavailable")
    ));
}

static CHECKED_FIXNUM_FALLBACK_CALLS: AtomicUsize = AtomicUsize::new(0);
static CHECKED_FIXNUM_FALLBACK_ARGS: [AtomicU64; 2] = [AtomicU64::new(0), AtomicU64::new(0)];
static CHECKED_FIXNUM_TEST_SERIAL: Mutex<()> = Mutex::new(());

extern "C" fn checked_fixnum_binary_fallback(_ctx: *mut Thread, left: u64, right: u64) -> u64 {
    CHECKED_FIXNUM_FALLBACK_ARGS[0].store(left, Ordering::SeqCst);
    CHECKED_FIXNUM_FALLBACK_ARGS[1].store(right, Ordering::SeqCst);
    CHECKED_FIXNUM_FALLBACK_CALLS.fetch_add(1, Ordering::SeqCst);
    Word::TRUE.bits()
}

extern "C" fn checked_fixnum_unary_fallback(_ctx: *mut Thread, value: u64) -> u64 {
    CHECKED_FIXNUM_FALLBACK_ARGS[0].store(value, Ordering::SeqCst);
    CHECKED_FIXNUM_FALLBACK_ARGS[1].store(0, Ordering::SeqCst);
    CHECKED_FIXNUM_FALLBACK_CALLS.fetch_add(1, Ordering::SeqCst);
    Word::TRUE.bits()
}

struct CheckedFixnumAbi;

impl RuntimeAbi for CheckedFixnumAbi {
    fn builtin_address(
        &self,
        identifier: ncl_object::BuiltinIdentifier,
    ) -> Result<u64, ncl_codegen::AbiError> {
        let address = match identifier.name.as_str() {
            "+" | "-" | "*" | "<" => checked_fixnum_binary_fallback as *const () as usize as u64,
            "1+" | "1-" => checked_fixnum_unary_fallback as *const () as usize as u64,
            _ => return Err(ncl_codegen::AbiError::MissingBuiltin(identifier)),
        };
        Ok(address)
    }

    fn field_offset(&self, field: ContextField) -> Result<i32, ncl_codegen::AbiError> {
        BuiltinAbi.field_offset(field)
    }

    fn runtime_address(&self, function: RuntimeFunction) -> Result<u64, ncl_codegen::AbiError> {
        BuiltinAbi.runtime_address(function)
    }
}

fn build_checked_fixnum_builtin(
    id: u32,
    name: &str,
    operation: ncl_ir::FixnumOp,
    specialization_disabled: bool,
) -> ncl_ir::Function {
    let arity = operation.arity();
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(id),
        "checked-fixnum-builtin",
        (0..arity)
            .map(|index| Param {
                name: format!("arg-{index}"),
                ty: Ty::Word,
            })
            .collect(),
        vec![Ty::Word],
    );
    let args = (0..arity)
        .map(|index| {
            builder
                .push_op(
                    OpKind::LoadArg {
                        index: u8::try_from(index).expect("fixture argument index"),
                    },
                    &[Ty::Word],
                )
                .expect("builtin argument")[0]
        })
        .collect::<Vec<_>>();
    builder
        .push_op(OpKind::Safepoint, &[])
        .expect("builtin safepoint");
    let specialization = ncl_ir::FixnumSpecialization::new(operation, &vec![true; arity])
        .expect("matching fixnum guards");
    let result = builder
        .push_op(
            OpKind::Builtin {
                name: name.into(),
                args,
                specialization: Some(specialization),
                specialization_disabled,
            },
            &[Ty::Word],
        )
        .expect("specialized builtin")[0];
    builder
        .terminate(Terminator::Return {
            values: vec![result],
        })
        .expect("return terminator");
    builder.finish()
}

fn execute_checked_fixnum_builtin(
    id: u32,
    name: &str,
    operation: ncl_ir::FixnumOp,
    specialization_disabled: bool,
    args: &[u64],
) -> Word {
    let compiled = compile_function_x86_64(
        &build_checked_fixnum_builtin(id, name, operation, specialization_disabled),
        &CheckedFixnumAbi,
    )
    .expect("fixnum builtin lowering");
    assert_eq!(
        compiled
            .safepoint_maps
            .iter()
            .filter(|map| map.map_flags & ncl_codegen::FLAG_CALL != 0)
            .count(),
        2,
        "explicit poll plus one generic fallback call site"
    );
    let mut code = alloc_code(compiled.code.len()).expect("code allocation");
    write_code(&mut code, 0, &compiled.code).expect("code write");
    publish_code(&mut code).expect("code publication");
    let mut thread = Thread::new();
    let arguments = core::array::from_fn(|index| args.get(index).copied().unwrap_or(0));
    let (value, count) = invoke_entry(
        &code,
        compiled.entry_offset as usize,
        &mut thread,
        u64::try_from(args.len()).expect("fixture argument count"),
        arguments,
        0,
    );
    assert_eq!(count, 1);
    Word::from_bits(value)
}

fn reset_checked_fixnum_fallback() {
    CHECKED_FIXNUM_FALLBACK_CALLS.store(0, Ordering::SeqCst);
    CHECKED_FIXNUM_FALLBACK_ARGS[0].store(0, Ordering::SeqCst);
    CHECKED_FIXNUM_FALLBACK_ARGS[1].store(0, Ordering::SeqCst);
}

fn assert_checked_fixnum_fallback(id: u32, name: &str, operation: ncl_ir::FixnumOp, args: &[u64]) {
    reset_checked_fixnum_fallback();
    assert_eq!(
        execute_checked_fixnum_builtin(id, name, operation, false, args),
        Word::TRUE,
        "{name} fallback result"
    );
    assert_eq!(CHECKED_FIXNUM_FALLBACK_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(
        CHECKED_FIXNUM_FALLBACK_ARGS[0].load(Ordering::SeqCst),
        args[0]
    );
    assert_eq!(
        CHECKED_FIXNUM_FALLBACK_ARGS[1].load(Ordering::SeqCst),
        args.get(1).copied().unwrap_or(0)
    );
}

#[test]
fn checked_fixnum_builtins_use_fast_paths() {
    use ncl_ir::FixnumOp;

    let _guard = CHECKED_FIXNUM_TEST_SERIAL
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let fast_cases = [
        ("+", FixnumOp::Add, vec![12, 30], Word::fixnum(42)),
        ("-", FixnumOp::Sub, vec![60, 18], Word::fixnum(42)),
        ("*", FixnumOp::Mul, vec![6, 7], Word::fixnum(42)),
        ("<", FixnumOp::LessThan, vec![1, 2], Word::TRUE),
        ("<", FixnumOp::LessThan, vec![2, 1], Word::NIL),
        ("1+", FixnumOp::OnePlus, vec![41], Word::fixnum(42)),
        ("1-", FixnumOp::OneMinus, vec![43], Word::fixnum(42)),
    ];
    for (index, (name, operation, values, expected)) in fast_cases.into_iter().enumerate() {
        reset_checked_fixnum_fallback();
        let args = values
            .into_iter()
            .map(|value| Word::fixnum(value).bits())
            .collect::<Vec<_>>();
        assert_eq!(
            execute_checked_fixnum_builtin(
                1_000 + u32::try_from(index).expect("fast case index"),
                name,
                operation,
                false,
                &args,
            ),
            expected,
            "{name} fast path"
        );
        assert_eq!(
            CHECKED_FIXNUM_FALLBACK_CALLS.load(Ordering::SeqCst),
            0,
            "{name} fast path skipped generic fallback"
        );
    }
}

#[test]
fn checked_fixnum_overflow_uses_generic_fallback_with_original_args() {
    use ncl_ir::FixnumOp;

    let _guard = CHECKED_FIXNUM_TEST_SERIAL
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let max_fixnum = i64::MAX >> ncl_sys::FIXNUM_TAG_BITS;
    let min_fixnum = i64::MIN >> ncl_sys::FIXNUM_TAG_BITS;
    let slow_cases = [
        (
            "+",
            FixnumOp::Add,
            vec![Word::fixnum(max_fixnum).bits(), Word::fixnum(1).bits()],
        ),
        (
            "-",
            FixnumOp::Sub,
            vec![Word::fixnum(min_fixnum).bits(), Word::fixnum(1).bits()],
        ),
        (
            "*",
            FixnumOp::Mul,
            vec![Word::fixnum(max_fixnum).bits(), Word::fixnum(2).bits()],
        ),
        (
            "1+",
            FixnumOp::OnePlus,
            vec![Word::fixnum(max_fixnum).bits()],
        ),
        (
            "1-",
            FixnumOp::OneMinus,
            vec![Word::fixnum(min_fixnum).bits()],
        ),
    ];
    for (index, (name, operation, args)) in slow_cases.into_iter().enumerate() {
        assert_checked_fixnum_fallback(
            1_100 + u32::try_from(index).expect("slow case index"),
            name,
            operation,
            &args,
        );
    }
}

#[test]
fn checked_fixnum_type_guard_and_disabled_policy_use_generic_fallback() {
    use ncl_ir::FixnumOp;

    let _guard = CHECKED_FIXNUM_TEST_SERIAL
        .lock()
        .unwrap_or_else(PoisonError::into_inner);

    let tag_failure = [Word::NIL.bits(), Word::fixnum(2).bits()];
    assert_checked_fixnum_fallback(1_200, "+", FixnumOp::Add, &tag_failure);

    let disabled_args = [Word::fixnum(20).bits(), Word::fixnum(22).bits()];
    reset_checked_fixnum_fallback();
    assert_eq!(
        execute_checked_fixnum_builtin(1_201, "+", FixnumOp::Add, true, &disabled_args),
        Word::TRUE
    );
    assert_eq!(CHECKED_FIXNUM_FALLBACK_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(
        CHECKED_FIXNUM_FALLBACK_ARGS[0].load(Ordering::SeqCst),
        disabled_args[0]
    );
    assert_eq!(
        CHECKED_FIXNUM_FALLBACK_ARGS[1].load(Ordering::SeqCst),
        disabled_args[1]
    );
}

#[test]
fn x86_64_type_hint_preserves_the_word_value() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(1_100),
        "type-hint-identity",
        vec![Param {
            name: "value".into(),
            ty: Ty::Word,
        }],
        vec![Ty::Word],
    );
    let value = builder
        .push_op(OpKind::LoadArg { index: 0 }, &[Ty::Word])
        .expect("argument")[0];
    let hinted = builder
        .push_op(
            OpKind::TypeHint {
                value,
                ty: ncl_ir::ScalarTypeHint::Fixnum,
            },
            &[Ty::Word],
        )
        .expect("type hint")[0];
    builder
        .terminate(Terminator::Return {
            values: vec![hinted],
        })
        .expect("return terminator");
    let compiled = compile_function_x86_64(&builder.finish(), &BuiltinAbi).expect("lowering");
    let mut code = alloc_code(compiled.code.len()).expect("code allocation");
    write_code(&mut code, 0, &compiled.code).expect("code write");
    publish_code(&mut code).expect("code publication");
    let mut thread = Thread::new();
    let value = Word::fixnum(-37);
    let (result, count) = invoke_entry(
        &code,
        compiled.entry_offset as usize,
        &mut thread,
        1,
        [value.bits(), 0, 0, 0],
        0,
    );
    assert_eq!((result, count), (value.bits(), 1));
}
