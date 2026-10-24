use super::super::BuiltinAbi;
use ncl_codegen::{AbiError, ContextField, RuntimeAbi, RuntimeFunction, compile_function_aarch64};
use ncl_ir::{
    FixnumOp, FixnumSpecialization, Function, FunctionBuilder, FunctionId, OpKind, Param,
    ScalarTypeHint, Terminator, Ty,
};
use ncl_object::BuiltinIdentifier;
use ncl_sys::{NativeError, NativeOperation, Thread, Word};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

static FALLBACK_CALLS: AtomicUsize = AtomicUsize::new(0);
static FALLBACK_LEFT: AtomicU64 = AtomicU64::new(0);
static FALLBACK_RIGHT: AtomicU64 = AtomicU64::new(0);
static TEST_SERIAL: Mutex<()> = Mutex::new(());

extern "C" fn binary_fallback(ctx: &mut Thread, left: u64, right: u64) -> u64 {
    FALLBACK_CALLS.fetch_add(1, Ordering::SeqCst);
    FALLBACK_LEFT.store(left, Ordering::SeqCst);
    FALLBACK_RIGHT.store(right, Ordering::SeqCst);
    if Word::from_bits(left).as_fixnum().is_none() || Word::from_bits(right).as_fixnum().is_none() {
        ctx.set_native_error(NativeError::ThreadNotRegistered {
            operation: NativeOperation::Cons,
        });
    }
    Word::TRUE.bits()
}

extern "C" fn unary_fallback(ctx: &mut Thread, value: u64) -> u64 {
    FALLBACK_CALLS.fetch_add(1, Ordering::SeqCst);
    FALLBACK_LEFT.store(value, Ordering::SeqCst);
    FALLBACK_RIGHT.store(0, Ordering::SeqCst);
    if Word::from_bits(value).as_fixnum().is_none() {
        ctx.set_native_error(NativeError::ThreadNotRegistered {
            operation: NativeOperation::Cons,
        });
    }
    Word::TRUE.bits()
}

struct FixnumAbi;

impl RuntimeAbi for FixnumAbi {
    fn builtin_address(&self, identifier: BuiltinIdentifier) -> Result<u64, AbiError> {
        let address = match identifier.name.as_str() {
            "+" | "-" | "*" | "<" => binary_fallback as *const () as usize as u64,
            "1+" | "1-" => unary_fallback as *const () as usize as u64,
            _ => return BuiltinAbi.builtin_address(identifier),
        };
        Ok(address)
    }

    fn field_offset(&self, field: ContextField) -> Result<i32, AbiError> {
        BuiltinAbi.field_offset(field)
    }

    fn runtime_address(&self, function: RuntimeFunction) -> Result<u64, AbiError> {
        BuiltinAbi.runtime_address(function)
    }
}

fn specialized_function(
    id: u32,
    name: &str,
    operation: FixnumOp,
    include_specialization: bool,
    specialization_disabled: bool,
) -> Function {
    let params = (0..operation.arity())
        .map(|index| Param {
            name: format!("arg{index}"),
            ty: Ty::Word,
        })
        .collect();
    let mut builder = FunctionBuilder::new(FunctionId(id), name, params, vec![Ty::Word]);
    let args = (0..operation.arity())
        .map(|index| {
            builder
                .push_op(
                    OpKind::LoadArg {
                        index: u8::try_from(index).expect("argument index fits"),
                    },
                    &[Ty::Word],
                )
                .expect("load argument")[0]
        })
        .collect::<Vec<_>>();
    let specialization = if include_specialization {
        Some(
            FixnumSpecialization::new(operation, &vec![true; operation.arity()])
                .expect("guard count matches operation arity"),
        )
    } else {
        None
    };
    let result = builder
        .push_op(
            OpKind::Builtin {
                name: name.into(),
                args,
                specialization,
                specialization_disabled,
            },
            &[Ty::Word],
        )
        .expect("builtin result")[0];
    builder
        .terminate(Terminator::Return {
            values: vec![result],
        })
        .expect("return");
    builder.finish()
}

