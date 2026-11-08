use std::collections::{HashMap, HashSet};

use ncl_hir::{
    FormId, HirForm, HirFormBuilder, OptimizationPolicy, ScopeId, SourceOriginId, SourceOrigins,
    SymbolId, SymbolTable,
};

use crate::symbols::SymbolRef;

use super::HirBuildError;

pub(super) struct BuildContext<'a> {
    pub(super) form: HirFormBuilder,
    pub(super) symbols: &'a mut SymbolTable,
    pub(super) form_id: FormId,
    pub(super) specials: HashMap<ScopeId, HashSet<SymbolId>>,
    pub(super) block_targets: Vec<HashMap<SymbolId, (ncl_hir::NodeId, usize)>>,
    pub(super) tag_targets: Vec<HashMap<SymbolId, (ncl_hir::BlockId, usize)>>,
    pub(super) policies: Vec<OptimizationPolicy>,
    pub(super) cleanups: Vec<ncl_hir::CleanupId>,
    pub(super) next_cleanup: usize,
}

impl<'a> BuildContext<'a> {
    pub(super) fn new(
        form: FormId,
        origins: SourceOrigins,
        default_origin: SourceOriginId,
        symbols: &'a mut SymbolTable,
    ) -> Result<Self, HirBuildError> {
        let form_builder = HirFormBuilder::new(form, origins, default_origin)?;
        Ok(Self {
            form: form_builder,
            symbols,
            form_id: form,
            specials: HashMap::new(),
            block_targets: vec![HashMap::new()],
            tag_targets: vec![HashMap::new()],
            policies: vec![OptimizationPolicy::default()],
            cleanups: Vec::new(),
            next_cleanup: 0,
        })
    }

    pub(super) fn intern(&mut self, symbol: &SymbolRef) -> Result<SymbolId, HirBuildError> {
        let id = self.symbols.intern(
            self.form_id,
            symbol.package.as_deref(),
            &symbol.name,
            symbol.uninterned,
        )?;
        self.form.register_symbol(id)?;
        Ok(id)
    }

    pub(super) fn special(
        &mut self,
        scope: ScopeId,
        symbol: SymbolId,
    ) -> Result<bool, HirBuildError> {
        let mut current = Some(scope);
        while let Some(candidate) = current {
            if self
                .specials
                .get(&candidate)
                .is_some_and(|symbols| symbols.contains(&symbol))
            {
                return Ok(true);
            }
            current = self.form.bindings_mut().scope_parent(candidate)?;
        }
        Ok(false)
    }

    pub(super) fn resolved_variable(
        &mut self,
        scope: ScopeId,
        symbol: &SymbolRef,
    ) -> Result<ncl_hir::VariableTarget, HirBuildError> {
        let symbol_id = self.intern(symbol)?;
        if self.special(scope, symbol_id)? {
            return Ok(ncl_hir::VariableTarget::Special(symbol_id));
        }
        if let Some(binding) = self
            .form
            .bindings_mut()
            .resolve_variable(scope, symbol_id)?
        {
            return Ok(ncl_hir::VariableTarget::Lexical(binding));
        }
        Ok(ncl_hir::VariableTarget::Global(symbol_id))
    }

    pub(super) fn push_scope_maps(&mut self) {
        self.block_targets.push(HashMap::new());
        self.tag_targets.push(HashMap::new());
        self.policies
            .push(self.policies.last().copied().unwrap_or_default());
    }

    pub(super) fn pop_scope_maps(&mut self) {
        self.block_targets.pop();
        self.tag_targets.pop();
        if self.policies.len() > 1 {
            self.policies.pop();
        }
    }

    pub(super) fn add_special(&mut self, scope: ScopeId, symbol: SymbolId) {
        let _ = self.specials.entry(scope).or_default().insert(symbol);
    }

    pub(super) fn add_block_target(&mut self, symbol: SymbolId, node: ncl_hir::NodeId) {
        if let Some(scope) = self.block_targets.last_mut() {
            let _ = scope.insert(symbol, (node, self.cleanups.len()));
        }
    }

    pub(super) fn find_block_target(&self, symbol: SymbolId) -> Option<(ncl_hir::NodeId, usize)> {
        self.block_targets
            .iter()
            .rev()
            .find_map(|scope| scope.get(&symbol).copied())
    }

    pub(super) fn add_tag_target(&mut self, symbol: SymbolId, block: ncl_hir::BlockId) {
        if let Some(scope) = self.tag_targets.last_mut() {
            let _ = scope.insert(symbol, (block, self.cleanups.len()));
        }
    }

    pub(super) fn find_tag_target(&self, symbol: SymbolId) -> Option<(ncl_hir::BlockId, usize)> {
        self.tag_targets
            .iter()
            .rev()
            .find_map(|scope| scope.get(&symbol).copied())
    }

    pub(super) fn has_current_tag_target(&self, symbol: SymbolId) -> bool {
        self.tag_targets
            .last()
            .is_some_and(|scope| scope.contains_key(&symbol))
    }

    pub(super) fn finish(self) -> Result<HirForm, HirBuildError> {
        self.form.finish().map_err(HirBuildError::Form)
    }
}
