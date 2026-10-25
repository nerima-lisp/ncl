use crate::isa_x86_64::THREAD_CONTEXT;
use crate::machine_verify::SafepointSite;
use crate::{CodegenError, ConstantName, FrameLayout, RuntimeAbi, SafepointMap};
use crate::{ContextField, RuntimeFunction};
use crate::{FLAG_ALLOCATION_SLOW, FLAG_CALL};
use crate::{checked_i64, checked_u16, checked_u32};
use ncl_asm_x86_64::{Assembler, BinOp, Cond, Imm, Inst, Mem, Reg};
use ncl_ir::{Compare, Constant, Function, Op, OpKind, Prim, ValueId};

#[derive(Default)]
pub(super) struct SafepointOutput {
    pub(super) maps: Vec<SafepointMap>,
    pub(super) sites: Vec<SafepointSite>,
}

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

fn lower_write_barrier(
    assembler: &mut Assembler,
    owner: ValueId,
    slot: i64,
    slots: &[(ValueId, u32)],
    abi: &dyn RuntimeAbi,
) -> Result<(), CodegenError> {
    emit(assembler, &Inst::MovRR(Reg::Rdi, THREAD_CONTEXT))?;
    emit(assembler, &Inst::MovRI(Reg::Rsi, Imm::I64(slot)))?;
    load(assembler, slots, owner, Reg::Rdx)?;
    let address = abi
        .runtime_address(RuntimeFunction::WriteBarrier)
        // check-added-lines: allow(unsupported) preserve runtime ABI lookup failures.
        .map_err(|error| CodegenError::Unsupported(error.to_string()))?;
    emit(
        assembler,
        &Inst::MovRI(Reg::R11, Imm::I64(address.cast_signed())),
    )?;
    emit(assembler, &Inst::CallReg(Reg::R11))
}

fn lower_builtin_call(
    assembler: &mut Assembler,
    name: &str,
    args: &[ValueId],
    slots: &[(ValueId, u32)],
    abi: &dyn RuntimeAbi,
) -> Result<SafepointSite, CodegenError> {
    if args.len() > 4 {
        // check-added-lines: allow(unsupported) preserve the x86-64 builtin arity diagnostic.
        return Err(CodegenError::Unsupported(
            "x86-64 builtins support at most four arguments".into(),
        ));
    }
    if name == "make-rest-list" {
        // check-added-lines: allow(unsupported) the fixed-template backend lacks rest-list ABI setup.
        return Err(CodegenError::Unsupported(
            "make-rest-list requires target-specific ABI lowering".into(),
        ));
    }
    let registers = [Reg::Rsi, Reg::Rdx, Reg::Rcx, Reg::R8];
    let address = abi
        .builtin_address(crate::common_lisp_builtin(name))
        .map_err(|error| CodegenError::Unsupported(error.to_string()))?; // check-added-lines: allow(unsupported) preserve runtime ABI lookup failures.
    emit(assembler, &Inst::MovRR(Reg::Rdi, THREAD_CONTEXT))?;
    for (argument, register) in args.iter().zip(registers) {
        load(assembler, slots, *argument, register)?;
    }
    emit(
        assembler,
        &Inst::MovRI(Reg::R11, Imm::I64(address.cast_signed())),
    )?;
    emit_safepoint_call(assembler)
}

