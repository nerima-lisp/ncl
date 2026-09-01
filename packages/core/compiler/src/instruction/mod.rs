//! The stack-bytecode instruction set emitted by the compiler.

mod rotate_shift_place;
mod instruction_set;

pub use instruction_set::{Instruction, PsetfPlace};
pub use rotate_shift_place::RotateShiftPlace;
