#![cfg(target_arch = "aarch64")]
#![allow(
    missing_docs,
    clippy::borrow_as_ptr,
    clippy::cast_sign_loss,
    clippy::fn_to_numeric_cast,
    clippy::expect_used
)]

use ncl_codegen::{
    Aarch64Abi, ContextField, RuntimeAbi, RuntimeFunction, compile_function_aarch64,
};
use ncl_ir::{Compare, Constant, FunctionBuilder, OpKind, Param, Prim, Terminator, Ty};
use ncl_sys::{
    Thread, alloc_code, invoke_entry, publish_code, request_safepoint, set_tlab, thread_layout,
    tlab_bump, write_code,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

static ALLOC_SLOW_CALLS: AtomicUsize = AtomicUsize::new(0);
static SAFEPOINT_SLOW_CALLS: AtomicUsize = AtomicUsize::new(0);
static SLOW_STORAGE: [u64; 8] = [0; 8];

const extern "C" fn builtin_add(_ctx: *mut Thread, left: u64, right: u64) -> u64 {
    left + right
}

extern "C" fn alloc_slow(_ctx: *mut Thread, words: u64) -> u64 {
    ALLOC_SLOW_CALLS.fetch_add(1, Ordering::SeqCst);
    assert_eq!(words, 2);
    SLOW_STORAGE.as_ptr() as u64
}

extern "C" fn safepoint_slow(_ctx: *mut Thread) {
    SAFEPOINT_SLOW_CALLS.fetch_add(1, Ordering::SeqCst);
}

struct BuiltinAbi;

impl RuntimeAbi for BuiltinAbi {
    fn encode_fixnum(&self, value: i64) -> i64 {
        value << 3
    }

    fn encode_character(&self, value: u32) -> i64 {
        i64::from(value) << 8 | 0x0f
    }

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
            ContextField::SafepointRequest => layout.safepoint_state,
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
    let abi = Aarch64Abi;
    let compiled = compile_function_aarch64(&builder.finish(), &abi).expect("lowering");
    let mut code = alloc_code(compiled.code.len()).expect("code allocation");
    write_code(&mut code, 0, &compiled.code).expect("code write");
    publish_code(&mut code).expect("code publication");
    let mut thread = Thread::new();
    let (value, count) = invoke_entry(
        &code,
        compiled.entry_offset as usize,
        &mut thread,
        2,
        [
            abi.encode_fixnum(1) as u64,
            abi.encode_fixnum(2) as u64,
            0,
            0,
        ],
        0,
    );
    assert_eq!(value, abi.encode_fixnum(3) as u64);
    assert_eq!(count, 1);
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
    let compiled = compile_function_aarch64(&builder.finish(), &abi).expect("lowering");
    let mut code = alloc_code(compiled.code.len()).expect("code allocation");
    write_code(&mut code, 0, &compiled.code).expect("code write");
    publish_code(&mut code).expect("code publication");
    let mut thread = Thread::new();
    let (value, count) = invoke_entry(
        &code,
        compiled.entry_offset as usize,
        &mut thread,
        2,
        [
            abi.encode_fixnum(4) as u64,
            abi.encode_fixnum(5) as u64,
            0,
            0,
        ],
        0,
    );
    assert_eq!(value, abi.encode_fixnum(9) as u64);
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
    let abi = Aarch64Abi;
    let compiled = compile_function_aarch64(&builder.finish(), &abi).expect("lowering");
    let mut code = alloc_code(compiled.code.len()).expect("code allocation");
    write_code(&mut code, 0, &compiled.code).expect("code write");
    publish_code(&mut code).expect("code publication");
    let rest = [abi.encode_fixnum(7) as u64];
    let mut thread = Thread::new();
    let (value, count) = invoke_entry(
        &code,
        compiled.entry_offset as usize,
        &mut thread,
        5,
        [1, 2, 3, 4],
        rest.as_ptr() as u64,
    );
    assert_eq!(value, abi.encode_fixnum(7) as u64);
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

    let abi = Aarch64Abi;
    let compiled = compile_function_aarch64(&builder.finish(), &abi).expect("lowering");
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
            [abi.encode_fixnum(value) as u64, 0, 0, 0],
            0,
        )
    };
    assert_eq!(invoke(1), (abi.encode_fixnum(1) as u64, 1));
    assert_eq!(invoke(2), (abi.encode_fixnum(2) as u64, 1));
}

