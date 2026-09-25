use ncl_asm_aarch64::{Assembler, CodeBlob, Inst, Label, Reg};

#[allow(
    clippy::option_if_let_else,
    reason = "The helper is const so test register literals remain compile-time values."
)]
pub const fn x(n: u8) -> Reg {
    match Reg::new(n) {
        Ok(register) => register,
        Err(_) => Reg(0),
    }
}

pub fn word_at(blob: &CodeBlob, offset: usize) -> u32 {
    u32::from_le_bytes(blob.bytes[offset..offset + 4].try_into().unwrap())
}

pub fn assert_negative_delta<F>(make: F, expected: u32)
where
    F: FnOnce(Label) -> Inst,
{
    let mut assembler = Assembler::new();
    let label = assembler.new_label();
    assembler.bind(label).unwrap();
    assembler.emit(&Inst::Nop).unwrap();
    assembler.emit(&make(label)).unwrap();
    assert_eq!(word_at(&assembler.finish().unwrap(), 4), expected);
}

pub fn assert_boundary_deltas<F>(make: F, max_delta: usize)
where
    F: Fn(Label) -> Inst + Copy,
{
    let mut assembler = Assembler::new();
    let label = assembler.new_label();
    assembler.emit(&make(label)).unwrap();
    for _ in 0..(max_delta / 4 - 1) {
        assembler.emit(&Inst::Nop).unwrap();
    }
    assembler.bind(label).unwrap();
    assert!(assembler.finish().is_ok());

    let mut assembler = Assembler::new();
    let label = assembler.new_label();
    assembler.emit(&make(label)).unwrap();
    for _ in 0..(max_delta / 4) {
        assembler.emit(&Inst::Nop).unwrap();
    }
    assembler.bind(label).unwrap();
    assert!(matches!(
        assembler.finish(),
        Err(ncl_asm_aarch64::EncodeError::RelocationOutOfRange { .. })
    ));

    let mut assembler = Assembler::new();
    let label = assembler.new_label();
    assembler.bind(label).unwrap();
    for _ in 0..=(max_delta / 4) {
        assembler.emit(&Inst::Nop).unwrap();
    }
    assembler.emit(&make(label)).unwrap();
    assert!(assembler.finish().is_ok());

    let mut assembler = Assembler::new();
    let label = assembler.new_label();
    assembler.bind(label).unwrap();
    for _ in 0..(max_delta / 4 + 2) {
        assembler.emit(&Inst::Nop).unwrap();
    }
    assembler.emit(&make(label)).unwrap();
    assert!(matches!(
        assembler.finish(),
        Err(ncl_asm_aarch64::EncodeError::RelocationOutOfRange { .. })
    ));
}
