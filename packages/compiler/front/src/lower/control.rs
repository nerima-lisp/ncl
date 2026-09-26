//! Dynamic control lowering for the IR v2 path.

use ncl_ir::{HandlerKind, HandlerRegion, HandlerRegionId, OpKind, Terminator, Ty, ValueId};

use crate::ast::{Expr, TagbodyItem};
use crate::symbols::SymbolRef;

use super::super::env::{BlockEntry, TagEntry};
use super::super::error::LowerError;
use super::super::function::FunctionLowerer;
use super::Context;
use super::NonLocalTarget;
use super::analysis::body_has_nested_return;

impl Context<'_> {
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
        let token_name = Context::token(name, "BLOCK");
        let token = f.symbol(&token_name)?;
        let region_id = HandlerRegionId(self.next_region);
        self.next_region += 1;
        self.enter(f, region_id)?;
        self.targets.push(NonLocalTarget {
            name: name.clone(),
            token: token_name,
        });
        let value = self.lower_body(f, body)?;
        self.targets.pop();
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
        let token = f.symbol(&target.token)?;
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
        for (name, target) in &targets {
            f.env().bind_tag(TagEntry {
                name: name.clone(),
                target: *target,
            });
        }
        let mark = self.targets.len();
        for (name, _) in &targets {
            self.targets.push(NonLocalTarget {
                name: name.clone(),
                token: Context::token(name, "TAG"),
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

    pub(super) fn lower_go(
        &self,
        f: &mut FunctionLowerer,
        tag: &SymbolRef,
    ) -> Result<ValueId, LowerError> {
        if let Some(entry) = f.env().lookup_tag(tag) {
            let value = f.nil()?;
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
        let token = f.symbol(&target.token)?;
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
            f.terminate(Terminator::Jump {
                target: merge,
                args: vec![value],
            })?;
        }
        let cleanup_block = self.block(f, Vec::new());
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
