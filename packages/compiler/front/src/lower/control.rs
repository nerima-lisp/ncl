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
use super::analysis::{body_has_nested_go, body_has_nested_return};

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
        if tags.iter().any(|tag| body_has_nested_go(&forms, tag)) {
            return self.lower_escaping_tagbody(f, items);
        }
        let start = f.current_block();
        let live = capture::analyze(&forms)
            .assigned_names()
            .into_iter()
            .filter(|name| matches!(f.env().lookup_variable(name), Some(Slot::Value(_))))
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
                live: live.clone(),
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
                            .filter_map(|name| match f.env().lookup_variable(name) {
                                Some(Slot::Value(value)) => Some(value),
                                // check-added-lines: allow(wildcard) only value slots are live here.
                                _ => None,
                            })
                            .collect();
                        Self::terminate_jump(f, target, args)?;
                    }
                    f.position(target)?;
                    for (name, value) in live.iter().zip(params) {
                        f.env().rebind_variable(name, Slot::Value(*value));
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
        if !f.is_terminated() {
            let args = live
                .iter()
                .filter_map(|name| match f.env().lookup_variable(name) {
                    Some(Slot::Value(value)) => Some(value),
                    // check-added-lines: allow(wildcard) only value slots are live here.
                    _ => None,
                })
                .collect();
            Self::terminate_jump(f, exit, args)?;
        }
        f.position(exit)?;
        for (name, value) in live.iter().zip(exit_values) {
            f.env().rebind_variable(name, Slot::Value(value));
        }
        f.env().pop();
        f.nil()
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
        let start = f.current_block();
        f.position(start)?;
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
        f.position(start)?;
        let tags = items
            .iter()
            .filter_map(|item| match item {
                TagbodyItem::Tag(tag) => Some(tag.clone()),
                TagbodyItem::Form(_) => None,
            })
            .collect::<Vec<_>>();
        let mut entries = Vec::new();
        for tag in &tags {
            let id = HandlerRegionId(self.next_region);
            self.next_region += 1;
            let capture = Context::token(tag, "TAG", id);
            let token = Self::fresh_token(f)?;
            f.env()
                .bind_variable(capture.clone(), super::super::env::Slot::Value(token));
            self.enter(f, id)?;
            let params = live.iter().map(|_| f.fresh_value()).collect::<Vec<_>>();
            let target = self.block(
                f,
                params
                    .iter()
                    .copied()
                    .map(|value| (Ty::Word, value))
                    .collect::<Vec<_>>(),
            );
            entries.push((tag.clone(), id, capture, target, token, params));
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
        let protected = self
            .blocks
            .iter()
            .copied()
            .filter(|block| block.0 >= start.0 && *block != exit)
            .collect::<Vec<_>>();
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
        for (_, id, _capture, target, token, _) in &entries {
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
        f.position(exit)?;
        for (name, value) in live.into_iter().zip(exit_params) {
            f.env().rebind_variable(&name, Slot::Cell(value));
        }
        f.env().pop();
        f.nil()
    }

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
                f.none(OpKind::SetMultipleValues {
                    values: vec![value],
                })?;
                f.terminate(Terminator::Jump {
                    target: merge,
                    args: vec![value],
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
