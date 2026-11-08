use std::collections::HashSet;

use ncl_ir::{
    Constant, HandlerKind, HandlerRegion, HandlerRegionId, OpKind, Terminator as IrTerminator, Ty,
    ValueId,
};

use crate::control::ExitTarget;
use crate::ids::{CleanupId, NodeId};

use super::{FunctionLowerer, LowerError};

impl FunctionLowerer<'_> {
    pub(super) fn catch(
        &mut self,
        _node: NodeId,
        tag: NodeId,
        body: &[NodeId],
    ) -> Result<ValueId, LowerError> {
        let tag = self.node(tag)?;
        if self.is_terminated() {
            return Ok(tag);
        }
        let outer_handlers = self.active_handlers.clone();
        let id = self.allocate_handler_id()?;
        let region_start = self.handler_regions.len();
        let start = self.enter_handler(id)?;
        let first_body_block = self.created_blocks.len();
        let result = self.builder.fresh_value();
        let value = self.sequence(body)?;
        let protected = self.protected_blocks(start, first_body_block, region_start);
        let body_end = self.current_block;
        let normal = !self.is_terminated();
        let merge = self.create_block(vec![(Ty::Word, result)]);
        if normal {
            self.position(body_end)?;
            self.leave_handler(id)?;
            self.terminate(IrTerminator::Jump {
                target: merge,
                args: vec![value],
            })?;
        }
        let caught = self.builder.fresh_value();
        let handler = self.create_block(vec![(Ty::Word, caught)]);
        self.push_none(OpKind::LeaveHandler { region: id })?;
        self.terminate(IrTerminator::Jump {
            target: merge,
            args: vec![caught],
        })?;
        self.handler_regions.push(HandlerRegion {
            id,
            kind: HandlerKind::Catch,
            protected,
            handler,
            cleanup: None,
            catch_tag: Some(tag),
            binding_targets: Vec::new(),
            depth: 0,
            parent: None,
        });
        self.active_handlers = outer_handlers;
        self.position(merge)?;
        Ok(result)
    }

    pub(super) fn throw(
        &mut self,
        node: NodeId,
        tag: NodeId,
        value: NodeId,
    ) -> Result<ValueId, LowerError> {
        let tag = self.node(tag)?;
        if self.is_terminated() {
            return Ok(tag);
        }
        let value = self.node(value)?;
        if !self.is_terminated() {
            self.emit_throw(node, tag, value)?;
        }
        Ok(value)
    }

    pub(super) fn dynamic_binding(
        &mut self,
        _node: NodeId,
        symbols: NodeId,
        values: NodeId,
        body: &[NodeId],
    ) -> Result<ValueId, LowerError> {
        let symbols = self.node(symbols)?;
        if self.is_terminated() {
            return Ok(symbols);
        }
        let values = self.node(values)?;
        if self.is_terminated() {
            return Ok(values);
        }
        let outer_handlers = self.active_handlers.clone();
        let id = self.allocate_handler_id()?;
        let region_start = self.handler_regions.len();
        let start = self.enter_handler(id)?;
        let first_body_block = self.created_blocks.len();
        let value = self.sequence(body)?;
        let protected = self.protected_blocks(start, first_body_block, region_start);
        let body_end = self.current_block;
        let normal = !self.is_terminated();
        let result = if normal {
            let result = self.builder.fresh_value();
            let merge = self.create_block(vec![(Ty::Word, result)]);
            self.position(body_end)?;
            self.leave_handler(id)?;
            self.terminate(IrTerminator::Jump {
                target: merge,
                args: vec![value],
            })?;
            Some((merge, result))
        } else {
            None
        };
        let handler = self.create_block(Vec::new());
        self.push_none(OpKind::LeaveHandler { region: id })?;
        let nil = self.word_constant(Constant::Nil)?;
        self.terminate(IrTerminator::Return { values: vec![nil] })?;
        self.handler_regions.push(HandlerRegion {
            id,
            kind: HandlerKind::Progv,
            protected,
            handler,
            cleanup: None,
            catch_tag: None,
            binding_targets: vec![symbols, values],
            depth: 0,
            parent: None,
        });
        self.active_handlers = outer_handlers;
        if let Some((merge, result)) = result {
            self.position(merge)?;
            Ok(result)
        } else {
            Ok(value)
        }
    }

    pub(super) fn unwind_protect(
        &mut self,
        _node: NodeId,
        cleanup: CleanupId,
        protected_form: NodeId,
        cleanup_forms: &[NodeId],
    ) -> Result<ValueId, LowerError> {
        let old_cleanup = self.cleanup_forms.insert(cleanup, cleanup_forms.to_vec());
        let outer_handlers = self.active_handlers.clone();
        let id = self.allocate_handler_id()?;
        let region_start = self.handler_regions.len();
        let start = self.enter_handler(id)?;
        let first_body_block = self.created_blocks.len();
        let value = self.node(protected_form)?;
        let protected_blocks = self.protected_blocks(start, first_body_block, region_start);
        let body_end = self.current_block;
        let normal = !self.is_terminated();
        let merge = if normal {
            self.position(body_end)?;
            self.leave_handler(id)?;
            let saved = self.capture_multiple_values(value)?;
            for form in cleanup_forms {
                if self.is_terminated() {
                    break;
                }
                let _ = self.node(*form)?;
            }
            if self.is_terminated() {
                None
            } else {
                let restored = self.restore_multiple_values(saved)?;
                let cleanup_end = self.current_block;
                let result = self.builder.fresh_value();
                let merge = self.create_block(vec![(Ty::Word, result)]);
                self.position(cleanup_end)?;
                self.terminate(IrTerminator::Jump {
                    target: merge,
                    args: vec![restored],
                })?;
                Some(merge)
            }
        } else {
            None
        };
        let handler = self.create_block(Vec::new());
        self.push_none(OpKind::LeaveHandler { region: id })?;
        for form in cleanup_forms {
            if self.is_terminated() {
                break;
            }
            let _ = self.node(*form)?;
        }
        if !self.is_terminated() {
            self.push_none(OpKind::LeaveHandler { region: id })?;
            let nil = self.word_constant(Constant::Nil)?;
            self.terminate(IrTerminator::Return { values: vec![nil] })?;
        }
        self.handler_regions.push(HandlerRegion {
            id,
            kind: HandlerKind::UnwindProtect,
            protected: protected_blocks,
            handler,
            cleanup: Some(handler),
            catch_tag: None,
            binding_targets: Vec::new(),
            depth: 0,
            parent: None,
        });
        self.active_handlers = outer_handlers;
        if let Some(old_cleanup) = old_cleanup {
            let _ = self.cleanup_forms.insert(cleanup, old_cleanup);
        } else {
            let _ = self.cleanup_forms.remove(&cleanup);
        }
        if let Some(merge) = merge {
            self.position(merge)?;
        }
        Ok(value)
    }

    pub(super) fn non_local_exit(
        &mut self,
        node: NodeId,
        target: ExitTarget,
        values: &[NodeId],
        cleanups: &[CleanupId],
    ) -> Result<ValueId, LowerError> {
        match target {
            ExitTarget::Block(block) => {
                let Some(target) = self.block_targets.get(&block).copied() else {
                    return Self::unsupported(node, "non-local-exit-cross-function-block");
                };
                let value = self.exit_value(values)?;
                let saved = self.capture_multiple_values(value)?;
                self.run_cleanups(node, cleanups)?;
                if !self.is_terminated() {
                    self.close_handlers_since(target.active_depth)?;
                    self.terminate(IrTerminator::Jump {
                        target: target.block,
                        args: vec![saved],
                    })?;
                }
                Ok(value)
            }
            ExitTarget::Tag(block) => {
                let Some((target, active_depth)) = self.tag_targets.get(&block).copied() else {
                    return Self::unsupported(node, "non-local-exit-outside-lowered-tagbody");
                };
                let _ = self.exit_value(values)?;
                self.run_cleanups(node, cleanups)?;
                if !self.is_terminated() {
                    self.close_handlers_since(active_depth)?;
                    self.terminate(IrTerminator::Jump {
                        target,
                        args: Vec::new(),
                    })?;
                }
                self.word_constant(Constant::Nil)
            }
            ExitTarget::Dynamic(tag) => {
                let tag = self.node(tag)?;
                let value = self.exit_value(values)?;
                if !self.is_terminated() {
                    // Active UnwindProtect regions run their cleanup handlers while
                    // this throw propagates, so do not execute `cleanups` here.
                    self.emit_throw(node, tag, value)?;
                }
                Ok(value)
            }
            ExitTarget::Catch(_) => Self::unsupported(node, "non-local-exit-catch-target"),
            ExitTarget::Function(_) => Self::unsupported(node, "non-local-function-return"),
        }
    }

    fn exit_value(&mut self, values: &[NodeId]) -> Result<ValueId, LowerError> {
        match values.first() {
            Some(value) => self.node(*value),
            None => self.word_constant(Constant::Nil),
        }
    }

    fn run_cleanups(&mut self, node: NodeId, cleanups: &[CleanupId]) -> Result<(), LowerError> {
        for cleanup in cleanups {
            let Some(forms) = self.cleanup_forms.get(cleanup).cloned() else {
                return Self::unsupported(node, "non-local-exit-cleanup-not-in-active-scope");
            };
            let _ = self.sequence(&forms)?;
            if self.is_terminated() {
                break;
            }
        }
        Ok(())
    }

    pub(super) fn emit_throw(
        &mut self,
        _node: NodeId,
        tag: ValueId,
        value: ValueId,
    ) -> Result<(), LowerError> {
        self.push_none(OpKind::Safepoint)?;
        let thrown = self.push_one(
            OpKind::Builtin {
                name: "throw".to_owned(),
                args: vec![tag, value],
                specialization: None,
                specialization_disabled: false,
            },
            Ty::Word,
        )?;
        if !self.is_terminated() {
            self.terminate(IrTerminator::Throw { condition: thrown })?;
        }
        Ok(())
    }

    fn allocate_handler_id(&mut self) -> Result<HandlerRegionId, LowerError> {
        let id = self.next_handler_id;
        self.next_handler_id = self
            .next_handler_id
            .checked_add(1)
            .ok_or_else(|| LowerError::Ir("handler region identity overflow".to_owned()))?;
        Ok(HandlerRegionId(id))
    }

    fn enter_handler(&mut self, id: HandlerRegionId) -> Result<ncl_ir::BlockId, LowerError> {
        let source = self.current_block;
        let connects = !self.is_terminated();
        let entry = self.create_block(Vec::new());
        if connects {
            self.position(source)?;
            self.terminate(IrTerminator::Jump {
                target: entry,
                args: Vec::new(),
            })?;
        }
        self.position(entry)?;
        self.active_handlers.push(id);
        self.push_none(OpKind::EnterHandler { region: id })?;
        Ok(entry)
    }

    fn leave_handler(&mut self, id: HandlerRegionId) -> Result<(), LowerError> {
        self.push_none(OpKind::LeaveHandler { region: id })?;
        if self.active_handlers.last().copied() == Some(id) {
            let _ = self.active_handlers.pop();
        }
        Ok(())
    }

    pub(super) fn close_handlers_since(&mut self, depth: usize) -> Result<(), LowerError> {
        while self.active_handlers.len() > depth && !self.is_terminated() {
            let Some(id) = self.active_handlers.last().copied() else {
                break;
            };
            self.leave_handler(id)?;
        }
        Ok(())
    }

    fn protected_blocks(
        &self,
        start: ncl_ir::BlockId,
        first_new_block: usize,
        first_new_region: usize,
    ) -> Vec<ncl_ir::BlockId> {
        let candidates = std::iter::once(start)
            .chain(self.created_blocks.iter().skip(first_new_block).copied())
            .collect::<Vec<_>>();
        let candidate_set = candidates.iter().copied().collect::<HashSet<_>>();
        let nested = self.handler_regions.get(first_new_region..).unwrap_or(&[]);
        let mut pending = vec![start];
        let mut reachable = HashSet::new();
        while let Some(block) = pending.pop() {
            if !candidate_set.contains(&block) || !reachable.insert(block) {
                continue;
            }
            if let Some(term) = self.terminators.get(&block) {
                pending.extend(ir_successors(term));
            }
            for region in nested {
                if region.protected.contains(&block) {
                    pending.push(region.handler);
                    pending.extend(region.cleanup);
                }
            }
        }
        candidates
            .into_iter()
            .filter(|block| reachable.contains(block))
            .collect()
    }
}

fn ir_successors(terminator: &IrTerminator) -> Vec<ncl_ir::BlockId> {
    match terminator {
        IrTerminator::Jump { target, .. } => vec![*target],
        IrTerminator::Branch {
            then_target,
            else_target,
            ..
        } => vec![*then_target, *else_target],
        IrTerminator::Switch { cases, default, .. } => cases
            .iter()
            .map(|(_, target, _)| *target)
            .chain(std::iter::once(*default))
            .collect(),
        IrTerminator::CallReturn { .. }
        | IrTerminator::TailCall { .. }
        | IrTerminator::Return { .. }
        | IrTerminator::Throw { .. }
        | IrTerminator::Unreachable => Vec::new(),
    }
}
