//! Target-independent target contracts and the `AArch64` fixed-template backend.

use crate::templates::TemplateKind;
use crate::{CodegenError, CompiledFunction, FrameLayout, RuntimeAbi, SafepointMap};
use crate::{FLAG_ALLOCATION_SLOW, FLAG_CALL, FLAG_LOOP_BACKEDGE};
use ncl_asm_aarch64::{Assembler, Cond, Inst, MemOperand, Reg, RegOrSp};
use ncl_ir::{Function, OpKind, Terminator};

/// A machine target capable of encoding one fixed-template operation.
pub trait TargetIsa: core::fmt::Debug {
    /// Stable target name.
    fn name(&self) -> &'static str;
    /// Encode a template skeleton for inspection and golden tests.
    ///
    /// # Errors
    ///
    /// Returns [`CodegenError`] when the target encoder rejects the skeleton.
    fn encode_template(&self, kind: TemplateKind) -> Result<Vec<u8>, CodegenError>;
}

/// `AArch64` target using x21 as the pinned context register.
#[derive(Clone, Copy, Debug, Default)]
pub struct AArch64TargetIsa;

impl AArch64TargetIsa {
    /// The pinned `ThreadContext` register.
    pub const CONTEXT: Reg = Reg(21);
    /// The two scratch registers reserved by stage 1a.
    pub const SCRATCH: [Reg; 2] = [Reg(16), Reg(17)];

    /// Return the fixed skeleton instruction sequence for a template.
    #[must_use]
    pub fn skeleton(kind: TemplateKind) -> Vec<Inst> {
        match kind {
            TemplateKind::Return => vec![Inst::Ret { rn: Reg(30) }],
            TemplateKind::Branch => vec![Inst::BCond {
                cond: Cond::Ne,
                label: ncl_asm_aarch64::Label(0),
            }],
            TemplateKind::Jump => vec![Inst::B {
                label: ncl_asm_aarch64::Label(0),
            }],
            TemplateKind::Call | TemplateKind::CallIndirect | TemplateKind::Builtin => {
                vec![Inst::Blr { rn: Reg(17) }]
            }
            TemplateKind::Alloc | TemplateKind::Safepoint => vec![Inst::Nop],
            _ => vec![Inst::Mov {
                rd: RegOrSp::Reg(Reg(16)),
                rn: RegOrSp::Reg(Reg(16)),
            }],
        }
    }
}

impl TargetIsa for AArch64TargetIsa {
    fn name(&self) -> &'static str {
        "aarch64"
    }

    fn encode_template(&self, kind: TemplateKind) -> Result<Vec<u8>, CodegenError> {
        let mut assembler = Assembler::new();
        let label = ncl_asm_aarch64::Label(0);
        for instruction in Self::skeleton(kind) {
            assembler
                .emit(&instruction)
                .map_err(|error| CodegenError::Encode(error.to_string()))?;
        }
        if matches!(kind, TemplateKind::Branch | TemplateKind::Jump) {
            assembler
                .bind(label)
                .map_err(|error| CodegenError::Encode(error.to_string()))?;
        }
        assembler
            .finish()
            .map(|blob| blob.bytes)
            .map_err(|error| CodegenError::Encode(error.to_string()))
    }
}

#[allow(clippy::needless_pass_by_value)]
fn emit(assembler: &mut Assembler, instruction: Inst) -> Result<(), CodegenError> {
    assembler
        .emit(&instruction)
        .map_err(|error| CodegenError::Encode(error.to_string()))
}

fn add_map(
    maps: &mut Vec<SafepointMap>,
    pc: u32,
    frame: FrameLayout,
    flags: u32,
) -> Result<(), CodegenError> {
    let slots = u16::try_from(frame.frame_words).map_err(|_| CodegenError::FrameOverflow)?;
    SafepointMap::new(pc, slots, slots, &[], &[0], flags)
        .map(|map| maps.push(map))
        .map_err(|error| CodegenError::Encode(error.to_string()))
}

