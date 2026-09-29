//! Catch and throw lowering for the IR v2 path.

use ncl_ir::{HandlerKind, HandlerRegion, HandlerRegionId, OpKind, Terminator, Ty, ValueId};

use crate::ast::Expr;

use super::super::error::LowerError;
use super::super::function::FunctionLowerer;
use super::Context;

impl Context<'_> {
    pub(super) fn lower_catch(
        &mut self,
        f: &mut FunctionLowerer,
        tag: &Expr,
        body: &[Expr],
    ) -> Result<ValueId, LowerError> {
        let tag = self.lower_expr(f, tag)?;
        let result = f.fresh_value();
        let region_id = HandlerRegionId(self.next_region);
        self.next_region += 1;
        let start = f.current_block();
        self.enter(f, region_id)?;
        let value = self.lower_body(f, body)?;
        let protected = self
            .blocks
            .iter()
            .copied()
            .filter(|block| block.0 >= start.0)
            .collect::<Vec<_>>();
        let body_end = f.current_block();
        let normal_path = !f.is_terminated();
        let exit = self.block(f, vec![(Ty::Word, result)]);
        if normal_path {
            f.position(body_end)?;
            self.leave(f, region_id)?;
            f.terminate(Terminator::Jump {
                target: exit,
                args: vec![value],
            })?;
        }
        let caught = f.fresh_value();
        let handler = self.block(f, vec![(Ty::Word, caught)]);
        self.leave(f, region_id)?;
        f.terminate(Terminator::Jump {
            target: exit,
            args: vec![caught],
        })?;
        self.regions.push(HandlerRegion {
            id: region_id,
            kind: HandlerKind::Catch,
            protected,
            handler,
            cleanup: None,
            catch_tag: Some(tag),
            binding_targets: Vec::new(),
            depth: 0,
            parent: None,
        });
        f.position(exit)?;
        Ok(result)
    }

    pub(super) fn lower_throw(
        &mut self,
        f: &mut FunctionLowerer,
        tag: &Expr,
        value: &Expr,
    ) -> Result<ValueId, LowerError> {
        let tag = self.lower_expr(f, tag)?;
        let value = self.lower_expr(f, value)?;
        f.safepoint()?;
        let thrown = f.one(
            OpKind::Builtin {
                name: "throw".to_owned(),
                args: vec![tag, value],
            },
            Ty::Word,
        )?;
        if !f.is_terminated() {
            f.terminate(Terminator::Throw { condition: thrown })?;
        }
        Ok(value)
    }
}
