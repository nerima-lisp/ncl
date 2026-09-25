use crate::{DecodeError, DecodedInstruction};

#[derive(Clone, Copy)]
struct Rex {
    w: bool,
    r: u8,
    x: u8,
    b: u8,
}

#[derive(Clone, Copy)]
enum Operand {
    Reg(u8),
    Mem {
        base: Option<u8>,
        index: Option<u8>,
        scale: u8,
        disp: i32,
        rip: bool,
    },
}

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
    address: u64,
    rex: Rex,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8], address: u64) -> Self {
        Self {
            bytes,
            pos: 0,
            address,
            rex: Rex {
                w: false,
                r: 0,
                x: 0,
                b: 0,
            },
        }
    }
    fn byte(&mut self) -> Result<u8, DecodeError> {
        let Some(&v) = self.bytes.get(self.pos) else {
            return Err(DecodeError::Truncated {
                address: self.address,
            });
        };
        self.pos += 1;
        Ok(v)
    }
    fn take<const N: usize>(&mut self) -> Result<[u8; N], DecodeError> {
        if self.bytes.len().saturating_sub(self.pos) < N {
            return Err(DecodeError::Truncated {
                address: self.address,
            });
        }
        let mut out = [0; N];
        out.copy_from_slice(&self.bytes[self.pos..self.pos + N]);
        self.pos += N;
        Ok(out)
    }
    fn imm_i8(&mut self) -> Result<i8, DecodeError> {
        Ok(self.byte()? as i8)
    }
    fn imm_i32(&mut self) -> Result<i32, DecodeError> {
        Ok(i32::from_le_bytes(self.take()?))
    }
    fn imm_i64(&mut self) -> Result<i64, DecodeError> {
        Ok(i64::from_le_bytes(self.take()?))
    }
    fn reg(&self, field: u8, ext: u8) -> u8 {
        field | (ext << 3)
    }
    fn modrm(&mut self) -> Result<(u8, u8, u8), DecodeError> {
        let v = self.byte()?;
        Ok((v >> 6, (v >> 3) & 7, v & 7))
    }
    fn operand(&mut self, reg_field: u8, memory_only: bool) -> Result<(u8, Operand), DecodeError> {
        let (mode, _r, rm) = self.modrm()?;
        let reg = self.reg(reg_field, self.rex.r);
        if mode == 3 {
            if memory_only {
                return Err(DecodeError::Invalid {
                    address: self.address,
                    reason: "register used where memory was required".into(),
                });
            }
            return Ok((reg, Operand::Reg(self.reg(rm, self.rex.b))));
        }
        let (base, index, scale, rip, disp) = if rm == 4 {
            let sib = self.byte()?;
            let scale = 1 << (sib >> 6);
            let ix = (sib >> 3) & 7;
            let b = sib & 7;
            let index = if ix == 4 {
                None
            } else {
                Some(self.reg(ix, self.rex.x))
            };
            if mode == 0 && b == 5 {
                (None, index, scale, false, self.imm_i32()?)
            } else {
                (
                    Some(self.reg(b, self.rex.b)),
                    index,
                    scale,
                    false,
                    match mode {
                        1 => self.imm_i8()? as i32,
                        2 => self.imm_i32()?,
                        _ => 0,
                    },
                )
            }
        } else if mode == 0 && rm == 5 {
            (None, None, 1, true, self.imm_i32()?)
        } else {
            (
                Some(self.reg(rm, self.rex.b)),
                None,
                1,
                false,
                match mode {
                    1 => self.imm_i8()? as i32,
                    2 => self.imm_i32()?,
                    _ => 0,
                },
            )
        };
        Ok((
            reg,
            Operand::Mem {
                base,
                index,
                scale,
                disp,
                rip,
            },
        ))
    }
}

