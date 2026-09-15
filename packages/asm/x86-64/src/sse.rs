use crate::assembler::EncodeError;
use crate::encode::{mem, modrm, rex};
use crate::model::{Inst, Reg, SseOp};

fn code(op: SseOp) -> u8 {
    match op {
        SseOp::Addsd => 0x58,
        SseOp::Subsd => 0x5c,
        SseOp::Mulsd => 0x59,
        SseOp::Divsd => 0x5e,
        SseOp::Ucomisd => 0x2e,
        SseOp::Sqrtsd => 0x51,
    }
}

pub(crate) fn encode(i: &Inst, b: &mut Vec<u8>) -> Result<(), EncodeError> {
    match *i {
        Inst::MovsdRM(x, m) => {
            b.push(0xf2);
            rex(
                b,
                false,
                x.0,
                m.index.map_or(4, Reg::code),
                m.base.map_or(0, Reg::code),
            );
            b.extend([0x0f, 0x10]);
            mem(b, x.0, m)
        }
        Inst::MovsdMR(m, x) => {
            b.push(0xf2);
            rex(
                b,
                false,
                x.0,
                m.index.map_or(4, Reg::code),
                m.base.map_or(0, Reg::code),
            );
            b.extend([0x0f, 0x11]);
            mem(b, x.0, m)
        }
        Inst::MovqXR(x, r) => {
            b.push(0x66);
            rex(b, false, x.0, 4, r.code());
            b.extend([0x0f, 0x6e]);
            modrm(b, 3, x.0, r.code());
            Ok(())
        }
        Inst::MovqRX(r, x) => {
            b.push(0x66);
            rex(b, false, x.0, 4, r.code());
            b.extend([0x0f, 0x7e]);
            modrm(b, 3, x.0, r.code());
            Ok(())
        }
        Inst::Sse(o, d, s) => {
            b.push(0xf2);
            rex(b, false, d.0, 4, s.0);
            b.extend([0x0f, code(o)]);
            modrm(b, 3, d.0, s.0);
            Ok(())
        }
        Inst::SseRM(o, d, m) => {
            b.push(0xf2);
            rex(
                b,
                false,
                d.0,
                m.index.map_or(4, Reg::code),
                m.base.map_or(0, Reg::code),
            );
            b.extend([0x0f, code(o)]);
            mem(b, d.0, m)
        }
        Inst::Xorpd(d, s) => {
            b.push(0x66);
            rex(b, false, d.0, 4, s.0);
            b.extend([0x0f, 0x57]);
            modrm(b, 3, d.0, s.0);
            Ok(())
        }
        Inst::Cvtsi2sd(x, r) => {
            b.push(0xf2);
            rex(b, true, x.0, 4, r.code());
            b.extend([0x0f, 0x2a]);
            modrm(b, 3, x.0, r.code());
            Ok(())
        }
        Inst::Cvtsi2sdRM(x, m) => {
            b.push(0xf2);
            rex(
                b,
                true,
                x.0,
                m.index.map_or(4, Reg::code),
                m.base.map_or(0, Reg::code),
            );
            b.extend([0x0f, 0x2a]);
            mem(b, x.0, m)
        }
        Inst::Cvttsd2si(r, x) => {
            b.push(0xf2);
            rex(b, true, r.code(), 4, x.0);
            b.extend([0x0f, 0x2c]);
            modrm(b, 3, r.code(), x.0);
            Ok(())
        }
        Inst::Cvttsd2siRM(r, m) => {
            b.push(0xf2);
            rex(
                b,
                true,
                r.code(),
                m.index.map_or(4, Reg::code),
                m.base.map_or(0, Reg::code),
            );
            b.extend([0x0f, 0x2c]);
            mem(b, r.code(), m)
        }
        _ => Err(EncodeError::InvalidOperand("not an SSE instruction")),
    }
}
