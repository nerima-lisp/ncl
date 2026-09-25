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
#[path = "expr.rs"]
mod expr;
#[path = "params.rs"]
mod params;

#[derive(Clone, Debug)]
struct NonLocalTarget {
    name: SymbolRef,
    token: SymbolRef,
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

    fn token(name: &SymbolRef, kind: &str) -> SymbolRef {
        SymbolRef::interned("NCL-NLX", format!("{kind}:{}", name.name))
    }

    fn target(&self, name: &SymbolRef) -> Option<NonLocalTarget> {
        self.targets
            .iter()
            .rev()
            .find(|target| target.name == *name)
            .cloned()
    }
}
