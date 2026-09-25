//! Decoder for the AArch64 instruction subset emitted by `ncl-asm-aarch64`.

use crate::{DecodeError, DecodedInstruction};

fn reg(n: u32) -> String { format!("x{n}") }
fn freg(n: u32, double: bool) -> String { format!("{}{}", if double { 'd' } else { 's' }, n) }
fn signed(value: u32, bits: u32) -> i64 {
    let value = i64::from(value);
    let sign = 1_i64 << (bits - 1);
    if value & sign != 0 { value - (1_i64 << bits) } else { value }
}
fn target(address: u64, displacement: i64) -> Result<u64, String> {
    if displacement >= 0 { address.checked_add(displacement as u64) }
    else { address.checked_sub(displacement.unsigned_abs()) }
        .ok_or_else(|| "branch target overflows address space".to_string())
}
fn hex_bytes(word: u32) -> Vec<u8> { word.to_le_bytes().to_vec() }
fn branch_result(address: u64, text: String, displacement: i64) -> Result<(String, Option<u64>), String> {
    Ok((text, Some(target(address, displacement)?)))
}
fn cond(value: u32) -> &'static str {
    ["eq", "ne", "cs", "cc", "mi", "pl", "vs", "vc", "hi", "ls", "ge", "lt", "gt", "le", "al", "nv"][value as usize]
}
fn spreg(n: u32) -> String { if n == 31 { "sp".into() } else { reg(n) } }
fn extend(option: u32) -> &'static str {
    ["uxtb", "uxth", "uxtw", "uxtx", "sxtb", "sxth", "sxtw", "sxtx"][option as usize]
}
fn memory(word: u32, size: u32, load: bool, signed_word: bool, fp: bool, unscaled: bool) -> Option<String> {
    let rt = if fp { freg(word & 31, size == 8) } else if signed_word { reg(word & 31) } else if size == 8 { reg(word & 31) } else { format!("w{}", word & 31) };
    let base = spreg((word >> 5) & 31);
    let is_register = word & 0x0000_0800 != 0;
    let operand = if is_register {
        let option = (word >> 13) & 7;
        let amount = ((word >> 12) & 1) * size;
        if amount == 0 && option == 3 { format!("[{base}, {}]", reg((word >> 16) & 31)) }
        else if amount == 0 { format!("[{base}, {}, {}]", reg((word >> 16) & 31), extend(option)) }
        else { format!("[{base}, {}, {} #{amount}]", reg((word >> 16) & 31), extend(option)) }
    } else if unscaled {
        let offset = signed((word >> 12) & 0x1ff, 9);
        let mode = (word >> 10) & 3;
        if mode == 1 { format!("[{base}], #{offset}") }
        else if mode == 3 { format!("[{base}, #{offset}]!") }
        else { format!("[{base}, #{offset}]") }
    } else {
        let scale = if fp { 8 } else { size };
        format!("[{base}, #{}]", ((word >> 10) & 0xfff) * scale)
    };
    Some(format!("{} {}, {}", if load { "ldr" } else { "str" }, rt, operand))
}
fn logical_immediate(n: u32, immr: u32, imms: u32) -> Option<u64> {
    let nbits = (n << 6) | (!imms & 0x3f);
    let len = 31 - nbits.leading_zeros();
    if len < 1 { return None; }
    let width = 1_u32 << len;
    let levels = width - 1;
    let ones = (imms & levels) + 1;
    if ones == width { return None; }
    let pattern = (1_u64 << ones) - 1;
    let element = pattern.rotate_right(immr & levels);
    let mut result = 0;
    for offset in (0..64).step_by(width as usize) { result |= element << offset; }
    Some(result)
}
fn decode_word(address: u64, word: u32) -> Result<(String, Option<u64>), DecodeError> {
    let unsupported = || DecodeError::Unsupported { address, bytes: hex_bytes(word) };
    let invalid = |reason: String| DecodeError::Invalid { address, reason };
    let result: Result<(String, Option<u64>), String> = if word == 0xd503_201f { Ok(("nop".into(), None)) }
    else if word == 0xd503_3bbf { Ok(("dmb ish".into(), None)) }
    else if word & 0xffe0_001f == 0xd420_0000 { Ok((format!("brk #{}", (word >> 5) & 0xffff), None)) }
    else if word & 0xffff_fc1f == 0xd65f_0000 { Ok((format!("ret {}", reg((word >> 5) & 31)), None)) }
    else if word & 0xffff_fc1f == 0xd61f_0000 { Ok((format!("br {}", reg((word >> 5) & 31)), None)) }
    else if word & 0xffff_fc1f == 0xd63f_0000 { Ok((format!("blr {}", reg((word >> 5) & 31)), None)) }
    else if word & 0xfc00_0000 == 0x1400_0000 {
        branch_result(address, format!("b #{}", signed(word & 0x03ff_ffff, 26) * 4), signed(word & 0x03ff_ffff, 26) * 4)
    } else if word & 0xfc00_0000 == 0x9400_0000 {
        branch_result(address, format!("bl #{}", signed(word & 0x03ff_ffff, 26) * 4), signed(word & 0x03ff_ffff, 26) * 4)
    } else if word & 0xff00_0010 == 0x5400_0000 {
        let d = signed((word >> 5) & 0x7ffff, 19) * 4;
        branch_result(address, format!("b.{} #{}", cond(word & 15), d), d)
    } else if word & 0x7f00_0000 == 0x3400_0000 || word & 0x7f00_0000 == 0x3500_0000 {
        let d = signed((word >> 5) & 0x7ffff, 19) * 4;
        let op = if word & 0x0100_0000 == 0 { "cbz" } else { "cbnz" };
        branch_result(address, format!("{op} {}, #{}", reg(word & 31), d), d)
    } else if word & 0x7f00_0000 == 0x3600_0000 || word & 0x7f00_0000 == 0x3700_0000 {
        let d = signed((word >> 5) & 0x3fff, 14) * 4;
        let op = if word & 0x0100_0000 == 0 { "tbz" } else { "tbnz" };
        let bit = ((word >> 31) & 1) * 32 + ((word >> 19) & 31);
        branch_result(address, format!("{op} {}, #{bit}, #{}", reg(word & 31), d), d)
    } else if word & 0x9f00_0000 == 0x1000_0000 || word & 0x9f00_0000 == 0x9000_0000 {
        let page = word & 0x8000_0000 != 0;
        let imm = signed(((word >> 5) & 0x7ffff) | ((word >> 24) & 0x3) << 19, 21) | (if page { 0 } else { signed((word >> 29) & 3, 2) << 19 });
        let d = if page { imm << 12 } else { imm };
        let base = if page { address & !0xfff } else { address };
        match target(base, d) {
            Ok(t) => Ok((format!("{} {}, #0x{t:x}", if page { "adrp" } else { "adr" }, reg(word & 31)), Some(t))),
            Err(reason) => Err(reason),
        }
    } else if word & 0xff00_0000 == 0x5800_0000 {
        let d = signed((word >> 5) & 0x7ffff, 19) * 4;
        match target(address, d) {
            Ok(t) => Ok((format!("ldr {}, #0x{t:x}", reg(word & 31)), Some(t))),
            Err(reason) => Err(reason),
        }
    } else if word & 0xff80_0000 == 0xd280_0000 || word & 0xff80_0000 == 0xf280_0000 || word & 0xff80_0000 == 0x9280_0000 {
        let op = match word & 0xff80_0000 { 0xd280_0000 => "movz", 0xf280_0000 => "movk", _ => "movn" };
        Ok((format!("{op} {}, #0x{:x}, lsl #{}", reg(word & 31), (word >> 5) & 0xffff, ((word >> 21) & 3) * 16), None))
    } else if word & 0xff00_0000 == 0x9100_0000 || word & 0xff00_0000 == 0xd100_0000 || word & 0xff00_0000 == 0xb100_0000 || word & 0xff00_0000 == 0xf100_0000 {
        let sub = word & 0x4000_0000 != 0;
        let set = word & 0x2000_0000 != 0;
        let op = if sub { "sub" } else { "add" };
        let op = if set { format!("{op}s") } else { op.into() };
        Ok((format!("{op} {}, {}, #{}{}", spreg(word & 31), spreg((word >> 5) & 31), ((word >> 10) & 0xfff) << (if word & (1 << 22) != 0 { 12 } else { 0 }), if word & (1 << 22) != 0 { " (lsl #12)" } else { "" }), None))
    } else if word & 0xffe0_001f == 0xaa00_03e0 { Ok((format!("mov {}, {}", spreg(word & 31), spreg((word >> 16) & 31)), None)) }
    else if word & 0xff00_0000 == 0x8b00_0000 || word & 0xff00_0000 == 0xcb00_0000 || word & 0xff00_0000 == 0xab00_0000 || word & 0xff00_0000 == 0xeb00_0000 {
        let sub = word & 0x4000_0000 != 0;
        let set = word & 0x2000_0000 != 0;
        let op = if sub { "sub" } else { "add" };
        let op = if set { format!("{op}s") } else { op.into() };
        let amount = (word >> 10) & 63;
        Ok((format!("{op} {}, {}, {}, lsl #{}", spreg(word & 31), spreg((word >> 5) & 31), reg((word >> 16) & 31), amount), None))
    } else if word & 0xff00_0000 == 0x8a00_0000 || word & 0xff00_0000 == 0xaa00_0000 || word & 0xff00_0000 == 0xca00_0000 || word & 0xff00_0000 == 0xea00_0000 {
        let op = match word & 0xff00_0000 { 0x8a00_0000 => "and", 0xaa00_0000 => "orr", 0xca00_0000 => "eor", _ => "tst" };
        let rd = if op == "tst" { "xzr".into() } else { reg(word & 31) };
        Ok((format!("{op} {}, {}, {}, lsl #{}", rd, reg((word >> 5) & 31), reg((word >> 16) & 31), (word >> 10) & 63), None))
    } else if word & 0xff80_0000 == 0x9200_0000 || word & 0xff80_0000 == 0xb200_0000 || word & 0xff80_0000 == 0xd200_0000 || word & 0xff80_0000 == 0xf200_0000 {
        let op = match word & 0xff80_0000 { 0x9200_0000 => "and", 0xb200_0000 => "orr", 0xd200_0000 => "eor", _ => "tst" };
        match logical_immediate((word >> 22) & 1, (word >> 16) & 63, (word >> 10) & 63) {
            Some(value) => {
                let rd = if op == "tst" { "xzr".to_string() } else { reg(word & 31) };
                Ok((format!("{op} {rd}, {}, #0x{value:x}", reg((word >> 5) & 31)), None))
            }
            None => Err("invalid logical immediate".into()),
        }
    } else if word & 0xffff_fc00 == 0x9ac0_2000 || word & 0xffff_fc00 == 0x9ac0_2400 || word & 0xffff_fc00 == 0x9ac0_2800 {
        let op = match word & 0x3c00 { 0x2000 => "lsl", 0x2400 => "lsr", _ => "asr" };
        Ok((format!("{op} {}, {}, {}", reg(word & 31), reg((word >> 5) & 31), reg((word >> 16) & 31)), None))
    } else if word & 0xffe0_0000 == 0xd340_0000 || word & 0xffe0_0000 == 0x9340_0000 {
        let asr = word & 0x8000_0000 != 0;
        let lsr = !asr && word & 0x0000_fc00 == 0x0000_fc00;
        let op = if asr { "asr" } else if lsr { "lsr" } else { "lsl" };
        Ok((format!("{op} {}, {}, #{}", reg(word & 31), reg((word >> 5) & 31), (word >> 10) & 63), None))
    }
    else if word & 0xffc0_0000 == 0xf940_0000 { Ok((memory(word, 8, true, false, false, false).unwrap(), None)) }
    else if word & 0xffc0_0000 == 0xf900_0000 { Ok((memory(word, 8, false, false, false, false).unwrap(), None)) }
    else if word & 0xff80_0000 == 0xf800_0000 { Ok((memory(word, 8, word & 0x0040_0000 != 0, false, false, true).unwrap(), None)) }
    else if word & 0x3e00_0000 == 0x2800_0000 {
        let load = word & 0x0040_0000 != 0;
        let offset = signed((word >> 15) & 0x7f, 7) * 8;
        let mode = (word >> 23) & 3;
        let suffix = if mode == 1 { format!("[{}], #{}", spreg((word >> 5) & 31), offset) } else if mode == 3 { format!("[{}, #{}]!", spreg((word >> 5) & 31), offset) } else { format!("[{}, #{}]", spreg((word >> 5) & 31), offset) };
        Ok((format!("{} {}, {}, {}", if load { "ldp" } else { "stp" }, reg(word & 31), reg((word >> 10) & 31), suffix), None))
    } else if word & 0xffc0_0000 == 0x3940_0000 { Ok((memory(word, 1, true, false, false, false).unwrap(), None)) }
    else if word & 0xffc0_0000 == 0x3900_0000 { Ok((memory(word, 1, false, false, false, false).unwrap(), None)) }
    else if word & 0xffc0_0000 == 0x7940_0000 { Ok((memory(word, 2, true, false, false, false).unwrap(), None)) }
    else if word & 0xffc0_0000 == 0x7900_0000 { Ok((memory(word, 2, false, false, false, false).unwrap(), None)) }
    else if word & 0xffc0_0000 == 0xb940_0000 { Ok((memory(word, 4, true, false, false, false).unwrap(), None)) }
    else if word & 0xffc0_0000 == 0xb900_0000 { Ok((memory(word, 4, false, false, false, false).unwrap(), None)) }
    else if word & 0xffc0_0000 == 0xb840_0000 { Ok((memory(word, 4, true, true, false, true).unwrap(), None)) }
    else if word & 0xffc0_0000 == 0xfd40_0000 { Ok((memory(word, 8, true, false, true, false).unwrap(), None)) }
    else if word & 0xffc0_0000 == 0xfd00_0000 { Ok((memory(word, 8, false, false, true, false).unwrap(), None)) }
    else if word & 0xff00_0000 == 0x9a80_0000 { Ok((format!("csel {}, {}, {}, {}", reg(word & 31), reg((word >> 5) & 31), reg((word >> 16) & 31), cond((word >> 12) & 15)), None)) }
    else if word & 0xffe0_001f == 0x9b00_7c00 { Ok((format!("mul {}, {}, {}", reg(word & 31), reg((word >> 5) & 31), reg((word >> 16) & 31)), None)) }
    else if word & 0xffe0_fc00 == 0x9ac0_0800 || word & 0xffe0_fc00 == 0x9ac0_0c00 { Ok((format!("{} {}, {}, {}", if word & 0x400 == 0 { "udiv" } else { "sdiv" }, reg(word & 31), reg((word >> 5) & 31), reg((word >> 16) & 31)), None)) }
    else { return Err(unsupported()); };
    result.map_err(invalid)
}

/// Decodes a little-endian AArch64 byte stream.
pub fn decode(bytes: &[u8], base: u64) -> Result<Vec<DecodedInstruction>, DecodeError> {
    let mut result = Vec::with_capacity(bytes.len() / 4);
    let mut offset = 0;
    while offset < bytes.len() {
        if bytes.len() - offset < 4 {
            let address = base.checked_add(offset as u64).unwrap_or(base);
            return Err(DecodeError::Truncated { address });
        }
        let address = base.checked_add(offset as u64).ok_or_else(|| DecodeError::Invalid { address: base, reason: "instruction address overflows address space".into() })?;
        let word = u32::from_le_bytes([bytes[offset], bytes[offset + 1], bytes[offset + 2], bytes[offset + 3]]);
        let (text, branch_target) = decode_word(address, word)?;
        result.push(DecodedInstruction { address, size: 4, bytes: bytes[offset..offset + 4].to_vec(), text, branch_target });
        offset += 4;
    }
    Ok(result)
}
