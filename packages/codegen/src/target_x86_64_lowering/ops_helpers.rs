use crate::{CodegenError, ConstantName, RuntimeAbi};
use ncl_asm_x86_64::{Assembler, BinOp, Cond, Inst};

pub(super) fn constant_word(
    constant: &ncl_ir::Constant,
    abi: &dyn RuntimeAbi,
) -> Result<i64, CodegenError> {
    match constant {
        ncl_ir::Constant::Fixnum(value) => Ok(i64::from_ne_bytes(
            ncl_sys::Word::fixnum(*value).bits().to_ne_bytes(),
        )),
        ncl_ir::Constant::Character(value) => Ok(i64::from_ne_bytes(
            ncl_sys::Word::character(*value).bits().to_ne_bytes(),
        )),
        ncl_ir::Constant::Nil => Ok(i64::from_ne_bytes(ncl_sys::Word::NIL.bits().to_ne_bytes())),
        ncl_ir::Constant::Unbound => Ok(i64::from_ne_bytes(
            ncl_sys::Word::UNBOUND.bits().to_ne_bytes(), // check-added-lines: allow(unbound) sentinel constant
        )),
        ncl_ir::Constant::T => Ok(i64::from_ne_bytes(ncl_sys::Word::TRUE.bits().to_ne_bytes())),
        ncl_ir::Constant::FunctionEntry(function) => abi
            .constant_word_named(ConstantName::new(&format!("function-entry:{}", function.0)))
            .ok_or_else(|| {
                CodegenError::Unsupported("function entry constant is unavailable".into()) // check-added-lines: allow(unsupported) unavailable ABI constant
            }),
        ncl_ir::Constant::SingleFloat(_)
        | ncl_ir::Constant::DoubleFloat(_)
        | ncl_ir::Constant::Symbol { .. }
        | ncl_ir::Constant::Object(_)
        | ncl_ir::Constant::StringBytes(_)
        | ncl_ir::Constant::Bignum { .. }
        | ncl_ir::Constant::Ratio { .. }
        | ncl_ir::Constant::Complex { .. } // check-added-lines: allow(unsupported) runtime-table constant
        | ncl_ir::Constant::Structure { .. } => Err(CodegenError::Unsupported(
            // check-added-lines: allow(unsupported) runtime-table constant
            "constant requires a runtime table".into(), // check-added-lines: allow(unsupported) runtime-table constant
        )),
    }
}

pub(super) const fn compare_condition(op: ncl_ir::Compare) -> Cond {
    match op {
        ncl_ir::Compare::Eq => Cond::E,
        ncl_ir::Compare::Ne => Cond::Ne,
        ncl_ir::Compare::Lt => Cond::L,
        ncl_ir::Compare::Le => Cond::Le,
        ncl_ir::Compare::Gt => Cond::G,
        ncl_ir::Compare::Ge => Cond::Ge,
    }
}

pub(super) fn materialise_boolean(
    assembler: &mut Assembler,
    condition: Cond,
) -> Result<(), CodegenError> {
    super::emit(assembler, Inst::Setcc(condition, super::FUNCTION_OBJECT))?;
    super::emit(
        assembler,
        Inst::Movzx(super::FUNCTION_OBJECT, super::FUNCTION_OBJECT, 8),
    )
}

pub(super) fn untag_function_object(assembler: &mut Assembler) -> Result<(), CodegenError> {
    let mask = i32::try_from(i64::from_ne_bytes((!ncl_sys::LOWTAG_MASK).to_ne_bytes()))
        .map_err(|_| CodegenError::FrameOverflow)?;
    super::emit(
        assembler,
        Inst::BinRI(BinOp::And, super::FUNCTION_OBJECT, mask),
    )
}
