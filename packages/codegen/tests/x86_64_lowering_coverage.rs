#![allow(missing_docs, clippy::expect_used)]

#[path = "x86_64_lowering_coverage/fixture.rs"]
mod fixture;
use fixture::X86_64FixtureAbi;
use ncl_asm_x86_64::{Assembler, BinOp, Cond, Imm, Inst, Mem, Reg, Shift};
use ncl_codegen::{
    AbiError, Allocation, AllocationTarget, CodegenError, ContextField, FLAG_ALLOCATION_SLOW,
    FLAG_CALL, Location, RuntimeAbi, RuntimeFunction, X86_64Abi, allocate, compile_function,
    compile_function_x86_64, compile_function_x86_64_preserving_multiple_values,
};
use ncl_ir::{
    Constant, Convert, FixnumOp, FixnumSpecialization, Function, FunctionBuilder, OpKind, Param,
    Prim, Terminator, Ty, ValueId,
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

fn assert_call_safepoint(
    compiled: &ncl_codegen::CompiledFunction,
    address: u64,
    expected_flags: u32,
) {
    let address_instruction = encode([Inst::MovRI(
        Reg::R11,
        Imm::I32(i32::try_from(address).expect("fixture address fits an immediate")),
    )]);
    let address_offset = compiled
        .code
        .windows(address_instruction.len())
        .position(|window| window == address_instruction.as_slice())
        .expect("runtime address instruction is present");
    let call_instruction = encode([Inst::CallReg(Reg::R11)]);
    let call_offset = compiled.code[address_offset + address_instruction.len()..]
        .windows(call_instruction.len())
        .position(|window| window == call_instruction.as_slice())
        .map(|offset| address_offset + address_instruction.len() + offset)
        .expect("runtime address is followed by its indirect call");
    let call_end = u32::try_from(call_offset + call_instruction.len())
        .expect("call offset fits safepoint metadata");
    assert!(
        compiled
            .safepoint_maps
            .iter()
            .any(|map| map.pc_offset == call_end && map.map_flags == expected_flags),
        "call ending at {call_end} must have a safepoint map with flags {expected_flags:#x}: {:?}",
        compiled.safepoint_maps
    );
}

struct LoweringCoverageAbi;

impl RuntimeAbi for LoweringCoverageAbi {
    fn builtin_address(&self, _identifier: ncl_object::BuiltinIdentifier) -> Result<u64, AbiError> {
        Ok(0x1000)
    }

    fn field_offset(&self, field: ContextField) -> Result<i32, AbiError> {
        X86_64Abi.field_offset(field)
    }

    fn runtime_address(&self, function: RuntimeFunction) -> Result<u64, AbiError> {
        match function {
            RuntimeFunction::AllocateSlow => Ok(0x2000),
            RuntimeFunction::SafepointSlow => Ok(0x3000),
            RuntimeFunction::WriteBarrier => Ok(0x4000),
            RuntimeFunction::MakeValueCell => Ok(0x5000),
            _ => Err(AbiError::UnsupportedRuntimeFunction(function)),
        }
    }
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

#[test]
fn fixed_template_builtin_marshals_four_words_and_records_the_call_pc() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(230),
        "fixed-template-builtin-four-arguments",
        ["first", "second", "third", "fourth"]
            .into_iter()
            .map(|name| Param {
                name: name.into(),
                ty: Ty::Word,
            })
            .collect(),
        vec![Ty::Word],
    );
    let args = (0..4)
        .map(|index| {
            builder
                .push_op(
                    OpKind::LoadArg {
                        index: u8::try_from(index).expect("argument index fits"),
                    },
                    &[Ty::Word],
                )
                .expect("incoming builtin argument")[0]
        })
        .collect::<Vec<_>>();
    let result = builder
        .push_op(
            OpKind::Builtin {
                name: "coverage-probe".into(),
                args: args.to_vec(),
                specialization: None,
                specialization_disabled: false,
            },
            &[Ty::Word],
        )
        .expect("builtin result")[0];
    builder
        .terminate(Terminator::Return {
            values: vec![result],
        })
        .expect("builtin return");

    let compiled = compile_function(&builder.finish(), &LoweringCoverageAbi)
        .expect("fixed-template builtin lowers");
    let expected = encode([
        Inst::MovRR(Reg::Rdi, Reg::R15),
        Inst::MovRM(Reg::Rsi, Mem::base(Reg::Rbp, -40)),
        Inst::MovRM(Reg::Rdx, Mem::base(Reg::Rbp, -48)),
        Inst::MovRM(Reg::Rcx, Mem::base(Reg::Rbp, -56)),
        Inst::MovRM(Reg::R8, Mem::base(Reg::Rbp, -64)),
        Inst::MovRI(Reg::R11, Imm::I64(0x1000)),
        Inst::CallReg(Reg::R11),
    ]);
    assert_contains(
        &compiled.code,
        &expected,
        "fixed-template builtins must pass four SSA words in RSI, RDX, RCX, and R8",
    );
    assert_eq!(compiled.safepoint_maps.len(), 1);
    assert_eq!(compiled.safepoint_maps[0].map_flags, FLAG_CALL);
    assert_eq!(
        usize::try_from(compiled.safepoint_maps[0].pc_offset).expect("call PC fits usize"),
        compiled
            .code
            .windows(3)
            .position(|window| window == [0x41, 0xff, 0xd3].as_slice())
            .expect("builtin emits call r11")
            + 3,
        "fixed-template call map must point immediately after call r11"
    );
}

