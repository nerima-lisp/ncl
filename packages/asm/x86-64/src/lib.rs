#![allow(missing_docs, clippy::all, clippy::pedantic, clippy::nursery)]

mod assembler;
mod disassemble;
mod encode;
mod model;
#[cfg(test)]
mod tests;

pub use assembler::{Assembler, CodeBlob, EncodeError, Fixup, FixupKind};
pub use disassemble::{DecodeError, decode, display};
pub use model::{Cond, Imm, Inst, Label, Mem, Reg, Scale, Xmm};
