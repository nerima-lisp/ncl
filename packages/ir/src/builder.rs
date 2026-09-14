//! Construction helpers for SSA functions.
#![allow(clippy::too_many_lines)]
#![allow(clippy::all)]

use crate::{
    BasicBlock, BlockId, Constant, Function, FunctionId, HandlerRegion, Local, LocalId, Op, OpKind,
    Param, Terminator, Ty, ValueId,
};

/// A fluent builder that allocates block and SSA identities monotonically.
#[derive(Debug)]
pub struct FunctionBuilder {
    function: Function,
    next_block: u32,
    next_value: u32,
    current: Option<usize>,
}

impl FunctionBuilder {
    /// Creates a builder with an empty entry block.
    #[must_use]
    pub fn new(
        id: FunctionId,
        name: impl Into<String>,
        params: Vec<Param>,
        return_types: Vec<Ty>,
    ) -> Self {
        let mut builder = Self {
            function: Function {
                id,
                name: name.into(),
                params,
                return_types,
                blocks: Vec::new(),
                locals: Vec::new(),
                constants: Vec::new(),
                handler_regions: Vec::new(),
                debug: Vec::new(),
            },
            next_block: 0,
            next_value: 0,
            current: None,
        };
        builder.create_block(Vec::new());
        builder
    }

    /// Adds a local declaration.
    ///
    /// # Panics
    ///
    /// Panics if the local table cannot be represented by a `u32` index.
    pub fn add_local(&mut self, name: impl Into<String>, ty: Ty) -> LocalId {
        let id = LocalId(u32::try_from(self.function.locals.len()).unwrap_or(u32::MAX));
        self.function.locals.push(Local {
            id,
            name: name.into(),
            ty,
        });
        id
    }

    /// Adds a constant and returns its table index.
    ///
    /// # Panics
    ///
    /// Panics if the constant table cannot be represented by a `u32` index.
    pub fn add_constant(&mut self, constant: Constant) -> crate::ConstantIndex {
        let index =
            crate::ConstantIndex(u32::try_from(self.function.constants.len()).unwrap_or(u32::MAX));
        self.function.constants.push(constant);
        index
    }

    /// Creates a block and selects it for subsequent operations.
    pub fn create_block(&mut self, params: Vec<(Ty, ValueId)>) -> BlockId {
        let id = BlockId(self.next_block);
        self.next_block += 1;
        let params = params
            .into_iter()
            .map(|(ty, value)| {
                self.next_value = self.next_value.max(value.0 + 1);
                crate::BlockParam { value, ty }
            })
            .collect();
        self.function.blocks.push(BasicBlock {
            id,
            params,
            ops: Vec::new(),
            terminator: Terminator::Unreachable,
        });
        self.current = Some(self.function.blocks.len() - 1);
        id
    }

    /// Selects an existing block for editing.
    ///
    /// # Errors
    ///
    /// Returns an error when `block` is not present in this function.
    pub fn position_at(&mut self, block: BlockId) -> Result<(), String> {
        self.current = Some(
            self.function
                .blocks
                .iter()
                .position(|candidate| candidate.id == block)
                .ok_or_else(|| format!("unknown block {block:?}"))?,
        );
        Ok(())
    }

    /// Allocates a fresh SSA value identity.
    pub const fn fresh_value(&mut self) -> ValueId {
        let value = ValueId(self.next_value);
        self.next_value += 1;
        value
    }

    /// Appends an operation and returns its result identities.
    ///
    /// # Errors
    ///
    /// Returns an error when no block is currently selected.
    pub fn push_op(&mut self, kind: OpKind, result_types: &[Ty]) -> Result<Vec<ValueId>, String> {
        let index = self.current.ok_or_else(|| "no current block".to_owned())?;
        let results = result_types
            .iter()
            .map(|ty| (self.fresh_value(), *ty))
            .collect::<Vec<_>>();
        let ids = results.iter().map(|(id, _)| *id).collect();
        self.function.blocks[index].ops.push(Op {
            results,
            kind,
            loc: None,
        });
        Ok(ids)
    }

    /// Sets the current block terminator.
    ///
    /// # Errors
    ///
    /// Returns an error when no block is currently selected.
    pub fn terminate(&mut self, terminator: Terminator) -> Result<(), String> {
        let index = self.current.ok_or_else(|| "no current block".to_owned())?;
        self.function.blocks[index].terminator = terminator;
        Ok(())
    }

    /// Adds an exception handler region.
    pub fn add_handler_region(&mut self, region: HandlerRegion) {
        self.function.handler_regions.push(region);
    }

    /// Finishes construction and returns the function.
    #[must_use]
    pub fn finish(self) -> Function {
        self.function
    }
}