#[test]
fn five_argument_builtin_returns_the_exact_arity_error_from_both_lowerers() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(239),
        "builtin-five-argument-error",
        Vec::new(),
        vec![Ty::Word],
    );
    let args = [1, 2, 3, 4, 5]
        .into_iter()
        .map(|value| add_fixnum_word(&mut builder, value))
        .collect::<Vec<_>>();
    let result = builder
        .push_op(
            OpKind::Builtin {
                name: "coverage-probe".into(),
                args,
                specialization: None,
                specialization_disabled: false,
            },
            &[Ty::Word],
        )
        .expect("builtin result")[0];
    builder
        .terminate(Terminator::Return {
            values: vec![result],
        })
        .expect("builtin return");
    let function = builder.finish();
    let expected_error =
        CodegenError::Unsupported("x86-64 builtins support at most four arguments".into());

    let fixed_template_error = match compile_function(&function, &LoweringCoverageAbi) {
        Ok(_) => panic!("fixed-template builtin accepted five arguments"),
        Err(error) => error,
    };
    assert_eq!(fixed_template_error, expected_error);

    let target_error = match compile_function_x86_64(&function, &LoweringCoverageAbi) {
        Ok(_) => panic!("target builtin accepted five arguments"),
        Err(error) => error,
    };
    assert_eq!(target_error, expected_error);
}

#[test]
fn fixed_template_rejects_make_rest_list_without_its_special_abi_setup() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(240),
        "fixed-template-make-rest-list-unsupported",
        Vec::new(),
        vec![Ty::Word],
    );
    let argc = add_fixnum_word(&mut builder, 6);
    let start = add_fixnum_word(&mut builder, 2);
    let result = builder
        .push_op(
            OpKind::Builtin {
                name: "make-rest-list".into(),
                args: vec![argc, start],
                specialization: None,
                specialization_disabled: false,
            },
            &[Ty::Word],
        )
        .expect("builtin result")[0];
    builder
        .terminate(Terminator::Return {
            values: vec![result],
        })
        .expect("builtin return");

    let error = match compile_function(&builder.finish(), &LoweringCoverageAbi) {
        Ok(_) => panic!("fixed-template lowering accepted make-rest-list's different ABI"),
        Err(error) => error,
    };
    assert_eq!(
        error,
        CodegenError::Unsupported("make-rest-list requires target-specific ABI lowering".into())
    );
}

