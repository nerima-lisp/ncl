//! Target-independent target contracts and the `AArch64` fixed-template backend.

use crate::templates::TemplateKind;
use crate::{CodegenError, CompiledFunction, FrameLayout, RuntimeAbi, SafepointMap};
use crate::{FLAG_ALLOCATION_SLOW, FLAG_CALL, FLAG_LOOP_BACKEDGE};
use ncl_asm_aarch64::{Assembler, Cond, Inst, MemOperand, Reg, RegOrSp, Shift};
use ncl_ir::{Compare, Function, Op, OpKind, Prim, Terminator, ValueId};

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

fn slots(function: &Function) -> (Vec<(ValueId, u32)>, u32) {
    let mut result = Vec::new();
    let mut next = 0_u32;
    for block in &function.blocks {
        for parameter in &block.params {
            result.push((parameter.value, next));
            next = next.saturating_add(1);
        }
        for op in &block.ops {
            for (value, _) in &op.results {
                result.push((*value, next));
                next = next.saturating_add(1);
            }
        }
    }
    (result, next)
}

fn slot(slots: &[(ValueId, u32)], value: ValueId) -> Result<u32, CodegenError> {
    slots
        .iter()
        .find(|(id, _)| *id == value)
        .map(|(_, index)| *index)
        .ok_or(CodegenError::UnknownValue(value))
}

fn slot_mem(slots: &[(ValueId, u32)], value: ValueId) -> Result<MemOperand, CodegenError> {
    let index = slot(slots, value)?;
    let offset = i16::try_from((index.saturating_add(1)).saturating_mul(8))
        .map_err(|_| CodegenError::FrameOverflow)?;
    Ok(MemOperand::Unscaled {
        base: RegOrSp::Reg(Reg(29)),
        offset: -offset,
    })
}

fn load_slot(
    assembler: &mut Assembler,
    slots: &[(ValueId, u32)],
    value: ValueId,
    register: Reg,
) -> Result<(), CodegenError> {
    emit(
        assembler,
        Inst::Ldr {
            rt: register,
            mem: slot_mem(slots, value)?,
        },
    )
}

fn store_slot(
    assembler: &mut Assembler,
    slots: &[(ValueId, u32)],
    value: ValueId,
    register: Reg,
) -> Result<(), CodegenError> {
    emit(
        assembler,
        Inst::Str {
            rt: register,
            mem: slot_mem(slots, value)?,
        },
    )
}

fn constant_word(constant: &ncl_ir::Constant, abi: &dyn RuntimeAbi) -> Result<u64, CodegenError> {
    match constant {
        ncl_ir::Constant::Fixnum(value) => Ok(abi.encode_fixnum(*value).cast_unsigned()),
        ncl_ir::Constant::Character(value) => Ok(abi.encode_character(*value).cast_unsigned()),
        ncl_ir::Constant::Nil | ncl_ir::Constant::Unbound => Ok(0),
        ncl_ir::Constant::T => Ok(abi.encode_fixnum(1).cast_unsigned()),
        _ => Err(CodegenError::Unsupported(
            "constant requires a runtime table".into(),
        )),
    }
}

fn compare_condition(op: Compare) -> Cond {
    match op {
        Compare::Eq => Cond::Eq,
        Compare::Ne => Cond::Ne,
        Compare::Lt => Cond::Lt,
        Compare::Le => Cond::Le,
        Compare::Gt => Cond::Gt,
        Compare::Ge => Cond::Ge,
    }
}

