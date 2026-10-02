use super::*;
use crate::{Aarch64Abi, CodegenError};
use ncl_asm_aarch64::{Assembler, Inst, MemOperand, Reg, RegOrSp, Shift};
use ncl_ir::{BasicBlock, Constant, FunctionBuilder, FunctionId, OpKind, Param, Terminator, Ty};

#[allow(clippy::expect_used)]
fn encoded(instructions: impl IntoIterator<Item = Inst>) -> Vec<u8> {
    let mut assembler = Assembler::new();
    for instruction in instructions {
        assembler.emit(&instruction).expect("instruction encoding");
    }
    assembler.finish().expect("instruction encoding").bytes
}

fn function(params: Vec<Param>, terminator: Terminator) -> Function {
    Function {
        id: FunctionId(1),
        name: "target-boundary-test".into(),
        params,
        return_types: Vec::new(),
        blocks: vec![BasicBlock {
            id: ncl_ir::BlockId(0),
            params: Vec::new(),
            ops: Vec::new(),
            terminator,
        }],
        locals: Vec::new(),
        constants: Vec::new(),
        handler_regions: Vec::new(),
        debug: Vec::new(),
    }
}

#[test]
fn aarch64_compile_rejects_empty_and_unknown_control_flow_targets() {
    let empty = Function {
        id: FunctionId(0),
        name: "empty".into(),
        params: Vec::new(),
        return_types: Vec::new(),
        blocks: Vec::new(),
        locals: Vec::new(),
        constants: Vec::new(),
        handler_regions: Vec::new(),
        debug: Vec::new(),
    };
    assert_eq!(
        compile_function_aarch64(&empty, &Aarch64Abi),
        Err(CodegenError::EmptyFunction)
    );

    let jump = function(
        Vec::new(),
        Terminator::Jump {
            target: ncl_ir::BlockId(9),
            args: Vec::new(),
        },
    );
    assert_eq!(
        compile_function_aarch64(&jump, &Aarch64Abi),
        Err(CodegenError::UnknownBlock(ncl_ir::BlockId(9)))
    );

    let branch = function(
        vec![Param {
            name: "condition".into(),
            ty: Ty::Word,
        }],
        Terminator::Branch {
            condition: ncl_ir::ValueId(0),
            then_target: ncl_ir::BlockId(9),
            then_args: Vec::new(),
            else_target: ncl_ir::BlockId(0),
            else_args: Vec::new(),
        },
    );
    assert_eq!(
        compile_function_aarch64(&branch, &Aarch64Abi),
        Err(CodegenError::UnknownBlock(ncl_ir::BlockId(9)))
    );
}

#[test]
#[allow(clippy::expect_used)]
fn aarch64_large_frame_tail_transfers_emit_the_dynamic_epilogues() {
    for (function_id, tail_call) in [(FunctionId(2), false), (FunctionId(3), true)] {
        let mut builder = FunctionBuilder::new(
            function_id,
            if tail_call {
                "large-frame-tail-call"
            } else {
                "large-frame-call-return"
            },
            std::iter::once(Param {
                name: "callee".into(),
                ty: Ty::Address,
            })
            .chain((0..599).map(|index| Param {
                name: format!("value-{index}"),
                ty: Ty::Word,
            }))
            .collect(),
            Vec::new(),
        );
        let argc = builder.add_constant(Constant::Fixnum(0));
        let argc = builder
            .push_op(OpKind::Const { result: argc }, &[Ty::Word])
            .expect("argc")[0];
        builder
            .terminate(if tail_call {
                Terminator::TailCall {
                    function: ncl_ir::ValueId(0),
                    args: vec![argc],
                }
            } else {
                Terminator::CallReturn {
                    function: ncl_ir::ValueId(0),
                    args: vec![argc],
                }
            })
            .expect("terminal call");
        let compiled = compile_function_aarch64(&builder.finish(), &Aarch64Abi)
            .expect("large-frame terminal call");
        let body_bytes = compiled.frame_size - 32;
        assert!(body_bytes > 4095);
        let mut expected = ncl_asm_aarch64::mov_imm64(Reg(16), u64::from(body_bytes));
        expected.extend([
            Inst::Add {
                rd: RegOrSp::Sp,
                rn: RegOrSp::Sp,
                rm: Reg(16),
                shift: Shift::Lsl(0),
            },
            Inst::Ldp {
                rt: Reg(29),
                rt2: Reg(30),
                mem: MemOperand::PostIndex {
                    base: RegOrSp::Sp,
                    offset: 32,
                },
            },
        ]);
        expected.push(if tail_call {
            Inst::Br { rn: Reg(17) }
        } else {
            Inst::Ret { rn: Reg(30) }
        });
        assert!(compiled.code.ends_with(&encoded(expected)));
    }
}

#[test]
#[allow(clippy::expect_used)]
fn aarch64_small_frame_tail_transfers_emit_the_fixed_epilogues() {
    for (function_id, tail_call) in [(FunctionId(4), false), (FunctionId(5), true)] {
        let mut builder = FunctionBuilder::new(
            function_id,
            if tail_call {
                "small-frame-tail-call"
            } else {
                "small-frame-call-return"
            },
            std::iter::once(Param {
                name: "callee".into(),
                ty: Ty::Address,
            })
            .chain((0..20).map(|index| Param {
                name: format!("value-{index}"),
                ty: Ty::Word,
            }))
            .collect(),
            Vec::new(),
        );
        let argc = builder.add_constant(Constant::Fixnum(0));
        let argc = builder
            .push_op(OpKind::Const { result: argc }, &[Ty::Word])
            .expect("argc")[0];
        builder
            .terminate(if tail_call {
                Terminator::TailCall {
                    function: ncl_ir::ValueId(0),
                    args: vec![argc],
                }
            } else {
                Terminator::CallReturn {
                    function: ncl_ir::ValueId(0),
                    args: vec![argc],
                }
            })
            .expect("terminal call");
        let compiled = compile_function_aarch64(&builder.finish(), &Aarch64Abi)
            .expect("small-frame terminal call");
        let body_bytes = compiled.frame_size - 32;
        assert!(body_bytes > 0 && body_bytes <= 4095);
        let mut expected = vec![Inst::AddImm {
            rd: RegOrSp::Sp,
            rn: RegOrSp::Sp,
            imm: u16::try_from(body_bytes).expect("small frame"),
            shift: false,
        }];
        expected.push(Inst::Ldp {
            rt: Reg(29),
            rt2: Reg(30),
            mem: MemOperand::PostIndex {
                base: RegOrSp::Sp,
                offset: 32,
            },
        });
        expected.push(if tail_call {
            Inst::Br { rn: Reg(17) }
        } else {
            Inst::Ret { rn: Reg(30) }
        });
        assert!(compiled.code.ends_with(&encoded(expected)));
    }
}
