//! Catch and throw lowering for the IR v2 path.

use ncl_ir::{HandlerKind, HandlerRegion, HandlerRegionId, OpKind, Terminator, Ty, ValueId};

use crate::ast::Expr;

use super::super::capture;
use super::super::env::Slot;
use super::super::error::LowerError;
use super::super::function::FunctionLowerer;
use super::Context;
use super::NonLocalTarget;
use super::analysis;

impl Context<'_> {
    pub(super) fn lower_escaping_block(
        &mut self,
        f: &mut FunctionLowerer,
        name: &crate::symbols::SymbolRef,
        body: &[Expr],
    ) -> Result<ValueId, LowerError> {
        let result = f.fresh_value();
        let start = f.current_block();
        let live = f.env().visible_variables();
        let region_id = HandlerRegionId(self.next_region);
        self.next_region += 1;
        let token_name = Context::token(name, "BLOCK", region_id);
        let token = Self::fresh_token(f)?;
        self.enter(f, region_id)?;
        f.env().push();
        f.env()
            .bind_variable(token_name.clone(), Slot::Value(token));
        self.targets.push(NonLocalTarget {
            name: name.clone(),
            capture: token_name,
        });
        let value = self.lower_body(f, body)?;
        self.targets.pop();
        f.env().pop();
        let body_end = f.current_block();
        let normal_path = !f.is_terminated() && f.is_reachable();
        let protected = self
            .blocks
            .clone()
            .into_iter()
            .filter(|block| block.0 >= start.0)
            .collect::<Vec<_>>();
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
            let normal_live = live
                .iter()
                .map(|(name, _)| {
                    f.env()
                        .lookup_variable(name)
                        .map(|slot| (name.clone(), slot))
                })
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| LowerError::Ir {
                    detail: "escaping block lost a live lexical binding".to_owned(),
                })?;
            f.terminate(Terminator::Jump {
                target: exit,
                args: std::iter::once(value)
                    .chain(normal_live.iter().map(|(_, slot)| match slot {
                        Slot::Cell(value) | Slot::Value(value) => *value,
                    }))
                    .collect(),
            })?;
        } else if !f.is_terminated() {
            f.position(body_end)?;
            self.leave(f, region_id)?;
            let dead = f.nil()?;
            f.terminate(Terminator::Return { values: vec![dead] })?;
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
        self.leave(f, region_id)?;
        f.terminate(Terminator::Jump {
            target: exit,
            args: std::iter::once(handler_value)
                .chain(handler_values.iter().map(|(_, value)| *value))
                .collect(),
        })?;
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
                &name,
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
        let analysis = capture::analyze(body);
        let has_escape = self
            .targets
            .iter()
            .any(|target| analysis::mentions_exit(body, &target.name));
        let live = if analysis.assigned_names().is_empty() && !has_escape {
            Vec::new()
        } else {
            f.env().visible_variables()
        };
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
            let normal_live = live
                .iter()
                .map(|(name, _)| {
                    f.env()
                        .lookup_variable(name)
                        .map(|slot| (name.clone(), slot))
                })
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| LowerError::Ir {
                    detail: "catch lost a live lexical binding".to_owned(),
                })?;
            f.terminate(Terminator::Jump {
                target: exit,
                args: std::iter::once(value)
                    .chain(normal_live.iter().map(|(_, slot)| match slot {
                        Slot::Cell(value) | Slot::Value(value) => *value,
                    }))
                    .collect(),
            })?;
        }
        let caught = f.fresh_value();
        let handler_values = live
            .iter()
            .map(|_| (Ty::Word, f.fresh_value()))
            .collect::<Vec<_>>();
        let handler = self.block(
            f,
            std::iter::once((Ty::Word, caught))
                .chain(handler_values.iter().copied())
                .collect(),
        );
        self.leave(f, region_id)?;
        f.terminate(Terminator::Jump {
            target: exit,
            args: std::iter::once(caught)
                .chain(handler_values.iter().map(|(_, value)| *value))
                .collect(),
        })?;
        self.regions.push(HandlerRegion {
            id: region_id,
            kind: HandlerKind::Catch,
            protected,
            handler,
            cleanup: None,
            catch_tag: Some(tag),
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
                &name,
                match slot {
                    Slot::Cell(_) => Slot::Cell(value),
                    Slot::Value(_) => Slot::Value(value),
                },
            );
        }
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
