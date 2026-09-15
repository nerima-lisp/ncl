use crate::{EncodeError, Reg, RegOrSp, Shift};

fn r(r: Reg) -> u32 {
    u32::from(r.0)
}

fn rs(value: RegOrSp) -> u32 {
    match value {
        RegOrSp::Reg(x) => r(x),
        RegOrSp::Sp => 31,
    }
}

fn shift(s: Shift) -> (u32, u32) {
    match s {
        Shift::Lsl(n) => (0, u32::from(n)),
        Shift::Lsr(n) => (1, u32::from(n)),
        Shift::Asr(n) => (2, u32::from(n)),
    }
}

pub(crate) fn wide(base: u32, rd: Reg, immv: u16, s: u8, _k: u8) -> Result<u32, EncodeError> {
    if !s.is_multiple_of(16) || s > 48 {
        return Err(EncodeError::ImmediateOutOfRange {
            value: i64::from(s),
            bits: 6,
        });
    }
    Ok(base | u32::from(immv) << 5 | u32::from(s / 16) << 21 | rs(rd.into()))
}

pub(crate) fn addsub(
    rd: RegOrSp,
    rn: RegOrSp,
    im: u16,
    s: bool,
    sub: bool,
) -> Result<u32, EncodeError> {
    if im > 4095 {
        return Err(EncodeError::ImmediateOutOfRange {
            value: i64::from(im),
            bits: 12,
        });
    }
    Ok((if sub { 0xD100_0000 } else { 0x9100_0000 })
        | if s { 1 << 22 } else { 0 }
        | u32::from(im) << 10
        | rs(rn) << 5
        | rs(rd))
}

pub(crate) fn reg3<R: Into<RegOrSp>>(
    base: u32,
    rd: R,
    rn: RegOrSp,
    rm: Reg,
    s: Shift,
) -> Result<u32, EncodeError> {
    let (k, n) = shift(s);
    if n > 63 {
        return Err(EncodeError::ImmediateOutOfRange {
            value: i64::from(n),
            bits: 6,
        });
    }
    Ok(base | r(rm) << 16 | k << 22 | n << 10 | rs(rn) << 5 | rs(rd.into()))
}
