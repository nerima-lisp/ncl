// The lint allow must precede the architecture gate: on other targets the
// `#![cfg]` empties the crate, and a following allow would be stripped with it.
#![allow(
    missing_docs,
    clippy::borrow_as_ptr,
    clippy::cast_sign_loss,
    clippy::fn_to_numeric_cast,
    clippy::expect_used
)]
#![cfg(target_arch = "x86_64")]

use ncl_codegen::{ContextField, RuntimeAbi, RuntimeFunction, X86_64Abi, compile_function_x86_64};
use ncl_ir::{Compare, Constant, FunctionBuilder, OpKind, Param, Prim, Terminator, Ty};
use ncl_sys::{
    Thread, Word, alloc_code, enter_native, invoke_entry, invoke_entry_with_function, leave_native,
    publish_code, request_safepoint, set_tlab, thread_layout, tlab_bump, write_code,
};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};
use std::time::Instant;
static ALLOC_SLOW_CALLS: AtomicUsize = AtomicUsize::new(0);
static SAFEPOINT_SLOW_CALLS: AtomicUsize = AtomicUsize::new(0);
static COLLECT_IN_SAFEPOINT: AtomicBool = AtomicBool::new(false);
static FRAME_WORD_BEFORE: AtomicU64 = AtomicU64::new(0);
static FRAME_WORD_AFTER: AtomicU64 = AtomicU64::new(0);
static FRAME_LOCAL_BEFORE: AtomicU64 = AtomicU64::new(0);
static FRAME_LOCAL_AFTER: AtomicU64 = AtomicU64::new(0);
static SLOW_STORAGE: [u64; 8] = [0; 8];
static TEST_SERIAL: Mutex<()> = Mutex::new(());

const extern "C" fn builtin_add(_ctx: *mut Thread, left: u64, right: u64) -> u64 {
    left + right
}
extern "C" fn alloc_slow(_ctx: *mut Thread, words: u64) -> u64 {
    ALLOC_SLOW_CALLS.fetch_add(1, Ordering::SeqCst);
    assert_eq!(words, 2);
    SLOW_STORAGE.as_ptr() as u64
}
extern "C" fn safepoint_slow(ctx: &mut Thread, frame_fp: usize, return_pc: usize) {
    SAFEPOINT_SLOW_CALLS.fetch_add(1, Ordering::SeqCst);
    if COLLECT_IN_SAFEPOINT.swap(false, Ordering::SeqCst) {
        ctx.capture_native_frame(frame_fp, return_pc);
        FRAME_WORD_BEFORE.store(
            ctx.frame_word(2)
                .expect("captured frame function object")
                .bits(),
            Ordering::SeqCst,
        );
        FRAME_LOCAL_BEFORE.store(
            ctx.frame_word(5).expect("captured live local").bits(),
            Ordering::SeqCst,
        );
        ctx.clear_safepoint_request();
        ctx.enter_native();
        ncl_sys::collect(ctx, true);
        ctx.leave_native();
        ctx.clear_safepoint_request();
        let after = ctx
            .last_written_frame_word(2)
            .expect("written-back frame function object");
        FRAME_WORD_AFTER.store(after.bits(), Ordering::SeqCst);
        FRAME_LOCAL_AFTER.store(
            ctx.last_written_frame_word(5)
                .expect("written-back live local")
                .bits(),
            Ordering::SeqCst,
        );
        assert!(ctx.frame_word(2).is_none());
        println!(
            "frame word 2: before=0x{:x}, after=0x{:x}",
            FRAME_WORD_BEFORE.load(Ordering::SeqCst),
            FRAME_WORD_AFTER.load(Ordering::SeqCst)
        );
    }
}
struct BuiltinAbi;
impl RuntimeAbi for BuiltinAbi {
    fn builtin_address(&self, name: &str) -> Option<u64> {
        (name == "add").then_some(builtin_add as *const () as usize as u64)
    }

    fn context_offset(&self, _field: &str) -> Option<i32> {
        None
    }

    fn field_offset(&self, field: ContextField) -> Option<i32> {
        let layout = thread_layout();
        let offset = match field {
            ContextField::TlabBump => layout.tlab_bump,
            ContextField::TlabLimit => layout.tlab_limit,
            ContextField::SafepointRequest => layout.safepoint_request,
            ContextField::MultipleValueArea => layout.mv,
            _ => return None,
        };
        i32::try_from(offset).ok()
    }

