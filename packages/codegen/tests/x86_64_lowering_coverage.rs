#![allow(missing_docs, clippy::expect_used)]

#[path = "x86_64_lowering_coverage/fixture.rs"]
mod fixture;
use fixture::X86_64FixtureAbi;
use ncl_asm_x86_64::{Assembler, BinOp, Cond, Imm, Inst, Mem, Reg, Shift};
use ncl_codegen::{
    AbiError, Allocation, AllocationTarget, ContextField, Location, RuntimeAbi, RuntimeFunction,
    allocate, compile_function_x86_64, compile_function_x86_64_preserving_multiple_values,
};
use ncl_ir::{
    Constant, Convert, Function, FunctionBuilder, OpKind, Param, Prim, Terminator, Ty, ValueId,
};

fn encode(instructions: impl IntoIterator<Item = Inst>) -> Vec<u8> {
    let mut assembler = Assembler::new();
    for instruction in instructions {
        assembler
            .emit(&instruction)
            .expect("expected x86-64 instruction encoding");
    }
    assembler.bytes().to_vec()
}

fn assert_contains(code: &[u8], expected: &[u8], behavior: &str) {
    assert!(
        code.windows(expected.len())
            .any(|window| window == expected),
        "{behavior}: expected {expected:02x?} in {code:02x?}"
    );
}

struct WriteBarrierFixtureAbi;

impl RuntimeAbi for WriteBarrierFixtureAbi {
    fn builtin_address(&self, identifier: ncl_object::BuiltinIdentifier) -> Result<u64, AbiError> {
        X86_64FixtureAbi.builtin_address(identifier)
    }

    fn field_offset(&self, field: ContextField) -> Result<i32, AbiError> {
        X86_64FixtureAbi.field_offset(field)
    }

    fn runtime_address(&self, function: RuntimeFunction) -> Result<u64, AbiError> {
        if function == RuntimeFunction::WriteBarrier {
            Ok(0x3000)
        } else {
            X86_64FixtureAbi.runtime_address(function)
        }
    }

    fn constant_word(&self, name: &str) -> Option<i64> {
        X86_64FixtureAbi.constant_word(name)
    }
}

#[test]
fn x86_64_fixnum_comparisons_materialize_their_own_conditions() {
    for (index, (primitive, condition)) in [
        (Prim::FixnumEq, Cond::E),
        (Prim::FixnumLt, Cond::L),
        (Prim::FixnumLe, Cond::Le),
    ]
    .into_iter()
    .enumerate()
    {
        let function_id = 200 + u32::try_from(index).expect("function index fits");
        let mut builder = FunctionBuilder::new(
            ncl_ir::FunctionId(function_id),
            "fixnum-comparison-lowering",
            vec![
                Param {
                    name: "left".into(),
                    ty: Ty::I64,
                },
                Param {
                    name: "right".into(),
                    ty: Ty::I64,
                },
            ],
            vec![Ty::Bool],
        );
        let left = ValueId(0);
        let right = ValueId(1);
        let result = builder
            .push_op(
                OpKind::Prim {
                    op: primitive.clone(),
                    args: vec![left, right],
                    condition: None,
                },
                &[Ty::Bool],
            )
            .expect("comparison primitive")[0];
        builder
            .terminate(Terminator::Return {
                values: vec![result],
            })
            .expect("comparison return");

        let function = builder.finish();
        // compile_function_x86_64 recomputes allocation from this function.
        let allocation = allocate(&function, AllocationTarget::X86_64);
        let compiled =
            compile_function_x86_64(&function, &X86_64FixtureAbi).expect("comparison lowers");
        let expected = encode([
            load_value_into_register(&function, &allocation, left, Reg::R10),
            load_value_into_register(&function, &allocation, right, Reg::R11),
            Inst::CmpRR(Reg::R10, Reg::R11),
            Inst::Setcc(condition, Reg::R10),
            Inst::Movzx(Reg::R10, Reg::R10, 8),
            store_register_into_value(&function, &allocation, result, Reg::R10),
        ]);
        assert_contains(
            &compiled.code,
            &expected,
            &format!(
                "{primitive:?} must load left into r10 and right into r11 before {condition:?} and zero-extension"
            ),
        );
    }
}

