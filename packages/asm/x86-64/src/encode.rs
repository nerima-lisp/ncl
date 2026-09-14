use crate::assembler::{EncodeError, Fixup, FixupKind};
use crate::model::*;
fn rex(b: &mut Vec<u8>, w: bool, r: u8, x: u8, m: u8) {
    let v =
        0x40 | ((w as u8) << 3) | (((r >> 3) & 1) << 2) | (((x >> 3) & 1) << 1) | ((m >> 3) & 1);
    if v != 0x40 {
        b.push(v)
    }
}
fn modrm(b: &mut Vec<u8>, m: u8, r: u8, rm: u8) {
    b.push((m << 6) | ((r & 7) << 3) | (rm & 7));
}
fn i8v(b: &mut Vec<u8>, v: i8) {
    b.push(v as u8)
}
fn i32v(b: &mut Vec<u8>, v: i32) {
    b.extend(v.to_le_bytes())
}
fn i64v(b: &mut Vec<u8>, v: i64) {
    b.extend(v.to_le_bytes())
}
fn mem(b: &mut Vec<u8>, r: u8, m: Mem) -> Result<(), EncodeError> {
    let Some(base) = m.base else {
        if m.index.is_some() {
            return Err(EncodeError::InvalidOperand("index without base"));
        };
        modrm(b, 0, r, 4);
        b.push((m.scale.bits() << 6) | 5);
        i32v(b, m.disp);
        return Ok(());
    };
    let bc = base.code();
    let idx = m.index.map_or(4, Reg::code);
    let sib = bc & 7 == 4 || m.index.is_some();
    let mode = if m.disp == 0 && bc & 7 != 5 {
        0
    } else if i8::try_from(m.disp).is_ok() {
        1
    } else {
        2
    };
    modrm(b, mode, r, if sib { 4 } else { bc });
    if sib {
        b.push((m.scale.bits() << 6) | ((idx & 7) << 3) | (bc & 7));
    }
    if mode == 1 {
        i8v(b, m.disp as i8)
    } else if mode == 2 || mode == 0 && bc & 7 == 5 {
        i32v(b, m.disp)
    }
    Ok(())
}
fn bin(op: BinOp) -> (u8, u8) {
    match op {
        BinOp::Add => (0x01, 0),
        BinOp::Or => (0x09, 1),
        BinOp::And => (0x21, 4),
        BinOp::Sub => (0x29, 5),
        BinOp::Xor => (0x31, 6),
    }
}
fn shift(s: Shift) -> u8 {
    match s {
        Shift::Shl => 4,
        Shift::Shr => 5,
        Shift::Sar => 7,
    }
}
fn unary(b: &mut Vec<u8>, ext: u8, r: Reg) {
    rex(b, true, 0, 4, r.code());
    b.push(0xf7);
    modrm(b, 3, ext, r.code())
}
fn rel(b: &mut Vec<u8>, f: &mut Vec<Fixup>, op: u8, l: Label) {
    b.push(op);
    let o = b.len();
    b.extend([0; 4]);
    f.push(Fixup {
        offset: o,
        kind: FixupKind::Rel32,
        target: l,
    });
}
fn nop(b: &mut Vec<u8>, n: u8) -> Result<(), EncodeError> {
    if !(1..=9).contains(&n) {
        return Err(EncodeError::InvalidNopLength);
    };
    const NOPS: [&[u8]; 9] = [
        &[0x90],
        &[0x66, 0x90],
        &[0x0f, 0x1f, 0x00],
        &[0x0f, 0x1f, 0x40, 0x00],
        &[0x0f, 0x1f, 0x44, 0x00, 0x00],
        &[0x66, 0x0f, 0x1f, 0x44, 0x00, 0x00],
        &[0x0f, 0x1f, 0x80, 0x00, 0x00, 0x00, 0x00],
        &[0x0f, 0x1f, 0x84, 0x00, 0x00, 0x00, 0x00, 0x00],
        &[0x66, 0x0f, 0x1f, 0x84, 0x00, 0x00, 0x00, 0x00, 0x00],
    ];
    b.extend(NOPS[n as usize - 1]);
    Ok(())
}
fn sse_code(o: SseOp) -> u8 {
    match o {
        SseOp::Addsd => 0x58,
        SseOp::Subsd => 0x5c,
        SseOp::Mulsd => 0x59,
        SseOp::Divsd => 0x5e,
        SseOp::Ucomisd => 0x2e,
        SseOp::Sqrtsd => 0x51,
    }
}
pub(crate) fn encode_inst(
    i: &Inst,
    b: &mut Vec<u8>,
    f: &mut Vec<Fixup>,
) -> Result<(), EncodeError> {
    match *i {
        Inst::MovRR(d, s) => {
            rex(b, true, s.code(), 4, d.code());
            b.push(0x89);
            modrm(b, 3, s.code(), d.code())
        }
        Inst::MovRI(r, Imm::I64(v)) => {
            rex(b, true, 0, 4, r.code());
            b.push(0xb8 + (r.code() & 7));
            i64v(b, v)
        }
        Inst::MovRI(r, Imm::I32(v)) => {
            rex(b, true, 0, 4, r.code());
            b.push(0xc7);
            modrm(b, 3, 0, r.code());
            i32v(b, v)
        }
        Inst::MovRI(r, Imm::I8(v)) => {
            rex(b, true, 0, 4, r.code());
            b.push(0xc7);
            modrm(b, 3, 0, r.code());
            i32v(b, v as i32)
        }
        Inst::MovRM(r, m) => {
            rex(
                b,
                true,
                r.code(),
                m.index.map_or(4, Reg::code),
                m.base.map_or(0, Reg::code),
            );
            b.push(0x8b);
            mem(b, r.code(), m)?
        }
        Inst::MovMR(m, r) => {
            rex(
                b,
                true,
                r.code(),
                m.index.map_or(4, Reg::code),
                m.base.map_or(0, Reg::code),
            );
            b.push(0x89);
            mem(b, r.code(), m)?
        }
        Inst::MovMI(m, v) => {
            rex(
                b,
                true,
                0,
                m.index.map_or(4, Reg::code),
                m.base.map_or(0, Reg::code),
            );
            b.push(0xc7);
            mem(b, 0, m)?;
            i32v(b, v)
        }
        Inst::Lea(r, m) => {
            rex(
                b,
                true,
                r.code(),
                m.index.map_or(4, Reg::code),
                m.base.map_or(0, Reg::code),
            );
            b.push(0x8d);
            mem(b, r.code(), m)?
        }
        Inst::BinRR(o, d, s) => {
            let (op, _) = bin(o);
            rex(b, true, s.code(), 4, d.code());
            b.push(op);
            modrm(b, 3, s.code(), d.code())
        }
        Inst::BinRI(o, r, v) => {
            let (_, e) = bin(o);
            rex(b, true, 0, 4, r.code());
            if (-128..=127).contains(&v) {
                b.push(0x83);
                modrm(b, 3, e, r.code());
                i8v(b, v as i8)
            } else {
                b.push(0x81);
                modrm(b, 3, e, r.code());
                i32v(b, v)
            }
        }
        Inst::BinRM(o, r, m) => {
            let (op, _) = bin(o);
            rex(
                b,
                true,
                r.code(),
                m.index.map_or(4, Reg::code),
                m.base.map_or(0, Reg::code),
            );
            b.push(op + 2);
            mem(b, r.code(), m)?
        }
        Inst::BinMR(o, m, r) => {
            let (op, _) = bin(o);
            rex(
                b,
                true,
                r.code(),
                m.index.map_or(4, Reg::code),
                m.base.map_or(0, Reg::code),
            );
            b.push(op);
            mem(b, r.code(), m)?
        }
        Inst::CmpRR(d, s) => {
            rex(b, true, s.code(), 4, d.code());
            b.push(0x39);
            modrm(b, 3, s.code(), d.code())
        }
        Inst::CmpRI(r, v) => {
            rex(b, true, 0, 4, r.code());
            if (-128..=127).contains(&v) {
                b.push(0x83);
                modrm(b, 3, 7, r.code());
                i8v(b, v as i8)
            } else {
                b.push(0x81);
                modrm(b, 3, 7, r.code());
                i32v(b, v)
            }
        }
        Inst::CmpRM(r, m) => {
            rex(
                b,
                true,
                r.code(),
                m.index.map_or(4, Reg::code),
                m.base.map_or(0, Reg::code),
            );
            b.push(0x3b);
            mem(b, r.code(), m)?
        }
        Inst::CmpMR(m, r) => {
            rex(
                b,
                true,
                r.code(),
                m.index.map_or(4, Reg::code),
                m.base.map_or(0, Reg::code),
            );
            b.push(0x39);
            mem(b, r.code(), m)?
        }
        Inst::TestRR(d, s) => {
            rex(b, true, s.code(), 4, d.code());
            b.push(0x85);
            modrm(b, 3, s.code(), d.code())
        }
        Inst::ImulRR(d, s) => {
            rex(b, true, d.code(), 4, s.code());
            b.extend([0x0f, 0xaf]);
            modrm(b, 3, d.code(), s.code())
        }
        Inst::ImulRRI(d, s, v) => {
            rex(b, true, d.code(), 4, s.code());
            b.push(0x69);
            modrm(b, 3, d.code(), s.code());
            i32v(b, v)
        }
        Inst::Neg(r) => unary(b, 3, r),
        Inst::Not(r) => unary(b, 2, r),
        Inst::Inc(r) => unary(b, 0, r),
        Inst::Dec(r) => unary(b, 1, r),
        Inst::Idiv(r) => unary(b, 7, r),
        Inst::Cqo => b.extend([0x48, 0x99]),
        Inst::ShiftImm(s, r, v) => {
            rex(b, true, 0, 4, r.code());
            b.push(0xc1);
            modrm(b, 3, shift(s), r.code());
            b.push(v)
        }
        Inst::ShiftCl(s, r) => {
            rex(b, true, 0, 4, r.code());
            b.push(0xd3);
            modrm(b, 3, shift(s), r.code())
        }
        Inst::Setcc(c, r) => {
            rex(b, false, 0, 4, r.code());
            b.extend([0x0f, 0x90 | c.code()]);
            modrm(b, 3, 0, r.code())
        }
        Inst::Cmovcc(c, d, s) => {
            rex(b, true, d.code(), 4, s.code());
            b.extend([0x0f, 0x40 | c.code()]);
            modrm(b, 3, d.code(), s.code())
        }
        Inst::Jmp(l) => rel(b, f, 0xe9, l),
        Inst::Call(l) => rel(b, f, 0xe8, l),
        Inst::Jcc(c, l) => {
            b.extend([0x0f, 0x80 | c.code()]);
            let o = b.len();
            b.extend([0; 4]);
            f.push(Fixup {
                offset: o,
                kind: FixupKind::Rel32,
                target: l,
            })
        }
        Inst::JmpReg(r) => unary(b, 4, r),
        Inst::CallReg(r) => unary(b, 2, r),
        Inst::JmpMem(m) => {
            rex(
                b,
                false,
                0,
                m.index.map_or(4, Reg::code),
                m.base.map_or(0, Reg::code),
            );
            b.push(0xff);
            mem(b, 4, m)?
        }
        Inst::CallMem(m) => {
            rex(
                b,
                false,
                0,
                m.index.map_or(4, Reg::code),
                m.base.map_or(0, Reg::code),
            );
            b.push(0xff);
            mem(b, 2, m)?
        }
        Inst::Ret => b.push(0xc3),
        Inst::Push(r) => {
            if r.code() >= 8 {
                b.push(0x41)
            }
            b.push(0x50 + (r.code() & 7))
        }
        Inst::Pop(r) => {
            if r.code() >= 8 {
                b.push(0x41)
            }
            b.push(0x58 + (r.code() & 7))
        }
        Inst::Nop(n) => nop(b, n)?,
        Inst::Ud2 => b.extend([0x0f, 0x0b]),
        Inst::Int3 => b.push(0xcc),
        Inst::Xchg(d, s) => {
            rex(b, true, s.code(), 4, d.code());
            b.push(0x87);
            modrm(b, 3, s.code(), d.code())
        }
        Inst::Bt(r, v) => {
            rex(b, true, 0, 4, r.code());
            b.extend([0x0f, 0xba]);
            modrm(b, 3, 4, r.code());
            b.push(v)
        }
        Inst::Movzx(d, s, w) => {
            if w != 8 && w != 16 {
                return Err(EncodeError::InvalidOperand("movzx width"));
            };
            rex(b, true, d.code(), 4, s.code());
            b.extend(if w == 8 { [0x0f, 0xb6] } else { [0x0f, 0xb7] });
            modrm(b, 3, d.code(), s.code())
        }
        Inst::Movsx(d, s, w) => {
            if w != 8 && w != 16 && w != 32 {
                return Err(EncodeError::InvalidOperand("movsx width"));
            };
            rex(b, true, d.code(), 4, s.code());
            if w == 32 {
                b.push(0x63)
            } else {
                b.extend(if w == 8 { [0x0f, 0xbe] } else { [0x0f, 0xbf] })
            };
            modrm(b, 3, d.code(), s.code())
        }
        Inst::MovsdRM(x, m) => {
            rex(
                b,
                false,
                x.0,
                m.index.map_or(4, Reg::code),
                m.base.map_or(0, Reg::code),
            );
            b.extend([0xf2, 0x0f, 0x10]);
            mem(b, x.0, m)?
        }
        Inst::MovsdMR(m, x) => {
            rex(
                b,
                false,
                x.0,
                m.index.map_or(4, Reg::code),
                m.base.map_or(0, Reg::code),
            );
            b.extend([0xf2, 0x0f, 0x11]);
            mem(b, x.0, m)?
        }
        Inst::MovqXR(x, r) => {
            rex(b, false, x.0, 4, r.code());
            b.extend([0x66, 0x0f, 0x6e]);
            modrm(b, 3, x.0, r.code())
        }
        Inst::MovqRX(r, x) => {
            rex(b, false, x.0, 4, r.code());
            b.extend([0x66, 0x0f, 0x7e]);
            modrm(b, 3, x.0, r.code())
        }
        Inst::Sse(o, d, s) => {
            rex(b, false, d.0, 4, s.0);
            b.extend([0xf2, 0x0f, sse_code(o)]);
            modrm(b, 3, d.0, s.0)
        }
        Inst::Xorpd(d, s) => {
            rex(b, false, d.0, 4, s.0);
            b.extend([0x66, 0x0f, 0x57]);
            modrm(b, 3, d.0, s.0)
        }
        Inst::Cvtsi2sd(x, r) => {
            rex(b, true, x.0, 4, r.code());
            b.extend([0xf2, 0x0f, 0x2a]);
            modrm(b, 3, x.0, r.code())
        }
        Inst::Cvttsd2si(r, x) => {
            rex(b, true, r.code(), 4, x.0);
            b.extend([0xf2, 0x0f, 0x2c]);
            modrm(b, 3, r.code(), x.0)
        }
    }
    Ok(())
}