fn reg_name(r: u8) -> String {
    match r {
        0 => "rax",
        1 => "rcx",
        2 => "rdx",
        3 => "rbx",
        4 => "rsp",
        5 => "rbp",
        6 => "rsi",
        7 => "rdi",
        8..=15 => return format!("r{r}"),
        _ => unreachable!(),
    }
    .into()
}
fn xmm_name(r: u8) -> String {
    format!("xmm{r}")
}
fn mem_name(base: Option<u8>, index: Option<u8>, scale: u8, disp: i32, rip: bool) -> String {
    let mut s = String::new();
    if disp != 0 || (base.is_none() && !rip) {
        s.push_str(&disp.to_string());
    }
    s.push('(');
    if rip {
        s.push_str("%rip");
    } else if let Some(b) = base {
        s.push('%');
        s.push_str(&reg_name(b));
    }
    if let Some(i) = index {
        s.push(',');
        s.push('%');
        s.push_str(&reg_name(i));
        if scale != 1 {
            s.push(',');
            s.push_str(&scale.to_string());
        }
    }
    s.push(')');
    s
}
fn operand_name(o: Operand) -> String {
    match o {
        Operand::Reg(r) => format!("%{}", reg_name(r)),
        Operand::Mem {
            base,
            index,
            scale,
            disp,
            rip,
        } => mem_name(base, index, scale, disp, rip),
    }
}
fn cond_name(c: u8) -> &'static str {
    [
        "o", "no", "b", "ae", "e", "ne", "be", "a", "s", "ns", "p", "np", "l", "ge", "le", "g",
    ][c as usize]
}
fn bin_name(op: u8) -> Option<&'static str> {
    Some(match op {
        0x01 | 0x03 | 0x05 => "add",
        0x09 | 0x0b | 0x0d => "or",
        0x21 | 0x23 | 0x25 => "and",
        0x29 | 0x2b | 0x2d => "sub",
        0x31 | 0x33 | 0x35 => "xor",
        _ => return None,
    })
}
fn invalid(address: u64, reason: impl Into<String>) -> DecodeError {
    DecodeError::Invalid {
        address,
        reason: reason.into(),
    }
}

fn decode_one(bytes: &[u8], address: u64) -> Result<(u8, String, Option<i64>), DecodeError> {
    let mut r = Reader::new(bytes, address);
    let mut prefix = 0;
    loop {
        match r.bytes.get(r.pos).copied() {
            Some(0x66 | 0xf2) => {
                prefix = r.byte()?;
            }
            Some(b @ 0x40..=0x4f) => {
                r.pos += 1;
                r.rex = Rex {
                    w: b & 8 != 0,
                    r: (b >> 2) & 1,
                    x: (b >> 1) & 1,
                    b: b & 1,
                };
            }
            _ => break,
        }
    }
    let op = r.byte()?;
    if (prefix == 0 || prefix == 0x66) && op == 0x90 {
        return Ok((r.pos as u8, "nop".into(), None));
    }
    if prefix == 0 && op == 0xc3 {
        return Ok((r.pos as u8, "ret".into(), None));
    }
    if prefix == 0 && op == 0xcc {
        return Ok((r.pos as u8, "int3".into(), None));
    }
    if op == 0x0f {
        let op2 = r.byte()?;
        if prefix == 0 && op2 == 0x0b {
            return Ok((r.pos as u8, "ud2".into(), None));
        }
        if prefix != 0 && op2 != 0x1f {
            return decode_sse(&mut r, prefix, op2, address);
        }
        return decode_0f(&mut r, op2, address);
    }
    if prefix == 0 && (0x50..=0x5f).contains(&op) {
        let n = if op < 0x58 { "push" } else { "pop" };
        let rr = (op & 7) | (r.rex.b << 3);
        return Ok((r.pos as u8, format!("{n} %{}", reg_name(rr)), None));
    }
    if prefix == 0 && op == 0x99 && r.rex.w {
        return Ok((r.pos as u8, "cqo".into(), None));
    }
    if prefix == 0 && (0xb8..=0xbf).contains(&op) && r.rex.w {
        let rr = (op & 7) | (r.rex.b << 3);
        return Ok((
            r.pos as u8,
            format!("mov ${}, %{}", r.imm_i64()?, reg_name(rr)),
            None,
        ));
    }
    if prefix == 0 && (0x70..=0x7f).contains(&op) {
        let d = r.imm_i8()? as i64;
        return Ok((r.pos as u8, format!("j{}", cond_name(op & 15)), Some(d)));
    }
    if prefix == 0 && op == 0xe9 {
        return Ok((r.pos as u8 + 4, "jmp".into(), Some(r.imm_i32()? as i64)));
    }
    if prefix == 0 && op == 0xe8 {
        return Ok((r.pos as u8 + 4, "call".into(), Some(r.imm_i32()? as i64)));
    }
    decode_primary(&mut r, prefix, op, address)
}

fn decode_0f(
    r: &mut Reader<'_>,
    op: u8,
    address: u64,
) -> Result<(u8, String, Option<i64>), DecodeError> {
    if op == 0x1f {
        let (_, o) = r.operand(0, true)?;
        return Ok((r.pos as u8, format!("nop {}", operand_name(o)), None));
    }
    if (0x80..=0x8f).contains(&op) {
        let d = r.imm_i32()? as i64;
        return Ok((r.pos as u8, format!("j{}", cond_name(op & 15)), Some(d)));
    }
    decode_0f_modrm(r, op, address)
}

