//! Machine-level containers and emitted function metadata.

#![allow(missing_docs)]

use crate::{FrameLayout, Relocation, SafepointMap};
use ncl_ir::{BlockId, DebugLocationId, ValueId};
use std::collections::HashSet;

/// An invariant violation found in a lowered machine function.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MachineVerifyError {
    /// The machine function has no blocks.
    Empty,
    /// The entry block is not present in the block list.
    MissingEntry(BlockId),
    /// A block id occurs more than once.
    DuplicateBlock(BlockId),
    /// A value is assigned more than once.
    DuplicateValue(ValueId),
    /// Two values use the same frame slot.
    DuplicateSlot(u32),
    /// A value slot lies outside the frame.
    SlotOutOfFrame { value: ValueId, slot: u32 },
    /// A safepoint map violates its wire-format invariants.
    InvalidSafepoint(usize),
}

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
    pub const fn new(id: BlockId, operations: Vec<MachineOp>) -> Self {
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
    pub const fn new(
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

    /// Checks machine-level invariants before encoding.
    ///
    /// The IR verifier remains responsible for SSA dominance and operand
    /// typing. This checker verifies the independent machine representation:
    /// block identity, unique value/slot assignments, frame bounds, and stack
    /// map wire invariants.
    pub fn verify(&self) -> Result<(), MachineVerifyError> {
        if self.blocks.is_empty() {
            return Err(MachineVerifyError::Empty);
        }
        let mut blocks = HashSet::new();
        for block in &self.blocks {
            if !blocks.insert(block.id) {
                return Err(MachineVerifyError::DuplicateBlock(block.id));
            }
        }
        if !blocks.contains(&self.entry) {
            return Err(MachineVerifyError::MissingEntry(self.entry));
        }

        let mut values = HashSet::new();
        let mut slots = HashSet::new();
        for &(value, slot) in &self.slots {
            if !values.insert(value) {
                return Err(MachineVerifyError::DuplicateValue(value));
            }
            if !slots.insert(slot) {
                return Err(MachineVerifyError::DuplicateSlot(slot));
            }
            if slot >= self.frame.frame_words {
                return Err(MachineVerifyError::SlotOutOfFrame { value, slot });
            }
        }
        for (index, map) in self.safepoints.iter().enumerate() {
            if map.validate().is_err() {
                return Err(MachineVerifyError::InvalidSafepoint(index));
            }
        }
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::{Block, MachineFunction, MachineOp, MachineVerifyError};
    use crate::FrameLayout;
    use ncl_ir::{BlockId, ValueId};

    fn function(slots: Vec<(ValueId, u32)>) -> MachineFunction {
        MachineFunction::new(
            BlockId(0),
            vec![Block::new(BlockId(0), vec![MachineOp::Return])],
            FrameLayout::new(0, 2, 0).expect("frame"),
            Vec::new(),
            Vec::new(),
            slots,
        )
    }

    #[test]
    fn verifies_unique_slots_and_frame_bounds() {
        assert!(
            function(vec![(ValueId(0), 4), (ValueId(1), 5)])
                .verify()
                .is_ok()
        );
        assert_eq!(
            function(vec![(ValueId(0), 4), (ValueId(1), 4)]).verify(),
            Err(MachineVerifyError::DuplicateSlot(4))
        );
        assert_eq!(
            function(vec![(ValueId(0), 6)]).verify(),
            Err(MachineVerifyError::SlotOutOfFrame {
                value: ValueId(0),
                slot: 6
            })
        );
    }
}
