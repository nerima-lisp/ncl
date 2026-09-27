use super::{emit, load_value};
use crate::{Allocation, CodegenError, RuntimeAbi, RuntimeFunction};
use ncl_asm_aarch64::{Assembler, Cond, Inst, MemOperand, Reg, RegOrSp, Shift};
use ncl_ir::ValueId;

pub(super) fn lower_named_global_call(
    assembler: &mut Assembler,
    closure: ValueId,
    symbol: ValueId,
    args: &[ValueId],
    allocation: &Allocation,
    abi: &dyn RuntimeAbi,
) -> Result<(), CodegenError> {
    let Some((argc_value, rest)) = args.split_first() else {
        // check-added-lines: allow(unsupported) malformed IR lacks the required argc value.
        return Err(CodegenError::Unsupported(
            "closure calls require a tagged argc argument".into(),
        ));
    };
    if rest.len() > 4 {
        // check-added-lines: allow(unsupported) the native ABI has four argument registers.
        return Err(CodegenError::Unsupported(
            "AArch64 calls support at most four register arguments".into(),
        ));
    }
    load_value(assembler, allocation, symbol, Reg(16))?;
    load_value(assembler, allocation, closure, Reg(17))?;
    load_value(assembler, allocation, *argc_value, Reg(0))?;
    for (index, argument) in rest.iter().enumerate() {
        load_value(
            assembler,
            allocation,
            *argument,
            Reg(u8::try_from(index + 1).map_err(|_| CodegenError::FrameOverflow)?),
        )?;
    }
    // check-added-lines: allow(unbound) compare against the function-cell sentinel.
    for instruction in ncl_asm_aarch64::mov_imm64(Reg(5), ncl_sys::Word::UNBOUND.bits()) {
        emit(assembler, instruction)?;
    }
    emit(
        assembler,
        Inst::Cmp {
            rn: Reg(17),
            rm: Reg(5),
            shift: Shift::Lsl(0),
        },
    )?;
    let normal = assembler.new_label();
    let call = assembler.new_label();
    emit(
        assembler,
        Inst::BCond {
            cond: Cond::Ne,
            label: normal,
        },
    )?;
    for instruction in ncl_asm_aarch64::mov_imm64(
        Reg(17),
        // check-added-lines: allow(unsupported) surface ABI lookup failure as codegen failure.
        abi.runtime_address(RuntimeFunction::UndefinedFunction)
            // check-added-lines: allow(unsupported) surface ABI lookup failure as codegen failure.
            .map_err(|error| CodegenError::Unsupported(error.to_string()))?,
    ) {
        emit(assembler, instruction)?;
    }
    emit(assembler, Inst::B { label: call })?;
    assembler
        .bind(normal)
        .map_err(|error| CodegenError::Encode(error.to_string()))?;
    emit(
        assembler,
        Inst::Mov {
            rd: RegOrSp::Reg(Reg(16)),
            rn: RegOrSp::Reg(Reg(17)),
        },
    )?;
    emit(
        assembler,
        Inst::Mov {
            rd: RegOrSp::Reg(Reg(5)),
            rn: RegOrSp::Reg(Reg(16)),
        },
    )?;
    emit(
        assembler,
        Inst::AndImm {
            rd: Reg(5),
            rn: Reg(5),
            imm: !ncl_sys::LOWTAG_MASK,
        },
    )?;
    emit(
        assembler,
        Inst::Ldr {
            rt: Reg(17),
            mem: MemOperand::Unscaled {
                base: RegOrSp::Reg(Reg(5)),
                offset: i16::try_from((ncl_object::function_offset::ENTRY + 1) * 8)
                    .map_err(|_| CodegenError::FrameOverflow)?,
            },
        },
    )?;
    emit(
        assembler,
        Inst::AsrImm {
            rd: Reg(17),
            rn: Reg(17),
            amount: u8::try_from(ncl_sys::FIXNUM_TAG_BITS)
                .map_err(|_| CodegenError::FrameOverflow)?,
        },
    )?;
    assembler
        .bind(call)
        .map_err(|error| CodegenError::Encode(error.to_string()))
}
