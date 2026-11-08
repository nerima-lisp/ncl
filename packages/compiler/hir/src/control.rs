//! Structured non-local exits and per-form control-flow graphs.

#[path = "control/dominance.rs"]
mod dominance;

use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt::{Display, Formatter};

use crate::arena::Arena;
use crate::ids::{BlockId, CleanupId, FormId, IdError, NodeId};

/// The target carried by an exit that may cross a function boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ExitTarget {
    /// A lexical BLOCK node.
    Block(NodeId),
    /// A local TAGBODY block.
    Tag(BlockId),
    /// A dynamically tagged CATCH node.
    Catch(NodeId),
    /// A function return target.
    Function(NodeId),
    /// A runtime-resolved THROW tag expression.
    Dynamic(NodeId),
}

/// A basic-block terminator.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Terminator {
    /// Transfer to `target`, the block's only successor.
    Jump(BlockId),
    /// Branch according to a Lisp truth value.
    Conditional {
        /// The branch condition.
        condition: NodeId,
        /// The target when the condition is non-NIL.
        if_true: BlockId,
        /// The target when the condition is NIL.
        if_false: BlockId,
    },
    /// Return the listed values in order.
    Return {
        /// Values in source order.
        values: Vec<NodeId>,
    },
    /// Leave through a non-local target after running cleanup records.
    NonLocalExit {
        /// The semantic exit target.
        target: ExitTarget,
        /// The values carried by the exit, in order.
        values: Vec<NodeId>,
        /// Cleanup records to run before transferring control.
        cleanups: Vec<CleanupId>,
    },
    /// A block with no reachable continuation.
    Unreachable,
}

/// A completed basic block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BasicBlock {
    function: NodeId,
    operations: Vec<NodeId>,
    terminator: Terminator,
}

impl BasicBlock {
    /// Returns the function node that owns this block.
    #[must_use]
    pub const fn function(&self) -> NodeId {
        self.function
    }

    /// Returns the operations in this block in execution order.
    #[must_use]
    pub fn operations(&self) -> &[NodeId] {
        &self.operations
    }

    /// Returns this block's unique terminator.
    #[must_use]
    pub const fn terminator(&self) -> &Terminator {
        &self.terminator
    }
}

/// The blocks belonging to one function in the form's CFG.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionFlow {
    function: NodeId,
    entry: BlockId,
    blocks: Vec<BlockId>,
}

impl FunctionFlow {
    /// Returns the HIR node identifying this function.
    #[must_use]
    pub const fn function(&self) -> NodeId {
        self.function
    }

    /// Returns the entry block.
    #[must_use]
    pub const fn entry(&self) -> BlockId {
        self.entry
    }

    /// Returns all blocks in allocation order.
    #[must_use]
    pub fn blocks(&self) -> &[BlockId] {
        &self.blocks
    }
}

/// A completed CFG sharing one block arena across all functions in a form.
#[derive(Debug)]
pub struct ControlFlowGraph {
    blocks: Arena<BasicBlock, BlockId>,
    functions: Vec<FunctionFlow>,
}

impl ControlFlowGraph {
    /// Returns a block when `id` belongs to this graph.
    #[must_use]
    pub fn block(&self, id: BlockId) -> Option<&BasicBlock> {
        self.blocks.get(id)
    }

    /// Returns the flow record for a function node.
    #[must_use]
    pub fn function(&self, function: NodeId) -> Option<&FunctionFlow> {
        self.functions.iter().find(|flow| flow.function == function)
    }

    /// Returns all function-flow records in declaration order.
    #[must_use]
    pub fn functions(&self) -> &[FunctionFlow] {
        &self.functions
    }

    /// Returns the dominator set for every block in `function`'s control flow.
    ///
    /// Unreachable blocks are dominated only by themselves. Returns `None` if
    /// `function` has no registered control-flow graph.
    #[must_use]
    pub fn dominators(&self, function: NodeId) -> Option<HashMap<BlockId, HashSet<BlockId>>> {
        let flow = self.function(function)?;
        Some(dominance::compute(self, flow))
    }
}

