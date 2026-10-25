use crate::tests_x86_64_fixture::X86_64FixtureAbi;
use crate::{AllocationTarget, Location, allocate, compile_function_x86_64};
use ncl_asm_x86_64::{Assembler, BinOp, Cond, Imm, Inst, Reg};
use ncl_ir::{Constant, Convert, FunctionBuilder, OpKind, Param, Terminator, Ty};
use ncl_sys::{Thread, alloc_code, invoke_entry, publish_code};

const CALLEE_SAVED_SENTINELS: [i64; 3] = [
    0x1122_3344_5566_7788,
    0x2233_4455_6677_8899,
    0x3344_5566_7788_99aa,
];

fn emit(assembler: &mut Assembler, instruction: &Inst) -> Result<(), String> {
    assembler
        .emit(instruction)
        .map_err(|error| format!("encode ABI probe: {error:?}"))
}

fn abi_probe(entry: usize, argc: i32, first_argument: Option<usize>) -> Result<Vec<u8>, String> {
    let entry = i64::try_from(entry).map_err(|_| "generated entry address exceeds i64")?;
    let first_argument = match first_argument {
        Some(argument) => {
            i64::try_from(argument).map_err(|_| "first argument address exceeds i64")?
        }
        None => 0,
    };
    let mut assembler = Assembler::new();
    let failed = assembler.new_label();
    let done = assembler.new_label();

    for instruction in [
        Inst::Push(Reg::Rbp),
        Inst::MovRR(Reg::Rbp, Reg::Rsp),
        Inst::Push(Reg::R12),
        Inst::Push(Reg::R13),
        Inst::Push(Reg::R14),
        Inst::BinRI(BinOp::Sub, Reg::Rsp, 24),
        Inst::MovRI(Reg::R12, Imm::I64(CALLEE_SAVED_SENTINELS[0])),
        Inst::MovRI(Reg::R13, Imm::I64(CALLEE_SAVED_SENTINELS[1])),
        Inst::MovRI(Reg::R14, Imm::I64(CALLEE_SAVED_SENTINELS[2])),
        Inst::MovRI(Reg::Rdi, Imm::I32(argc)),
        Inst::MovRI(Reg::Rsi, Imm::I64(first_argument)),
        Inst::MovRI(Reg::R10, Imm::I32(0)),
        Inst::MovRI(Reg::R11, Imm::I64(entry)),
        Inst::CallReg(Reg::R11),
        Inst::BinRI(BinOp::Add, Reg::Rsp, 24),
        Inst::MovRI(Reg::Rax, Imm::I64(CALLEE_SAVED_SENTINELS[0])),
        Inst::CmpRR(Reg::R12, Reg::Rax),
        Inst::Jcc(Cond::Ne, failed),
        Inst::MovRI(Reg::Rax, Imm::I64(CALLEE_SAVED_SENTINELS[1])),
        Inst::CmpRR(Reg::R13, Reg::Rax),
        Inst::Jcc(Cond::Ne, failed),
        Inst::MovRI(Reg::Rax, Imm::I64(CALLEE_SAVED_SENTINELS[2])),
        Inst::CmpRR(Reg::R14, Reg::Rax),
        Inst::Jcc(Cond::Ne, failed),
        Inst::MovRI(Reg::Rax, Imm::I32(1)),
        Inst::Jmp(done),
    ] {
        emit(&mut assembler, &instruction)?;
    }
    assembler.bind(failed);
    emit(&mut assembler, &Inst::MovRI(Reg::Rax, Imm::I32(0)))?;
    assembler.bind(done);
    for instruction in [
        Inst::Pop(Reg::R14),
        Inst::Pop(Reg::R13),
        Inst::Pop(Reg::R12),
        Inst::Pop(Reg::Rbp),
        Inst::Ret,
    ] {
        emit(&mut assembler, &instruction)?;
    }
    assembler
        .finish()
        .map(|blob| blob.bytes)
        .map_err(|error| format!("finish ABI probe: {error:?}"))
}

fn caller_function(function_id: u32, tail: bool) -> Result<ncl_ir::Function, String> {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(function_id),
        "callee-saved-transfer",
        vec![Param {
            name: "callee".into(),
            ty: Ty::Word,
        }],
        vec![Ty::Word; 3],
    );
    let callee = builder
        .push_op(OpKind::LoadArg { index: 0 }, &[Ty::Word])
        .map_err(|error| format!("load transfer callee: {error:?}"))?
        .first()
        .copied()
        .ok_or_else(|| "transfer callee value missing".to_owned())?;
    let mut args = Vec::new();
    for value in [3_i64, 11, 22, 33] {
        let constant = builder.add_constant(Constant::Fixnum(value));
        let integer = builder
            .push_op(OpKind::Const { result: constant }, &[Ty::I64])
            .map_err(|error| format!("add transfer argument: {error:?}"))?
            .first()
            .copied()
            .ok_or_else(|| "transfer argument value missing".to_owned())?;
        args.push(
            builder
                .push_op(
                    OpKind::Convert {
                        op: Convert::I64ToWord,
                        value: integer,
                    },
                    &[Ty::Word],
                )
                .map_err(|error| format!("convert transfer argument: {error:?}"))?
                .first()
                .copied()
                .ok_or_else(|| "converted transfer argument value missing".to_owned())?,
        );
    }
    let terminator = if tail {
        Terminator::TailCall {
            function: callee,
            args,
        }
    } else {
        Terminator::CallReturn {
            function: callee,
            args,
        }
    };
    builder
        .terminate(terminator)
        .map_err(|error| format!("terminate transfer caller: {error:?}"))?;
    let function = builder.finish();
    ncl_ir::verify(&function).map_err(|errors| format!("verify transfer caller: {errors:?}"))?;
    Ok(function)
}

