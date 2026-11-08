#![expect(
    missing_docs,
    reason = "The exported x86-64 instruction model mirrors the ISA operand vocabulary; its typed fields define the encoding contract."
)]
#![expect(
    clippy::pedantic,
    clippy::nursery,
    reason = "The legacy x86-64 ISA encoder uses generated opcode dispatch and model patterns that currently trigger many existing all, pedantic, and nursery diagnostics."
)]

mod assembler;
mod encode;
mod model;
mod sse;
#[cfg(test)]
mod tests;

pub use assembler::{Assembler, CodeBlob, EncodeError, Fixup, FixupKind};
pub use model::{BinOp, Cond, Imm, Inst, Label, Mem, Reg, Scale, Shift, SseOp, Xmm};