/// Errors from constructing or validating a control-flow graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ControlError {
    /// A block or reference index exceeded the supported range.
    Id(IdError),
    /// A function node belongs to another form.
    ForeignFunction(NodeId),
    /// The same function node was registered more than once.
    DuplicateFunction(NodeId),
    /// No function with this node identity was registered.
    UnknownFunction(NodeId),
    /// A basic block belongs to another form.
    ForeignBlock(BlockId),
    /// A block does not exist in the graph.
    UnknownBlock(BlockId),
    /// A HIR node belongs to another form.
    ForeignNode(NodeId),
    /// A cleanup identity belongs to another form.
    ForeignCleanup(CleanupId),
    /// The block already has a terminator.
    BlockAlreadyTerminated(BlockId),
    /// A function has no entry block.
    MissingEntry(NodeId),
    /// A block has no terminator.
    MissingTerminator(BlockId),
    /// An edge targets a block outside its function.
    InvalidSuccessor {
        /// The block containing the invalid edge.
        source: BlockId,
        /// The edge's target block.
        target: BlockId,
    },
}

impl Display for ControlError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "control-flow error: {self:?}")
    }
}

impl Error for ControlError {}

impl From<IdError> for ControlError {
    fn from(error: IdError) -> Self {
        Self::Id(error)
    }
}

#[derive(Clone, Debug)]
struct OpenBlock {
    function: NodeId,
    operations: Vec<NodeId>,
    terminator: Option<Terminator>,
}

#[derive(Clone, Debug)]
struct OpenFunctionFlow {
    function: NodeId,
    entry: Option<BlockId>,
    blocks: Vec<BlockId>,
}

/// A builder that allocates unique block identities across a whole form.
#[derive(Debug)]
pub struct ControlFlowBuilder {
    form: FormId,
    blocks: Arena<OpenBlock, BlockId>,
    functions: Vec<OpenFunctionFlow>,
}

impl ControlFlowBuilder {
    /// Creates a builder for one top-level form.
    #[must_use]
    pub fn new(form: FormId) -> Self {
        Self {
            form,
            blocks: Arena::new(form),
            functions: Vec::new(),
        }
    }

    /// Registers a function before allocating its blocks.
    ///
    /// # Errors
    ///
    /// Returns `ControlError::ForeignFunction` for a node from another form or
    /// `ControlError::DuplicateFunction` if the node was already registered.
    pub fn begin_function(&mut self, function: NodeId) -> Result<(), ControlError> {
        if function.form() != self.form {
            return Err(ControlError::ForeignFunction(function));
        }
        if self.functions.iter().any(|flow| flow.function == function) {
            return Err(ControlError::DuplicateFunction(function));
        }
        self.functions.push(OpenFunctionFlow {
            function,
            entry: None,
            blocks: Vec::new(),
        });
        Ok(())
    }

    /// Allocates a block for a registered function.
    ///
    /// The first block allocated for a function becomes its entry block.
    ///
    /// # Errors
    ///
    /// Returns `ControlError::UnknownFunction` if `function` is not registered
    /// or `ControlError::Id` when the block identity space is exhausted.
    pub fn create_block(&mut self, function: NodeId) -> Result<BlockId, ControlError> {
        if function.form() != self.form {
            return Err(ControlError::ForeignFunction(function));
        }
        let flow = self
            .functions
            .iter_mut()
            .find(|flow| flow.function == function)
            .ok_or(ControlError::UnknownFunction(function))?;
        let block = self.blocks.allocate(OpenBlock {
            function,
            operations: Vec::new(),
            terminator: None,
        })?;
        if flow.entry.is_none() {
            flow.entry = Some(block);
        }
        flow.blocks.push(block);
        Ok(block)
    }

    /// Appends an operation to an unterminated block.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown block, foreign node, or terminated block.
    pub fn append_operation(&mut self, block: BlockId, node: NodeId) -> Result<(), ControlError> {
        if node.form() != self.form {
            return Err(ControlError::ForeignNode(node));
        }
        let open = self.open_block_mut(block)?;
        if open.terminator.is_some() {
            return Err(ControlError::BlockAlreadyTerminated(block));
        }
        open.operations.push(node);
        Ok(())
    }

    /// Sets the unique terminator for a block.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown block, foreign reference, or a second
    /// terminator assignment.
    pub fn set_terminator(
        &mut self,
        block: BlockId,
        terminator: Terminator,
    ) -> Result<(), ControlError> {
        validate_terminator_references(self.form, &terminator)?;
        let open = self.open_block_mut(block)?;
        if open.terminator.is_some() {
            return Err(ControlError::BlockAlreadyTerminated(block));
        }
        open.terminator = Some(terminator);
        Ok(())
    }

