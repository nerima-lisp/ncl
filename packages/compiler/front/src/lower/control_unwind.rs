use ncl_ir::{HandlerKind, HandlerRegion, HandlerRegionId, OpKind, Terminator, Ty, ValueId};

use super::super::error::LowerError;
use super::super::function::FunctionLowerer;
use super::Context;
use crate::ast::Expr;

impl Context<'_> {
    pub(super) fn lower_unwind(
        &mut self,
        f: &mut FunctionLowerer,
        protected_form: &Expr,
        cleanup: &[Expr],
    ) -> Result<ValueId, LowerError> {
        let result = f.fresh_value();
        let region_id = HandlerRegionId(self.next_region);
        self.next_region += 1;
        let start = f.current_block();
        self.enter(f, region_id)?;
        let value = self.lower_expr(f, protected_form)?;
        let protected = self
            .blocks
            .iter()
            .copied()
            .filter(|block| block.0 >= start.0)
            .collect::<Vec<_>>();
        let body_end = f.current_block();
        let normal_path = !f.is_terminated();
        let saved_values = if normal_path && self.preserve_multiple_values {
            Some(Self::capture_multiple_values(f, value)?)
        } else {
            None
        };
        let merge = self.block(f, vec![(Ty::Word, result)]);
        if normal_path {
            f.position(body_end)?;
            self.leave(f, region_id)?;
            for form in cleanup {
                if !f.is_terminated() {
                    self.lower_expr(f, form)?;
                }
            }
            if !f.is_terminated() {
                let result = if let Some(saved_values) = saved_values {
                    Self::restore_multiple_values(f, saved_values)?
                } else {
                    f.none(OpKind::SetMultipleValues {
                        values: vec![value],
                    })?;
                    value
                };
                f.terminate(Terminator::Jump {
                    target: merge,
                    args: vec![result],
                })?;
            }
        }
        let cleanup_block = self.block(f, Vec::new());
        self.leave(f, region_id)?;
        for form in cleanup {
            if !f.is_terminated() {
                self.lower_expr(f, form)?;
            }
        }
        if !f.is_terminated() {
            self.leave(f, region_id)?;
            let cleanup_value = f.nil()?;
            f.terminate(Terminator::Return {
                values: vec![cleanup_value],
            })?;
        }
        self.regions.push(HandlerRegion {
            id: region_id,
            kind: HandlerKind::UnwindProtect,
            protected,
            handler: cleanup_block,
            cleanup: Some(cleanup_block),
            catch_tag: None,
            binding_targets: Vec::new(),
            depth: 0,
            parent: None,
        });
        f.position(merge)?;
        Ok(result)
    }
}