fn run(
    function: &Function,
    args: &[Word],
    thread: &mut Thread,
    expected_fallback_sites: usize,
) -> (u64, u64) {
    let compiled = compile_function_aarch64(function, &FixnumAbi).expect("compile function");
    assert_eq!(
        compiled
            .safepoint_maps
            .iter()
            .filter(|map| map.map_flags & ncl_codegen::FLAG_CALL != 0)
            .count(),
        expected_fallback_sites,
        "compiled fallback call sites"
    );
    let mut code = ncl_sys::alloc_code(compiled.code.len()).expect("allocate code");
    ncl_sys::write_code(&mut code, 0, &compiled.code).expect("write code");
    ncl_sys::publish_code(&mut code).expect("publish code");
    let values = [
        args.first().copied().unwrap_or(Word::ZERO).bits(),
        args.get(1).copied().unwrap_or(Word::ZERO).bits(),
        0,
        0,
    ];
    ncl_sys::invoke_entry(
        &code,
        compiled.entry_offset as usize,
        thread,
        u64::try_from(args.len()).expect("argument count fits"),
        values,
        0,
    )
}

fn reset_fallback() {
    FALLBACK_CALLS.store(0, Ordering::SeqCst);
    FALLBACK_LEFT.store(0, Ordering::SeqCst);
    FALLBACK_RIGHT.store(0, Ordering::SeqCst);
}

#[test]
fn checked_fixnum_fast_paths_return_tagged_results_without_calling_fallback() {
    let _guard = TEST_SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let cases = [
        (
            "+",
            FixnumOp::Add,
            vec![Word::fixnum(19), Word::fixnum(23)],
            Word::fixnum(42),
        ),
        (
            "-",
            FixnumOp::Sub,
            vec![Word::fixnum(19), Word::fixnum(23)],
            Word::fixnum(-4),
        ),
        (
            "*",
            FixnumOp::Mul,
            vec![Word::fixnum(-6), Word::fixnum(7)],
            Word::fixnum(-42),
        ),
        (
            "<",
            FixnumOp::LessThan,
            vec![Word::fixnum(-3), Word::fixnum(7)],
            Word::TRUE,
        ),
        (
            "<",
            FixnumOp::LessThan,
            vec![Word::fixnum(7), Word::fixnum(-3)],
            Word::NIL,
        ),
        (
            "1+",
            FixnumOp::OnePlus,
            vec![Word::fixnum(41)],
            Word::fixnum(42),
        ),
        (
            "1-",
            FixnumOp::OneMinus,
            vec![Word::fixnum(43)],
            Word::fixnum(42),
        ),
    ];

    for (index, (name, operation, args, expected)) in cases.into_iter().enumerate() {
        reset_fallback();
        let function = specialized_function(
            700 + u32::try_from(index).expect("case index fits"),
            name,
            operation,
            true,
            false,
        );
        let mut thread = Thread::new();
        let (actual, count) = run(&function, &args, &mut thread, 1);

        assert_eq!(actual, expected.bits(), "{name} fast result");
        assert_eq!(count, 1, "{name} returned one value");
        assert_eq!(
            FALLBACK_CALLS.load(Ordering::SeqCst),
            0,
            "{name} used fast path"
        );
        assert!(
            !thread.runtime_error_pending(),
            "{name} set a runtime error"
        );
    }
}

