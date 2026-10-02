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
    fn aarch64_decodes_encoded_instruction_families() {
        use ncl_asm_aarch64::{Extend, Inst, MemOperand, Reg, RegOrSp, Shift};

        let x0 = Reg(0);
        let x1 = Reg(1);
        let x2 = Reg(2);
        let sp = RegOrSp::Sp;
        let unsigned = MemOperand::Unsigned {
            base: sp,
            offset: 8,
            scale: 8,
        };
        let instructions = [
            Inst::DmbIsh,
            Inst::Brk { imm: 7 },
            Inst::Ret { rn: x0 },
            Inst::Br { rn: x1 },
            Inst::Blr { rn: x2 },
            Inst::MovZ {
                rd: x0,
                imm: 0x12,
                shift: 16,
            },
            Inst::MovK {
                rd: x0,
                imm: 0x34,
                shift: 32,
            },
            Inst::MovN {
                rd: x0,
                imm: 0x56,
                shift: 48,
            },
            Inst::AddImm {
                rd: sp,
                rn: sp,
                imm: 1,
                shift: true,
            },
            Inst::SubsImm {
                rd: sp,
                rn: sp,
                imm: 2,
                shift: false,
            },
            Inst::Mov {
                rd: RegOrSp::Reg(x0),
                rn: RegOrSp::Reg(x1),
            },
            Inst::Add {
                rd: RegOrSp::Reg(x0),
                rn: RegOrSp::Reg(x1),
                rm: x2,
                shift: Shift::Lsl(3),
            },
            Inst::Sub {
                rd: RegOrSp::Reg(x0),
                rn: RegOrSp::Reg(x1),
                rm: x2,
                shift: Shift::Lsr(2),
            },
            Inst::Adds {
                rd: x0,
                rn: x1,
                rm: x2,
                shift: Shift::Asr(1),
            },
            Inst::Subs {
                rd: x0,
                rn: x1,
                rm: x2,
                shift: Shift::Lsl(1),
            },
            Inst::Add {
                rd: sp,
                rn: sp,
                rm: x0,
                shift: Shift::Lsl(1),
            },
            Inst::And {
                rd: x0,
                rn: x1,
                rm: x2,
                shift: Shift::Lsr(1),
            },
            Inst::Orr {
                rd: x0,
                rn: x1,
                rm: x2,
                shift: Shift::Lsl(1),
            },
            Inst::Eor {
                rd: x0,
                rn: x1,
                rm: x2,
                shift: Shift::Asr(1),
            },
            Inst::Tst {
                rn: x1,
                rm: x2,
                shift: Shift::Lsl(1),
            },
            Inst::AndImm {
                rd: x0,
                rn: x1,
                imm: 0xff,
            },
            Inst::OrrImm {
                rd: x0,
                rn: x1,
                imm: 0xff,
            },
            Inst::EorImm {
                rd: x0,
                rn: x1,
                imm: 0xff,
            },
            Inst::TstImm { rn: x1, imm: 0xff },
            Inst::LslReg {
                rd: x0,
                rn: x1,
                rm: x2,
            },
            Inst::LsrReg {
                rd: x0,
                rn: x1,
                rm: x2,
            },
            Inst::AsrReg {
                rd: x0,
                rn: x1,
                rm: x2,
            },
            Inst::LslImm {
                rd: x0,
                rn: x1,
                amount: 3,
            },
            Inst::LsrImm {
                rd: x0,
                rn: x1,
                amount: 3,
            },
            Inst::AsrImm {
                rd: x0,
                rn: x1,
                amount: 3,
            },
            Inst::Csel {
                rd: x0,
                rn: x1,
                rm: x2,
                cond: ncl_asm_aarch64::Cond::Ne,
            },
            Inst::Mul {
                rd: x0,
                rn: x1,
                rm: x2,
            },
            Inst::Udiv {
                rd: x0,
                rn: x1,
                rm: x2,
            },
            Inst::Sdiv {
                rd: x0,
                rn: x1,
                rm: x2,
            },
            Inst::Ldr {
                rt: x0,
                mem: unsigned,
            },
            Inst::StrW {
                rt: x0,
                mem: MemOperand::Unsigned {
                    base: sp,
                    offset: 4,
                    scale: 4,
                },
            },
            Inst::Ldrb {
                rt: x0,
                mem: MemOperand::Unsigned {
                    base: sp,
                    offset: 1,
                    scale: 1,
                },
            },
            Inst::Ldrh {
                rt: x0,
                mem: MemOperand::Unsigned {
                    base: sp,
                    offset: 2,
                    scale: 2,
                },
            },
            Inst::Ldr {
                rt: x0,
                mem: MemOperand::Unscaled {
                    base: sp,
                    offset: -8,
                },
            },
            Inst::Str {
                rt: x0,
                mem: MemOperand::PreIndex {
                    base: sp,
                    offset: 8,
                },
            },
            Inst::Ldr {
                rt: x0,
                mem: MemOperand::PostIndex {
                    base: sp,
                    offset: 8,
                },
            },
            Inst::Ldr {
                rt: x0,
                mem: MemOperand::Register {
                    base: sp,
                    index: x1,
                    extend: None,
                    shift: 0,
                },
            },
            Inst::Ldr {
                rt: x0,
                mem: MemOperand::Register {
                    base: sp,
                    index: x1,
                    extend: Some(Extend::Uxtw),
                    shift: 0,
                },
            },
            Inst::Ldr {
                rt: x0,
                mem: MemOperand::Register {
                    base: sp,
                    index: x1,
                    extend: Some(Extend::Uxtw),
                    shift: 8,
                },
            },
            Inst::Ldp {
                rt: x0,
                rt2: x1,
                mem: unsigned,
            },
            Inst::Stp {
                rt: x0,
                rt2: x1,
                mem: MemOperand::PreIndex {
                    base: sp,
                    offset: -8,
                },
            },
            Inst::Ldr {
                rt: x0,
                mem: unsigned,
            },
            Inst::Str {
                rt: x0,
                mem: unsigned,
            },
        ];
        let decoded =
            decode(Architecture::Aarch64, &aarch64_encoded(&instructions), 0).expect("decode");
        assert_eq!(decoded.len(), instructions.len());
        assert!(decoded.iter().all(|instruction| instruction.size == 4));
        assert!(
            decoded
                .iter()
                .all(|instruction| instruction.branch_target.is_none())
        );
        assert_eq!(decoded[0].text, "dmb ish");
        assert_eq!(decoded[1].text, "brk #7");
        assert_eq!(decoded[2].text, "ret x0");
        assert_eq!(decoded[3].text, "br x1");
        assert_eq!(decoded[4].text, "blr x2");
        assert_eq!(decoded[5].text, "movz x0, #0x12, lsl #16");
        assert_eq!(decoded[6].text, "movk x0, #0x34, lsl #32");
        assert_eq!(decoded[7].text, "movn x0, #0x56, lsl #48");
        assert_eq!(decoded[8].text, "add sp, sp, #4096 (lsl #12)");
        assert_eq!(decoded[9].text, "subs sp, sp, #2");
        assert_eq!(decoded[10].text, "orr x0, x31, x1, lsl #0");
        assert_eq!(decoded[11].text, "add x0, x1, x2, lsl #3");
        assert_eq!(decoded[12].text, "sub x0, x1, x2, lsl #2");
        assert_eq!(decoded[13].text, "adds x0, x1, x2, lsl #1");
        assert_eq!(decoded[14].text, "subs x0, x1, x2, lsl #1");
        assert_eq!(decoded[15].text, "add sp, sp, x0, lsl #1");
        assert_eq!(decoded[16].text, "and x0, x1, x2, lsl #1");
        assert_eq!(decoded[17].text, "orr x0, x1, x2, lsl #1");
        assert_eq!(decoded[18].text, "eor x0, x1, x2, lsl #1");
        assert_eq!(decoded[19].text, "tst xzr, x1, x2, lsl #1");
        assert_eq!(decoded[20].text, "and x0, x1, #0xff");
        assert_eq!(decoded[21].text, "orr x0, x1, #0xff");
        assert_eq!(decoded[22].text, "eor x0, x1, #0xff");
        assert_eq!(decoded[23].text, "tst xzr, x1, #0xff");
        assert_eq!(decoded[24].text, "lsl x0, x1, x2");
        assert_eq!(decoded[25].text, "lsr x0, x1, x2");
        assert_eq!(decoded[26].text, "asr x0, x1, x2");
        assert_eq!(decoded[27].text, "lsl x0, x1, #3");
        assert_eq!(decoded[28].text, "lsr x0, x1, #3");
        assert_eq!(decoded[29].text, "asr x0, x1, #3");
        assert_eq!(decoded[30].text, "csel x0, x1, x2, ne");
        assert_eq!(decoded[31].text, "mul x0, x1, x2");
        assert_eq!(decoded[32].text, "udiv x0, x1, x2");
        assert_eq!(decoded[33].text, "sdiv x0, x1, x2");
        assert_eq!(decoded[34].text, "ldr x0, [sp, #8]");
        assert_eq!(decoded[35].text, "str w0, [sp, #4]");
        assert_eq!(decoded[36].text, "ldr w0, [sp, #1]");
        assert_eq!(decoded[37].text, "ldr w0, [sp, #2]");
        assert_eq!(decoded[38].text, "ldr x0, [sp, #-8]");
        assert_eq!(decoded[39].text, "str x0, [sp, #8]!");
        assert_eq!(decoded[40].text, "ldr x0, [sp], #8");
        assert_eq!(decoded[41].text, "ldr x0, [sp, x1]");
        assert_eq!(decoded[42].text, "ldr x0, [sp, x1, uxtw]");
        assert_eq!(decoded[43].text, "ldr x0, [sp, x1, uxtw #8]");
        assert_eq!(decoded[44].text, "ldp x0, x1, [sp, #8]");
        assert_eq!(decoded[45].text, "stp x0, x1, [sp, #-8]!");
        assert_eq!(decoded[46].text, "ldr x0, [sp, #8]");
        assert_eq!(decoded[47].text, "str x0, [sp, #8]");
        let floating_point = decode(
            Architecture::Aarch64,
            &aarch64_words(&[0xfd40_07e0, 0xfd00_07e0]),
            0,
        )
        .expect("decode");
        assert_eq!(floating_point[0].text, "ldr d0, [sp, #8]");
        assert_eq!(floating_point[1].text, "str d0, [sp, #8]");
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

        use ncl_asm_aarch64::{Inst, MemOperand, Reg, RegOrSp};
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