fn decode_0f_modrm(
    r: &mut Reader<'_>,
    op: u8,
    address: u64,
) -> Result<(u8, String, Option<i64>), DecodeError> {
    let (reg, o) = r.operand(0, false)?;
    let name = operand_name(o);
    let text = match op {
        0x10 if r.rex.w => format!("movsd {name}, %{}", xmm_name(reg)),
        0x11 if r.rex.w => format!("movsd %{}, {name}", xmm_name(reg)),
        0x10 | 0x11 | 0x2e | 0x51 | 0x58 | 0x59 | 0x5c | 0x5e | 0x57 | 0x6e | 0x7e | 0x2a
        | 0x2c => return Err(unsupported(address, r)),
        0x40..=0x4f => format!("cmov{} {name}, %{}", cond_name(op & 15), reg_name(reg)),
        0x69 => format!("imul %{}, {name}", reg_name(reg)),
        0x90..=0x9f => {
            if matches!(o, Operand::Reg(_)) {
                format!("set{} {}", cond_name(op & 15), name.replace("%r", "%"))
            } else {
                return Err(invalid(address, "setcc destination is not a register"));
            }
        }
        0xaf => format!("imul {name}, %{}", reg_name(reg)),
        0xb6 | 0xb7 => format!(
            "movz{} {name}, %{}",
            if op == 0xb6 { "b" } else { "w" },
            reg_name(reg)
        ),
        0xbe | 0xbf => format!(
            "movs{} {name}, %{}",
            if op == 0xbe { "b" } else { "w" },
            reg_name(reg)
        ),
        0xba if (reg & 7) == 4 => format!("bt ${}, {name}", r.byte()?),
        _ => return Err(unsupported(address, r)),
    };
    Ok((r.pos as u8, text, None))
}

fn decode_primary(
    r: &mut Reader<'_>,
    prefix: u8,
    op: u8,
    address: u64,
) -> Result<(u8, String, Option<i64>), DecodeError> {
    if prefix != 0 {
        return decode_sse(r, prefix, op, address);
    }
    if (0x40..=0x4f).contains(&op) {
        return Err(unsupported(address, r));
    }
    if (0x50..=0x5f).contains(&op) {
        return Err(unsupported(address, r));
    }
    if (0x58..=0x5f).contains(&op) {
        return Err(unsupported(address, r));
    }
    if op == 0x8d
        || op == 0x8b
        || op == 0x89
        || op == 0x3b
        || op == 0x39
        || op == 0x85
        || op == 0x87
    {
        let (reg, o) = r.operand(0, false)?;
        let n = operand_name(o);
        let rr = format!("%{}", reg_name(reg));
        let text = match op {
            0x8d => format!("lea {n}, {rr}"),
            0x8b => format!("mov {n}, {rr}"),
            0x89 => format!("mov {rr}, {n}"),
            0x3b => format!("cmp {n}, {rr}"),
            0x39 => format!("cmp {rr}, {n}"),
            0x85 => format!("test {rr}, {n}"),
            0x87 => format!("xchg {rr}, {n}"),
            _ => unreachable!(),
        };
        return Ok((r.pos as u8, text, None));
    }
    if let Some(name) = bin_name(op) {
        let (reg, o) = r.operand(0, false)?;
        let n = operand_name(o);
        let rr = format!("%{}", reg_name(reg));
        let text = if op & 2 != 0 {
            format!("{name} {n}, {rr}")
        } else {
            format!("{name} {rr}, {n}")
        };
        return Ok((r.pos as u8, text, None));
    }
    if matches!(op, 0x81 | 0x83) {
        let (ext, o) = r.operand(0, false)?;
        let amount = if op == 0x83 {
            r.imm_i8()? as i32
        } else {
            r.imm_i32()?
        };
        let name = match ext & 7 {
            0 => "add",
            1 => "or",
            4 => "and",
            5 => "sub",
            6 => "xor",
            7 => "cmp",
            _ => return Err(unsupported(address, r)),
        };
        return Ok((
            r.pos as u8,
            format!("{name} ${amount}, {}", operand_name(o)),
            None,
        ));
    }
    if matches!(op, 0xc1 | 0xd3 | 0xf7) {
        return decode_group(r, op, address);
    }
    if op == 0xff {
        let (reg, o) = r.operand(0, false)?;
        let n = operand_name(o);
        return match reg & 7 {
            2 => Ok((r.pos as u8, format!("call *{n}"), None)),
            4 => Ok((r.pos as u8, format!("jmp *{n}"), None)),
            _ => Err(unsupported(address, r)),
        };
    }
    if op == 0x69 {
        let (reg, o) = r.operand(0, false)?;
        return Ok((
            r.pos as u8 + 4,
            format!(
                "imul {}, {}, ${}",
                operand_name(o),
                reg_name(reg),
                r.imm_i32()?
            ),
            None,
        ));
    }
    if op == 0xc7 {
        let (_, o) = r.operand(0, false)?;
        let value = r.imm_i32()?;
        return Ok((
            r.pos as u8,
            format!("mov ${value}, {}", operand_name(o)),
            None,
        ));
    }
    Err(unsupported(address, r))
}

