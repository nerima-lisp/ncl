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
        | ncl_ir::Constant::Array { .. }
        | ncl_ir::Constant::Structure { .. } => Err(CodegenError::Unsupported( // check-added-lines: allow(unsupported) runtime-table fallback
            // check-added-lines: allow(unsupported) runtime-table constant
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

#[cfg(test)]
#[allow(
    clippy::cast_possible_wrap,
    clippy::expect_used,
    clippy::unwrap_used,
    missing_docs
)]
mod tests {
    use super::{compare_condition, constant_word, materialise_boolean, untag_function_object};
    use crate::{CodegenError, RuntimeAbi, RuntimeFunction};
    use ncl_asm_x86_64::{Assembler, Cond};
    use ncl_ir::{Constant, ConstantIndex, StructureKind};

    struct EntryAbi;

    impl RuntimeAbi for EntryAbi {
        fn builtin_address(
            &self,
            identifier: ncl_object::BuiltinIdentifier,
        ) -> Result<u64, crate::AbiError> {
            Err(crate::AbiError::MissingBuiltin(identifier))
        }

        fn field_offset(&self, field: crate::ContextField) -> Result<i32, crate::AbiError> {
            Err(crate::AbiError::UnsupportedContextField(field))
        }

        fn runtime_address(&self, function: RuntimeFunction) -> Result<u64, crate::AbiError> {
            Err(crate::AbiError::UnsupportedRuntimeFunction(function))
        }

        fn constant_word(&self, name: &str) -> Option<i64> {
            (name == "function-entry:7").then_some(0x2200)
        }
    }

    #[test]
    fn constant_words_preserve_tagged_values_and_typed_runtime_failures() {
        let abi = EntryAbi;
        assert_eq!(
            constant_word(&Constant::Fixnum(-3), &abi),
            Ok(ncl_sys::Word::fixnum(-3).bits() as i64)
        );
        assert_eq!(
            constant_word(&Constant::Character('x' as u32), &abi),
            Ok(ncl_sys::Word::character('x' as u32).bits() as i64)
        );
        assert_eq!(
            constant_word(&Constant::Nil, &abi),
            Ok(ncl_sys::Word::NIL.bits() as i64)
        );
        assert_eq!(
            constant_word(&Constant::Unbound, &abi),
            Ok(ncl_sys::Word::UNBOUND.bits() as i64)
        );
        assert_eq!(
            constant_word(&Constant::T, &abi),
            Ok(ncl_sys::Word::TRUE.bits() as i64)
        );
        assert_eq!(
            constant_word(&Constant::FunctionEntry(ncl_ir::FunctionId(7)), &abi),
            Ok(0x2200)
        );
        assert!(matches!(
            constant_word(&Constant::FunctionEntry(ncl_ir::FunctionId(8)), &abi),
            Err(CodegenError::Unsupported(message))
                if message.contains("function entry constant is unavailable")
        ));

        for constant in [
            Constant::SingleFloat(1.0),
            Constant::DoubleFloat(1.0),
            Constant::Symbol {
                package: "CL".into(),
                name: "X".into(),
            },
            Constant::Object(ConstantIndex(0)),
            Constant::StringBytes(vec![1]),
            Constant::Structure {
                kind: StructureKind::Cons,
                elements: Vec::new(),
            },
            Constant::Bignum {
                negative: false,
                limbs: vec![1],
            },
            Constant::Ratio {
                numerator: ConstantIndex(0),
                denominator: ConstantIndex(1),
            },
            Constant::Complex {
                real: ConstantIndex(0),
                imaginary: ConstantIndex(1),
            },
        ] {
            assert!(matches!(
                constant_word(&constant, &abi),
                Err(CodegenError::Unsupported(message))
                    if message.contains("runtime table")
            ));
        }
    }

    #[test]
    fn boolean_materialisation_and_untagging_emit_fixed_machine_templates() {
        let mut boolean = Assembler::new();
        materialise_boolean(&mut boolean, Cond::E).expect("boolean materialisation");
        assert_eq!(
            boolean.bytes(),
            &[0x41, 0x0f, 0x94, 0xc2, 0x4d, 0x0f, 0xb6, 0xd2]
        );

        let mut untag = Assembler::new();
        untag_function_object(&mut untag).expect("function object untagging");
        assert_eq!(untag.bytes(), &[0x49, 0x83, 0xe2, 0xf8]);
    }

    #[test]
    fn comparison_conditions_cover_each_ir_ordering() {
        for (comparison, condition) in [
            (ncl_ir::Compare::Eq, Cond::E),
            (ncl_ir::Compare::Ne, Cond::Ne),
            (ncl_ir::Compare::Lt, Cond::L),
            (ncl_ir::Compare::Le, Cond::Le),
            (ncl_ir::Compare::Gt, Cond::G),
            (ncl_ir::Compare::Ge, Cond::Ge),
        ] {
            assert_eq!(compare_condition(comparison), condition);
        }
    }
}
