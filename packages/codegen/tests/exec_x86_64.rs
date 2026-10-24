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
use ncl_ir::{
    Compare, Constant, Convert, FunctionBuilder, OpKind, Param, Prim, Terminator, Ty, ValueId,
};
use ncl_sys::{
    NativeError, NativeOperation, Thread, Word, alloc_code, enter_native, invoke_entry,
    invoke_entry_with_function, leave_native, publish_code, request_safepoint, set_tlab,
    thread_layout, tlab_bump, write_code,
};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};
use std::time::Instant;
static ALLOC_SLOW_CALLS: AtomicUsize = AtomicUsize::new(0);
static SAFEPOINT_SLOW_CALLS: AtomicUsize = AtomicUsize::new(0);
static AFTER_ERROR_BUILTIN_CALLS: AtomicUsize = AtomicUsize::new(0);
static COLLECT_IN_SAFEPOINT: AtomicBool = AtomicBool::new(false);
static FRAME_WORD_BEFORE: AtomicU64 = AtomicU64::new(0);
static FRAME_WORD_AFTER: AtomicU64 = AtomicU64::new(0);
static SLOW_STORAGE: [u64; 8] = [0; 8];
static TEST_SERIAL: Mutex<()> = Mutex::new(());

fn fixnum_i64(builder: &mut FunctionBuilder, value: i64) -> ValueId {
    let constant = builder.add_constant(Constant::Fixnum(value));
    builder
        .push_op(OpKind::Const { result: constant }, &[Ty::I64])
        .expect("fixnum constant")[0]
}

fn word_to_i64(builder: &mut FunctionBuilder, value: ValueId) -> ValueId {
    builder
        .push_op(
            OpKind::Convert {
                op: Convert::WordToI64,
                value,
            },
            &[Ty::I64],
        )
        .expect("word to fixnum")[0]
}

fn i64_to_word(builder: &mut FunctionBuilder, value: ValueId) -> ValueId {
    builder
        .push_op(
            OpKind::Convert {
                op: Convert::I64ToWord,
                value,
            },
            &[Ty::Word],
        )
        .expect("fixnum to word")[0]
}

fn fixnum_word(builder: &mut FunctionBuilder, value: i64) -> ValueId {
    let value = fixnum_i64(builder, value);
    i64_to_word(builder, value)
}

const fn set_runtime_error_for_codegen_test(ctx: &mut Thread) {
    ctx.set_native_error(NativeError::ThreadNotRegistered {
        operation: NativeOperation::Cons,
    });
}

const extern "C" fn builtin_add(_ctx: *mut Thread, left: u64, right: u64) -> u64 {
    left + right
}
extern "C" fn write_barrier(thread: NonNull<Thread>, slot: usize, owner: Word) {
    ncl_sys::write_barrier(thread, owner, slot);
}
extern "C" fn make_closure_error(ctx: &mut Thread, capture_count: u64, entry: u64) -> u64 {
    assert_eq!(capture_count, Word::fixnum(1).bits());
    assert_eq!(entry, Word::fixnum(9).bits());
    set_runtime_error_for_codegen_test(ctx);
    Word::NIL.bits()
}
extern "C" fn make_value_cell_error(ctx: &mut Thread, value: u64) -> u64 {
    assert_eq!(value, Word::fixnum(37).bits());
    set_runtime_error_for_codegen_test(ctx);
    Word::NIL.bits()
}
extern "C" fn error_builtin(ctx: &mut Thread, value: u64) -> u64 {
    assert_eq!(value, Word::fixnum(23).bits());
    set_runtime_error_for_codegen_test(ctx);
    Word::NIL.bits()
}
extern "C" fn observe_after_error_builtin(_ctx: &mut Thread, value: u64) -> u64 {
    AFTER_ERROR_BUILTIN_CALLS.fetch_add(1, Ordering::SeqCst);
    value
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
        ctx.clear_safepoint_request();
        ctx.enter_native();
        assert_eq!(ncl_sys::collect(ctx, true), Ok(()));
        ctx.leave_native();
        ctx.clear_safepoint_request();
        let after = ctx
            .last_written_frame_word(2)
            .expect("written-back frame function object");
        FRAME_WORD_AFTER.store(after.bits(), Ordering::SeqCst);
        assert!(ctx.frame_word(2).is_none());
    }
}
struct BuiltinAbi;
impl RuntimeAbi for BuiltinAbi {
    fn builtin_address(
        &self,
        identifier: ncl_object::BuiltinIdentifier,
    ) -> Result<u64, ncl_codegen::AbiError> {
        match identifier.name.as_str() {
            "add" => Ok(builtin_add as *const () as usize as u64),
            "error" => Ok(error_builtin as *const () as usize as u64),
            "observe-after-error" => Ok(observe_after_error_builtin as *const () as usize as u64),
            _ => Err(ncl_codegen::AbiError::MissingBuiltin(identifier)),
        }
    }

