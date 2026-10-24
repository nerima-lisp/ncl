use ncl_ir::{HandlerKind, HandlerRegion, HandlerRegionId, OpKind, Terminator, Ty, ValueId};

use crate::ast::Expr;

use super::super::capture;
use super::super::env::Slot;
use super::super::error::LowerError;
use super::super::function::FunctionLowerer;
use super::Context;

impl Context<'_> {
    pub(super) fn lower_unwind(
        &mut self,
        f: &mut FunctionLowerer,
        protected_form: &Expr,
        cleanup: &[Expr],
    ) -> Result<ValueId, LowerError> {
        // Cleanup runs from both normal and handler paths, so it cannot read a
        // value assigned only on the normal path through an SSA value.
        let assigned = capture::analyze(std::slice::from_ref(protected_form)).assigned_names();
        let cleanup_variables = capture::free_names_in_forms(cleanup).variables;
        for name in assigned {
            if !cleanup_variables.contains(&name) {
                continue;
            }
            if let Some(Slot::Value(value)) = f.env().lookup_variable(&name) {
                let cell = f.one(OpKind::MakeValueCell { value }, Ty::Word)?;
                f.env().rebind_variable(&name, Slot::Cell(cell));
            }
        }

        let result = f.fresh_value();
        let region_id = HandlerRegionId(self.next_region);
        self.next_region += 1;
        let first_new_region = self.regions.len();
        let start = self.enter(f, region_id)?;
        let first_new_block = self.blocks.len();
        let value = self.lower_expr(f, protected_form)?;
        let protected = self.protected_blocks(f, start, first_new_block, first_new_region);
        let body_end = f.current_block();
        let normal_path = !f.is_terminated();
        let saved_values = if normal_path && self.preserve_multiple_values {
            Some(Self::capture_multiple_values(f, value)?)
        } else {
            None
        };
        let merge = normal_path.then(|| self.block(f, vec![(Ty::Word, result)]));
        if let Some(merge) = merge {
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
        if let Some(merge) = merge {
            f.position(merge)?;
        }
        Ok(result)
    }
}