#[test]
fn x86_64_cons_primitives_use_their_field_offsets() {
    for (index, (primitive, offset, is_load)) in [
        (Prim::Car, 0, true),
        (Prim::Cdr, 8, true),
        (Prim::Rplaca, 0, false),
        (Prim::Rplacd, 8, false),
    ]
    .into_iter()
    .enumerate()
    {
        let function_id = 210 + u32::try_from(index).expect("function index fits");
        let mut params = vec![Param {
            name: "object".into(),
            ty: Ty::Word,
        }];
        let args = if is_load {
            vec![ValueId(0)]
        } else {
            params.push(Param {
                name: "value".into(),
                ty: Ty::Word,
            });
            vec![ValueId(0), ValueId(1)]
        };
        let result_types = if is_load { vec![Ty::Word] } else { Vec::new() };
        let mut builder = FunctionBuilder::new(
            ncl_ir::FunctionId(function_id),
            "cons-primitive-offset",
            params,
            result_types,
        );
        let op_result_types = [if is_load { Ty::Word } else { Ty::Unit }];
        let results = builder
            .push_op(
                OpKind::Prim {
                    op: primitive.clone(),
                    args,
                    condition: None,
                },
                &op_result_types,
            )
            .expect("cons primitive");
        // Rplaca/Rplacd have a Unit IR result even though the fixture returns no values.
        let result = results[0];
        builder
            .terminate(Terminator::Return {
                values: if is_load { vec![result] } else { Vec::new() },
            })
            .expect("cons primitive return");

        let function = builder.finish();
        let allocation = allocate(&function, AllocationTarget::X86_64);
        let compiled = compile_function_x86_64(&function, &WriteBarrierFixtureAbi)
            .expect("cons primitive lowers");
        let expected = if is_load {
            encode([
                load_value_into_register(&function, &allocation, ValueId(0), Reg::R10),
                Inst::BinRI(BinOp::And, Reg::R10, -8),
                Inst::MovRM(Reg::R10, Mem::base(Reg::R10, offset)),
                store_register_into_value(&function, &allocation, result, Reg::R10),
            ])
        } else {
            encode([
                load_value_into_register(&function, &allocation, ValueId(0), Reg::R10),
                load_value_into_register(&function, &allocation, ValueId(1), Reg::R11),
                Inst::BinRI(BinOp::And, Reg::R10, -8),
                Inst::MovMR(Mem::base(Reg::R10, offset), Reg::R11),
            ])
        };
        assert_contains(
            &compiled.code,
            &expected,
            &format!("{primitive:?} must access byte offset {offset}"),
        );
    }
}

fn load_value_into_register(
    function: &Function,
    allocation: &Allocation,
    value: ValueId,
    target: Reg,
) -> Inst {
    match allocation.location(value).expect("value is allocated") {
        Location::Register(allocated) => Inst::MovRR(target, allocated_register(allocated)),
        Location::Spill(spill) => Inst::MovRM(target, spill_memory(function, spill)),
    }
}

fn store_register_into_value(
    function: &Function,
    allocation: &Allocation,
    value: ValueId,
    source: Reg,
) -> Inst {
    match allocation.location(value).expect("value is allocated") {
        Location::Register(allocated) => Inst::MovRR(allocated_register(allocated), source),
        Location::Spill(spill) => Inst::MovMR(spill_memory(function, spill), source),
    }
}

