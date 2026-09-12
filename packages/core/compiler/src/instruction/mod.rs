//! The stack-bytecode instruction set emitted by the compiler.

mod instruction_set;
mod psetf_place;
mod rotate_shift_place;

pub use instruction_set::{Instruction, PushnewOption};
pub use psetf_place::PsetfPlace;
pub use rotate_shift_place::RotateShiftPlace;