fn lower_prim(
    assembler: &mut Assembler,
    prim: &Prim,
    args: &[ValueId],
    result: Option<ValueId>,
    slots: &[(ValueId, u32)],
) -> Result<(), CodegenError> {
    let Some(first) = args.first() else {
        return Err(CodegenError::Unsupported(
            "primitive has no operands".into(),
        ));
    };
    load_slot(assembler, slots, *first, Reg(16))?;
    if let Some(second) = args.get(1) {
        load_slot(assembler, slots, *second, Reg(17))?;
    }
    match prim {
        Prim::FixnumAdd => emit(
            assembler,
            Inst::Add {
                rd: Reg(16).into(),
                rn: Reg(16).into(),
                rm: Reg(17),
                shift: Shift::Lsl(0),
            },
        )?,
        Prim::FixnumSub => emit(
            assembler,
            Inst::Sub {
                rd: Reg(16).into(),
                rn: Reg(16).into(),
                rm: Reg(17),
                shift: Shift::Lsl(0),
            },
        )?,
        Prim::FixnumMul => emit(
            assembler,
            Inst::Mul {
                rd: Reg(16),
                rn: Reg(16),
                rm: Reg(17),
            },
        )?,
        Prim::FixnumEq | Prim::Eq | Prim::Eql => {
            emit(
                assembler,
                Inst::Cmp {
                    rn: Reg(16),
                    rm: Reg(17),
                    shift: Shift::Lsl(0),
                },
            )?;
            emit(
                assembler,
                Inst::Cset {
                    rd: Reg(16),
                    cond: Cond::Eq,
                },
            )?;
        }
        Prim::FixnumLt => {
            emit(
                assembler,
                Inst::Cmp {
                    rn: Reg(16),
                    rm: Reg(17),
                    shift: Shift::Lsl(0),
                },
            )?;
            emit(
                assembler,
                Inst::Cset {
                    rd: Reg(16),
                    cond: Cond::Lt,
                },
            )?;
        }
        Prim::FixnumLe => {
            emit(
                assembler,
                Inst::Cmp {
                    rn: Reg(16),
                    rm: Reg(17),
                    shift: Shift::Lsl(0),
                },
            )?;
            emit(
                assembler,
                Inst::Cset {
                    rd: Reg(16),
                    cond: Cond::Le,
                },
            )?;
        }
        Prim::Car | Prim::Cdr | Prim::Svref | Prim::Aref => {
            let offset = if matches!(prim, Prim::Cdr) { 8 } else { 0 };
            emit(
                assembler,
                Inst::Ldr {
                    rt: Reg(16),
                    mem: MemOperand::Unscaled {
                        base: RegOrSp::Reg(Reg(16)),
                        offset,
                    },
                },
            )?;
        }
        Prim::Rplaca | Prim::Rplacd | Prim::Aset => {
            let offset = if matches!(prim, Prim::Rplacd) { 8 } else { 0 };
            emit(
                assembler,
                Inst::Str {
                    rt: Reg(17),
                    mem: MemOperand::Unscaled {
                        base: RegOrSp::Reg(Reg(16)),
                        offset,
                    },
                },
            )?;
        }
        _ => {
            return Err(CodegenError::Unsupported(format!(
                "primitive is not available: {prim:?}"
            )));
        }
    }
    if let Some(result) = result {
        store_slot(assembler, slots, result, Reg(16))?;
    }
    Ok(())
}

