//! Native instruction decoding for NCL development tools.

#![allow(clippy::missing_errors_doc, clippy::must_use_candidate)]

mod aarch64;
#[cfg(test)]
mod tests_aarch64;
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

    use ncl_asm_aarch64::{Inst, MemOperand, Reg, RegOrSp};

    use super::{Architecture, DecodeError, decode, resolve_labels};

    fn aarch64_words(words: &[u32]) -> Vec<u8> {
        words.iter().flat_map(|word| word.to_le_bytes()).collect()
    }

    fn aarch64_encoded(instructions: &[ncl_asm_aarch64::Inst]) -> Vec<u8> {
        instructions
            .iter()
            .enumerate()
            .flat_map(|(offset, instruction)| {
                ncl_asm_aarch64::encode(instruction, offset * 4)
                    .expect("encode")
                    .to_le_bytes()
            })
            .collect()
    }

    #[test]
    fn x86_decode_has_addresses() {
        let decoded = decode(Architecture::X86_64, &[0x90, 0xc3], 0x1000).expect("decode");
        assert_eq!(decoded[0].address, 0x1000);
        assert_eq!(decoded[1].address, 0x1001);
    }

    #[test]
    fn public_decode_returns_instruction_bytes_text_and_targets() {
        let decoded =
            decode(Architecture::X86_64, &[0xe9, 0, 0, 0, 0, 0xc3], 0x1000).expect("decode");

        assert_eq!(
            decoded,
            vec![
                super::DecodedInstruction {
                    address: 0x1000,
                    size: 5,
                    bytes: vec![0xe9, 0, 0, 0, 0],
                    text: "jmp".into(),
                    branch_target: Some(0x1005),
                },
                super::DecodedInstruction {
                    address: 0x1005,
                    size: 1,
                    bytes: vec![0xc3],
                    text: "ret".into(),
                    branch_target: None,
                },
            ]
        );
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
    fn aarch64_decodes_control_flow_memory_and_arithmetic_forms() {
        let decoded = decode(
            Architecture::Aarch64,
            &aarch64_words(&[
                0x1400_0002, // b #8
                0x5400_0041, // b.ne #8
                0xf940_0883, // ldr x3, [x4, #16]
                0xf900_07e3, // str x3, [sp, #8]
                0x9104_8c20, // add x0, x1, #0x123
                0xd100_43ff, // sub sp, sp, #16
                0xa941_53f3, // ldp x19, x20, [sp, #16]
                0x5800_0047, // ldr x7, #8
            ]),
            0x1000,
        )
        .expect("decode");

        assert_eq!(
            decoded
                .iter()
                .map(|instruction| (
                    instruction.size,
                    instruction.text.as_str(),
                    instruction.branch_target
                ))
                .collect::<Vec<_>>(),
            vec![
                (4, "b #8", Some(0x1008)),
                (4, "b.ne #8", Some(0x100c)),
                (4, "ldr x3, [x4, #16]", None),
                (4, "str x3, [sp, #8]", None),
                (4, "add x0, x1, #291", None),
                (4, "sub sp, sp, #16", None),
                (4, "ldp x19, x20, [sp, #16]", None),
                (4, "ldr x7, #0x1024", Some(0x1024)),
            ]
        );
    }

    #[test]
    fn aarch64_branch_targets_resolve_to_local_labels() {
        let decoded = decode(
            Architecture::Aarch64,
            &aarch64_words(&[
                0x1400_0001, // b #4
                0xd503_201f, // nop
            ]),
            0x2000,
        )
        .expect("decode");

        assert_eq!(
            decoded
                .iter()
                .map(|instruction| (
                    instruction.address,
                    instruction.size,
                    instruction.text.as_str(),
                    instruction.branch_target
                ))
                .collect::<Vec<_>>(),
            vec![(0x2000, 4, "b #4", Some(0x2004)), (0x2004, 4, "nop", None),]
        );
        assert_eq!(resolve_labels(&decoded), vec![Some("L1".to_owned()), None]);
    }

    #[test]
    fn aarch64_reports_unsupported_truncated_and_invalid_input() {
        assert_eq!(
            decode(Architecture::Aarch64, &[0, 0, 0, 0], 0x3000),
            Err(DecodeError::Unsupported {
                address: 0x3000,
                bytes: vec![0, 0, 0, 0],
            })
        );
        assert_eq!(
            decode(Architecture::Aarch64, &[0, 0, 0], 0x3000),
            Err(DecodeError::Truncated { address: 0x3000 })
        );
        assert_eq!(
            decode(
                Architecture::Aarch64,
                &aarch64_words(&[0x1400_0001]),
                u64::MAX
            ),
            Err(DecodeError::Invalid {
                address: u64::MAX,
                reason: "branch target overflows address space".into(),
            })
        );
    }

    #[test]
    fn aarch64_decodes_direct_branch_and_address_forms() {
        let decoded = decode(
            Architecture::Aarch64,
            &aarch64_words(&[
                0x9400_0002, // bl #8
                0x3500_0042, // cbnz x2, #8
                0x3400_0043, // cbz x3, #8
                0x3710_0024, // tbnz x4, #2, #4
                0x3610_0025, // tbz x5, #2, #4
                0x1000_0006, // adr x6, #0
                0x9000_0007, // adrp x7, #0
                0x17ff_ffff, // b #-4
            ]),
            0x1000,
        )
        .expect("decode");
        assert_eq!(
            decoded
                .iter()
                .map(|instruction| (instruction.text.as_str(), instruction.branch_target))
                .collect::<Vec<_>>(),
            vec![
                ("bl #8", Some(0x1008)),
                ("cbnz x2, #8", Some(0x100c)),
                ("cbz x3, #8", Some(0x1010)),
                ("tbnz x4, #2, #4", Some(0x1010)),
                ("tbz x5, #2, #4", Some(0x1014)),
                ("adr x6, #0x1014", Some(0x1014)),
                ("adrp x7, #0x1000", Some(0x1000)),
                ("b #-4", Some(0x1018)),
            ]
        );
    }

    #[test]
    fn aarch64_rejects_invalid_logical_immediates_and_signed_word_loads() {
        assert_eq!(
            decode(
                Architecture::Aarch64,
                &aarch64_words(&[0x9200_fc00]),
                0x3000,
            ),
            Err(DecodeError::Invalid {
                address: 0x3000,
                reason: "invalid logical immediate".into(),
            })
        );
        assert_eq!(
            decode(
                Architecture::Aarch64,
                &aarch64_words(&[0x9200_f800]),
                0x3000,
            ),
            Err(DecodeError::Invalid {
                address: 0x3000,
                reason: "invalid logical immediate".into(),
            })
        );
        assert_eq!(
            decode(
                Architecture::Aarch64,
                &aarch64_words(&[0x9240_fc00]),
                0x3000,
            ),
            Err(DecodeError::Invalid {
                address: 0x3000,
                reason: "invalid logical immediate".into(),
            })
        );
        let decoded =
            decode(Architecture::Aarch64, &aarch64_words(&[0xb840_03e0]), 0).expect("decode");
        assert_eq!(decoded[0].text, "ldr x0, [sp, #0]");
    }

    #[test]
    fn aarch64_decodes_remaining_memory_forms_and_reports_target_overflow() {
        let widths = decode(
            Architecture::Aarch64,
            &aarch64_words(&[0x3900_07e0, 0x7900_07e0, 0xb940_07e0]),
            0,
        )
        .expect("decode");
        assert_eq!(
            widths
                .iter()
                .map(|instruction| instruction.text.as_str())
                .collect::<Vec<_>>(),
            ["str w0, [sp, #1]", "str w0, [sp, #2]", "ldr w0, [sp, #4]"]
        );

        let pair = decode(
            Architecture::Aarch64,
            &aarch64_encoded(&[Inst::Ldp {
                rt: Reg(0),
                rt2: Reg(1),
                mem: MemOperand::PostIndex {
                    base: RegOrSp::Sp,
                    offset: 8,
                },
            }]),
            0,
        )
        .expect("decode");
        assert_eq!(pair[0].text, "ldp x0, x1, [sp], #8");

        assert_eq!(
            decode(
                Architecture::Aarch64,
                &aarch64_words(&[0x9000_0020]),
                u64::MAX,
            ),
            Err(DecodeError::Invalid {
                address: u64::MAX,
                reason: "branch target overflows address space".into(),
            })
        );
        assert_eq!(
            decode(
                Architecture::Aarch64,
                &aarch64_words(&[0x587f_ffe0]),
                u64::MAX,
            ),
            Err(DecodeError::Invalid {
                address: u64::MAX,
                reason: "branch target overflows address space".into(),
            })
        );
    }

    #[test]
    fn decode_errors_have_stable_display_text() {
        assert_eq!(
            DecodeError::Truncated { address: 0x1000 }.to_string(),
            "truncated instruction at 0x1000"
        );
        assert_eq!(
            DecodeError::Unsupported {
                address: 0x2000,
                bytes: vec![0xff],
            }
            .to_string(),
            "unsupported instruction at 0x2000"
        );
        assert_eq!(
            DecodeError::Invalid {
                address: 0x3000,
                reason: "bad encoding".into(),
            }
            .to_string(),
            "invalid instruction at 0x3000: bad encoding"
        );
    }
}