fn decode_group(
    r: &mut Reader<'_>,
    op: u8,
    address: u64,
) -> Result<(u8, String, Option<i64>), DecodeError> {
    let (mode, ext, rm) = r.modrm()?;
    if mode != 3 {
        return Err(unsupported(address, r));
    }
    let reg = r.reg(rm, r.rex.b);
    let name = format!("%{}", reg_name(reg));
    match op {
        0xf7 => Ok((
            r.pos as u8,
            format!(
                "{} {name}",
                match ext {
                    0 => "inc",
                    1 => "dec",
                    2 => "not",
                    3 => "neg",
                    7 => "idiv",
                    _ => return Err(unsupported(address, r)),
                }
            ),
            None,
        )),
        0xc1 => {
            let amount = r.byte()?;
            Ok((
                r.pos as u8,
                format!(
                    "{} ${amount}, {name}",
                    match ext {
                        4 => "shl",
                        5 => "shr",
                        7 => "sar",
                        _ => return Err(unsupported(address, r)),
                    }
                ),
                None,
            ))
        }
        0xd3 => Ok((
            r.pos as u8,
            format!(
                "{} %cl, {name}",
                match ext {
                    4 => "shl",
                    5 => "shr",
                    7 => "sar",
                    _ => return Err(unsupported(address, r)),
                }
            ),
            None,
        )),
        _ => Err(unsupported(address, r)),
    }
}

fn decode_sse(
    r: &mut Reader<'_>,
    prefix: u8,
    op: u8,
    address: u64,
) -> Result<(u8, String, Option<i64>), DecodeError> {
    let (reg, o) = r.operand(0, false)?;
    let n = operand_name(o);
    let x = format!("%{}", xmm_name(reg));
    let n = match o {
        Operand::Reg(v) if !((prefix == 0x66) && matches!(op, 0x6e | 0x7e)) => {
            format!("%{}", xmm_name(v))
        }
        _ => n,
    };
    let name = match (prefix, op) {
        (0xf2, 0x10) => "movsd",
        (0xf2, 0x11) => "movsd",
        (0xf2, 0x2a) => "cvtsi2sd",
        (0xf2, 0x2c) => "cvttsd2si",
        (0xf2, 0x51) => "sqrtsd",
        (0xf2, 0x58) => "addsd",
        (0xf2, 0x59) => "mulsd",
        (0xf2, 0x5c) => "subsd",
        (0xf2, 0x5e) => "divsd",
        (0x66, 0x2e) => "ucomisd",
        (0x66, 0x57) => "xorpd",
        (0x66, 0x6e) => "movq",
        (0x66, 0x7e) => "movq",
        _ => return Err(unsupported(address, r)),
    };
    let text = if op == 0x11 || (prefix == 0xf2 && op == 0x2c) || (prefix == 0x66 && op == 0x7e) {
        format!("{name} {x}, {n}")
    } else {
        format!("{name} {n}, {x}")
    };
    Ok((r.pos as u8, text, None))
}

fn unsupported(address: u64, r: &Reader<'_>) -> DecodeError {
    DecodeError::Unsupported {
        address,
        bytes: r.bytes[..r.pos.min(r.bytes.len())].to_vec(),
    }
}

pub fn decode(bytes: &[u8], base: u64) -> Result<Vec<DecodedInstruction>, DecodeError> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    while pos < bytes.len() {
        let address = base
            .checked_add(pos as u64)
            .ok_or_else(|| invalid(base, "instruction address overflow"))?;
        let (size, text, relative) = decode_one(&bytes[pos..], address)?;
        let size = usize::from(size);
        if size == 0 || size > bytes.len() - pos {
            return Err(DecodeError::Truncated { address });
        }
        let target = match relative {
            Some(d) => Some(
                address
                    .checked_add(size as u64)
                    .and_then(|v| v.checked_add_signed(d))
                    .ok_or_else(|| invalid(address, "branch target overflow"))?,
            ),
            None => None,
        };
        out.push(DecodedInstruction {
            address,
            size: size as u8,
            bytes: bytes[pos..pos + size].to_vec(),
            text,
            branch_target: target,
        });
        pos += size;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn branches_are_absolute() {
        let x = decode(&[0xe9, 1, 0, 0, 0, 0xc3], 0x1000).unwrap();
        assert_eq!(x[0].branch_target, Some(0x1006));
        assert_eq!(x[1].address, 0x1005);
    }
    #[test]
    fn truncation_is_an_error() {
        assert!(matches!(
            decode(&[0x48, 0x89], 0),
            Err(DecodeError::Truncated { .. })
        ));
    }
}