#[test]
fn fixed_template_load_arg_copies_the_selected_entry_register() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(231),
        "fixed-template-load-second-argument",
        vec![
            Param {
                name: "first".into(),
                ty: Ty::Word,
            },
            Param {
                name: "second".into(),
                ty: Ty::Word,
            },
        ],
        vec![Ty::Word],
    );
    let loaded = builder
        .push_op(OpKind::LoadArg { index: 1 }, &[Ty::Word])
        .expect("second argument load")[0];
    builder
        .terminate(Terminator::Return {
            values: vec![loaded],
        })
        .expect("argument return");

    let compiled = compile_function(&builder.finish(), &LoweringCoverageAbi)
        .expect("fixed-template argument load lowers");
    let expected = encode([
        Inst::MovRR(Reg::R10, Reg::Rdx),
        Inst::MovMR(Mem::base(Reg::Rbp, -40), Reg::R10),
    ]);
    assert_contains(
        &compiled.code,
        &expected,
        "LoadArg index 1 must copy RDX into its allocated frame slot",
    );
}

#[test]
fn fixed_template_store_field_uses_the_word_offset_and_barrier_arguments() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(232),
        "fixed-template-store-field-barrier",
        vec![ncl_ir::Param {
            name: "owner".into(),
            ty: Ty::Word,
        }],
        Vec::new(),
    );
    let owner = builder
        .push_op(OpKind::LoadArg { index: 0 }, &[Ty::Word])
        .expect("word owner argument")[0];
    let value = add_fixnum_word(&mut builder, 13);
    builder
        .push_op(
            OpKind::StoreField {
                object: owner,
                field: 2,
                value,
            },
            &[],
        )
        .expect("field store");
    builder
        .terminate(Terminator::Return { values: Vec::new() })
        .expect("void return");

    let compiled = compile_function(&builder.finish(), &LoweringCoverageAbi)
        .expect("fixed-template field store lowers");
    let field_store = encode([
        Inst::MovRM(Reg::R10, Mem::base(Reg::Rbp, -40)),
        Inst::MovRM(Reg::R11, Mem::base(Reg::Rbp, -56)),
        Inst::MovMR(Mem::base(Reg::R10, 16), Reg::R11),
    ]);
    assert_contains(
        &compiled.code,
        &field_store,
        "field 2 must store at byte offset 16 using the allocated owner and value",
    );
    let barrier_call = encode([
        Inst::MovRR(Reg::Rdi, Reg::R15),
        Inst::MovRI(Reg::Rsi, Imm::I64(2)),
        Inst::MovRM(Reg::Rdx, Mem::base(Reg::Rbp, -40)),
        Inst::MovRI(Reg::R11, Imm::I64(0x4000)),
        Inst::CallReg(Reg::R11),
    ]);
    assert_contains(
        &compiled.code,
        &barrier_call,
        "write barrier must receive context, field index, and the stored object's owner",
    );
}

#[test]
fn x86_64_load_arg_and_field_load_use_the_selected_argument_and_header_offset() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(233),
        "load-argument-and-field",
        vec![Param {
            name: "object".into(),
            ty: Ty::Word,
        }],
        vec![Ty::Word],
    );
    let object = builder
        .push_op(OpKind::LoadArg { index: 0 }, &[Ty::Word])
        .expect("object argument load")[0];
    let field = builder
        .push_op(OpKind::LoadField { object, field: 3 }, &[Ty::Word])
        .expect("field load")[0];
    builder
        .terminate(Terminator::Return {
            values: vec![field],
        })
        .expect("field return");

    let function = builder.finish();
    let allocation = allocate(&function, AllocationTarget::X86_64);
    let compiled = compile_function_x86_64(&function, &LoweringCoverageAbi)
        .expect("argument and field loads lower");
    let argument_load = encode([
        load_value_into_register(&function, &allocation, ValueId(0), Reg::R10),
        store_register_into_value(&function, &allocation, object, Reg::R10),
    ]);
    assert_contains(
        &compiled.code,
        &argument_load,
        "LoadArg index 0 must copy the first incoming value to its SSA result",
    );
    let untag_mask = i32::try_from(i64::from_ne_bytes((!ncl_sys::LOWTAG_MASK).to_ne_bytes()))
        .expect("lowtag mask fits an x86 immediate");
    let field_load = encode([
        load_value_into_register(&function, &allocation, object, Reg::R10),
        Inst::BinRI(BinOp::And, Reg::R10, untag_mask),
        Inst::MovRM(Reg::R10, Mem::base(Reg::R10, 32)),
        store_register_into_value(&function, &allocation, field, Reg::R10),
    ]);
    assert_contains(
        &compiled.code,
        &field_load,
        "field 3 must untag its object and load after the one-word object header",
    );
}

