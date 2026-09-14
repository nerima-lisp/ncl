#![allow(missing_docs)]

mod encode;
mod model;
#[cfg(test)]
mod tests;

pub use encode::{Assembler, CodeBlob, DecodeError, EncodeError, Fixup, FixupKind, decode, display};
pub use model::{Cond, Imm, Inst, Label, Mem, Reg, Scale, Xmm};
