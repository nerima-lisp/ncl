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
/// A lowered basic block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Block {
    /// Source block identity.
    pub id: BlockId,
    /// Emitted operations.
    pub operations: Vec<MachineOp>,
    /// Code-relative offset.
    pub offset: u32,
}
/// A lowered function before final byte encoding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MachineFunction {
    /// Entry block.
    pub entry: BlockId,
    /// Lowered blocks.
    pub blocks: Vec<Block>,
    /// Fixed frame layout.
    pub frame: FrameLayout,
    /// Safepoint metadata.
    pub safepoints: Vec<SafepointMap>,
    /// Relocations.
    pub relocations: Vec<Relocation>,
    /// Value-to-slot assignments.
    pub slots: Vec<(ValueId, u32)>,
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