    fn runtime_address(&self, function: RuntimeFunction, _name: Option<&str>) -> Option<u64> {
        match function {
            RuntimeFunction::AllocateSlow => Some(alloc_slow as *const () as usize as u64),
            RuntimeFunction::SafepointSlow => Some(safepoint_slow as *const () as usize as u64),
            _ => None,
        }
    }
}
#[test]
fn executes_fixnum_add_of_two_arguments() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(1),
        "add",
        vec![
            Param {
                name: "left".into(),
                ty: Ty::Word,
            },
            Param {
                name: "right".into(),
                ty: Ty::Word,
            },
        ],
        vec![Ty::Word],
    );
    let left = builder
        .push_op(OpKind::LoadArg { index: 0 }, &[Ty::Word])
        .expect("left argument")[0];
    let right = builder
        .push_op(OpKind::LoadArg { index: 1 }, &[Ty::Word])
        .expect("right argument")[0];
    let values = builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumAdd,
                args: vec![left, right],
                condition: None,
            },
            &[Ty::Word],
        )
        .expect("add operation");
    builder
        .terminate(Terminator::Return { values })
        .expect("return terminator");
    let abi = X86_64Abi;
    let compiled = compile_function_x86_64(&builder.finish(), &abi).expect("lowering");
    let mut code = alloc_code(compiled.code.len()).expect("code allocation");
    write_code(&mut code, 0, &compiled.code).expect("code write");
    publish_code(&mut code).expect("code publication");
    let mut thread = Thread::new();
    let (value, count) = invoke_entry(
        &code,
        compiled.entry_offset as usize,
        &mut thread,
        2,
        [Word::fixnum(1).bits(), Word::fixnum(2).bits(), 0, 0],
        0,
    );
    assert_eq!(value, Word::fixnum(3).bits());
    assert_eq!(count, 1);
}

#[test]
fn preserves_arguments_across_entry_safepoint() {
    let _guard = TEST_SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
    SAFEPOINT_SLOW_CALLS.store(0, Ordering::SeqCst);
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(27),
        "entry-safepoint-add",
        vec![
            Param {
                name: "left".into(),
                ty: Ty::Word,
            },
            Param {
                name: "right".into(),
                ty: Ty::Word,
            },
        ],
        vec![Ty::Word],
    );
    builder.push_op(OpKind::Safepoint, &[]).expect("safepoint");
    let left = builder
        .push_op(OpKind::LoadArg { index: 0 }, &[Ty::Word])
        .expect("left")[0];
    let right = builder
        .push_op(OpKind::LoadArg { index: 1 }, &[Ty::Word])
        .expect("right")[0];
    let sum = builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumAdd,
                args: vec![left, right],
                condition: None,
            },
            &[Ty::Word],
        )
        .expect("sum")[0];
    builder
        .terminate(Terminator::Return { values: vec![sum] })
        .expect("return");
    let compiled = compile_function_x86_64(&builder.finish(), &BuiltinAbi).expect("lowering");
    let mut code = alloc_code(compiled.code.len()).expect("code allocation");
    write_code(&mut code, 0, &compiled.code).expect("code write");
    publish_code(&mut code).expect("code publication");
    let mut thread = Thread::new();
    request_safepoint(&mut thread);
    let (value, count) = invoke_entry(
        &code,
        compiled.entry_offset as usize,
        &mut thread,
        2,
        [Word::fixnum(11).bits(), Word::fixnum(31).bits(), 0, 0],
        0,
    );
    assert_eq!(value, Word::fixnum(42).bits());
    assert_eq!(count, 1);
    assert_eq!(SAFEPOINT_SLOW_CALLS.load(Ordering::SeqCst), 1);
}

#[test]
fn executes_builtin_call_with_context_and_arguments() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(4),
        "builtin-add",
        vec![
            Param {
                name: "left".into(),
                ty: Ty::Word,
            },
            Param {
                name: "right".into(),
                ty: Ty::Word,
            },
        ],
        vec![Ty::Word],
    );
    let left = builder
        .push_op(OpKind::LoadArg { index: 0 }, &[Ty::Word])
        .expect("left")[0];
    let right = builder
        .push_op(OpKind::LoadArg { index: 1 }, &[Ty::Word])
        .expect("right")[0];
    let result = builder
        .push_op(
            OpKind::Builtin {
                name: "add".into(),
                args: vec![left, right],
            },
            &[Ty::Word],
        )
        .expect("builtin")[0];
    builder
        .terminate(Terminator::Return {
            values: vec![result],
        })
        .expect("return");
    let abi = BuiltinAbi;
    let compiled = compile_function_x86_64(&builder.finish(), &abi).expect("lowering");
    let mut code = alloc_code(compiled.code.len()).expect("code allocation");
    write_code(&mut code, 0, &compiled.code).expect("code write");
    publish_code(&mut code).expect("code publication");
    let mut thread = Thread::new();
    let (value, count) = invoke_entry(
        &code,
        compiled.entry_offset as usize,
        &mut thread,
        2,
        [Word::fixnum(4).bits(), Word::fixnum(5).bits(), 0, 0],
        0,
    );
    assert_eq!(value, Word::fixnum(9).bits());
    assert_eq!(count, 1);
}

