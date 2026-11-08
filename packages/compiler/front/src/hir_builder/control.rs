use ncl_hir::{CleanupId, ExitTarget, NodeKind, ScopeId, TagbodyTag, Terminator};

use crate::ast::{Expr, TagbodyItem};
use crate::symbols::SymbolRef;

use super::context::BuildContext;
use super::{HirBuildError, UnsupportedHirForm};

impl BuildContext<'_> {
    pub(super) fn lower_block(
        &mut self,
        scope: ScopeId,
        name: &SymbolRef,
        body: &[Expr],
    ) -> Result<ncl_hir::NodeId, HirBuildError> {
        let symbol = self.intern(name)?;
        let block = self.node(
            scope,
            NodeKind::Block {
                name: symbol,
                body: Vec::new(),
            },
        )?;
        self.push_scope_maps();
        self.add_block_target(symbol, block);
        let body = body
            .iter()
            .map(|form| self.lower_expr(form, scope))
            .collect::<Result<Vec<_>, _>>()?;
        self.pop_scope_maps();
        let Some(node) = self.form.nodes_mut().get_mut(block) else {
            return Err(HirBuildError::Form(ncl_hir::FormError::InvalidNode(block)));
        };
        node.kind = NodeKind::Block { name: symbol, body };
        Ok(block)
    }

    pub(super) fn lower_return_from(
        &mut self,
        scope: ScopeId,
        name: &SymbolRef,
        value: Option<&Expr>,
    ) -> Result<ncl_hir::NodeId, HirBuildError> {
        let symbol = self.intern(name)?;
        let Some((target, cleanup_depth)) = self.find_block_target(symbol) else {
            return Err(HirBuildError::Unsupported(UnsupportedHirForm::UnknownBlock));
        };
        let value = value.map(|form| self.lower_expr(form, scope)).transpose()?;
        let cleanups = self.cleanups_from(cleanup_depth);
        if cleanups.is_empty() {
            self.node(scope, NodeKind::ReturnFrom { target, value })
        } else {
            let values = value.into_iter().collect();
            self.node(
                scope,
                NodeKind::NonLocalExit {
                    target: ExitTarget::Block(target),
                    values,
                    cleanups,
                },
            )
        }
    }

    pub(super) fn lower_tagbody(
        &mut self,
        scope: ScopeId,
        items: &[TagbodyItem],
    ) -> Result<ncl_hir::NodeId, HirBuildError> {
        let tagbody = self.node(
            scope,
            NodeKind::Tagbody {
                tags: Vec::new(),
                forms: Vec::new(),
            },
        )?;
        let control = self.form.control_flow_mut();
        control.begin_function(tagbody)?;
        let entry = self.form.control_flow_mut().create_block(tagbody)?;

        self.push_scope_maps();
        let mut tags = Vec::new();
        let mut tag_blocks = Vec::new();
        let mut has_form_before_first_tag = false;
        for item in items {
            match item {
                TagbodyItem::Tag(name) => {
                    let symbol = self.intern(name)?;
                    if self.has_current_tag_target(symbol) {
                        return Err(HirBuildError::Unsupported(UnsupportedHirForm::DuplicateTag));
                    }
                    let target = if tags.is_empty() && !has_form_before_first_tag {
                        entry
                    } else {
                        self.form.control_flow_mut().create_block(tagbody)?
                    };
                    self.add_tag_target(symbol, target);
                    tags.push(TagbodyTag {
                        name: symbol,
                        block: target,
                    });
                    tag_blocks.push(target);
                }
                TagbodyItem::Form(_) if tags.is_empty() => {
                    has_form_before_first_tag = true;
                }
                TagbodyItem::Form(_) => {}
            }
        }

        let mut forms = Vec::new();
        let mut current = entry;
        let mut current_terminated = false;
        let mut tag_index = 0;

        for item in items {
            match item {
                TagbodyItem::Tag(_) => {
                    let Some(target) = tag_blocks.get(tag_index).copied() else {
                        return Err(HirBuildError::Form(ncl_hir::FormError::InvalidNode(
                            tagbody,
                        )));
                    };
                    tag_index += 1;
                    if current != target && !current_terminated {
                        self.form
                            .control_flow_mut()
                            .set_terminator(current, Terminator::Jump(target))?;
                    }
                    current = target;
                    current_terminated = false;
                }
                TagbodyItem::Form(form) => {
                    let node = self.lower_expr(form, scope)?;
                    forms.push(node);
                    if !current_terminated {
                        self.form
                            .control_flow_mut()
                            .append_operation(current, node)?;
                        if let Some(terminator) = self.exit_terminator(node)? {
                            self.form
                                .control_flow_mut()
                                .set_terminator(current, terminator)?;
                            current_terminated = true;
                        }
                    }
                }
            }
        }
        if !current_terminated {
            self.form
                .control_flow_mut()
                .set_terminator(current, Terminator::Return { values: Vec::new() })?;
        }
        self.pop_scope_maps();
        let Some(node) = self.form.nodes_mut().get_mut(tagbody) else {
            return Err(HirBuildError::Form(ncl_hir::FormError::InvalidNode(
                tagbody,
            )));
        };
        node.kind = NodeKind::Tagbody { tags, forms };
        Ok(tagbody)
    }

    pub(super) fn lower_go(
        &mut self,
        scope: ScopeId,
        tag: &SymbolRef,
    ) -> Result<ncl_hir::NodeId, HirBuildError> {
        let symbol = self.intern(tag)?;
        let Some((target, cleanup_depth)) = self.find_tag_target(symbol) else {
            return Err(HirBuildError::Unsupported(UnsupportedHirForm::UnknownTag));
        };
        let cleanups = self.cleanups_from(cleanup_depth);
        if cleanups.is_empty() {
            self.node(scope, NodeKind::Go { target })
        } else {
            self.node(
                scope,
                NodeKind::NonLocalExit {
                    target: ExitTarget::Tag(target),
                    values: Vec::new(),
                    cleanups,
                },
            )
        }
    }

    pub(super) fn lower_catch(
        &mut self,
        scope: ScopeId,
        tag: &Expr,
        body: &[Expr],
    ) -> Result<ncl_hir::NodeId, HirBuildError> {
        let tag = self.lower_expr(tag, scope)?;
        let body = body
            .iter()
            .map(|form| self.lower_expr(form, scope))
            .collect::<Result<Vec<_>, _>>()?;
        self.node(scope, NodeKind::Catch { tag, body })
    }

    pub(super) fn lower_throw(
        &mut self,
        scope: ScopeId,
        tag: &Expr,
        value: &Expr,
    ) -> Result<ncl_hir::NodeId, HirBuildError> {
        let tag = self.lower_expr(tag, scope)?;
        let value = self.lower_expr(value, scope)?;
        let cleanups = self.cleanups_from(0);
        if cleanups.is_empty() {
            self.node(scope, NodeKind::Throw { tag, value })
        } else {
            self.node(
                scope,
                NodeKind::NonLocalExit {
                    target: ExitTarget::Dynamic(tag),
                    values: vec![value],
                    cleanups,
                },
            )
        }
    }

    pub(super) fn lower_unwind_protect(
        &mut self,
        scope: ScopeId,
        protected: &Expr,
        cleanup: &[Expr],
    ) -> Result<ncl_hir::NodeId, HirBuildError> {
        let cleanup_id = CleanupId::try_from_index(self.form_id, self.next_cleanup)?;
        self.next_cleanup = self.next_cleanup.checked_add(1).ok_or_else(|| {
            HirBuildError::Id(ncl_hir::IdError::IndexOverflow {
                index: self.next_cleanup,
            })
        })?;
        self.cleanups.push(cleanup_id);
        let protected = self.lower_expr(protected, scope)?;
        self.cleanups.pop();
        let cleanup_forms = cleanup
            .iter()
            .map(|form| self.lower_expr(form, scope))
            .collect::<Result<Vec<_>, _>>()?;
        self.node(
            scope,
            NodeKind::UnwindProtect {
                cleanup: cleanup_id,
                protected,
                cleanup_forms,
            },
        )
    }

    pub(super) fn lower_progv(
        &mut self,
        scope: ScopeId,
        symbols: &Expr,
        values: &Expr,
        body: &[Expr],
    ) -> Result<ncl_hir::NodeId, HirBuildError> {
        let symbols = self.lower_expr(symbols, scope)?;
        let values = self.lower_expr(values, scope)?;
        let body = body
            .iter()
            .map(|form| self.lower_expr(form, scope))
            .collect::<Result<Vec<_>, _>>()?;
        self.node(
            scope,
            NodeKind::DynamicBinding {
                symbols,
                values,
                body,
            },
        )
    }

    fn cleanups_from(&self, depth: usize) -> Vec<CleanupId> {
        self.cleanups.iter().skip(depth).rev().copied().collect()
    }

    fn exit_terminator(
        &mut self,
        node: ncl_hir::NodeId,
    ) -> Result<Option<Terminator>, HirBuildError> {
        let Some(node) = self.form.nodes_mut().get(node).cloned() else {
            return Err(HirBuildError::Form(ncl_hir::FormError::InvalidNode(node)));
        };
        Ok(Some(match node.kind {
            NodeKind::Go { target } => Terminator::Jump(target),
            NodeKind::ReturnFrom { target, value } => Terminator::NonLocalExit {
                target: ExitTarget::Block(target),
                values: value.into_iter().collect(),
                cleanups: Vec::new(),
            },
            NodeKind::Throw { tag, value } => Terminator::NonLocalExit {
                target: ExitTarget::Dynamic(tag),
                values: vec![value],
                cleanups: Vec::new(),
            },
            NodeKind::NonLocalExit {
                target,
                values,
                cleanups,
            } => Terminator::NonLocalExit {
                target,
                values,
                cleanups,
            },
            _ => return Ok(None),
        }))
    }
}
