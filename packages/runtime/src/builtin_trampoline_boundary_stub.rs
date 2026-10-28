use super::{NativeCallResult, NativeInvocation, RuntimeError, error_result};
use ncl_asm_aarch64::{Assembler as Aarch64Assembler, Inst as Aarch64Inst, Reg as Aarch64Reg};
use ncl_asm_x86_64::{Assembler as X86Assembler, BinOp, Imm, Inst as X86Inst, Mem, Reg as X86Reg};
use ncl_sys::NativeState;

pub(super) fn with_generated_frame(
    invocation: &mut NativeInvocation<'_>,
    frame_fp: usize,
    return_pc: usize,
    dispatch: impl FnOnce(&mut NativeInvocation<'_>) -> NativeCallResult,
) -> NativeCallResult {
    let captured_frame = invocation.entry_codes.contains_frame_capture_pc(return_pc);
    let native_state = invocation.context.native_state();
    if captured_frame && !invocation.context.capture_native_frame(frame_fp, return_pc) {
        invocation
            .context
            .set_pending(ncl_object::ObjectError::Layout);
        return error_result();
    }
    if native_state != NativeState::Native {
        invocation.context.thread_access().enter_native();
    }

    let result = dispatch(invocation);

    if native_state != NativeState::Native {
        invocation.context.thread_access().leave_native();
    }
    if captured_frame {
        invocation.context.write_back_frame_snapshot();
    }
    result
}

pub(super) fn build_aarch64_stub(address: u64) -> Result<Vec<u8>, RuntimeError> {
    let mut assembler = Aarch64Assembler::new();
    let emit = |assembler: &mut Aarch64Assembler, inst: Aarch64Inst| {
        assembler
            .emit(&inst)
            .map_err(|error| RuntimeError::Native(error.to_string()))
    };
    emit(
        &mut assembler,
        Aarch64Inst::SubImm {
            rd: ncl_asm_aarch64::RegOrSp::Sp,
            rn: ncl_asm_aarch64::RegOrSp::Sp,
            imm: 32,
            shift: false,
        },
    )?;
    for instruction in [
        Aarch64Inst::Str {
            rt: Aarch64Reg(29),
            mem: ncl_asm_aarch64::MemOperand::Unscaled {
                base: ncl_asm_aarch64::RegOrSp::Sp,
                offset: 0,
            },
        },
        Aarch64Inst::Str {
            rt: Aarch64Reg(30),
            mem: ncl_asm_aarch64::MemOperand::Unscaled {
                base: ncl_asm_aarch64::RegOrSp::Sp,
                offset: 8,
            },
        },
        Aarch64Inst::Str {
            rt: Aarch64Reg(30),
            mem: ncl_asm_aarch64::MemOperand::Unscaled {
                base: ncl_asm_aarch64::RegOrSp::Sp,
                offset: 16,
            },
        },
        Aarch64Inst::Mov {
            rd: ncl_asm_aarch64::RegOrSp::Reg(Aarch64Reg(6)),
            rn: ncl_asm_aarch64::RegOrSp::Reg(Aarch64Reg(16)),
        },
        Aarch64Inst::Mov {
            rd: ncl_asm_aarch64::RegOrSp::Reg(Aarch64Reg(7)),
            rn: ncl_asm_aarch64::RegOrSp::Reg(Aarch64Reg(21)),
        },
    ] {
        emit(&mut assembler, instruction)?;
    }
    for instruction in ncl_asm_aarch64::mov_imm64(Aarch64Reg(9), address) {
        emit(&mut assembler, instruction)?;
    }
    emit(&mut assembler, Aarch64Inst::Blr { rn: Aarch64Reg(9) })?;
    emit(
        &mut assembler,
        Aarch64Inst::Ldr {
            rt: Aarch64Reg(30),
            mem: ncl_asm_aarch64::MemOperand::Unscaled {
                base: ncl_asm_aarch64::RegOrSp::Sp,
                offset: 16,
            },
        },
    )?;
    emit(
        &mut assembler,
        Aarch64Inst::AddImm {
            rd: ncl_asm_aarch64::RegOrSp::Sp,
            rn: ncl_asm_aarch64::RegOrSp::Sp,
            imm: 32,
            shift: false,
        },
    )?;
    emit(&mut assembler, Aarch64Inst::Ret { rn: Aarch64Reg(30) })?;
    assembler
        .finish()
        .map(|blob| blob.bytes)
        .map_err(|error| RuntimeError::Native(error.to_string()))
}

pub(super) fn build_x86_64_stub(address: u64) -> Result<Vec<u8>, RuntimeError> {
    const RESERVED: i32 = 40;
    let address = i64::try_from(address).map_err(|_| {
        RuntimeError::Native("generic builtin dispatcher address does not fit in i64".to_owned())
    })?;
    let mut assembler = X86Assembler::new();
    let emit = |assembler: &mut X86Assembler, inst: X86Inst| {
        assembler
            .emit(&inst)
            .map_err(|error| RuntimeError::Native(error.to_string()))
    };
    emit(
        &mut assembler,
        X86Inst::BinRI(BinOp::Sub, X86Reg::Rsp, RESERVED),
    )?;
    for instruction in [
        X86Inst::MovMR(Mem::base(X86Reg::Rsp, 0), X86Reg::R10),
        X86Inst::MovMR(Mem::base(X86Reg::Rsp, 8), X86Reg::R15),
        X86Inst::MovMR(Mem::base(X86Reg::Rsp, 16), X86Reg::Rbp),
        X86Inst::MovRM(X86Reg::Rax, Mem::base(X86Reg::Rsp, RESERVED)),
        X86Inst::MovMR(Mem::base(X86Reg::Rsp, 24), X86Reg::Rax),
        X86Inst::MovRI(X86Reg::Rax, Imm::I64(address)),
        X86Inst::CallReg(X86Reg::Rax),
        X86Inst::BinRI(BinOp::Add, X86Reg::Rsp, RESERVED),
        X86Inst::Ret,
    ] {
        emit(&mut assembler, instruction)?;
    }
    assembler
        .finish()
        .map(|blob| blob.bytes)
        .map_err(|error| RuntimeError::Native(error.to_string()))
}