#[test]
fn x86_64_builtin_marshals_four_arguments_and_maps_its_call() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(234),
        "builtin-four-arguments",
        ["first", "second", "third", "fourth"]
            .into_iter()
            .map(|name| Param {
                name: name.into(),
                ty: Ty::Word,
            })
            .collect(),
        vec![Ty::Word],
    );
    let args = [ValueId(0), ValueId(1), ValueId(2), ValueId(3)];
    let result = builder
        .push_op(
            OpKind::Builtin {
                name: "coverage-probe".into(),
                args: args.to_vec(),
                specialization: None,
                specialization_disabled: false,
            },
            &[Ty::Word],
        )
        .expect("builtin result")[0];
    builder
        .terminate(Terminator::Return {
            values: vec![result],
        })
        .expect("builtin return");

    let function = builder.finish();
    let allocation = allocate(&function, AllocationTarget::X86_64);
    let compiled =
        compile_function_x86_64(&function, &LoweringCoverageAbi).expect("builtin call lowers");
    let expected = encode([
        Inst::MovRR(Reg::Rdi, Reg::R15),
        Inst::MovRI(Reg::R11, Imm::I32(0x1000)),
        load_value_into_register(&function, &allocation, args[0], Reg::Rsi),
        load_value_into_register(&function, &allocation, args[1], Reg::Rdx),
        load_value_into_register(&function, &allocation, args[2], Reg::Rcx),
        load_value_into_register(&function, &allocation, args[3], Reg::R8),
        Inst::BinRI(BinOp::Sub, Reg::Rsp, 16),
        Inst::CallReg(Reg::R11),
        Inst::BinRI(BinOp::Add, Reg::Rsp, 16),
    ]);
    assert_contains(
        &compiled.code,
        &expected,
        "target builtins must pass all four values in the runtime argument registers",
    );
    assert_call_safepoint(&compiled, 0x1000, FLAG_CALL);
}

#[test]
fn x86_64_fixnum_specialization_checks_tags_and_keeps_overflow_fallback() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(235),
        "specialized-fixnum-add",
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
    let args = [ValueId(0), ValueId(1)];
    let specialization = FixnumSpecialization::new(FixnumOp::Add, &[true, true])
        .expect("two-operand fixnum specialization");
    let result = builder
        .push_op(
            OpKind::Builtin {
                name: "+".into(),
                args: args.to_vec(),
                specialization: Some(specialization),
                specialization_disabled: false,
            },
            &[Ty::Word],
        )
        .expect("specialized result")[0];
    builder
        .terminate(Terminator::Return {
            values: vec![result],
        })
        .expect("specialized return");

    let function = builder.finish();
    let allocation = allocate(&function, AllocationTarget::X86_64);
    let compiled = compile_function_x86_64(&function, &LoweringCoverageAbi)
        .expect("fixnum specialization lowers");
    let tag_mask = i32::try_from((1_u64 << ncl_sys::FIXNUM_TAG_BITS) - 1)
        .expect("fixnum tag mask fits an immediate");
    let fixnum_tag = i32::try_from(ncl_sys::FIXNUM_TAG).expect("fixnum tag fits an immediate");
    for argument in args {
        let tag_test = encode([
            load_value_into_register(&function, &allocation, argument, Reg::R10),
            Inst::BinRI(BinOp::And, Reg::R10, tag_mask),
            Inst::CmpRI(Reg::R10, fixnum_tag),
        ]);
        assert_contains(
            &compiled.code,
            &tag_test,
            "each guarded operand must be checked for the fixnum tag",
        );
    }
    assert_eq!(
        compiled
            .code
            .windows(2)
            .filter(|window| window[0] == 0x0f && window[1] == 0x85)
            .count(),
        2,
        "both failed tag checks must branch to the generic builtin path"
    );
    let fast_add = encode([
        load_value_into_register(&function, &allocation, args[0], Reg::R10),
        load_value_into_register(&function, &allocation, args[1], Reg::R11),
        Inst::BinRR(BinOp::Add, Reg::R10, Reg::R11),
    ]);
    assert_contains(
        &compiled.code,
        &fast_add,
        "the fast path must add the two tagged fixnum operands and branch on overflow",
    );
    let fallback_setup = encode([
        Inst::MovRR(Reg::Rdi, Reg::R15),
        Inst::MovRI(Reg::R11, Imm::I32(0x1000)),
        load_value_into_register(&function, &allocation, args[0], Reg::Rsi),
        load_value_into_register(&function, &allocation, args[1], Reg::Rdx),
        Inst::BinRI(BinOp::Sub, Reg::Rsp, 16),
        Inst::CallReg(Reg::R11),
        Inst::BinRI(BinOp::Add, Reg::Rsp, 16),
    ]);
    assert_contains(
        &compiled.code,
        &fallback_setup,
        "the slow path must call the generic builtin with both original operands",
    );
    assert_call_safepoint(&compiled, 0x1000, FLAG_CALL);
}

