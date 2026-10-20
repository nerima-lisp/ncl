use super::*;
use ncl_asm_aarch64::{Assembler, Cond, Inst, Label, MemOperand, Reg, RegOrSp, Shift};

fn emit_probe(assembler: &mut Assembler, instruction: &Inst) {
    assembler
        .emit(instruction)
        .expect("ABI probe instruction encoding");
}

fn save_probe_registers(assembler: &mut Assembler) {
    emit_probe(
        assembler,
        &Inst::Stp {
            rt: Reg(29),
            rt2: Reg(30),
            mem: MemOperand::PreIndex {
                base: RegOrSp::Sp,
                offset: -96,
            },
        },
    );
    for (first, second, offset) in [
        (Reg(19), Reg(20), 16),
        (Reg(21), Reg(22), 32),
        (Reg(23), Reg(24), 48),
        (Reg(25), Reg(26), 64),
        (Reg(27), Reg(28), 80),
    ] {
        emit_probe(
            assembler,
            &Inst::Stp {
                rt: first,
                rt2: second,
                mem: MemOperand::Unsigned {
                    base: RegOrSp::Sp,
                    offset,
                    scale: 8,
                },
            },
        );
    }
}

fn initialize_probe_registers(assembler: &mut Assembler) {
    emit_probe(
        assembler,
        &Inst::Mov {
            rd: RegOrSp::Reg(Reg(17)),
            rn: RegOrSp::Reg(Reg(1)),
        },
    );
    for register in [19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29] {
        for instruction in ncl_asm_aarch64::mov_imm64(Reg(register), u64::from(register)) {
            emit_probe(assembler, &instruction);
        }
    }
    for register in 0..=5 {
        for instruction in ncl_asm_aarch64::mov_imm64(Reg(register), 0) {
            emit_probe(assembler, &instruction);
        }
    }
}

fn call_probe_target(assembler: &mut Assembler) {
    emit_probe(assembler, &Inst::Blr { rn: Reg(17) });
    emit_probe(
        assembler,
        &Inst::Mov {
            rd: RegOrSp::Reg(Reg(16)),
            rn: RegOrSp::Reg(Reg(0)),
        },
    );
    emit_probe(
        assembler,
        &Inst::Mov {
            rd: RegOrSp::Reg(Reg(17)),
            rn: RegOrSp::Reg(Reg(1)),
        },
    );
}

fn check_probe_registers(assembler: &mut Assembler, failed: Label) {
    for register in [19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29] {
        for instruction in ncl_asm_aarch64::mov_imm64(Reg(18), u64::from(register)) {
            emit_probe(assembler, &instruction);
        }
        emit_probe(
            assembler,
            &Inst::Cmp {
                rn: Reg(register),
                rm: Reg(18),
                shift: Shift::Lsl(0),
            },
        );
        emit_probe(
            assembler,
            &Inst::BCond {
                cond: Cond::Ne,
                label: failed,
            },
        );
    }
}

fn emit_probe_result_paths(assembler: &mut Assembler, failed: Label, done: Label) {
    emit_probe(
        assembler,
        &Inst::Mov {
            rd: RegOrSp::Reg(Reg(0)),
            rn: RegOrSp::Reg(Reg(16)),
        },
    );
    emit_probe(
        assembler,
        &Inst::Mov {
            rd: RegOrSp::Reg(Reg(1)),
            rn: RegOrSp::Reg(Reg(17)),
        },
    );
    emit_probe(assembler, &Inst::B { label: done });
    assembler.bind(failed).expect("failed label");
    for register in [Reg(0), Reg(1)] {
        for instruction in ncl_asm_aarch64::mov_imm64(register, 0) {
            emit_probe(assembler, &instruction);
        }
    }
    assembler.bind(done).expect("done label");
}

fn restore_probe_registers(assembler: &mut Assembler) {
    for (first, second, offset) in [
        (Reg(27), Reg(28), 80),
        (Reg(25), Reg(26), 64),
        (Reg(23), Reg(24), 48),
        (Reg(21), Reg(22), 32),
        (Reg(19), Reg(20), 16),
    ] {
        emit_probe(
            assembler,
            &Inst::Ldp {
                rt: first,
                rt2: second,
                mem: MemOperand::Unsigned {
                    base: RegOrSp::Sp,
                    offset,
                    scale: 8,
                },
            },
        );
    }
    emit_probe(
        assembler,
        &Inst::Ldp {
            rt: Reg(29),
            rt2: Reg(30),
            mem: MemOperand::PostIndex {
                base: RegOrSp::Sp,
                offset: 96,
            },
        },
    );
    emit_probe(assembler, &Inst::Ret { rn: Reg(30) });
}

fn callee_saved_probe_bytes() -> Vec<u8> {
    let mut assembler = Assembler::new();
    let failed = assembler.new_label();
    let done = assembler.new_label();
    save_probe_registers(&mut assembler);
    initialize_probe_registers(&mut assembler);
    call_probe_target(&mut assembler);
    check_probe_registers(&mut assembler, failed);
    emit_probe_result_paths(&mut assembler, failed, done);
    restore_probe_registers(&mut assembler);
    assembler.finish().expect("ABI probe assembly").bytes
}

#[test]
fn generated_entry_preserves_aarch64_callee_saved_registers() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(104),
        "callee-saved-registers",
        Vec::new(),
        vec![Ty::Word],
    );
    let constant = builder.add_constant(Constant::Fixnum(73));
    let value = builder
        .push_op(OpKind::Const { result: constant }, &[Ty::Word])
        .expect("return value")[0];
    builder
        .terminate(Terminator::Return {
            values: vec![value],
        })
        .expect("return");
    let compiled = compile_function_aarch64(&builder.finish(), &Aarch64Abi)
        .expect("generated function lowering");
    let mut generated_code = alloc_code(compiled.code.len()).expect("generated code allocation");
    write_code(&mut generated_code, 0, &compiled.code).expect("generated code write");
    publish_code(&mut generated_code).expect("generated code publication");

    let probe_bytes = callee_saved_probe_bytes();
    let mut probe_code = alloc_code(probe_bytes.len()).expect("ABI probe code allocation");
    write_code(&mut probe_code, 0, &probe_bytes).expect("ABI probe code write");
    publish_code(&mut probe_code).expect("ABI probe code publication");
    let target_entry = generated_code.address() + compiled.entry_offset as usize;
    let mut thread = Thread::new();
    let result = invoke_entry(
        &probe_code,
        0,
        &mut thread,
        1,
        [target_entry as u64, 0, 0, 0],
        0,
    );
    assert_eq!(result, (Word::fixnum(73).bits(), 1));
}