fn spill_memory(function: &Function, spill: u32) -> Mem {
    let local_words = function
        .blocks
        .iter()
        .map(|block| {
            block.params.len() + block.ops.iter().map(|op| op.results.len()).sum::<usize>()
        })
        .sum::<usize>();
    let spill_base =
        u32::try_from(function.params.len() + local_words).expect("frame slot count fits");
    let bytes = spill_base
        .checked_add(spill)
        .and_then(|slot| slot.checked_add(1))
        .and_then(|slot| slot.checked_mul(8))
        .and_then(|bytes| i32::try_from(bytes).ok())
        .expect("spill byte offset fits");
    Mem::base(Reg::Rbp, -bytes)
}

fn allocated_register(id: u16) -> Reg {
    Reg::from_id(u8::try_from(id).expect("register id fits")).expect("allocated register exists")
}

fn add_fixnum_word(builder: &mut FunctionBuilder, value: i64) -> ValueId {
    let constant = builder.add_constant(Constant::Fixnum(value));
    let integer = builder
        .push_op(OpKind::Const { result: constant }, &[Ty::I64])
        .expect("fixnum constant")[0];
    builder
        .push_op(
            OpKind::Convert {
                op: Convert::I64ToWord,
                value: integer,
            },
            &[Ty::Word],
        )
        .expect("fixnum word")[0]
}

#[test]
fn x86_64_set_multiple_values_stores_distinct_consecutive_values_and_count() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(220),
        "multiple-value-stores",
        Vec::new(),
        Vec::new(),
    );
    let first = add_fixnum_word(&mut builder, 11);
    let second = add_fixnum_word(&mut builder, 22);
    builder
        .push_op(
            OpKind::SetMultipleValues {
                values: vec![first, second],
            },
            &[],
        )
        .expect("set multiple values");
    builder
        .terminate(Terminator::Return { values: Vec::new() })
        .expect("void return after the multiple-value operation");
    let function = builder.finish();
    let allocation = allocate(&function, AllocationTarget::X86_64);
    assert_ne!(
        allocation.location(first),
        allocation.location(second),
        "both values must have distinct live locations"
    );
    let compiled = compile_function_x86_64(&function, &X86_64FixtureAbi)
        .expect("multiple-value function lowers");
    let layout = ncl_sys::thread_layout();
    let area = i32::try_from(layout.mv).expect("multiple-value area offset fits");
    let second_area = area.checked_add(8).expect("second slot offset fits");
    let count = i32::try_from(layout.mv_count).expect("multiple-value count offset fits");

    let expected_stores = encode([
        load_value_into_register(&function, &allocation, first, Reg::R10),
        Inst::MovMR(Mem::base(Reg::R15, area), Reg::R10),
        load_value_into_register(&function, &allocation, second, Reg::R10),
        Inst::MovMR(Mem::base(Reg::R15, second_area), Reg::R10),
        Inst::MovRI(Reg::Rdx, Imm::I32(2)),
        Inst::MovMR(Mem::base(Reg::R15, count), Reg::Rdx),
    ]);
    assert_contains(
        &compiled.code,
        &expected_stores,
        "SetMultipleValues must load each input, store adjacent slots, and set count to two",
    );
}

#[test]
fn x86_64_eval_return_reads_the_published_multiple_value_count() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(221),
        "eval-multiple-values",
        Vec::new(),
        vec![Ty::Word],
    );
    let first = add_fixnum_word(&mut builder, 11);
    let second = add_fixnum_word(&mut builder, 22);
    builder
        .push_op(
            OpKind::SetMultipleValues {
                values: vec![first, second],
            },
            &[],
        )
        .expect("set multiple values");
    builder
        .terminate(Terminator::Return {
            values: vec![first],
        })
        .expect("return first value");

    let compiled =
        compile_function_x86_64_preserving_multiple_values(&builder.finish(), &X86_64FixtureAbi)
            .expect("EVAL return lowers");
    let layout = ncl_sys::thread_layout();
    let count = i32::try_from(layout.mv_count).expect("multiple-value count offset fits");
    let expected_count_load = encode([Inst::MovRM(Reg::Rdx, Mem::base(Reg::R15, count))]);

    assert_contains(
        &compiled.code,
        &expected_count_load,
        "EVAL return must read the published multiple-value count",
    );
}

