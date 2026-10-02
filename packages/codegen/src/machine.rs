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

#[cfg(test)]
mod tests {
    use super::{Block, CompiledFunction, MachineFunction, MachineOp};
    use crate::{FrameLayout, Relocation, SafepointMap};
    use ncl_ir::{BlockId, ValueId};

    #[test]
    fn machine_operation_move_accessor_preserves_both_slots() {
        let operation = MachineOp::move_value(3, 11);

        assert_eq!(operation.as_move(), Some((3, 11)));
        assert_eq!(MachineOp::Return.as_move(), None);
        assert_ne!(operation, MachineOp::Return);
    }

    #[test]
    fn exposes_lowered_block_function_and_compiled_metadata() {
        let block = Block::new(BlockId(4), vec![MachineOp::Return]);
        assert_eq!(block.id(), BlockId(4));
        assert_eq!(block.operations(), &[MachineOp::Return]);
        assert_eq!(block.offset(), 0);

        let frame = FrameLayout::new(0, 1, 0).expect("frame");
        let map = SafepointMap::new(3, 5, 5, &[4], &[], 1).expect("map");
        let relocation = Relocation {
            offset: 8,
            kind: crate::RelocationKind::PcRelative32,
            target: ncl_asm_x86_64::Label(2),
            addend: 0,
        };
        let function = MachineFunction::new(
            BlockId(4),
            vec![block],
            frame,
            vec![map.clone()],
            vec![relocation],
            vec![(ValueId(7), 4)],
        );
        assert_eq!(function.entry(), BlockId(4));
        assert_eq!(function.blocks().len(), 1);
        assert_eq!(function.frame(), frame);
        assert_eq!(function.safepoints(), &[map]);
        assert_eq!(function.relocations(), &[relocation]);
        assert_eq!(function.slots(), &[(ValueId(7), 4)]);

        let compiled = CompiledFunction {
            code: vec![0xc3],
            entry_offset: 3,
            relocations: vec![relocation],
            safepoint_maps: function.safepoints().to_vec(),
            frame_size: frame.size_bytes(),
            debug: vec![super::DebugLocation {
                pc_offset: 4,
                location: None,
            }],
        };
        assert_eq!(compiled.code, vec![0xc3]);
        assert_eq!(compiled.entry_offset, 3);
        assert_eq!(compiled.frame_size, 48);
        assert_eq!(compiled.debug.len(), 1);
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
