//! Target-independent target contracts and the `AArch64` fixed-template backend.

use crate::CodegenError;
use crate::templates::TemplateKind;
use ncl_asm_aarch64::{Assembler, Cond, Inst, Reg, RegOrSp};

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
            TemplateKind::Call
            | TemplateKind::CallIndirect
            | TemplateKind::MakeClosure
            | TemplateKind::MakeValueCell
            | TemplateKind::CallClosure
            | TemplateKind::Builtin
            | TemplateKind::EnterHandler
            | TemplateKind::LeaveHandler => {
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

#[cfg(test)]
mod tests {
    use super::{AArch64TargetIsa, TargetIsa};
    use crate::TemplateKind;
    use ncl_asm_aarch64::{Cond, Inst, Label, Reg, RegOrSp};

    #[test]
    fn aarch64_target_exposes_fixed_register_contract() {
        assert_eq!(AArch64TargetIsa.name(), "aarch64");
        assert_eq!(AArch64TargetIsa::CONTEXT, Reg(21));
        assert_eq!(AArch64TargetIsa::SCRATCH, [Reg(16), Reg(17)]);
    }

    #[test]
    fn aarch64_skeleton_selects_control_flow_and_call_instructions() {
        assert_eq!(
            AArch64TargetIsa::skeleton(TemplateKind::Return),
            vec![Inst::Ret { rn: Reg(30) }]
        );
        assert_eq!(
            AArch64TargetIsa::skeleton(TemplateKind::Branch),
            vec![Inst::BCond {
                cond: Cond::Ne,
                label: Label(0),
            }]
        );
        assert_eq!(
            AArch64TargetIsa::skeleton(TemplateKind::Jump),
            vec![Inst::B { label: Label(0) }]
        );
        for kind in [
            TemplateKind::Call,
            TemplateKind::CallIndirect,
            TemplateKind::MakeClosure,
            TemplateKind::MakeValueCell,
            TemplateKind::CallClosure,
            TemplateKind::Builtin,
            TemplateKind::EnterHandler,
            TemplateKind::LeaveHandler,
        ] {
            assert_eq!(
                AArch64TargetIsa::skeleton(kind),
                vec![Inst::Blr { rn: Reg(17) }],
                "call-like template {kind:?}"
            );
        }
        assert_eq!(
            AArch64TargetIsa::skeleton(TemplateKind::Alloc),
            vec![Inst::Nop]
        );
        assert_eq!(
            AArch64TargetIsa::skeleton(TemplateKind::Const),
            vec![Inst::Mov {
                rd: RegOrSp::Reg(Reg(16)),
                rn: RegOrSp::Reg(Reg(16)),
            }]
        );
    }

    #[test]
    fn aarch64_template_encoding_resolves_local_branch_labels() {
        let target = AArch64TargetIsa;

        for (kind, expected) in [
            (TemplateKind::Branch, [0x21, 0x00, 0x00, 0x54]),
            (TemplateKind::Jump, [0x01, 0x00, 0x00, 0x14]),
        ] {
            let bytes = target
                .encode_template(kind)
                .unwrap_or_else(|error| panic!("template encoding: {error:?}"));
            assert_eq!(bytes, expected, "encoded branch for {kind:?}");
        }
        assert_eq!(
            target
                .encode_template(TemplateKind::Return)
                .unwrap_or_else(|error| panic!("template encoding: {error:?}"))
                .as_slice(),
            [0xc0, 0x03, 0x5f, 0xd6]
        );
    }
}

#[path = "target_aarch64.rs"]
mod target_aarch64;
pub use target_aarch64::compile_function_aarch64;
