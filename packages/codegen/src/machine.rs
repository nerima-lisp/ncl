//! Machine-level containers and emitted function metadata.

#![allow(missing_docs)]

use crate::{FrameLayout, Relocation, SafepointMap};
use ncl_ir::{BlockId, DebugLocationId, ValueId};

/// A machine operation retained for diagnostics and template inspection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MachineOp {
    /// Move a value between frame slots.
    Move { source: u32, destination: u32 },
    /// Return from the function.
    Return,
}

impl MachineOp {
    /// Creates a value move operation.
    #[must_use]
    pub const fn move_value(source: u32, destination: u32) -> Self {
        Self::Move {
            source,
            destination,
        }
    }

    /// Returns the source and destination slots for a move operation.
    #[must_use]
    pub const fn as_move(&self) -> Option<(u32, u32)> {
        match self {
            Self::Move {
                source,
                destination,
            } => Some((*source, *destination)),
            Self::Return => None,
        }
    }
}
/// A lowered basic block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Block {
    /// Source block identity.
    pub(crate) id: BlockId,
    /// Emitted operations.
    pub(crate) operations: Vec<MachineOp>,
    /// Code-relative offset.
    pub(crate) offset: u32,
}

impl Block {
    /// Creates a block with an initial code-relative offset of zero.
    #[must_use]
    pub fn new(id: BlockId, operations: Vec<MachineOp>) -> Self {
        Self {
            id,
            operations,
            offset: 0,
        }
    }

    /// Returns the source block identity.
    #[must_use]
    pub const fn id(&self) -> BlockId {
        self.id
    }

    /// Returns the emitted operations.
    #[must_use]
    pub fn operations(&self) -> &[MachineOp] {
        &self.operations
    }

    /// Returns the code-relative offset.
    #[must_use]
    pub const fn offset(&self) -> u32 {
        self.offset
    }
}
/// A lowered function before final byte encoding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MachineFunction {
    /// Entry block.
    pub(crate) entry: BlockId,
    /// Lowered blocks.
    pub(crate) blocks: Vec<Block>,
    /// Fixed frame layout.
    pub(crate) frame: FrameLayout,
    /// Safepoint metadata.
    pub(crate) safepoints: Vec<SafepointMap>,
    /// Relocations.
    pub(crate) relocations: Vec<Relocation>,
    /// Value-to-slot assignments.
    pub(crate) slots: Vec<(ValueId, u32)>,
}

impl MachineFunction {
    /// Creates a lowered machine function.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        entry: BlockId,
        blocks: Vec<Block>,
        frame: FrameLayout,
        safepoints: Vec<SafepointMap>,
        relocations: Vec<Relocation>,
        slots: Vec<(ValueId, u32)>,
    ) -> Self {
        Self {
            entry,
            blocks,
            frame,
            safepoints,
            relocations,
            slots,
        }
    }

    /// Returns the entry block without exposing the mutable representation.
    #[must_use]
    pub const fn entry(&self) -> BlockId {
        self.entry
    }

    /// Returns the lowered blocks without exposing the backing vector.
    #[must_use]
    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }

    /// Returns the fixed frame layout.
    #[must_use]
    pub const fn frame(&self) -> FrameLayout {
        self.frame
    }

    /// Returns safepoint metadata without exposing the backing vector.
    #[must_use]
    pub fn safepoints(&self) -> &[SafepointMap] {
        &self.safepoints
    }

    /// Returns relocations without exposing the backing vector.
    #[must_use]
    pub fn relocations(&self) -> &[Relocation] {
        &self.relocations
    }

    /// Returns value-to-slot assignments without exposing the backing vector.
    #[must_use]
    pub fn slots(&self) -> &[(ValueId, u32)] {
        &self.slots
    }
}
/// A source location attached to generated code.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DebugLocation {
    /// Code-relative offset.
    pub pc_offset: u32,
    /// IR debug identity.
    pub location: Option<DebugLocationId>,
}
/// Final code and metadata returned to the caller.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledFunction {
    /// Encoded machine bytes.
    pub code: Vec<u8>,
    /// Entry offset in `code`.
    pub entry_offset: u32,
    /// Relocations requiring an object writer.
    pub relocations: Vec<Relocation>,
    /// Safepoint records.
    pub safepoint_maps: Vec<SafepointMap>,
    /// Frame size in bytes.
    pub frame_size: u32,
    /// Debug locations.
    pub debug: Vec<DebugLocation>,
}
