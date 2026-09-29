//! Dynamic control lowering for the IR v2 path.

use ncl_ir::{
    Convert, HandlerKind, HandlerRegion, HandlerRegionId, OpKind, Terminator, Ty, ValueId,
};

use crate::ast::{Expr, TagbodyItem};
use crate::symbols::SymbolRef;

use super::super::env::{BlockEntry, TagEntry};
use super::super::error::LowerError;
use super::super::function::FunctionLowerer;
use super::Context;
use super::NonLocalTarget;
use super::analysis::{body_has_nested_go, body_has_nested_return};

impl Context<'_> {
    fn fresh_token(f: &mut FunctionLowerer) -> Result<ValueId, LowerError> {
        let symbol = f.symbol(&SymbolRef::interned("COMMON-LISP", "GENSYM"))?;
        let function = f.one(
            OpKind::LoadField {
                object: symbol,
                field: u32::try_from(ncl_object::symbol_offset::FUNCTION).map_err(|_| {
                    LowerError::Ir {
                        detail: "gensym function offset does not fit u32".to_owned(),
                    }
                })?,
            },
            Ty::Word,
        )?;
        let zero = f.fixnum(0)?;
        let argc = f.one(
            OpKind::Convert {
                op: Convert::I64ToWord,
                value: zero,
            },
            Ty::Word,
        )?;
        f.safepoint()?;
        f.one(
            OpKind::CallClosure {
                closure: function,
                args: vec![argc],
                named_symbol: None,
            },
            Ty::Word,
        )
    }

    fn target_value(
        f: &mut FunctionLowerer,
        target: &NonLocalTarget,
        name: &SymbolRef,
    ) -> Result<ValueId, LowerError> {
        match f.env().lookup_variable(&target.capture) {
            Some(super::super::env::Slot::Value(value) | super::super::env::Slot::Cell(value)) => {
                Ok(value)
            }
            None => Err(LowerError::EscapingControl { name: name.clone() }),
        }
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
        let start = f.current_block();
        let exit = self.block(f, vec![(Ty::Word, result)]);
        f.position(start)?;
        f.env().push();
        f.env().bind_block(BlockEntry {
            name: name.clone(),
            target: exit,
            active_depth: self.active_len(),
        });
        let value = self.lower_body(f, body)?;
        f.env().pop();
        if !f.is_terminated() {
            f.terminate(Terminator::Jump {
                target: exit,
                args: vec![value],
            })?;
        }
        f.position(exit)?;
        Ok(result)
    }
    fn lower_escaping_block(
        &mut self,
        f: &mut FunctionLowerer,
        name: &SymbolRef,
        body: &[Expr],
    ) -> Result<ValueId, LowerError> {
        let result = f.fresh_value();
        let start = f.current_block();
        let region_id = HandlerRegionId(self.next_region);
        self.next_region += 1;
        let token_name = Context::token(name, "BLOCK", region_id);
        let token = Self::fresh_token(f)?;
        self.enter(f, region_id)?;
        f.env().push();
        f.env()
            .bind_variable(token_name.clone(), super::super::env::Slot::Value(token));
        self.targets.push(NonLocalTarget {
            name: name.clone(),
            capture: token_name,
        });
        let value = self.lower_body(f, body)?;
        self.targets.pop();
        f.env().pop();
        let body_end = f.current_block();
        let normal_path = !f.is_terminated();
        let protected = self
            .blocks
            .clone()
            .into_iter()
            .filter(|block| block.0 >= start.0)
            .collect::<Vec<_>>();
        let exit = self.block(f, vec![(Ty::Word, result)]);
        if normal_path {
            f.position(body_end)?;
            self.leave(f, region_id)?;
            f.terminate(Terminator::Jump {
                target: exit,
                args: vec![value],
            })?;
        }
        let handler_value = f.fresh_value();
        let handler = self.block(f, vec![(Ty::Word, handler_value)]);
        self.leave(f, region_id)?;
        f.terminate(Terminator::Jump {
            target: exit,
            args: vec![handler_value],
        })?;
        self.regions.push(HandlerRegion {
            id: region_id,
            kind: HandlerKind::Catch,
            protected,
            handler,
            cleanup: None,
            catch_tag: Some(token),
            binding_targets: Vec::new(),
            depth: 0,
            parent: None,
        });
        f.position(exit)?;
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
                f.terminate(Terminator::Jump {
                    target: entry.target,
                    args: vec![value],
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
        let exit = self.block(f, Vec::new());
        f.position(start)?;
        f.env().push();
        let mut targets = Vec::new();
        for item in items {
            if let TagbodyItem::Tag(tag) = item {
                let block = self.block(f, Vec::new());
                targets.push((tag.clone(), block));
            }
        }
        f.position(start)?;
        let tag_depth = self.active_len();
        for (name, target) in &targets {
            f.env().bind_tag(TagEntry {
                name: name.clone(),
                target: *target,
                active_depth: tag_depth,
            });
        }
        let mark = self.targets.len();
        for (name, _) in &targets {
            self.targets.push(NonLocalTarget {
                name: name.clone(),
                capture: Context::token(name, "TAG", HandlerRegionId(0)),
            });
        }
        let mut next = 0;
        for item in items {
            match item {
                TagbodyItem::Tag(_) => {
                    let target = targets.get(next).map(|(_, block)| *block).ok_or_else(|| {
                        LowerError::Ir {
                            detail: "tagbody tag index out of bounds".to_owned(),
                        }
                    })?;
                    next += 1;
                    if !f.is_terminated() {
                        f.terminate(Terminator::Jump {
                            target,
                            args: Vec::new(),
                        })?;
                    }
                    f.position(target)?;
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
            f.terminate(Terminator::Jump {
                target: exit,
                args: Vec::new(),
            })?;
        }
        f.position(exit)?;
        f.env().pop();
        f.nil()
    }

    fn lower_escaping_tagbody(
        &mut self,
        f: &mut FunctionLowerer,
        items: &[TagbodyItem],
    ) -> Result<ValueId, LowerError> {
        let start = f.current_block();
        let exit = self.block(f, Vec::new());
        f.position(start)?;
        f.env().push();
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
            let target = self.block(f, Vec::new());
            entries.push((tag.clone(), id, capture, target, token));
        }
        for (tag, _, capture, _, _) in &entries {
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
                    let target = entries[next].3;
                    next += 1;
                    if !f.is_terminated() {
                        f.terminate(Terminator::Jump {
                            target,
                            args: Vec::new(),
                        })?;
                    }
                    f.position(target)?;
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
            for (_, id, _, _, _) in entries.iter().rev() {
                self.leave(f, *id)?;
            }
            f.terminate(Terminator::Jump {
                target: exit,
                args: Vec::new(),
            })?;
        }
        for (_, id, _capture, target, token) in &entries {
            let value = f.fresh_value();
            let handler = self.block(f, vec![(Ty::Word, value)]);
            self.leave(f, *id)?;
            f.terminate(Terminator::Jump {
                target: *target,
                args: Vec::new(),
            })?;
            self.regions.push(HandlerRegion {
                id: *id,
                kind: HandlerKind::Catch,
                protected: protected.clone(),
                handler,
                cleanup: None,
                catch_tag: Some(*token),
                binding_targets: Vec::new(),
                depth: 0,
                parent: None,
            });
        }
        f.position(exit)?;
        f.env().pop();
        f.nil()
    }

    pub(super) fn lower_go(
        &mut self,
        f: &mut FunctionLowerer,
        tag: &SymbolRef,
    ) -> Result<ValueId, LowerError> {
        if let Some(entry) = f.env().lookup_tag(tag) {
            let value = f.nil()?;
            if !f.is_terminated() {
                self.close_active_since(f, entry.active_depth)?;
            }
            if !f.is_terminated() {
                f.terminate(Terminator::Jump {
                    target: entry.target,
                    args: Vec::new(),
                })?;
            }
            return Ok(value);
        }
        let target = self
            .target(tag)
            .ok_or_else(|| LowerError::EscapingControl { name: tag.clone() })?;
        let value = f.nil()?;
        let token = Self::target_value(f, &target, tag)?;
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