fn run_probe(entry: usize, argc: i32, first_argument: Option<usize>) -> Result<(u64, u64), String> {
    let probe = abi_probe(entry, argc, first_argument)?;
    let mut probe_code =
        alloc_code(probe.len()).map_err(|error| format!("allocate ABI probe: {error:?}"))?;
    probe_code
        .write_code(0, &probe)
        .map_err(|error| format!("write ABI probe: {error:?}"))?;
    publish_code(&mut probe_code).map_err(|error| format!("publish ABI probe: {error:?}"))?;

    let mut thread = Thread::new();
    Ok(invoke_entry(&probe_code, 0, &raw mut thread, 0, [0; 4], 0))
}

#[test]
fn generated_code_preserves_sysv_callee_saved_registers() -> Result<(), String> {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(90),
        "callee-saved-registers",
        Vec::new(),
        vec![Ty::Word; 3],
    );
    let mut values = Vec::new();
    for constant in [Constant::Nil, Constant::T, Constant::Unbound] {
        let constant = builder.add_constant(constant);
        let results = builder
            .push_op(OpKind::Const { result: constant }, &[Ty::Word])
            .map_err(|error| format!("add test constant: {error:?}"))?;
        let Some(result) = results.first().copied() else {
            return Err("constant produced no value".to_owned());
        };
        values.push(result);
    }
    builder
        .terminate(Terminator::Return {
            values: values.clone(),
        })
        .map_err(|error| format!("terminate test function: {error:?}"))?;
    let function = builder.finish();
    ncl_ir::verify(&function).map_err(|errors| format!("verify ABI test function: {errors:?}"))?;

    let allocation = allocate(&function, AllocationTarget::X86_64);
    let allocated_registers = values
        .iter()
        .map(|value| allocation.location(*value))
        .collect::<Vec<_>>();
    assert_eq!(
        allocated_registers,
        [
            Some(Location::Register(u16::from(Reg::R12.id()))),
            Some(Location::Register(u16::from(Reg::R13.id()))),
            Some(Location::Register(u16::from(Reg::R14.id()))),
        ],
        "the generated function must exercise R12-R14"
    );

    let compiled = compile_function_x86_64(&function, &X86_64FixtureAbi)
        .map_err(|error| format!("compile ABI test function: {error:?}"))?;
    let mut generated_code = alloc_code(compiled.code.len())
        .map_err(|error| format!("allocate generated code: {error:?}"))?;
    generated_code
        .write_code(0, &compiled.code)
        .map_err(|error| format!("write generated code: {error:?}"))?;
    publish_code(&mut generated_code)
        .map_err(|error| format!("publish generated code: {error:?}"))?;

    let (preserved, count) = run_probe(generated_code.address(), 0, None)?;
    assert_eq!(preserved, 1, "generated code changed R12, R13, or R14");
    assert_eq!(
        count, 3,
        "the generated function did not return three values"
    );

    for (function_id, tail) in [(91, false), (92, true)] {
        let caller = caller_function(function_id, tail)?;
        let allocation = allocate(&caller, AllocationTarget::X86_64);
        let entry_value = caller
            .blocks
            .iter()
            .flat_map(|block| &block.ops)
            .find(|op| matches!(&op.kind, OpKind::LoadArg { index: 0 }))
            .and_then(|op| op.results.first())
            .map(|(value, _)| *value)
            .ok_or_else(|| "transfer callee allocation missing".to_owned())?;
        assert!(
            matches!(
                allocation.location(entry_value),
                Some(Location::Register(register))
                    if (u16::from(Reg::R12.id())..=u16::from(Reg::R14.id())).contains(&register)
            ),
            "transfer caller must exercise a callee-saved register"
        );
        let compiled_caller = compile_function_x86_64(&caller, &X86_64FixtureAbi)
            .map_err(|error| format!("compile transfer caller: {error:?}"))?;
        let mut caller_code = alloc_code(compiled_caller.code.len())
            .map_err(|error| format!("allocate transfer caller: {error:?}"))?;
        caller_code
            .write_code(0, &compiled_caller.code)
            .map_err(|error| format!("write transfer caller: {error:?}"))?;
        publish_code(&mut caller_code)
            .map_err(|error| format!("publish transfer caller: {error:?}"))?;
        let (preserved, count) =
            run_probe(caller_code.address(), 1, Some(generated_code.address()))?;
        assert_eq!(
            preserved, 1,
            "call or tail transfer changed callee-saved registers"
        );
        assert_eq!(count, 3, "transfer callee did not return three values");
    }
    Ok(())
}
