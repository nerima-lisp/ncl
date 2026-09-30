//! Shared context for IR v2 lowering.

use ncl_ir::{BlockId, HandlerRegion, HandlerRegionId, OpKind, Ty, ValueId};

use crate::symbols::SymbolRef;

use super::error::LowerError;
use super::function::{FunctionLowerer, Lowerer};

#[path = "analysis.rs"]
mod analysis;
#[path = "bindings.rs"]
mod bindings;
#[path = "control.rs"]
mod control;
#[path = "control_escape.rs"]
mod control_escape;
#[path = "control_regions.rs"]
mod control_regions;
#[path = "dynamic_binding.rs"]
mod dynamic_binding;
#[path = "expr.rs"]
mod expr;
#[path = "params.rs"]
mod params;
#[path = "unbound_variable.rs"]
mod unbound_variable;

#[derive(Clone, Debug)]
pub(super) struct NonLocalTarget {
    pub(super) name: SymbolRef,
    pub(super) capture: SymbolRef,
}

/// Per-function state not represented by the frozen `FunctionLowerer` API.
pub(super) struct Context<'a> {
    pub(super) module: &'a mut Lowerer,
    pub(super) regions: Vec<HandlerRegion>,
    active: Vec<HandlerRegionId>,
    blocks: Vec<BlockId>,
    targets: Vec<NonLocalTarget>,
    next_region: u32,
}

impl<'a> Context<'a> {
    pub(super) fn new(module: &'a mut Lowerer) -> Self {
        Self::with_targets(module, Vec::new())
    }

    fn with_targets(module: &'a mut Lowerer, targets: Vec<NonLocalTarget>) -> Self {
        Self {
            module,
            regions: Vec::new(),
            active: Vec::new(),
            blocks: vec![BlockId(0)],
            targets,
            next_region: 0,
        }
    }

    fn block(&mut self, f: &mut FunctionLowerer, params: Vec<(Ty, ValueId)>) -> BlockId {
        let id = f.create_block(params);
        self.blocks.push(id);
        id
    }

    fn enter(&mut self, f: &mut FunctionLowerer, id: HandlerRegionId) -> Result<(), LowerError> {
        self.active.push(id);
        f.none(OpKind::EnterHandler { region: id })
    }

    fn leave(&mut self, f: &mut FunctionLowerer, id: HandlerRegionId) -> Result<(), LowerError> {
        f.none(OpKind::LeaveHandler { region: id })?;
        if self.active.last().copied() == Some(id) {
            self.active.pop();
        }
        Ok(())
    }

    /// Number of handler regions currently open (entered but not yet left).
    const fn active_len(&self) -> usize {
        self.active.len()
    }

    /// Leave every handler region opened since `depth`, innermost first.
    ///
    /// Used by a lexically-visible `return-from`/`go` fast path (jumping
    /// directly to the bound block rather than desugaring to `throw`) so
    /// `unwind-protect` cleanup and `progv` restores still run for regions
    /// nested between the jump and its target, even though no closure
    /// boundary is crossed.
    fn close_active_since(
        &mut self,
        f: &mut FunctionLowerer,
        depth: usize,
    ) -> Result<(), LowerError> {
        while self.active.len() > depth {
            let Some(id) = self.active.last().copied() else {
                break;
            };
            self.leave(f, id)?;
        }
        Ok(())
    }

    fn token(name: &SymbolRef, kind: &str, id: HandlerRegionId) -> SymbolRef {
        SymbolRef::interned("NCL-NLX", format!("{kind}:{}:{}", name.name, id.0))
    }

    fn target(&self, name: &SymbolRef) -> Option<NonLocalTarget> {
        self.targets
            .iter()
            .rev()
            .find(|target| target.name == *name)
            .cloned()
    }
}
