//! Fixed-template x86-64 lowering.
use crate::{
    Block, CodegenError, CompiledFunction, FLAG_ALLOCATION_SLOW, FLAG_CALL, FLAG_LOOP_BACKEDGE,
    FrameLayout, MachineFunction, MachineOp, RuntimeAbi, SafepointMap,
};
use crate::{checked_i64, checked_u16, checked_u32};
use ncl_asm_x86_64::{Assembler, BinOp, Cond, Imm, Inst, Mem, Reg};
use ncl_ir::{Compare, Constant, Function, Op, OpKind, Prim, Terminator, ValueId};
use std::collections::{HashMap, HashSet};
fn slots(function: &Function) -> (Vec<(ValueId, u32)>, u32) {
    let mut result = Vec::new();
    let mut next = 4;
    for block in &function.blocks {
        for param in &block.params {
            result.push((param.value, next));
            next += 1;
        }
        for op in &block.ops {
            for (value, _) in &op.results {
                result.push((*value, next));
                next += 1;
            }
        }
    }
    (result, next - 4)
}
/// Lowers one validated IR function using fixed stack-slot templates.
///
/// # Errors
///
/// Returns an error when the function is empty, references an unknown IR
/// value or block, exceeds frame limits, or requires an unavailable runtime
/// operation.
pub fn compile_function(
    function: &Function,
    abi: &dyn RuntimeAbi,
) -> Result<CompiledFunction, CodegenError> {
    let Some(entry) = function.blocks.first() else {
        return Err(CodegenError::EmptyFunction);
    };
    let (value_slots, count) = slots(function);
    let arguments =
        u32::try_from(function.params.len()).map_err(|_| CodegenError::FrameOverflow)?;
    let frame = FrameLayout::new(arguments, count, 0)?;
    let known: HashSet<_> = function.blocks.iter().map(|block| block.id).collect();
    for block in &function.blocks {
        validate_targets(&block.terminator, &known)?;
    }
    let blocks = function
        .blocks
        .iter()
        .map(|block| Block {
            id: block.id,
            operations: block
                .ops
                .iter()
                .map(|op| {
                    op.results.first().map_or(MachineOp::Return, |(value, _)| {
                        let slot = value_slots
                            .iter()
                            .find(|(id, _)| id == value)
                            .map_or(0, |(_, slot)| *slot);
                        MachineOp::Move {
                            source: slot,
                            destination: slot,
                        }
                    })
                })
                .chain(
                    matches!(block.terminator, Terminator::Return { .. })
                        .then_some(MachineOp::Return),
                )
                .collect(),
            offset: 0,
        })
        .collect();
    encode(
        function,
        MachineFunction {
            entry: entry.id,
            blocks,
            frame,
            safepoints: Vec::new(),
            relocations: Vec::new(),
            slots: value_slots,
        },
        abi,
    )
}
fn validate_targets(
    terminator: &Terminator,
    known: &HashSet<ncl_ir::BlockId>,
) -> Result<(), CodegenError> {
    let check = |id| {
        known
            .contains(&id)
            .then_some(())
            .ok_or(CodegenError::UnknownBlock(id))
    };
    match terminator {
        Terminator::Jump { target, .. } => check(*target),
        Terminator::Branch {
            then_target,
            else_target,
            ..
        } => {
            check(*then_target)?;
            check(*else_target)
        }
        Terminator::Switch { cases, default, .. } => {
            check(*default)?;
            for (_, target, _) in cases {
                check(*target)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}
#[rustfmt::skip]
fn slot(slots: &[(ValueId, u32)], value: ValueId) -> Result<u32, CodegenError> { slots.iter().find(|(id, _)| *id == value).map(|(_, slot)| *slot).ok_or(CodegenError::UnknownValue(value)) }
#[rustfmt::skip]
fn memory(slot: u32) -> Result<Mem, CodegenError> { let bytes = slot.checked_add(1).and_then(|v| v.checked_mul(8)).ok_or(CodegenError::FrameOverflow)?; Ok(Mem::base(Reg::Rbp, -i32::try_from(bytes).map_err(|_| CodegenError::FrameOverflow)?)) }
#[rustfmt::skip]
fn emit(assembler: &mut Assembler, inst: &Inst) -> Result<(), CodegenError> { assembler.emit(inst).map_err(|error| CodegenError::Encode(error.to_string())) }
#[rustfmt::skip]
fn load(assembler: &mut Assembler, slots: &[(ValueId, u32)], value: ValueId, register: Reg) -> Result<(), CodegenError> { emit(assembler, &Inst::MovRM(register, memory(slot(slots, value)?)?)) }
#[rustfmt::skip]
fn store(assembler: &mut Assembler, slots: &[(ValueId, u32)], value: ValueId, register: Reg) -> Result<(), CodegenError> { emit(assembler, &Inst::MovMR(memory(slot(slots, value)?)?, register)) }
#[rustfmt::skip]
const fn compare_condition(op: Compare) -> Cond { match op { Compare::Eq => Cond::E, Compare::Ne => Cond::Ne, Compare::Lt => Cond::L, Compare::Le => Cond::Le, Compare::Gt => Cond::G, Compare::Ge => Cond::Ge } }
fn constant_value(constant: &Constant, abi: &dyn RuntimeAbi) -> Result<i64, CodegenError> {
    match constant {
        Constant::Fixnum(value) => Ok(abi.encode_fixnum(*value)),
        Constant::Character(value) => Ok(abi.encode_character(*value)),
        Constant::SingleFloat(value) => Ok(i64::from(value.to_bits())),
        Constant::DoubleFloat(value) => Ok(i64::from_ne_bytes(value.to_bits().to_ne_bytes())),
        Constant::Nil | Constant::Unbound => Ok(0),
        Constant::T => Ok(abi.encode_fixnum(1)),
        Constant::Symbol { .. } | Constant::Object(_) | Constant::StringBytes(_) => Err(
            CodegenError::Unsupported("constant requires a runtime constant table".into()),
        ),
    }
}
#[allow(clippy::too_many_lines)]
fn lower_op(
    assembler: &mut Assembler,
    op: &Op,
    function: &Function,
    slots: &[(ValueId, u32)],
    abi: &dyn RuntimeAbi,
    frame: FrameLayout,
    maps: &mut Vec<SafepointMap>,
) -> Result<(), CodegenError> {
    let result = op.results.first().map(|(value, _)| *value);
    match &op.kind {
        OpKind::Const { result: constant } => {
            let value = function
                .constants
                .get(constant.0 as usize)
                .ok_or_else(|| CodegenError::Unsupported("constant index out of range".into()))?;
            emit(
                assembler,
                &Inst::MovRI(Reg::R10, Imm::I64(constant_value(value, abi)?)),
            )?;
            if let Some(result) = result {
                store(assembler, slots, result, Reg::R10)?;
            }
        }
        OpKind::Move { value } | OpKind::Convert { value, .. } => {
            load(assembler, slots, *value, Reg::R10)?;
            if let Some(result) = result {
                store(assembler, slots, result, Reg::R10)?;
            }
        }
        OpKind::Load { address } => {
            load(assembler, slots, *address, Reg::R10)?;
            emit(assembler, &Inst::MovRM(Reg::R10, Mem::base(Reg::R10, 0)))?;
            if let Some(result) = result {
                store(assembler, slots, result, Reg::R10)?;
            }
        }
        OpKind::Store { address, value } => {
            load(assembler, slots, *address, Reg::R10)?;
            load(assembler, slots, *value, Reg::R11)?;
            emit(assembler, &Inst::MovMR(Mem::base(Reg::R10, 0), Reg::R11))?;
        }
        OpKind::LoadField { object, field } => {
            load(assembler, slots, *object, Reg::R10)?;
            let offset = i32::try_from(field.checked_mul(8).ok_or(CodegenError::FrameOverflow)?)
                .map_err(|_| CodegenError::FrameOverflow)?;
            emit(
                assembler,
                &Inst::MovRM(Reg::R10, Mem::base(Reg::R10, offset)),
            )?;
            if let Some(result) = result {
                store(assembler, slots, result, Reg::R10)?;
            }
        }
        OpKind::StoreField {
            object,
            field,
            value,
        } => {
            load(assembler, slots, *object, Reg::R10)?;
            load(assembler, slots, *value, Reg::R11)?;
            let offset = i32::try_from(field.checked_mul(8).ok_or(CodegenError::FrameOverflow)?)
                .map_err(|_| CodegenError::FrameOverflow)?;
            emit(
                assembler,
                &Inst::MovMR(Mem::base(Reg::R10, offset), Reg::R11),
            )?;
        }
        OpKind::Alloc { .. } => {
            emit(assembler, &Inst::Nop(1))?;
            add_map(assembler, frame, slots, maps, FLAG_ALLOCATION_SLOW)?;
        }
        OpKind::LoadArg { index } => {
            let register = [Reg::Rsi, Reg::Rdx, Reg::Rcx, Reg::R8, Reg::R9]
                .get(usize::from(*index))
                .copied()
                .ok_or_else(|| {
                    CodegenError::Unsupported(
                        "stack argument loading is not available in stage 1a".into(),
                    )
                })?;
            if let Some(result) = result {
                emit(assembler, &Inst::MovRR(Reg::R10, register))?;
                store(assembler, slots, result, Reg::R10)?;
            }
        }
        OpKind::Call {
            function: callee, ..
        }
        | OpKind::CallIndirect { callee, .. } => {
            load(assembler, slots, *callee, Reg::R11)?;
            emit(assembler, &Inst::CallReg(Reg::R11))?;
            if let Some(result) = result {
                store(assembler, slots, result, Reg::Rax)?;
            }
            add_map(assembler, frame, slots, maps, FLAG_CALL)?;
        }
        OpKind::Builtin { name, .. } => {
            let address = abi.builtin_address(name).ok_or_else(|| {
                CodegenError::Unsupported(format!("builtin address is unavailable: {name}"))
            })?;
            emit(
                assembler,
                &Inst::MovRI(Reg::R11, Imm::I64(address.cast_signed())),
            )?;
            emit(assembler, &Inst::CallReg(Reg::R11))?;
            if let Some(result) = result {
                store(assembler, slots, result, Reg::Rax)?;
            }
            add_map(assembler, frame, slots, maps, FLAG_CALL)?;
        }
        OpKind::Prim { op: prim, args, .. } => lower_prim(assembler, prim, args, result, slots)?,
        OpKind::Compare { op, left, right } => {
            load(assembler, slots, *left, Reg::R10)?;
            load(assembler, slots, *right, Reg::R11)?;
            emit(assembler, &Inst::CmpRR(Reg::R10, Reg::R11))?;
            emit(assembler, &Inst::Setcc(compare_condition(*op), Reg::R10))?;
            if let Some(result) = result {
                store(assembler, slots, result, Reg::R10)?;
            }
        }
        OpKind::SetMultipleValues { values } => {
            emit(
                assembler,
                &Inst::MovRI(Reg::Rdx, Imm::I64(checked_i64(values.len())?)),
            )?;
            if let (Some(first), Some(result)) = (values.first(), result) {
                load(assembler, slots, *first, Reg::Rax)?;
                store(assembler, slots, result, Reg::Rax)?;
            }
        }
        OpKind::Safepoint => add_map(assembler, frame, slots, maps, FLAG_CALL)?,
    }
    Ok(())
}
#[rustfmt::skip]
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
    load(assembler, slots, *first, Reg::R10)?;
    if let Some(second) = args.get(1) {
        load(assembler, slots, *second, Reg::R11)?;
    }
    match prim {
        Prim::FixnumAdd => emit(assembler, &Inst::BinRR(BinOp::Add, Reg::R10, Reg::R11))?,
        Prim::FixnumSub => emit(assembler, &Inst::BinRR(BinOp::Sub, Reg::R10, Reg::R11))?,
        Prim::FixnumMul => emit(assembler, &Inst::ImulRR(Reg::R10, Reg::R11))?,
        Prim::FixnumEq | Prim::Eq | Prim::Eql => { emit(assembler, &Inst::CmpRR(Reg::R10, Reg::R11))?; emit(assembler, &Inst::Setcc(Cond::E, Reg::R10))?; }
        Prim::FixnumLt => { emit(assembler, &Inst::CmpRR(Reg::R10, Reg::R11))?; emit(assembler, &Inst::Setcc(Cond::L, Reg::R10))?; }
        Prim::FixnumLe => { emit(assembler, &Inst::CmpRR(Reg::R10, Reg::R11))?; emit(assembler, &Inst::Setcc(Cond::Le, Reg::R10))?; }
        Prim::Car | Prim::Cdr | Prim::Svref | Prim::Aref => { emit(assembler, &Inst::MovRM(Reg::R10, Mem::base(Reg::R10, 0)))?; }
        Prim::Rplaca | Prim::Rplacd | Prim::Aset => { emit(assembler, &Inst::MovMR(Mem::base(Reg::R10, 0), Reg::R11))?; }
        Prim::FixnumDiv | Prim::Typep | Prim::CharacterPredicate(_) | Prim::StructureSlot(_) => {
            return Err(CodegenError::Unsupported(format!(
                "primitive is not available in fixed templates: {prim:?}"
            )));
        }
    }
    if let Some(result) = result {
        store(assembler, slots, result, Reg::R10)?;
    }
    Ok(())
}
fn add_map(
    assembler: &Assembler,
    frame: FrameLayout,
    slots: &[(ValueId, u32)],
    maps: &mut Vec<SafepointMap>,
    flags: u32,
) -> Result<(), CodegenError> {
    let live: Vec<u16> = slots
        .iter()
        .map(|(_, slot)| u16::try_from(*slot).map_err(|_| CodegenError::FrameOverflow))
        .collect::<Result<_, _>>()?;
    maps.push(
        SafepointMap::new(
            checked_u32(assembler.bytes().len())?,
            checked_u16(frame.frame_words)?,
            checked_u16(frame.frame_words)?,
            &live,
            &[],
            flags,
        )
        .map_err(|error| CodegenError::Encode(error.to_string()))?,
    );
    Ok(())
}
fn emit_return(
    assembler: &mut Assembler,
    values: &[ValueId],
    slots: &[(ValueId, u32)],
    abi: &dyn RuntimeAbi,
) -> Result<(), CodegenError> {
    if let Some(value) = values.first() {
        load(assembler, slots, *value, Reg::Rax)?;
    } else {
        emit(
            assembler,
            &Inst::MovRI(Reg::Rax, Imm::I64(abi.encode_fixnum(0))),
        )?;
    }
    emit(
        assembler,
        &Inst::MovRI(
            Reg::Rdx,
            Imm::I64(checked_i64(values.len().saturating_sub(1))?),
        ),
    )?;
    emit(assembler, &Inst::Ret)
}
#[allow(clippy::too_many_lines)]
fn encode(
    function: &Function,
    mut machine: MachineFunction,
    abi: &dyn RuntimeAbi,
) -> Result<CompiledFunction, CodegenError> {
    let mut assembler = Assembler::new();
    let labels: HashMap<_, _> = machine
        .blocks
        .iter()
        .map(|block| (block.id, assembler.new_label()))
        .collect();
    let mut maps = Vec::new();
    let mut debug = Vec::new();
    emit(&mut assembler, &Inst::Push(Reg::Rbp))?;
    emit(&mut assembler, &Inst::MovRR(Reg::Rsp, Reg::Rbp))?;
    emit(
        &mut assembler,
        &Inst::BinRI(
            BinOp::Sub,
            Reg::Rsp,
            machine.frame.size_bytes().cast_signed(),
        ),
    )?;
    let entry_offset = checked_u32(assembler.bytes().len())?;
    for block in &mut machine.blocks {
        assembler.bind(labels[&block.id]);
        block.offset = checked_u32(assembler.bytes().len())?;
        let source = function
            .blocks
            .iter()
            .find(|candidate| candidate.id == block.id)
            .ok_or(CodegenError::UnknownBlock(block.id))?;
        for op in &source.ops {
            lower_op(
                &mut assembler,
                op,
                function,
                &machine.slots,
                abi,
                machine.frame,
                &mut maps,
            )?;
            debug.push(crate::DebugLocation {
                pc_offset: checked_u32(assembler.bytes().len())?,
                location: op.loc,
            });
        }
        match &source.terminator {
            Terminator::Return { values } => {
                emit_return(&mut assembler, values, &machine.slots, abi)?;
            }
            Terminator::Jump { target, .. } => {
                emit(&mut assembler, &Inst::Jmp(labels[target]))?;
                if *target == block.id {
                    add_map(
                        &assembler,
                        machine.frame,
                        &machine.slots,
                        &mut maps,
                        FLAG_LOOP_BACKEDGE,
                    )?;
                }
            }
            Terminator::Branch {
                condition,
                then_target,
                else_target,
                ..
            } => {
                load(&mut assembler, &machine.slots, *condition, Reg::R10)?;
                emit(&mut assembler, &Inst::CmpRI(Reg::R10, 0))?;
                emit(&mut assembler, &Inst::Jcc(Cond::Ne, labels[then_target]))?;
                emit(&mut assembler, &Inst::Jmp(labels[else_target]))?;
            }
            Terminator::Switch {
                value,
                cases,
                default,
                ..
            } => {
                load(&mut assembler, &machine.slots, *value, Reg::R10)?;
                for (case, target, _) in cases {
                    emit(
                        &mut assembler,
                        &Inst::CmpRI(
                            Reg::R10,
                            i32::try_from(*case).map_err(|_| CodegenError::FrameOverflow)?,
                        ),
                    )?;
                    emit(&mut assembler, &Inst::Jcc(Cond::E, labels[target]))?;
                }
                emit(&mut assembler, &Inst::Jmp(labels[default]))?;
            }
            Terminator::CallReturn {
                function: callee, ..
            }
            | Terminator::TailCall {
                function: callee, ..
            } => {
                load(&mut assembler, &machine.slots, *callee, Reg::R11)?;
                emit(&mut assembler, &Inst::CallReg(Reg::R11))?;
                add_map(
                    &assembler,
                    machine.frame,
                    &machine.slots,
                    &mut maps,
                    FLAG_CALL,
                )?;
                emit_return(&mut assembler, &[], &machine.slots, abi)?;
            }
            Terminator::Throw { .. } | Terminator::Unreachable => emit(&mut assembler, &Inst::Ud2)?,
        }
    }
    let blob = assembler
        .finish()
        .map_err(|error| CodegenError::Encode(error.to_string()))?;
    let relocations = crate::relocations_from_fixups(&blob.fixups).map_err(|error| {
        CodegenError::Encode(format!("relocation conversion failed: {error:?}"))
    })?;
    Ok(CompiledFunction {
        code: blob.bytes,
        entry_offset,
        relocations,
        safepoint_maps: maps,
        frame_size: machine.frame.size_bytes(),
        debug,
    })
}