pub(super) fn constant_value(
    constant: &Constant,
    abi: &dyn RuntimeAbi,
) -> Result<i64, CodegenError> {
    match constant {
        Constant::Fixnum(value) => Ok(i64::from_ne_bytes(
            ncl_sys::Word::fixnum(*value).bits().to_ne_bytes(),
        )),
        Constant::Character(value) => Ok(i64::from_ne_bytes(
            ncl_sys::Word::character(*value).bits().to_ne_bytes(),
        )),
        Constant::SingleFloat(value) => Ok(i64::from(value.to_bits())),
        Constant::DoubleFloat(value) => Ok(i64::from_ne_bytes(value.to_bits().to_ne_bytes())),
        Constant::Nil => Ok(i64::from_ne_bytes(ncl_sys::Word::NIL.bits().to_ne_bytes())),
        Constant::Unbound => Ok(i64::from_ne_bytes(
            ncl_sys::Word::UNBOUND.bits().to_ne_bytes(),
        )),
        Constant::T => Ok(i64::from_ne_bytes(ncl_sys::Word::TRUE.bits().to_ne_bytes())),
        Constant::FunctionEntry(function) => abi
            .constant_word_named(ConstantName::new(&format!("function-entry:{}", function.0)))
            .ok_or_else(|| CodegenError::Unsupported("entry unavailable".into())),
        Constant::Symbol { .. }
        | Constant::UninternedSymbol { .. }
        | Constant::Object(_)
        | Constant::StringBytes(_)
        | Constant::Bignum { .. }
        | Constant::Ratio { .. }
        | Constant::Complex { .. }
        // check-added-lines: allow(unsupported) runtime-table constants are loaded through code objects
        | Constant::Structure { .. } => Err(CodegenError::Unsupported(
            "constant requires a runtime constant table".into(),
        )),
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
    safepoints: &mut SafepointOutput,
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
        OpKind::Move { value } | OpKind::TypeHint { value, .. } | OpKind::Convert { value, .. } => {
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
            lower_write_barrier(assembler, *object, i64::from(*field), slots, abi)?;
        }
        OpKind::Alloc { words } => {
            emit(assembler, &Inst::MovRR(THREAD_CONTEXT, Reg::Rdi))?;
            emit(
                assembler,
                &Inst::MovRI(
                    Reg::Rsi,
                    Imm::I32(i32::try_from(*words).map_err(|_| CodegenError::FrameOverflow)?),
                ),
            )?;
            let address = abi
                .runtime_address(RuntimeFunction::AllocateSlow)
                .map_err(|error| CodegenError::Unsupported(error.to_string()))?; // check-added-lines: allow(unsupported) Runtime ABI may omit this required slow allocator.
            emit(
                assembler,
                &Inst::MovRI(Reg::R11, Imm::I64(address.cast_signed())),
            )?;
            let site = emit_safepoint_call(assembler)?;
            if let Some(result) = result {
                store(assembler, slots, result, Reg::Rax)?;
            }
            add_map(
                frame,
                slots,
                &mut safepoints.maps,
                site,
                FLAG_ALLOCATION_SLOW,
            )?;
            safepoints.sites.push(site);
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
            let site = emit_safepoint_call(assembler)?;
            if let Some(result) = result {
                store(assembler, slots, result, Reg::Rax)?;
            }
            add_map(frame, slots, &mut safepoints.maps, site, FLAG_CALL)?;
            safepoints.sites.push(site);
        }
        OpKind::Builtin { name, args, .. } => {
            let site = lower_builtin_call(assembler, name, args, slots, abi)?;
            if let Some(result) = result {
                store(assembler, slots, result, Reg::Rax)?;
            }
            add_map(frame, slots, &mut safepoints.maps, site, FLAG_CALL)?;
            safepoints.sites.push(site);
        }
        OpKind::Prim { op: prim, args, .. } => {
            lower_prim(assembler, prim, args, result, slots, abi)?;
        }
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
        OpKind::Safepoint => {
            let done = assembler.new_label();
            let request_offset = abi
                .field_offset(ContextField::SafepointRequest)
                .map_err(|error| CodegenError::Unsupported(error.to_string()))?; // check-added-lines: allow(unsupported) Runtime ABI may omit the safepoint request field.
            emit(
                assembler,
                &Inst::MovRM(Reg::R10, Mem::base(THREAD_CONTEXT, request_offset)),
            )?;
            emit(assembler, &Inst::CmpRI(Reg::R10, 0))?;
            emit(assembler, &Inst::Jcc(Cond::E, done))?;
            emit(assembler, &Inst::MovRR(THREAD_CONTEXT, Reg::Rdi))?;
            emit(assembler, &Inst::MovRR(Reg::Rbp, Reg::Rsi))?;
            let address = abi
                .runtime_address(RuntimeFunction::SafepointSlow)
                .map_err(|error| CodegenError::Unsupported(error.to_string()))?; // check-added-lines: allow(unsupported) Runtime ABI may omit this required slow path.
            emit(
                assembler,
                &Inst::MovRI(Reg::R11, Imm::I64(address.cast_signed())),
            )?;
            emit(assembler, &Inst::Lea(Reg::Rdx, Mem::rip(3)))?;
            let site = emit_safepoint_call(assembler)?;
            assembler.bind(done);
            add_map(frame, slots, &mut safepoints.maps, site, FLAG_CALL)?;
            safepoints.sites.push(site);
        }
        _ => Err(CodegenError::Unsupported(
            "target-specific operation".into(),
        ))?,
    }
    Ok(())
}

#[rustfmt::skip]
fn lower_prim(assembler: &mut Assembler, prim: &Prim, args: &[ValueId], result: Option<ValueId>, slots: &[(ValueId, u32)], abi: &dyn RuntimeAbi) -> Result<(), CodegenError> {
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
        Prim::Rplaca | Prim::Rplacd | Prim::Aset => {
            emit(assembler, &Inst::MovMR(Mem::base(Reg::R10, 0), Reg::R11))?;
            lower_write_barrier(assembler, *first, 0, slots, abi)?;
        }
        Prim::FixnumDiv | Prim::Typep | Prim::CharacterPredicate(_) | Prim::StructureSlot(_) => return Err(CodegenError::Unsupported(format!("primitive is not available in fixed templates: {prim:?}"))),
    }
    if let Some(result) = result {
        store(assembler, slots, result, Reg::R10)?;
    }
    Ok(())
}

pub(super) fn add_map(
    frame: FrameLayout,
    slots: &[(ValueId, u32)],
    maps: &mut Vec<SafepointMap>,
    site: SafepointSite,
    flags: u32,
) -> Result<(), CodegenError> {
    let live: Vec<u16> = slots
        .iter()
        .map(|(_, slot)| u16::try_from(*slot).map_err(|_| CodegenError::FrameOverflow))
        .collect::<Result<_, _>>()?;
    maps.push(
        SafepointMap::new(
            site.end_offset,
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

fn emit_safepoint_call(assembler: &mut Assembler) -> Result<SafepointSite, CodegenError> {
    let start_offset = checked_u32(assembler.bytes().len())?;
    emit(assembler, &Inst::CallReg(Reg::R11))?;
    let end_offset = checked_u32(assembler.bytes().len())?;
    Ok(SafepointSite::call(start_offset, end_offset))
}

pub(super) fn emit_return(
    assembler: &mut Assembler,
    values: &[ValueId],
    slots: &[(ValueId, u32)],
    _abi: &dyn RuntimeAbi,
) -> Result<(), CodegenError> {
    if let Some(value) = values.first() {
        load(assembler, slots, *value, Reg::Rax)?;
    } else {
        emit(
            assembler,
            &Inst::MovRI(
                Reg::Rax,
                Imm::I64(i64::from_ne_bytes(
                    ncl_sys::Word::fixnum(0).bits().to_ne_bytes(),
                )),
            ),
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