fn lower_op(
    assembler: &mut Assembler,
    op: &Op,
    function: &Function,
    slots: &[(ValueId, u32)],
    abi: &dyn RuntimeAbi,
) -> Result<(), CodegenError> {
    let result = op.results.first().map(|(value, _)| *value);
    match &op.kind {
        OpKind::Const { result: constant } => {
            let value = function
                .constants
                .get(constant.0 as usize)
                .ok_or_else(|| CodegenError::Unsupported("constant index out of range".into()))?;
            for instruction in ncl_asm_aarch64::mov_imm64(Reg(16), constant_word(value, abi)?) {
                emit(assembler, instruction)?;
            }
            if let Some(result) = result {
                store_slot(assembler, slots, result, Reg(16))?;
            }
        }
        OpKind::Move { value } | OpKind::Convert { value, .. } => {
            load_slot(assembler, slots, *value, Reg(16))?;
            if let Some(result) = result {
                store_slot(assembler, slots, result, Reg(16))?;
            }
        }
        OpKind::Load { address }
        | OpKind::LoadField {
            object: address, ..
        } => {
            load_slot(assembler, slots, *address, Reg(16))?;
            let offset = match &op.kind {
                OpKind::LoadField { field, .. } => i16::try_from(field.saturating_mul(8))
                    .map_err(|_| CodegenError::FrameOverflow)?,
                _ => 0,
            };
            emit(
                assembler,
                Inst::Ldr {
                    rt: Reg(16),
                    mem: MemOperand::Unscaled {
                        base: RegOrSp::Reg(Reg(16)),
                        offset,
                    },
                },
            )?;
            if let Some(result) = result {
                store_slot(assembler, slots, result, Reg(16))?;
            }
        }
        OpKind::Store { address, value }
        | OpKind::StoreField {
            object: address,
            value,
            ..
        } => {
            load_slot(assembler, slots, *address, Reg(16))?;
            load_slot(assembler, slots, *value, Reg(17))?;
            let offset = match &op.kind {
                OpKind::StoreField { field, .. } => i16::try_from(field.saturating_mul(8))
                    .map_err(|_| CodegenError::FrameOverflow)?,
                _ => 0,
            };
            emit(
                assembler,
                Inst::Str {
                    rt: Reg(17),
                    mem: MemOperand::Unscaled {
                        base: RegOrSp::Reg(Reg(16)),
                        offset,
                    },
                },
            )?;
        }
        OpKind::LoadArg { index } => {
            let source = match *index {
                0..=3 => Reg(1 + *index),
                _ => Reg(5),
            };
            if let Some(result) = result {
                emit(
                    assembler,
                    Inst::Mov {
                        rd: RegOrSp::Reg(Reg(16)),
                        rn: RegOrSp::Reg(source),
                    },
                )?;
                store_slot(assembler, slots, result, Reg(16))?;
            }
        }
        OpKind::Prim { op, args, .. } => lower_prim(assembler, op, args, result, slots)?,
        OpKind::Compare { op, left, right } => {
            load_slot(assembler, slots, *left, Reg(16))?;
            load_slot(assembler, slots, *right, Reg(17))?;
            emit(
                assembler,
                Inst::Cmp {
                    rn: Reg(16),
                    rm: Reg(17),
                    shift: Shift::Lsl(0),
                },
            )?;
            if let Some(result) = result {
                emit(
                    assembler,
                    Inst::Cset {
                        rd: Reg(16),
                        cond: compare_condition(*op),
                    },
                )?;
                store_slot(assembler, slots, result, Reg(16))?;
            }
        }
        OpKind::SetMultipleValues { values } => {
            for instruction in ncl_asm_aarch64::mov_imm64(Reg(1), values.len() as u64) {
                emit(assembler, instruction)?;
            }
            if let (Some(first), Some(result)) = (values.first(), result) {
                load_slot(assembler, slots, *first, Reg(16))?;
                store_slot(assembler, slots, result, Reg(16))?;
            }
        }
        OpKind::Alloc { .. } | OpKind::Safepoint => emit(assembler, Inst::Nop)?,
        OpKind::Call { .. } | OpKind::CallIndirect { .. } | OpKind::Builtin { .. } => {
            emit(assembler, Inst::Blr { rn: Reg(17) })?
        }
    }
    Ok(())
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
    let (value_slots, local_words) = slots(function);
    let frame = FrameLayout::new(
        u32::try_from(function.params.len()).map_err(|_| CodegenError::FrameOverflow)?,
        local_words,
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
    let body_bytes = frame.size_bytes().saturating_sub(16);
    if body_bytes > 0 {
        emit(
            &mut assembler,
            Inst::SubImm {
                rd: RegOrSp::Sp,
                rn: RegOrSp::Sp,
                imm: u16::try_from(body_bytes).map_err(|_| CodegenError::FrameOverflow)?,
                shift: false,
            },
        )?;
    }
    pc = pc.saturating_add(8 + if body_bytes > 0 { 4 } else { 0 });
    for block in &function.blocks {
        assembler
            .bind(labels[&block.id])
            .map_err(|error| CodegenError::Encode(error.to_string()))?;
        for op in &block.ops {
            lower_op(&mut assembler, op, function, &value_slots, abi)?;
            pc = pc.saturating_add(4);
            if matches!(op.kind, OpKind::Alloc { .. }) {
                add_map(&mut maps, pc, frame, FLAG_ALLOCATION_SLOW)?;
            } else if matches!(
                op.kind,
                OpKind::Call { .. }
                    | OpKind::CallIndirect { .. }
                    | OpKind::Builtin { .. }
                    | OpKind::Safepoint
            ) {
                add_map(&mut maps, pc, frame, FLAG_CALL)?;
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
            Terminator::Return { values } => {
                if let Some(value) = values.first() {
                    load_slot(&mut assembler, &value_slots, *value, Reg(0))?;
                } else {
                    for instruction in
                        ncl_asm_aarch64::mov_imm64(Reg(0), abi.encode_fixnum(0).cast_unsigned())
                    {
                        emit(&mut assembler, instruction)?;
                    }
                }
                for instruction in ncl_asm_aarch64::mov_imm64(
                    Reg(1),
                    u64::try_from(values.len()).map_err(|_| CodegenError::FrameOverflow)?,
                ) {
                    emit(&mut assembler, instruction)?;
                    pc = pc.saturating_add(4);
                }
                if body_bytes > 0 {
                    emit(
                        &mut assembler,
                        Inst::AddImm {
                            rd: RegOrSp::Sp,
                            rn: RegOrSp::Sp,
                            imm: u16::try_from(body_bytes)
                                .map_err(|_| CodegenError::FrameOverflow)?,
                            shift: false,
                        },
                    )?;
                }
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
