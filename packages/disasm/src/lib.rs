//! Native instruction decoding for NCL development tools.

#![allow(clippy::missing_errors_doc, clippy::must_use_candidate)]

mod aarch64;
mod x86_64;

/// A decoded machine instruction with its location in the input image.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedInstruction {
    /// Absolute address of the first byte.
    pub address: u64,
    /// Instruction length in bytes.
    pub size: u8,
    /// Original instruction bytes.
    pub bytes: Vec<u8>,
    /// Human-readable assembly text.
    pub text: String,
    /// Absolute branch destination, when this instruction branches directly.
    pub branch_target: Option<u64>,
}

/// Failure while decoding a machine-code stream.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodeError {
    /// The stream ended before a complete instruction was available.
    Truncated {
        /// Address at which decoding stopped.
        address: u64,
    },
    /// The bytes do not encode an instruction supported by this crate.
    Unsupported {
        /// Address of the unsupported instruction.
        address: u64,
        /// Bytes inspected for the instruction.
        bytes: Vec<u8>,
    },
    /// The bytes use an invalid encoding.
    Invalid {
        /// Address of the invalid instruction.
        address: u64,
        /// Explanation of why the encoding is invalid.
        reason: String,
    },
}

impl core::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Truncated { address } => write!(f, "truncated instruction at 0x{address:x}"),
            Self::Unsupported { address, .. } => {
                write!(f, "unsupported instruction at 0x{address:x}")
            }
            Self::Invalid { address, reason } => {
                write!(f, "invalid instruction at 0x{address:x}: {reason}")
            }
        }
    }
}

impl std::error::Error for DecodeError {}

/// Target architecture for a byte stream.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Architecture {
    /// Intel/AMD 64-bit instruction set.
    X86_64,
    /// ARM 64-bit instruction set.
    Aarch64,
}

/// Decode a native code region at an absolute address.
pub fn decode(
    architecture: Architecture,
    bytes: &[u8],
    base: u64,
) -> Result<Vec<DecodedInstruction>, DecodeError> {
    match architecture {
        Architecture::X86_64 => x86_64::decode(bytes, base),
        Architecture::Aarch64 => aarch64::decode(bytes, base),
    }
}

/// Resolve direct branch destinations to stable local labels.
pub fn resolve_labels(instructions: &[DecodedInstruction]) -> Vec<Option<String>> {
    instructions
        .iter()
        .map(|instruction| {
            instruction.branch_target.and_then(|target| {
                instructions
                    .iter()
                    .position(|candidate| candidate.address == target)
                    .map(|index| format!("L{index}"))
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use super::{Architecture, decode, resolve_labels};

    #[test]
    fn x86_decode_has_addresses() {
        let decoded = decode(Architecture::X86_64, &[0x90, 0xc3], 0x1000).expect("decode");
        assert_eq!(decoded[0].address, 0x1000);
        assert_eq!(decoded[1].address, 0x1001);
    }

    #[test]
    fn labels_resolve_to_local_instruction_names() {
        let decoded =
            decode(Architecture::X86_64, &[0xe9, 0, 0, 0, 0, 0xc3], 0x1000).expect("decode");
        assert_eq!(resolve_labels(&decoded), vec![Some("L1".to_owned()), None]);
    }

    #[test]
    fn x86_assembler_bytes_decode_as_their_instruction_text() {
        let mut assembler = ncl_asm_x86_64::Assembler::new();
        for instruction in [
            ncl_asm_x86_64::Inst::Nop(1),
            ncl_asm_x86_64::Inst::Ret,
            ncl_asm_x86_64::Inst::Int3,
            ncl_asm_x86_64::Inst::Ud2,
        ] {
            assembler.emit(&instruction).expect("encode");
        }
        let decoded = decode(Architecture::X86_64, assembler.bytes(), 0).expect("decode");
        assert_eq!(
            decoded
                .iter()
                .map(|instruction| instruction.text.as_str())
                .collect::<Vec<_>>(),
            ["nop", "ret", "int3", "ud2"]
        );
    }

    #[test]
    fn aarch64_assembler_words_decode_as_their_instruction_text() {
        let words = [
            ncl_asm_aarch64::encode(&ncl_asm_aarch64::Inst::Nop, 0).expect("encode"),
            ncl_asm_aarch64::encode(
                &ncl_asm_aarch64::Inst::Ret {
                    rn: ncl_asm_aarch64::Reg(30),
                },
                4,
            )
            .expect("encode"),
        ];
        let bytes = words
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>();
        let decoded = decode(Architecture::Aarch64, &bytes, 0).expect("decode");
        assert_eq!(
            decoded
                .iter()
                .map(|instruction| instruction.text.as_str())
                .collect::<Vec<_>>(),
            ["nop", "ret x30"]
        );
    }

    #[test]
    fn assembler_output_preserves_x86_operands_and_branch_fields() {
        use ncl_asm_x86_64::{Cond, Inst, Reg};

        let mut assembler = ncl_asm_x86_64::Assembler::new();
        let label = assembler.new_label();
        assembler
            .emit(&Inst::MovRR(Reg::Rax, Reg::Rcx))
            .expect("encode mov");
        assembler
            .emit(&Inst::Jcc(Cond::Ne, label))
            .expect("encode branch");
        assembler.bind(label);
        assembler.emit(&Inst::Ret).expect("encode ret");
        let blob = assembler.finish().expect("finish");
        let decoded = decode(Architecture::X86_64, &blob.bytes, 0x1000).expect("decode");

        assert_eq!(decoded[0].text, "mov %rcx, %rax");
        assert_eq!(decoded[1].size, 6);
        assert_eq!(decoded[1].branch_target, Some(decoded[2].address));
        assert_eq!(decoded[2].text, "ret");
    }

    #[test]
    fn assembler_output_preserves_aarch64_adr_and_adrp_targets() {
        use ncl_asm_aarch64::{Assembler, Inst, Reg};

        let mut adr_assembler = Assembler::new();
        let adr_label = adr_assembler.new_label();
        adr_assembler
            .emit(&Inst::Adr {
                rd: Reg(2),
                label: adr_label,
            })
            .expect("encode adr");
        adr_assembler.emit(&Inst::Nop).expect("encode nop");
        adr_assembler.bind(adr_label).expect("bind adr");
        let adr_blob = adr_assembler.finish().expect("finish adr");
        let adr = decode(Architecture::Aarch64, &adr_blob.bytes, 0x1000).expect("decode adr");
        assert_eq!(adr[0].text, "adr x2, #0x1008");
        assert_eq!(adr[0].branch_target, Some(0x1008));

        let mut page_assembler = Assembler::new();
        let page_label = page_assembler.new_label();
        page_assembler
            .emit(&Inst::Adrp {
                rd: Reg(3),
                label: page_label,
            })
            .expect("encode adrp");
        for _ in 0..1023 {
            page_assembler.emit(&Inst::Nop).expect("encode nop");
        }
        page_assembler.bind(page_label).expect("bind adrp");
        let page_blob = page_assembler.finish().expect("finish adrp");
        let page = decode(Architecture::Aarch64, &page_blob.bytes, 0x1000).expect("decode adrp");
        assert_eq!(page[0].text, "adrp x3, #0x2000");
        assert_eq!(page[0].branch_target, Some(0x2000));
    }
}