#[test]
#[allow(clippy::too_many_lines)]
fn executes_recursive_fib_twenty_five_with_four_word_frames() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(3),
        "fib",
        vec![
            Param {
                name: "callee".into(),
                ty: Ty::Address,
            },
            Param {
                name: "n".into(),
                ty: Ty::Word,
            },
        ],
        vec![Ty::Word],
    );
    let callee = builder
        .push_op(OpKind::LoadArg { index: 0 }, &[Ty::Address])
        .expect("callee argument")[0];
    let n = builder
        .push_op(OpKind::LoadArg { index: 1 }, &[Ty::Word])
        .expect("n argument")[0];
    let one = builder.add_constant(Constant::Fixnum(1));
    let one = builder
        .push_op(OpKind::Const { result: one }, &[Ty::Word])
        .expect("one")[0];
    let two = builder.add_constant(Constant::Fixnum(2));
    let two = builder
        .push_op(OpKind::Const { result: two }, &[Ty::Word])
        .expect("two")[0];
    let condition = builder
        .push_op(
            OpKind::Compare {
                op: Compare::Le,
                left: n,
                right: one,
            },
            &[Ty::Word],
        )
        .expect("base comparison")[0];
    let base_n = builder.fresh_value();
    let recursive_callee = builder.fresh_value();
    let recursive_n = builder.fresh_value();
    let base = builder.create_block(vec![(Ty::Word, base_n)]);
    let recursive = builder.create_block(vec![
        (Ty::Address, recursive_callee),
        (Ty::Word, recursive_n),
    ]);
    builder.position_at(ncl_ir::BlockId(0)).expect("entry");
    builder
        .terminate(Terminator::Branch {
            condition,
            then_target: base,
            then_args: vec![n],
            else_target: recursive,
            else_args: vec![callee, n],
        })
        .expect("branch");
    builder.position_at(base).expect("base");
    builder
        .terminate(Terminator::Return {
            values: vec![base_n],
        })
        .expect("base return");
    builder.position_at(recursive).expect("recursive");
    let n_minus_one = builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumSub,
                args: vec![recursive_n, one],
                condition: None,
            },
            &[Ty::Word],
        )
        .expect("n minus one")[0];
    let first = builder
        .push_op(
            OpKind::Call {
                function: recursive_callee,
                args: vec![recursive_callee, n_minus_one],
            },
            &[Ty::Word],
        )
        .expect("first recursive call")[0];
    let n_minus_two = builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumSub,
                args: vec![recursive_n, two],
                condition: None,
            },
            &[Ty::Word],
        )
        .expect("n minus two")[0];
    let second = builder
        .push_op(
            OpKind::Call {
                function: recursive_callee,
                args: vec![recursive_callee, n_minus_two],
            },
            &[Ty::Word],
        )
        .expect("second recursive call")[0];
    let sum = builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumAdd,
                args: vec![first, second],
                condition: None,
            },
            &[Ty::Word],
        )
        .expect("sum")[0];
    builder
        .terminate(Terminator::Return { values: vec![sum] })
        .expect("recursive return");

    let abi = Aarch64Abi;
    let compiled = compile_function_aarch64(&builder.finish(), &abi).expect("lowering");
    let mut code = alloc_code(compiled.code.len()).expect("code allocation");
    write_code(&mut code, 0, &compiled.code).expect("code write");
    publish_code(&mut code).expect("code publication");
    let mut thread = Thread::new();
    let mut samples = Vec::with_capacity(10);
    for _ in 0..10 {
        let started = Instant::now();
        let (value, count) = invoke_entry(
            &code,
            compiled.entry_offset as usize,
            &mut thread,
            2,
            [code.address() as u64, abi.encode_fixnum(25) as u64, 0, 0],
            0,
        );
        samples.push(started.elapsed().as_nanos());
        assert_eq!(value, abi.encode_fixnum(75_025) as u64);
        assert_eq!(count, 1);
    }
    samples.sort_unstable();
    println!("fib(25) median: {} ns", samples[samples.len() / 2]);
}

#[path = "exec_aarch64/cons.rs"]
mod cons;

#[path = "exec_aarch64/basic.rs"]
mod basic;