#[test]
fn x86_64_make_rest_list_places_explicit_words_in_the_callee_header() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(236),
        "make-rest-list-call",
        vec![
            Param {
                name: "argument-count".into(),
                ty: Ty::Word,
            },
            Param {
                name: "start-index".into(),
                ty: Ty::Word,
            },
        ],
        vec![Ty::Word],
    );
    let args = [ValueId(0), ValueId(1)];
    let result = builder
        .push_op(
            OpKind::Builtin {
                name: "make-rest-list".into(),
                args: args.to_vec(),
                specialization: None,
                specialization_disabled: false,
            },
            &[Ty::Word],
        )
        .expect("rest-list result")[0];
    builder
        .terminate(Terminator::Return {
            values: vec![result],
        })
        .expect("rest-list return");

    let function = builder.finish();
    let allocation = allocate(&function, AllocationTarget::X86_64);
    let compiled =
        compile_function_x86_64(&function, &LoweringCoverageAbi).expect("rest-list builtin lowers");
    let header_arguments = encode([
        load_value_into_register(&function, &allocation, args[0], Reg::R10),
        load_value_into_register(&function, &allocation, args[1], Reg::Rax),
        Inst::MovMR(Mem::base(Reg::Rsp, -16), Reg::R10),
        Inst::MovMR(Mem::base(Reg::Rsp, -8), Reg::Rax),
        Inst::MovRI(Reg::R11, Imm::I32(0x1000)),
        Inst::BinRI(BinOp::Sub, Reg::Rsp, 16),
        Inst::CallReg(Reg::R11),
        Inst::BinRI(BinOp::Add, Reg::Rsp, 16),
    ]);
    assert_contains(
        &compiled.code,
        &header_arguments,
        "make-rest-list must pass argc and start index in the callee frame header",
    );
    assert_call_safepoint(&compiled, 0x1000, FLAG_CALL);
}

#[test]
fn x86_64_make_value_cell_passes_its_value_to_the_runtime_helper() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(237),
        "make-value-cell-call",
        Vec::new(),
        vec![Ty::Word],
    );
    let value = add_fixnum_word(&mut builder, 17);
    let cell = builder
        .push_op(OpKind::MakeValueCell { value }, &[Ty::Word])
        .expect("value cell result")[0];
    builder
        .terminate(Terminator::Return { values: vec![cell] })
        .expect("value-cell return");

    let function = builder.finish();
    let allocation = allocate(&function, AllocationTarget::X86_64);
    let compiled =
        compile_function_x86_64(&function, &LoweringCoverageAbi).expect("value-cell helper lowers");
    let setup = encode([
        Inst::MovRR(Reg::Rdi, Reg::R15),
        Inst::MovRI(Reg::R11, Imm::I32(0x5000)),
        load_value_into_register(&function, &allocation, value, Reg::Rsi),
        Inst::BinRI(BinOp::Sub, Reg::Rsp, 16),
        Inst::CallReg(Reg::R11),
        Inst::BinRI(BinOp::Add, Reg::Rsp, 16),
    ]);
    assert_contains(
        &compiled.code,
        &setup,
        "MakeValueCell must pass its input in RSI after setting the thread context",
    );
    assert_call_safepoint(&compiled, 0x5000, FLAG_CALL);
}

