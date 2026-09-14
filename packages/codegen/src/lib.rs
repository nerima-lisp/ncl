//! Machine-level contracts shared by the native code-generation lanes.

mod abi;
mod frame;
mod isa_x86_64;
mod lowering;
mod machine;
mod relocation;
mod safepoint;
mod templates;

pub use abi::{RegisterId, RuntimeAbi, X86_64Abi};
pub use frame::{FRAME_HEADER_WORDS, FrameLayout};
pub use lowering::compile_function;
pub use machine::{Block, CompiledFunction, DebugLocation, MachineFunction, MachineOp};
pub use relocation::{Relocation, RelocationKind, relocations_from_fixups};
pub use safepoint::{
    FLAG_ALLOCATION_SLOW, FLAG_CALL, FLAG_HAS_DERIVED_ADDRESS, FLAG_LOOP_BACKEDGE, MapError,
    SafepointMap,
};

/// Errors produced while constructing machine-level code-generation data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CodegenError {
    /// The input IR function has no basic blocks.
    EmptyFunction,
    /// An IR block refers to an unknown block.
    UnknownBlock(ncl_ir::BlockId),
    /// An IR value has no assigned frame slot.
    UnknownValue(ncl_ir::ValueId),
    /// The target encoder rejected an instruction.
    Encode(String),
    /// Frame layout arithmetic overflowed.
    FrameOverflow,
}

impl core::fmt::Display for CodegenError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::EmptyFunction => f.write_str("function has no blocks"),
            Self::UnknownBlock(id) => write!(f, "unknown block {id}"),
            Self::UnknownValue(id) => write!(f, "unknown value {id}"),
            Self::Encode(message) => write!(f, "encoding failed: {message}"),
            Self::FrameOverflow => f.write_str("frame layout overflowed"),
        }
    }
}

impl std::error::Error for CodegenError {}