struct UndefinedFunctionFixtureAbi;

impl RuntimeAbi for UndefinedFunctionFixtureAbi {
    fn builtin_address(&self, identifier: ncl_object::BuiltinIdentifier) -> Result<u64, AbiError> {
        X86_64FixtureAbi.builtin_address(identifier)
    }

    fn field_offset(&self, field: ContextField) -> Result<i32, AbiError> {
        X86_64FixtureAbi.field_offset(field)
    }

    fn runtime_address(&self, function: RuntimeFunction) -> Result<u64, AbiError> {
        if function == RuntimeFunction::UndefinedFunction {
            Ok(0x1234)
        } else {
            X86_64FixtureAbi.runtime_address(function)
        }
    }

    fn constant_word(&self, name: &str) -> Option<i64> {
        X86_64FixtureAbi.constant_word(name)
    }
}

fn assert_capture_index_two_load(code: &[u8]) {
    let capture_offset = i32::try_from(
        ncl_object::function_offset::CAPTURES
            .checked_add(2)
            .and_then(|index| index.checked_add(1))
            .and_then(|index| index.checked_mul(8))
            .expect("capture offset fits usize"),
    )
    .expect("capture offset fits i32");
    let capture_load = encode([
        Inst::MovRM(Reg::Rax, Mem::base(Reg::Rbp, 16)),
        Inst::MovRI(Reg::R11, Imm::I32(-8)),
        Inst::BinRR(BinOp::And, Reg::Rax, Reg::R11),
        Inst::MovRM(Reg::R10, Mem::base(Reg::Rax, capture_offset)),
    ]);
    assert_contains(
        code,
        &capture_load,
        "capture index two must load CAPTURES + three words after untagging the closure",
    );
}

fn assert_named_closure_unbound_dispatch(
    code: &[u8],
    function: &Function,
    allocation: &Allocation,
    closure: ValueId,
    argument_count: ValueId,
) {
    let unbound = i64::from_ne_bytes(ncl_sys::Word::UNBOUND.bits().to_ne_bytes());
    let unbound_immediate =
        i32::try_from(unbound).expect("UNBOUND fits in a sign-extended x86-64 immediate");
    let closure_load_and_unbound_compare = encode([
        load_value_into_register(function, allocation, closure, Reg::R11),
        Inst::MovRI(Reg::Rax, Imm::I32(unbound_immediate)),
        Inst::CmpRR(Reg::R11, Reg::Rax),
    ]);
    let compare_start = code
        .windows(closure_load_and_unbound_compare.len())
        .position(|window| window == closure_load_and_unbound_compare)
        .expect("named dispatch loads its closure into r11 before the unbound comparison");
    let branch_start = compare_start + closure_load_and_unbound_compare.len();
    assert_eq!(
        &code[branch_start..branch_start + 2],
        &[0x0f, 0x85],
        "a bound closure must branch around the UndefinedFunction path"
    );
    let undefined_target_start = branch_start + 6;
    let undefined_target = encode([Inst::MovRI(Reg::R11, Imm::I32(0x1234))]);
    assert_eq!(
        &code[undefined_target_start..undefined_target_start + undefined_target.len()],
        undefined_target,
        "the unbound fallthrough must load the UndefinedFunction runtime address"
    );
    let jump_start = undefined_target_start + undefined_target.len();
    assert_eq!(
        code[jump_start], 0xe9,
        "the undefined path must jump to the call setup"
    );
    let call_setup = encode([
        Inst::BinRI(BinOp::Sub, Reg::Rsp, 16),
        Inst::CallReg(Reg::R11),
    ]);
    let call_setup_start = code[branch_start..]
        .windows(call_setup.len())
        .position(|window| window == call_setup)
        .map(|offset| branch_start + offset)
        .expect("named dispatch must reach the indirect call setup");
    let jump_target = rel32_target(code, jump_start + 1, jump_start + 5);
    assert_eq!(
        jump_target, call_setup_start,
        "the undefined path must skip the bound-closure setup and rejoin at its call"
    );
    let branch_target = rel32_target(code, branch_start + 2, branch_start + 6);
    assert_eq!(
        branch_target,
        jump_start + 5,
        "the bound path must branch to the first instruction after the undefined fallback"
    );
    let entry_offset = i32::try_from((ncl_object::function_offset::ENTRY + 1) * 8)
        .expect("closure entry offset fits");
    let bound_dispatch = encode([
        load_value_into_register(function, allocation, closure, Reg::R10),
        Inst::MovRR(Reg::R11, Reg::R10),
        Inst::MovRI(Reg::Rax, Imm::I32(-8)),
        Inst::BinRR(BinOp::And, Reg::R11, Reg::Rax),
        Inst::MovRR(Reg::Rax, Reg::R11),
        load_value_into_register(function, allocation, argument_count, Reg::Rdi),
        Inst::MovRM(Reg::R11, Mem::base(Reg::Rax, entry_offset)),
        Inst::ShiftImm(
            Shift::Sar,
            Reg::R11,
            u8::try_from(ncl_sys::FIXNUM_TAG_BITS).expect("tag width fits"),
        ),
    ]);
    let bound_path = &code[branch_target..call_setup_start];
    assert_contains(
        bound_path,
        &bound_dispatch,
        "bound dispatch must preserve the untagged closure pointer in rax through the entry load",
    );
}