#[test]
fn x86_64_alloc_and_safepoint_emit_fast_and_slow_paths() {
    let mut allocation_builder = FunctionBuilder::new(
        ncl_ir::FunctionId(238),
        "allocation-fast-and-slow-paths",
        Vec::new(),
        vec![Ty::Address],
    );
    let allocation = allocation_builder
        .push_op(OpKind::Alloc { words: 3 }, &[Ty::Address])
        .expect("allocation result")[0];
    allocation_builder
        .terminate(Terminator::Return {
            values: vec![allocation],
        })
        .expect("allocation return");
    let allocation_compiled =
        compile_function_x86_64(&allocation_builder.finish(), &LoweringCoverageAbi)
            .expect("allocation lowers");

    let mut safepoint_builder = FunctionBuilder::new(
        ncl_ir::FunctionId(239),
        "explicit-safepoint-slow-path",
        Vec::new(),
        Vec::new(),
    );
    safepoint_builder
        .push_op(OpKind::Safepoint, &[])
        .expect("explicit safepoint");
    safepoint_builder
        .terminate(Terminator::Return { values: Vec::new() })
        .expect("allocation return");
    let safepoint_compiled =
        compile_function_x86_64(&safepoint_builder.finish(), &LoweringCoverageAbi)
            .expect("explicit safepoint lowers");
    let layout = ncl_sys::thread_layout();
    let tlab_bump = i32::try_from(layout.tlab_bump).expect("TLAB bump offset fits");
    let tlab_limit = i32::try_from(layout.tlab_limit).expect("TLAB limit offset fits");
    let allocation_fast_path = encode([
        Inst::MovRM(Reg::R10, Mem::base(Reg::R15, tlab_bump)),
        Inst::MovRM(Reg::R11, Mem::base(Reg::R15, tlab_limit)),
        Inst::Lea(Reg::Rax, Mem::base(Reg::R10, 24)),
        Inst::CmpRR(Reg::Rax, Reg::R11),
    ]);
    assert_contains(
        &allocation_compiled.code,
        &allocation_fast_path,
        "three-word allocation must compute its end and compare it with the TLAB limit",
    );
    assert_contains(
        &allocation_compiled.code,
        &encode([Inst::MovMR(Mem::base(Reg::R15, tlab_bump), Reg::Rax)]),
        "the successful allocation path must publish the advanced TLAB bump pointer",
    );
    assert_contains(
        &allocation_compiled.code,
        &encode([
            Inst::MovRR(Reg::Rdi, Reg::R15),
            Inst::MovRI(Reg::Rsi, Imm::I32(3)),
            Inst::MovRI(Reg::R11, Imm::I32(0x2000)),
            Inst::CallReg(Reg::R11),
        ]),
        "the allocation slow path must pass its word count to AllocateSlow",
    );
    let safepoint_request =
        i32::try_from(layout.safepoint_request).expect("safepoint request offset fits");
    assert_contains(
        &safepoint_compiled.code,
        &encode([
            Inst::MovRM(Reg::R10, Mem::base(Reg::R15, safepoint_request)),
            Inst::CmpRI(Reg::R10, 0),
        ]),
        "an explicit safepoint must test the thread request field before calling the slow path",
    );
    assert_contains(
        &safepoint_compiled.code,
        &encode([
            Inst::MovRR(Reg::Rdi, Reg::R15),
            Inst::MovRR(Reg::Rsi, Reg::Rbp),
            Inst::MovRI(Reg::R11, Imm::I32(0x3000)),
            Inst::Lea(Reg::Rdx, Mem::rip(3)),
            Inst::CallReg(Reg::R11),
        ]),
        "the safepoint slow path must pass context, frame pointer, and its continuation PC",
    );
    assert_call_safepoint(&allocation_compiled, 0x2000, FLAG_ALLOCATION_SLOW);
    assert_call_safepoint(&safepoint_compiled, 0x3000, FLAG_CALL);
}