#[test]
fn loads_fifth_argument_from_rest_storage() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(5),
        "rest-argument",
        vec![
            Param {
                name: "a0".into(),
                ty: Ty::Word,
            },
            Param {
                name: "a1".into(),
                ty: Ty::Word,
            },
            Param {
                name: "a2".into(),
                ty: Ty::Word,
            },
            Param {
                name: "a3".into(),
                ty: Ty::Word,
            },
            Param {
                name: "rest".into(),
                ty: Ty::Word,
            },
        ],
        vec![Ty::Word],
    );
    let value = builder
        .push_op(OpKind::LoadArg { index: 4 }, &[Ty::Word])
        .expect("rest argument")[0];
    builder
        .terminate(Terminator::Return {
            values: vec![value],
        })
        .expect("return");
    let abi = X86_64Abi;
    let compiled = compile_function_x86_64(&builder.finish(), &abi).expect("lowering");
    let mut code = alloc_code(compiled.code.len()).expect("code allocation");
    write_code(&mut code, 0, &compiled.code).expect("code write");
    publish_code(&mut code).expect("code publication");
    let rest = [Word::fixnum(7).bits()];
    let mut thread = Thread::new();
    let (value, count) = invoke_entry(
        &code,
        compiled.entry_offset as usize,
        &mut thread,
        5,
        [1, 2, 3, 4],
        rest.as_ptr() as u64,
    );
    assert_eq!(value, Word::fixnum(7).bits());
    assert_eq!(count, 1);
}

#[test]
fn executes_both_branch_paths_with_block_arguments() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(2),
        "branch",
        vec![Param {
            name: "value".into(),
            ty: Ty::Word,
        }],
        vec![Ty::Word],
    );
    let argument = builder
        .push_op(OpKind::LoadArg { index: 0 }, &[Ty::Word])
        .expect("argument")[0];
    let expected = builder.add_constant(Constant::Fixnum(1));
    let expected = builder
        .push_op(OpKind::Const { result: expected }, &[Ty::Word])
        .expect("constant")[0];
    let condition = builder
        .push_op(
            OpKind::Compare {
                op: Compare::Eq,
                left: argument,
                right: expected,
            },
            &[Ty::Word],
        )
        .expect("comparison")[0];
    let then_value = builder.fresh_value();
    let else_value = builder.fresh_value();
    let then_target = builder.create_block(vec![(Ty::Word, then_value)]);
    let else_target = builder.create_block(vec![(Ty::Word, else_value)]);
    builder
        .position_at(ncl_ir::BlockId(0))
        .expect("entry block");
    builder
        .terminate(Terminator::Branch {
            condition,
            then_target,
            then_args: vec![expected],
            else_target,
            else_args: vec![argument],
        })
        .expect("branch");
    builder.position_at(then_target).expect("then block");
    builder
        .terminate(Terminator::Return {
            values: vec![then_value],
        })
        .expect("then return");
    builder.position_at(else_target).expect("else block");
    builder
        .terminate(Terminator::Return {
            values: vec![else_value],
        })
        .expect("else return");

    let abi = X86_64Abi;
    let compiled = compile_function_x86_64(&builder.finish(), &abi).expect("lowering");
    let mut code = alloc_code(compiled.code.len()).expect("code allocation");
    write_code(&mut code, 0, &compiled.code).expect("code write");
    publish_code(&mut code).expect("code publication");
    let mut thread = Thread::new();
    let mut invoke = |value| {
        invoke_entry(
            &code,
            compiled.entry_offset as usize,
            &mut thread,
            1,
            [Word::fixnum(value).bits(), 0, 0, 0],
            0,
        )
    };
    assert_eq!(invoke(1), (Word::fixnum(1).bits(), 1));
    assert_eq!(invoke(2), (Word::fixnum(2).bits(), 1));
}

#[path = "exec_x86_64/cons.rs"]
mod cons;

#[path = "exec_x86_64/fib.rs"]
mod fib;

#[path = "exec_x86_64/basic.rs"]
mod basic;

#[path = "exec_x86_64/tail.rs"]
mod tail;