/// Lower the fixed-template IR shape to `AArch64` code and metadata.
///
/// # Errors
///
/// Returns [`CodegenError`] when the IR is empty, a constant is unavailable,
/// or the target encoder rejects an instruction.
#[allow(clippy::too_many_lines)]
pub fn compile_function_aarch64(
    function: &Function,
    abi: &dyn RuntimeAbi,
) -> Result<CompiledFunction, CodegenError> {
    let Some(_entry) = function.blocks.first() else {
        return Err(CodegenError::EmptyFunction);
    };
    let frame = FrameLayout::new(
        u32::try_from(function.params.len()).map_err(|_| CodegenError::FrameOverflow)?,
        0,
        0,
    )?;
    let mut assembler = Assembler::new();
    let labels = function
        .blocks
        .iter()
        .map(|block| (block.id, assembler.new_label()))
        .collect::<std::collections::HashMap<_, _>>();
    let mut pc = 0_u32;
    let mut maps = Vec::new();
    emit(
        &mut assembler,
        Inst::Stp {
            rt: Reg(29),
            rt2: Reg(30),
            mem: MemOperand::PreIndex {
                base: RegOrSp::Sp,
                offset: -16,
            },
        },
    )?;
    emit(
        &mut assembler,
        Inst::Mov {
            rd: RegOrSp::Reg(Reg(29)),
            rn: RegOrSp::Sp,
        },
    )?;
    pc = pc.saturating_add(8);
    for block in &function.blocks {
        assembler
            .bind(labels[&block.id])
            .map_err(|error| CodegenError::Encode(error.to_string()))?;
        for op in &block.ops {
            match &op.kind {
                OpKind::Const { result } => {
                    let value = function.constants.get(result.0 as usize).ok_or_else(|| {
                        CodegenError::Unsupported("constant index out of range".into())
                    })?;
                    let word = match value {
                        ncl_ir::Constant::Fixnum(value) => {
                            abi.encode_fixnum(*value).cast_unsigned()
                        }
                        ncl_ir::Constant::Character(value) => {
                            abi.encode_character(*value).cast_unsigned()
                        }
                        ncl_ir::Constant::Nil | ncl_ir::Constant::Unbound => 0,
                        ncl_ir::Constant::T => abi.encode_fixnum(1).cast_unsigned(),
                        _ => {
                            return Err(CodegenError::Unsupported(
                                "constant requires a runtime table".into(),
                            ));
                        }
                    };
                    for instruction in ncl_asm_aarch64::mov_imm64(Reg(0), word) {
                        emit(&mut assembler, instruction)?;
                        pc = pc.saturating_add(4);
                    }
                }
                OpKind::Alloc { .. } => {
                    emit(&mut assembler, Inst::Nop)?;
                    pc = pc.saturating_add(4);
                    add_map(&mut maps, pc, frame, FLAG_ALLOCATION_SLOW)?;
                }
                OpKind::Call { .. } | OpKind::CallIndirect { .. } | OpKind::Builtin { .. } => {
                    emit(&mut assembler, Inst::Blr { rn: Reg(17) })?;
                    pc = pc.saturating_add(4);
                    add_map(&mut maps, pc, frame, FLAG_CALL)?;
                }
                OpKind::Safepoint => {
                    emit(&mut assembler, Inst::Nop)?;
                    pc = pc.saturating_add(4);
                    add_map(&mut maps, pc, frame, FLAG_CALL)?;
                }
                _ => {
                    emit(
                        &mut assembler,
                        Inst::Mov {
                            rd: RegOrSp::Reg(Reg(16)),
                            rn: RegOrSp::Reg(Reg(16)),
                        },
                    )?;
                    pc = pc.saturating_add(4);
                }
            }
        }
        match &block.terminator {
            Terminator::Jump { target, .. } => {
                emit(
                    &mut assembler,
                    Inst::B {
                        label: labels[target],
                    },
                )?;
                pc = pc.saturating_add(4);
                if *target == block.id {
                    add_map(&mut maps, pc, frame, FLAG_LOOP_BACKEDGE)?;
                }
            }
            Terminator::Branch {
                then_target,
                else_target,
                ..
            } => {
                emit(
                    &mut assembler,
                    Inst::BCond {
                        cond: Cond::Ne,
                        label: labels[then_target],
                    },
                )?;
                emit(
                    &mut assembler,
                    Inst::B {
                        label: labels[else_target],
                    },
                )?;
                pc = pc.saturating_add(8);
            }
            Terminator::Switch { default, .. } => {
                emit(
                    &mut assembler,
                    Inst::B {
                        label: labels[default],
                    },
                )?;
                pc = pc.saturating_add(4);
            }
            Terminator::Return { .. } => {
                emit(
                    &mut assembler,
                    Inst::Ldp {
                        rt: Reg(29),
                        rt2: Reg(30),
                        mem: MemOperand::PostIndex {
                            base: RegOrSp::Sp,
                            offset: 16,
                        },
                    },
                )?;
                emit(&mut assembler, Inst::Ret { rn: Reg(30) })?;
                pc = pc.saturating_add(8);
            }
            Terminator::CallReturn { .. } | Terminator::TailCall { .. } => {
                emit(&mut assembler, Inst::Blr { rn: Reg(17) })?;
                emit(&mut assembler, Inst::Ret { rn: Reg(30) })?;
                pc = pc.saturating_add(8);
                add_map(&mut maps, pc, frame, FLAG_CALL)?;
            }
            Terminator::Throw { .. } | Terminator::Unreachable => {
                emit(&mut assembler, Inst::Brk { imm: 0 })?;
                pc = pc.saturating_add(4);
            }
        }
    }
    let blob = assembler
        .finish()
        .map_err(|error| CodegenError::Encode(error.to_string()))?;
    Ok(CompiledFunction {
        code: blob.bytes,
        entry_offset: 0,
        relocations: Vec::new(),
        safepoint_maps: maps,
        frame_size: frame.size_bytes(),
        debug: Vec::new(),
    })
}