fn rel32_target(code: &[u8], start: usize, end: usize) -> usize {
    let displacement = i32::from_le_bytes(code[start..end].try_into().expect("rel32 bytes"));
    end + usize::try_from(displacement).expect("branch target is forward")
}

#[test]
fn x86_64_load_capture_and_named_closure_emit_indexed_and_unbound_paths() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(221),
        "capture-and-named-closure",
        vec![
            Param {
                name: "argument-count".into(),
                ty: Ty::Word,
            },
            Param {
                name: "closure".into(),
                ty: Ty::Word,
            },
        ],
        vec![Ty::Word],
    );
    let argument_count = ValueId(0);
    let capture = builder
        .push_op(OpKind::LoadCapture { index: 2 }, &[Ty::Word])
        .expect("capture load")[0];
    let symbol_constant = builder.add_constant(Constant::Symbol {
        package: "COMMON-LISP".into(),
        name: "CAR".into(),
    });
    let symbol = builder
        .push_op(
            OpKind::Const {
                result: symbol_constant,
            },
            &[Ty::Word],
        )
        .expect("named function symbol")[0];
    let entry_constant = builder.add_constant(Constant::FunctionEntry(ncl_ir::FunctionId(222)));
    let entry = builder
        .push_op(
            OpKind::Const {
                result: entry_constant,
            },
            &[Ty::Word],
        )
        .expect("closure entry")[0];
    let closure = builder
        .push_op(
            OpKind::MakeClosure {
                entry,
                captures: Vec::new(),
            },
            &[Ty::Word],
        )
        .expect("closure value")[0];
    let _call_result = builder
        .push_op(
            OpKind::CallClosure {
                closure,
                args: vec![argument_count],
                named_symbol: Some(symbol),
            },
            &[Ty::Word],
        )
        .expect("named closure call")[0];
    builder
        .terminate(Terminator::Return {
            values: vec![capture],
        })
        .expect("return captured value");

    let function = builder.finish();
    let allocation = allocate(&function, AllocationTarget::X86_64);
    let compiled = compile_function_x86_64(&function, &UndefinedFunctionFixtureAbi)
        .expect("named closure function lowers");
    assert_capture_index_two_load(&compiled.code);
    assert_named_closure_unbound_dispatch(
        &compiled.code,
        &function,
        &allocation,
        closure,
        argument_count,
    );
}