    fn field_offset(&self, field: ContextField) -> Result<i32, ncl_codegen::AbiError> {
        let layout = thread_layout();
        let offset = match field {
            ContextField::TlabBump => layout.tlab_bump,
            ContextField::TlabLimit => layout.tlab_limit,
            ContextField::SafepointRequest => layout.safepoint_request,
            ContextField::MultipleValueArea => layout.mv,
            ContextField::Pending => layout.pending,
            ContextField::MultipleValueCount => layout.mv_count,
            ContextField::Handler => layout.handler,
            ContextField::Cleanup => layout.cleanup,
            ContextField::Catch => layout.catch,
            ContextField::RuntimeErrorFlags => layout.runtime_error_flags,
        };
        i32::try_from(offset).map_err(|_| ncl_codegen::AbiError::UnsupportedContextField(field))
    }

    fn runtime_address(&self, function: RuntimeFunction) -> Result<u64, ncl_codegen::AbiError> {
        match function {
            RuntimeFunction::WriteBarrier => Ok(write_barrier as *const () as usize as u64),
            RuntimeFunction::MakeClosure => Ok(make_closure_error as *const () as usize as u64),
            RuntimeFunction::MakeValueCell => {
                Ok(make_value_cell_error as *const () as usize as u64)
            }
            RuntimeFunction::AllocateSlow => Ok(alloc_slow as *const () as usize as u64),
            RuntimeFunction::SafepointSlow => Ok(safepoint_slow as *const () as usize as u64),
            _ => Err(ncl_codegen::AbiError::UnsupportedRuntimeFunction(function)),
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
    let left = word_to_i64(&mut builder, left);
    let right = builder
        .push_op(OpKind::LoadArg { index: 1 }, &[Ty::Word])
        .expect("right argument")[0];
    let right = word_to_i64(&mut builder, right);
    let result = builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumAdd,
                args: vec![left, right],
                condition: None,
            },
            &[Ty::I64],
        )
        .expect("add operation")[0];
    let result = i64_to_word(&mut builder, result);
    builder
        .terminate(Terminator::Return {
            values: vec![result],
        })
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
    let left = word_to_i64(&mut builder, left);
    let right = builder
        .push_op(OpKind::LoadArg { index: 1 }, &[Ty::Word])
        .expect("right")[0];
    let right = word_to_i64(&mut builder, right);
    let sum = builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumAdd,
                args: vec![left, right],
                condition: None,
            },
            &[Ty::I64],
        )
        .expect("sum")[0];
    let sum = i64_to_word(&mut builder, sum);
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
    let expected = fixnum_word(&mut builder, 1);
    let condition = builder
        .push_op(
            OpKind::Compare {
                op: Compare::Eq,
                left: argument,
                right: expected,
            },
            &[Ty::Bool],
        )
        .expect("comparison")[0];
    let then_payload = fixnum_word(&mut builder, 101);
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
            then_args: vec![then_payload],
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
    assert_eq!(invoke(1), (Word::fixnum(101).bits(), 1));
    assert_eq!(invoke(2), (Word::fixnum(2).bits(), 1));
}

#[path = "exec_x86_64/cons.rs"]
mod cons;

#[path = "exec_x86_64/fib.rs"]
mod fib;

#[path = "exec_x86_64/basic.rs"]
mod basic;

#[path = "exec_x86_64/calls.rs"]
mod calls;

#[path = "exec_x86_64/tail.rs"]
mod tail;

#[path = "exec_x86_64/checked_calls.rs"]
mod checked_calls;

#[path = "exec_x86_64/errors.rs"]
mod errors;

#[path = "exec_x86_64/relocations.rs"]
mod relocations;
