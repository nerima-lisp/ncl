//! Dynamic control lowering for the IR v2 path.

use ncl_ir::{HandlerKind, HandlerRegion, HandlerRegionId, OpKind, Terminator, Ty, ValueId};

use crate::ast::{Expr, TagbodyItem};
use crate::symbols::SymbolRef;

use super::super::capture;
use super::super::env::{BlockEntry, Slot, TagEntry};
use super::super::error::LowerError;
use super::super::function::FunctionLowerer;
use super::Context;
use super::NonLocalTarget;
use super::analysis::{body_has_nested_return, body_requires_escaping_go};

impl Context<'_> {
    pub(super) fn terminate_jump(
        f: &mut FunctionLowerer,
        target: ncl_ir::BlockId,
        args: Vec<ValueId>,
    ) -> Result<(), LowerError> {
        if target.0 <= f.current_block().0 {
            f.safepoint()?;
        }
        f.terminate(Terminator::Jump { target, args })
    }

    pub(super) fn lower_block(
        &mut self,
        f: &mut FunctionLowerer,
        name: &SymbolRef,
        body: &[Expr],
    ) -> Result<ValueId, LowerError> {
        if body_has_nested_return(body, name) {
            return self.lower_escaping_block(f, name, body);
        }
        let result = f.fresh_value();
        let live = capture::analyze(body)
            .assigned_names()
            .into_iter()
            .filter(|name| matches!(f.env().lookup_variable(name), Some(Slot::Value(_))))
            .collect::<Vec<_>>();
        let live_values = live.iter().map(|_| f.fresh_value()).collect::<Vec<_>>();
        let start = f.current_block();
        let exit = self.block(
            f,
            std::iter::once((Ty::Word, result))
                .chain(live_values.iter().copied().map(|value| (Ty::Word, value)))
                .collect(),
        );
        f.position(start)?;
        f.env().push();
        f.env().bind_block(BlockEntry {
            name: name.clone(),
            target: exit,
            active_depth: self.active_len(),
            live: live.clone(),
        });
        let value = self.lower_body(f, body)?;
        f.env().pop();
        if !f.is_terminated() {
            let mut args = vec![value];
            args.extend(
                live.iter()
                    .filter_map(|name| match f.env().lookup_variable(name) {
                        Some(Slot::Value(value)) => Some(value),
                        // check-added-lines: allow(wildcard) slots may be values or cells.
                        _ => None,
                    }),
            );
            f.terminate(Terminator::Jump { target: exit, args })?;
        }
        f.position(exit)?;
        for (name, value) in live.into_iter().zip(live_values) {
            f.env().rebind_variable(&name, Slot::Value(value));
        }
        Ok(result)
    }
    pub(super) fn lower_return_from(
        &mut self,
        f: &mut FunctionLowerer,
        name: &SymbolRef,
        value: Option<&Expr>,
    ) -> Result<ValueId, LowerError> {
        if let Some(entry) = f.env().lookup_block(name) {
            let value = match value {
                Some(form) => self.lower_expr(f, form)?,
                None => f.nil()?,
            };
            if !f.is_terminated() {
                self.close_active_since(f, entry.active_depth)?;
            }
            if !f.is_terminated() {
                let mut args = vec![value];
                args.extend(entry.live.iter().filter_map(
                    |name| match f.env().lookup_variable(name) {
                        Some(Slot::Value(value)) => Some(value),
                        // check-added-lines: allow(wildcard) only cell slots are live here.
                        _ => None,
                    },
                ));
                f.terminate(Terminator::Jump {
                    target: entry.target,
                    args,
                })?;
            }
            return Ok(value);
        }
        let target = self
            .target(name)
            .ok_or_else(|| LowerError::EscapingControl { name: name.clone() })?;
        let value = match value {
            Some(form) => self.lower_expr(f, form)?,
            None => f.nil()?,
        };
        let token = Self::target_value(f, &target, name)?;
        f.safepoint()?;
        let thrown = f.one(
            OpKind::Builtin {
                name: "throw".to_owned(),
                args: vec![token, value],
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
    #[allow(
        clippy::too_many_lines,
        reason = "keeps fast and escaping tagbody lowering adjacent"
    )]
    pub(super) fn lower_tagbody(
        &mut self,
        f: &mut FunctionLowerer,
        items: &[TagbodyItem],
    ) -> Result<ValueId, LowerError> {
        let tags = items
            .iter()
            .filter_map(|item| match item {
                TagbodyItem::Tag(tag) => Some(tag),
                TagbodyItem::Form(_) => None,
            })
            .collect::<Vec<_>>();
        let forms = items
            .iter()
            .filter_map(|item| match item {
                TagbodyItem::Tag(_) => None,
                TagbodyItem::Form(form) => Some(form),
            })
            .cloned()
            .collect::<Vec<_>>();
        if tags
            .iter()
            .any(|tag| body_requires_escaping_go(&forms, tag))
        {
            return self.lower_escaping_tagbody(f, items);
        }
        let start = f.current_block();
        let live = capture::analyze(&forms)
            .assigned_names()
            .into_iter()
            .filter_map(|name| f.env().lookup_variable(&name).map(|slot| (name, slot)))
            .collect::<Vec<_>>();
        let exit_values = live.iter().map(|_| f.fresh_value()).collect::<Vec<_>>();
        let exit = self.block(
            f,
            exit_values
                .iter()
                .copied()
                .map(|value| (Ty::Word, value))
                .collect(),
        );
        f.position(start)?;
        f.env().push();
        let mut targets = Vec::new();
        for item in items {
            if let TagbodyItem::Tag(tag) = item {
                let values = live.iter().map(|_| f.fresh_value()).collect::<Vec<_>>();
                let block = self.block(
                    f,
                    values
                        .iter()
                        .copied()
                        .map(|value| (Ty::Word, value))
                        .collect(),
                );
                targets.push((tag.clone(), block, values));
            }
        }
        f.position(start)?;
        let tag_depth = self.active_len();
        for (name, target, _) in &targets {
            f.env().bind_tag(TagEntry {
                name: name.clone(),
                target: *target,
                active_depth: tag_depth,
                live: live.iter().map(|(name, _)| name.clone()).collect(),
            });
        }
        let mark = self.targets.len();
        for (name, _, _) in &targets {
            self.targets.push(NonLocalTarget {
                name: name.clone(),
                capture: Context::token(name, "TAG", HandlerRegionId(0)),
            });
        }
        let mut next = 0;
        for item in items {
            match item {
                TagbodyItem::Tag(_) => {
                    let (target, params) = targets
                        .get(next)
                        .map(|(_, block, params)| (*block, params))
                        .ok_or_else(|| LowerError::Ir {
                            detail: "tagbody tag index out of bounds".to_owned(),
                        })?;
                    next += 1;
                    if !f.is_terminated() {
                        let args = live
                            .iter()
                            .filter_map(|(name, _)| match f.env().lookup_variable(name) {
                                Some(Slot::Value(value) | Slot::Cell(value)) => Some(value),
                                None => None,
                            })
                            .collect();
                        Self::terminate_jump(f, target, args)?;
                    }
                    f.position(target)?;
                    for ((name, previous_slot), value) in live.iter().zip(params) {
                        let slot = match previous_slot {
                            Slot::Value(_) => Slot::Value(*value),
                            Slot::Cell(_) => Slot::Cell(*value),
                        };
                        f.env().rebind_variable(name, slot);
                    }
                }
                TagbodyItem::Form(form) => {
                    if !f.is_terminated() {
                        self.lower_expr(f, form)?;
                    }
                }
            }
        }
        self.targets.truncate(mark);
        let normal_path = !f.is_terminated();
        if normal_path {
            let args = live
                .iter()
                .filter_map(|(name, _)| match f.env().lookup_variable(name) {
                    Some(Slot::Value(value) | Slot::Cell(value)) => Some(value),
                    None => None,
                })
                .collect();
            Self::terminate_jump(f, exit, args)?;
        }
        f.position(exit)?;
        let value = f.nil()?;
        if !normal_path {
            f.terminate(Terminator::Throw { condition: value })?;
        }
        for ((name, previous_slot), value) in live.iter().zip(exit_values) {
            let slot = match previous_slot {
                Slot::Value(_) => Slot::Value(value),
                Slot::Cell(_) => Slot::Cell(value),
            };
            f.env().rebind_variable(name, slot);
        }
        f.env().pop();
        Ok(value)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "handler construction mirrors the block lowering"
    )]
    fn lower_escaping_tagbody(
        &mut self,
        f: &mut FunctionLowerer,
        items: &[TagbodyItem],
    ) -> Result<ValueId, LowerError> {
        let source = f.current_block();
        let outer_active = self.active.clone();
        let first_new_region = self.regions.len();
        f.position(source)?;
        f.env().push();
        let forms = items
            .iter()
            .filter_map(|item| match item {
                TagbodyItem::Form(form) => Some(form),
                TagbodyItem::Tag(_) => None,
            })
            .cloned()
            .collect::<Vec<_>>();
        let live = capture::analyze(&forms)
            .assigned_names()
            .into_iter()
            .filter(|name| matches!(f.env().lookup_variable(name), Some(Slot::Cell(_))))
            .collect::<Vec<_>>();
        let cell_sources = live
            .iter()
            .filter_map(|name| match f.env().lookup_variable(name) {
                Some(Slot::Cell(value)) => Some(value),
                // check-added-lines: allow(wildcard) only cell slots are live here.
                _ => None,
            })
            .collect::<Vec<_>>();
        let exit_params = live.iter().map(|_| f.fresh_value()).collect::<Vec<_>>();
        let exit = self.block(
            f,
            exit_params
                .iter()
                .copied()
                .map(|value| (Ty::Word, value))
                .collect::<Vec<_>>(),
        );
        f.position(source)?;
        let tags = items
            .iter()
            .filter_map(|item| match item {
                TagbodyItem::Tag(tag) => Some(tag.clone()),
                TagbodyItem::Form(_) => None,
            })
            .collect::<Vec<_>>();
        let mut pending_entries = Vec::new();
        for tag in &tags {
            let id = HandlerRegionId(self.next_region);
            self.next_region += 1;
            let capture = Context::token(tag, "TAG", id);
            let token = Self::fresh_token(f)?;
            f.env()
                .bind_variable(capture.clone(), super::super::env::Slot::Value(token));
            pending_entries.push((tag.clone(), id, capture, token));
        }
        let start = self.split_region_entry(f)?;
        let first_new_block = self.blocks.len();
        for (_, id, _, _) in &pending_entries {
            self.enter_current(f, *id)?;
        }
        let mut entries = Vec::with_capacity(pending_entries.len());
        for (tag, id, capture, token) in pending_entries {
            let params = live.iter().map(|_| f.fresh_value()).collect::<Vec<_>>();
            let target = self.block(
                f,
                params
                    .iter()
                    .copied()
                    .map(|value| (Ty::Word, value))
                    .collect::<Vec<_>>(),
            );
            entries.push((tag, id, capture, target, token, params));
        }
        for (tag, _, capture, _, _, _) in &entries {
            self.targets.push(NonLocalTarget {
                name: tag.clone(),
                capture: capture.clone(),
            });
        }
        f.position(start)?;
        let mut next = 0;
        for item in items {
            match item {
                TagbodyItem::Tag(_) => {
                    let target = entries
                        .get(next)
                        .ok_or_else(|| LowerError::Ir {
                            detail: "tagbody tag index is out of bounds".to_owned(),
                        })?
                        .3;
                    next += 1;
                    if !f.is_terminated() {
                        let args = live
                            .iter()
                            .filter_map(|name| match f.env().lookup_variable(name) {
                                Some(Slot::Cell(value)) => Some(value),
                                // check-added-lines: allow(wildcard) only cell slots are live here.
                                _ => None,
                            })
                            .collect();
                        f.terminate(Terminator::Jump { target, args })?;
                    }
                    f.position(target)?;
                    if let Some((_, _, _, _, _, params)) = entries.get(next.saturating_sub(1)) {
                        for (name, value) in live.iter().zip(params) {
                            f.env().rebind_variable(name, Slot::Cell(*value));
                        }
                    }
                }
                TagbodyItem::Form(form) => {
                    if !f.is_terminated() {
                        self.lower_expr(f, form)?;
                    }
                }
            }
        }
        self.targets
            .truncate(self.targets.len().saturating_sub(entries.len()));
        let mut protected = self.protected_blocks(f, start, first_new_block, first_new_region);
        for (_, _, _, target, _, _) in &entries {
            for block in self.protected_blocks(f, *target, first_new_block, first_new_region) {
                if !protected.contains(&block) {
                    protected.push(block);
                }
            }
        }
        let normal_path = !f.is_terminated();
        if normal_path {
            for (_, id, _, _, _, _) in entries.iter().rev() {
                self.leave(f, *id)?;
            }
            let args = live
                .iter()
                .filter_map(|name| match f.env().lookup_variable(name) {
                    Some(Slot::Cell(value)) => Some(value),
                    // check-added-lines: allow(wildcard) only cell slots are live here.
                    _ => None,
                })
                .collect();
            f.terminate(Terminator::Jump { target: exit, args })?;
        }
        for (index, (_, id, _capture, target, token, _)) in entries.iter().enumerate() {
            self.active.clone_from(&outer_active);
            self.active.extend(
                entries
                    .iter()
                    .take(index + 1)
                    .map(|(_, id, _, _, _, _)| *id),
            );
            let value = f.fresh_value();
            let cell_params = live
                .iter()
                .map(|_| (Ty::Word, f.fresh_value()))
                .collect::<Vec<_>>();
            let handler = self.block(
                f,
                std::iter::once((Ty::Word, value))
                    .chain(cell_params.iter().copied())
                    .collect(),
            );
            self.leave(f, *id)?;
            for (_, reenter_id, _, _, _, _) in entries.iter().skip(index) {
                f.none(OpKind::EnterHandler {
                    region: *reenter_id,
                })?;
            }
            f.terminate(Terminator::Jump {
                target: *target,
                args: cell_params.iter().map(|(_, value)| *value).collect(),
            })?;
            self.regions.push(HandlerRegion {
                id: *id,
                kind: HandlerKind::Catch,
                protected: protected.clone(),
                handler,
                cleanup: None,
                catch_tag: Some(*token),
                binding_targets: cell_sources.clone(),
                depth: 0,
                parent: None,
            });
        }
        self.active.clone_from(&outer_active);
        f.position(exit)?;
        let value = f.nil()?;
        if !normal_path {
            f.terminate(Terminator::Throw { condition: value })?;
        }
        for (name, value) in live.into_iter().zip(exit_params) {
            f.env().rebind_variable(&name, Slot::Cell(value));
        }
        f.env().pop();
        Ok(value)
    }
}
