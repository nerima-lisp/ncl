use crate::{checked_i64, checked_u16, checked_u32};
use crate::{CodegenError, ConstantName, FrameLayout, RuntimeAbi, SafepointMap};
use crate::{FLAG_ALLOCATION_SLOW, FLAG_CALL};
use ncl_asm_x86_64::{Assembler, BinOp, Cond, Imm, Inst, Mem, Reg};
use ncl_ir::{Compare, Constant, Function, Op, OpKind, Prim, ValueId};

#[rustfmt::skip]
pub(super) fn slot(slots: &[(ValueId, u32)], value: ValueId) -> Result<u32, CodegenError> { slots.iter().find(|(id, _)| *id == value).map(|(_, slot)| *slot).ok_or(CodegenError::UnknownValue(value)) }
#[rustfmt::skip]
pub(super) fn memory(slot: u32) -> Result<Mem, CodegenError> { let bytes = slot.checked_add(1).and_then(|v| v.checked_mul(8)).ok_or(CodegenError::FrameOverflow)?; Ok(Mem::base(Reg::Rbp, -i32::try_from(bytes).map_err(|_| CodegenError::FrameOverflow)?)) }
#[rustfmt::skip]
pub(super) fn emit(assembler: &mut Assembler, inst: &Inst) -> Result<(), CodegenError> { assembler.emit(inst).map_err(|error| CodegenError::Encode(error.to_string())) }
#[rustfmt::skip]
pub(super) fn load(assembler: &mut Assembler, slots: &[(ValueId, u32)], value: ValueId, register: Reg) -> Result<(), CodegenError> { emit(assembler, &Inst::MovRM(register, memory(slot(slots, value)?)?)) }
#[rustfmt::skip]
pub(super) fn store(assembler: &mut Assembler, slots: &[(ValueId, u32)], value: ValueId, register: Reg) -> Result<(), CodegenError> { emit(assembler, &Inst::MovMR(memory(slot(slots, value)?)?, register)) }
#[rustfmt::skip]
pub(super) const fn compare_condition(op: Compare) -> Cond { match op { Compare::Eq => Cond::E, Compare::Ne => Cond::Ne, Compare::Lt => Cond::L, Compare::Le => Cond::Le, Compare::Gt => Cond::G, Compare::Ge => Cond::Ge } }

pub(super) fn constant_value(
    constant: &Constant,
    abi: &dyn RuntimeAbi,
) -> Result<i64, CodegenError> {
    match constant {
        Constant::Fixnum(value) => Ok(abi.encode_fixnum(*value)),
        Constant::Character(value) => Ok(abi.encode_character(*value)),
        Constant::SingleFloat(value) => Ok(i64::from(value.to_bits())),
        Constant::DoubleFloat(value) => Ok(i64::from_ne_bytes(value.to_bits().to_ne_bytes())),
        Constant::Nil | Constant::Unbound => Ok(0),
        Constant::T => Ok(abi.encode_fixnum(1)),
        Constant::FunctionEntry(function) => abi
            .constant_word_named(ConstantName::new(&format!("function-entry:{}", function.0)))
            .ok_or_else(|| CodegenError::Unsupported("entry unavailable".into())),
        Constant::Symbol { .. } | Constant::Object(_) | Constant::StringBytes(_) => Err(
            CodegenError::Unsupported("constant requires a runtime constant table".into()),
        ),
    }
}

#[allow(clippy::too_many_lines)]
pub(super) fn lower_op(
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
            let address = abi
                .builtin_address_named(crate::BuiltinName::new(name))
                .ok_or_else(|| {
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
        _ => Err(CodegenError::Unsupported(
            "target-specific operation".into(),
        ))?,
    }
    Ok(())
}

#[rustfmt::skip]
fn lower_prim(assembler: &mut Assembler, prim: &Prim, args: &[ValueId], result: Option<ValueId>, slots: &[(ValueId, u32)]) -> Result<(), CodegenError> {
    let Some(first) = args.first() else { return Err(CodegenError::Unsupported("primitive has no operands".into())); };
    load(assembler, slots, *first, Reg::R10)?;
    if let Some(second) = args.get(1) { load(assembler, slots, *second, Reg::R11)?; }
    match prim {
        Prim::FixnumAdd => emit(assembler, &Inst::BinRR(BinOp::Add, Reg::R10, Reg::R11))?,
        Prim::FixnumSub => emit(assembler, &Inst::BinRR(BinOp::Sub, Reg::R10, Reg::R11))?,
        Prim::FixnumMul => emit(assembler, &Inst::ImulRR(Reg::R10, Reg::R11))?,
        Prim::FixnumEq | Prim::Eq | Prim::Eql => { emit(assembler, &Inst::CmpRR(Reg::R10, Reg::R11))?; emit(assembler, &Inst::Setcc(Cond::E, Reg::R10))?; }
        Prim::FixnumLt => { emit(assembler, &Inst::CmpRR(Reg::R10, Reg::R11))?; emit(assembler, &Inst::Setcc(Cond::L, Reg::R10))?; }
        Prim::FixnumLe => { emit(assembler, &Inst::CmpRR(Reg::R10, Reg::R11))?; emit(assembler, &Inst::Setcc(Cond::Le, Reg::R10))?; }
        Prim::Car | Prim::Cdr | Prim::Svref | Prim::Aref => { emit(assembler, &Inst::MovRM(Reg::R10, Mem::base(Reg::R10, 0)))?; }
        Prim::Rplaca | Prim::Rplacd | Prim::Aset => { emit(assembler, &Inst::MovMR(Mem::base(Reg::R10, 0), Reg::R11))?; }
        Prim::FixnumDiv | Prim::Typep | Prim::CharacterPredicate(_) | Prim::StructureSlot(_) => return Err(CodegenError::Unsupported(format!("primitive is not available in fixed templates: {prim:?}"))),
    }
    if let Some(result) = result { store(assembler, slots, result, Reg::R10)?; }
    Ok(())
}

pub(super) fn add_map(
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

pub(super) fn emit_return(
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