#[test]
fn checked_fixnum_tag_and_overflow_failures_call_fallback_with_original_words() {
    let _guard = TEST_SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let function = specialized_function(710, "+", FixnumOp::Add, true, false);
    let left = Word::fixnum(7);
    let right = Word::character(65);
    reset_fallback();
    let mut thread = Thread::new();
    let (actual, count) = run(&function, &[left, right], &mut thread, 1);

    assert_eq!(FALLBACK_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(FALLBACK_LEFT.load(Ordering::SeqCst), left.bits());
    assert_eq!(FALLBACK_RIGHT.load(Ordering::SeqCst), right.bits());
    assert_eq!(actual, Word::ZERO.bits());
    assert_eq!(count, 0);
    assert!(thread.runtime_error_pending());

    let max_fixnum = (1_i64 << 62) - 1;
    let left = Word::fixnum(max_fixnum);
    let right = Word::fixnum(1);
    reset_fallback();
    let mut thread = Thread::new();
    let (actual, count) = run(&function, &[left, right], &mut thread, 1);

    assert_eq!(FALLBACK_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(FALLBACK_LEFT.load(Ordering::SeqCst), left.bits());
    assert_eq!(FALLBACK_RIGHT.load(Ordering::SeqCst), right.bits());
    assert_eq!(actual, Word::TRUE.bits());
    assert_eq!(count, 1);
    assert!(!thread.runtime_error_pending());
}

#[test]
fn checked_fixnum_multiply_falls_back_on_machine_and_fixnum_overflow() {
    let _guard = TEST_SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let function = specialized_function(711, "*", FixnumOp::Mul, true, false);
    for (left, right) in [
        (Word::fixnum(1_i64 << 61), Word::fixnum(4)),
        (Word::fixnum((1_i64 << 62) - 1), Word::fixnum(2)),
    ] {
        reset_fallback();
        let mut thread = Thread::new();
        let (actual, count) = run(&function, &[left, right], &mut thread, 1);

        assert_eq!(FALLBACK_CALLS.load(Ordering::SeqCst), 1);
        assert_eq!(FALLBACK_LEFT.load(Ordering::SeqCst), left.bits());
        assert_eq!(FALLBACK_RIGHT.load(Ordering::SeqCst), right.bits());
        assert_eq!(actual, Word::TRUE.bits());
        assert_eq!(count, 1);
        assert!(!thread.runtime_error_pending());
    }
}

#[test]
fn checked_fixnum_subtract_falls_back_on_fixnum_underflow() {
    let _guard = TEST_SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let function = specialized_function(714, "-", FixnumOp::Sub, true, false);
    let left = Word::fixnum(i64::MIN >> ncl_sys::FIXNUM_TAG_BITS);
    let right = Word::fixnum(1);
    reset_fallback();
    let mut thread = Thread::new();
    let (actual, count) = run(&function, &[left, right], &mut thread, 1);

    assert_eq!(FALLBACK_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(FALLBACK_LEFT.load(Ordering::SeqCst), left.bits());
    assert_eq!(FALLBACK_RIGHT.load(Ordering::SeqCst), right.bits());
    assert_eq!(actual, Word::TRUE.bits());
    assert_eq!(count, 1);
    assert!(!thread.runtime_error_pending());
}

#[test]
fn checked_fixnum_unary_overflow_falls_back_with_original_word() {
    let _guard = TEST_SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let cases = [
        ("1+", FixnumOp::OnePlus, Word::fixnum((1_i64 << 62) - 1)),
        (
            "1-",
            FixnumOp::OneMinus,
            Word::fixnum(i64::MIN >> ncl_sys::FIXNUM_TAG_BITS),
        ),
    ];

    for (index, (name, operation, value)) in cases.into_iter().enumerate() {
        let function = specialized_function(
            715 + u32::try_from(index).expect("case index fits"),
            name,
            operation,
            true,
            false,
        );
        reset_fallback();
        let mut thread = Thread::new();
        let (actual, count) = run(&function, &[value], &mut thread, 1);

        assert_eq!(FALLBACK_CALLS.load(Ordering::SeqCst), 1, "{name} fallback");
        assert_eq!(
            FALLBACK_LEFT.load(Ordering::SeqCst),
            value.bits(),
            "{name} operand"
        );
        assert_eq!(
            FALLBACK_RIGHT.load(Ordering::SeqCst),
            0,
            "{name} unused operand"
        );
        assert_eq!(actual, Word::TRUE.bits(), "{name} fallback result");
        assert_eq!(count, 1, "{name} returned one value");
        assert!(
            !thread.runtime_error_pending(),
            "{name} set a runtime error"
        );
    }
}

#[test]
fn disabled_specialization_uses_generic_builtin_call() {
    let _guard = TEST_SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let function = specialized_function(712, "+", FixnumOp::Add, true, true);
    let left = Word::fixnum(2);
    let right = Word::fixnum(3);
    reset_fallback();
    let mut thread = Thread::new();
    let (actual, count) = run(&function, &[left, right], &mut thread, 1);

    assert_eq!(FALLBACK_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(FALLBACK_LEFT.load(Ordering::SeqCst), left.bits());
    assert_eq!(FALLBACK_RIGHT.load(Ordering::SeqCst), right.bits());
    assert_eq!(actual, Word::TRUE.bits());
    assert_eq!(count, 1);
    assert!(!thread.runtime_error_pending());
}

#[test]
fn type_hint_preserves_its_word_value_on_aarch64() {
    let mut builder = FunctionBuilder::new(
        FunctionId(713),
        "type-hint-copy",
        vec![Param {
            name: "value".into(),
            ty: Ty::Word,
        }],
        vec![Ty::Word],
    );
    let value = builder
        .push_op(OpKind::LoadArg { index: 0 }, &[Ty::Word])
        .expect("load argument")[0];
    let hinted = builder
        .push_op(
            OpKind::TypeHint {
                value,
                ty: ScalarTypeHint::Fixnum,
            },
            &[Ty::Word],
        )
        .expect("type hint")[0];
    builder
        .terminate(Terminator::Return {
            values: vec![hinted],
        })
        .expect("return");
    let function = builder.finish();
    let mut thread = Thread::new();
    let (actual, count) = run(&function, &[Word::fixnum(-17)], &mut thread, 0);

    assert_eq!(actual, Word::fixnum(-17).bits());
    assert_eq!(count, 1);
}