    /// Completes the graph after validating every block and local edge.
    ///
    /// # Errors
    ///
    /// Returns an error for a missing entry, unterminated block, invalid edge,
    /// or exhausted block identity space.
    pub fn finish(self) -> Result<ControlFlowGraph, ControlError> {
        let mut blocks = Arena::new(self.form);
        for open in self.blocks.into_values() {
            let expected = BlockId::try_from_index(self.form, blocks.len())?;
            let terminator = open
                .terminator
                .ok_or(ControlError::MissingTerminator(expected))?;
            blocks.allocate(BasicBlock {
                function: open.function,
                operations: open.operations,
                terminator,
            })?;
        }

        let mut functions = Vec::with_capacity(self.functions.len());
        for open in self.functions {
            let entry = open
                .entry
                .ok_or(ControlError::MissingEntry(open.function))?;
            functions.push(FunctionFlow {
                function: open.function,
                entry,
                blocks: open.blocks,
            });
        }
        validate_successors(&blocks, &functions)?;
        Ok(ControlFlowGraph { blocks, functions })
    }

    fn open_block_mut(&mut self, block: BlockId) -> Result<&mut OpenBlock, ControlError> {
        if block.form() != self.form {
            return Err(ControlError::ForeignBlock(block));
        }
        self.blocks
            .get_mut(block)
            .ok_or(ControlError::UnknownBlock(block))
    }
}

fn validate_terminator_references(
    form: FormId,
    terminator: &Terminator,
) -> Result<(), ControlError> {
    match terminator {
        Terminator::Jump(target) => validate_block_owner(form, *target),
        Terminator::Conditional {
            condition,
            if_true,
            if_false,
        } => {
            validate_node_owner(form, *condition)?;
            validate_block_owner(form, *if_true)?;
            validate_block_owner(form, *if_false)
        }
        Terminator::Return { values } => {
            for value in values {
                validate_node_owner(form, *value)?;
            }
            Ok(())
        }
        Terminator::NonLocalExit {
            target,
            values,
            cleanups,
        } => {
            validate_target_owner(form, *target)?;
            for value in values {
                validate_node_owner(form, *value)?;
            }
            for cleanup in cleanups {
                if cleanup.form() != form {
                    return Err(ControlError::ForeignCleanup(*cleanup));
                }
            }
            Ok(())
        }
        Terminator::Unreachable => Ok(()),
    }
}

fn validate_target_owner(form: FormId, target: ExitTarget) -> Result<(), ControlError> {
    match target {
        ExitTarget::Block(node)
        | ExitTarget::Catch(node)
        | ExitTarget::Function(node)
        | ExitTarget::Dynamic(node) => validate_node_owner(form, node),
        ExitTarget::Tag(block) => validate_block_owner(form, block),
    }
}

fn validate_node_owner(form: FormId, node: NodeId) -> Result<(), ControlError> {
    if node.form() == form {
        Ok(())
    } else {
        Err(ControlError::ForeignNode(node))
    }
}

fn validate_block_owner(form: FormId, block: BlockId) -> Result<(), ControlError> {
    if block.form() == form {
        Ok(())
    } else {
        Err(ControlError::ForeignBlock(block))
    }
}

fn validate_successors(
    blocks: &Arena<BasicBlock, BlockId>,
    functions: &[FunctionFlow],
) -> Result<(), ControlError> {
    for flow in functions {
        for source in &flow.blocks {
            let block = blocks
                .get(*source)
                .ok_or(ControlError::UnknownBlock(*source))?;
            for target in terminator_blocks(&block.terminator).into_iter().flatten() {
                let successor = blocks.get(target).ok_or(ControlError::InvalidSuccessor {
                    source: *source,
                    target,
                })?;
                if successor.function != flow.function {
                    return Err(ControlError::InvalidSuccessor {
                        source: *source,
                        target,
                    });
                }
            }
        }
    }
    Ok(())
}

const fn terminator_blocks(terminator: &Terminator) -> [Option<BlockId>; 2] {
    match terminator {
        Terminator::Jump(target) => [Some(*target), None],
        Terminator::Conditional {
            if_true, if_false, ..
        } => [Some(*if_true), Some(*if_false)],
        Terminator::NonLocalExit {
            target: ExitTarget::Tag(block),
            ..
        } => [Some(*block), None],
        Terminator::Return { .. } | Terminator::NonLocalExit { .. } | Terminator::Unreachable => {
            [None, None]
        }
    }
}
