//! Catch and throw lowering for the IR v2 path.

use ncl_ir::{HandlerKind, HandlerRegion, HandlerRegionId, OpKind, Terminator, Ty, ValueId};

use crate::ast::Expr;

use super::super::capture;
use super::super::env::Slot;
use super::super::error::LowerError;
use super::super::function::FunctionLowerer;
use super::Context;
use super::NonLocalTarget;

impl Context<'_> {
    fn lower_escaping_block_handler(
        &mut self,
        f: &mut FunctionLowerer,
        region_id: HandlerRegionId,
        exit: ncl_ir::BlockId,
        handler_value: ValueId,
        handler_values: &[(Ty, ValueId)],
    ) -> Result<(), LowerError> {
        self.leave(f, region_id)?;
        let handler_value = if self.preserve_multiple_values {
            Self::restore_multiple_values(f, handler_value)?
        } else {
            handler_value
        };
        f.terminate(Terminator::Jump {
            target: exit,
            args: std::iter::once(handler_value)
                .chain(handler_values.iter().map(|(_, value)| *value))
                .collect(),
        })?;
        Ok(())
    }

    pub(super) fn lower_escaping_block(
        &mut self,
        f: &mut FunctionLowerer,
        name: &crate::symbols::SymbolRef,
        body: &[Expr],
    ) -> Result<ValueId, LowerError> {
        let result = f.fresh_value();
        let first_new_region = self.regions.len();
        let region_id = HandlerRegionId(self.next_region);
        self.next_region += 1;
        let token_name = Context::token(name, "BLOCK", region_id);
        let token = Self::fresh_token(f)?;
        let start = self.enter(f, region_id)?;
        let first_new_block = self.blocks.len();
        f.env().push();
        f.env()
            .bind_variable(token_name.clone(), Slot::Value(token));
        self.targets.push(NonLocalTarget {
            name: name.clone(),
            capture: token_name,
        });
        let value = self.lower_body(f, body)?;
        let assigned = capture::analyze(body).assigned_names();
        let live = assigned
            .iter()
            .filter_map(|name| match f.env().lookup_variable(name) {
                Some(slot @ Slot::Cell(_)) => Some((name, slot)),
                Some(Slot::Value(_)) | None => None,
            })
            .collect::<Vec<_>>();
        self.targets.pop();
        f.env().pop();
        let body_end = f.current_block();
        let normal_path = !f.is_terminated();
        let protected = self.protected_blocks(f, start, first_new_block, first_new_region);
        let exit_values = live
            .iter()
            .map(|_| (Ty::Word, f.fresh_value()))
            .collect::<Vec<_>>();
        let exit = self.block(
            f,
            std::iter::once((Ty::Word, result))
                .chain(exit_values.iter().copied())
                .collect(),
        );
        if normal_path {
            f.position(body_end)?;
            self.leave(f, region_id)?;
            f.terminate(Terminator::Jump {
                target: exit,
                args: std::iter::once(value)
                    .chain(live.iter().map(|(_, slot)| match slot {
                        Slot::Cell(value) | Slot::Value(value) => *value,
                    }))
                    .collect(),
            })?;
        }
        let handler_value = f.fresh_value();
        let handler_values = live
            .iter()
            .map(|_| (Ty::Word, f.fresh_value()))
            .collect::<Vec<_>>();
        let handler = self.block(
            f,
            std::iter::once((Ty::Word, handler_value))
                .chain(handler_values.iter().copied())
                .collect(),
        );
        self.lower_escaping_block_handler(f, region_id, exit, handler_value, &handler_values)?;
        self.regions.push(HandlerRegion {
            id: region_id,
            kind: HandlerKind::Catch,
            protected,
            handler,
            cleanup: None,
            catch_tag: Some(token),
            binding_targets: live
                .iter()
                .map(|(_, slot)| match slot {
                    Slot::Cell(value) | Slot::Value(value) => *value,
                })
                .collect(),
            depth: 0,
            parent: None,
        });
        f.position(exit)?;
        for ((name, slot), (_, value)) in live.into_iter().zip(exit_values) {
            f.env().rebind_variable(
                name,
                match slot {
                    Slot::Cell(_) => Slot::Cell(value),
                    Slot::Value(_) => Slot::Value(value),
                },
            );
        }
        Ok(result)
    }

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
        let first_new_region = self.regions.len();
        let start = self.enter(f, region_id)?;
        let first_new_block = self.blocks.len();
        let value = self.lower_body(f, body)?;
        let protected = self.protected_blocks(f, start, first_new_block, first_new_region);
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
                specialization: None,
                specialization_disabled: self.specialization_disabled(),
            },
            Ty::Word,
        )?;
        if !f.is_terminated() {
            f.terminate(Terminator::Throw { condition: thrown })?;
        }
        Ok(value)
    }
}
