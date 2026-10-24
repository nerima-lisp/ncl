//! Shared context for IR v2 lowering.

use std::collections::{HashMap, HashSet};

use ncl_ir::{
    BlockId, HandlerRegion, HandlerRegionId, OpKind, ScalarTypeHint, Terminator, Ty, ValueId,
};

use crate::declaration::{Declaration, Quality};
use crate::literal::Literal;
use crate::symbols::SymbolRef;

use super::error::LowerError;
use super::function::{FunctionLowerer, Lowerer};

#[path = "analysis.rs"]
mod analysis;
#[path = "bindings.rs"]
mod bindings;
#[path = "conditional.rs"]
mod conditional;
#[path = "control.rs"]
mod control;
#[path = "control_escape.rs"]
mod control_escape;
#[path = "control_regions.rs"]
mod control_regions;
#[path = "dynamic_binding.rs"]
mod dynamic_binding;
#[path = "expr.rs"]
mod expr;
#[path = "params.rs"]
mod params;
#[path = "unbound_variable.rs"]
mod unbound_variable;
#[path = "unwind_protect.rs"]
mod unwind_protect;

#[derive(Clone, Debug)]
pub(super) struct NonLocalTarget {
    pub(super) name: SymbolRef,
    pub(super) capture: SymbolRef,
}

/// Per-function state not represented by the frozen `FunctionLowerer` API.
pub(super) struct Context<'a> {
    pub(super) module: &'a mut Lowerer,
    pub(super) regions: Vec<HandlerRegion>,
    active: Vec<HandlerRegionId>,
    blocks: Vec<BlockId>,
    targets: Vec<NonLocalTarget>,
    preserve_multiple_values: bool,
    next_region: u32,
    pub(super) lexical_facts: Vec<LexicalFacts>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct LexicalFacts {
    variables: HashMap<SymbolRef, ScalarTypeHint>,
    functions: HashMap<SymbolRef, ScalarTypeHint>,
    shadowed_variables: HashSet<SymbolRef>,
    shadowed_functions: HashSet<SymbolRef>,
    specialization_disabled: bool,
}

impl<'a> Context<'a> {
    pub(super) fn new(module: &'a mut Lowerer, preserve_multiple_values: bool) -> Self {
        Self::with_targets(module, Vec::new(), preserve_multiple_values)
    }

    fn with_targets(
        module: &'a mut Lowerer,
        targets: Vec<NonLocalTarget>,
        preserve_multiple_values: bool,
    ) -> Self {
        Self {
            module,
            regions: Vec::new(),
            active: Vec::new(),
            blocks: vec![BlockId(0)],
            targets,
            preserve_multiple_values,
            next_region: 0,
            lexical_facts: vec![LexicalFacts::default()],
        }
    }

    pub(super) fn push_declarations(&mut self, declarations: &[Declaration]) {
        self.push_declarations_shadowing(declarations, &[]);
    }

    pub(super) fn push_declarations_shadowing(
        &mut self,
        declarations: &[Declaration],
        shadowed: &[SymbolRef],
    ) {
        let mut facts = self.lexical_facts.last().cloned().unwrap_or_default();
        for name in shadowed {
            facts.shadowed_variables.insert(name.clone());
        }
        for declaration in declarations {
            match declaration {
                Declaration::Type {
                    type_specifier,
                    names,
                } => {
                    let hint = scalar_hint(type_specifier.form());
                    for name in names {
                        facts.shadowed_variables.insert(name.clone());
                        facts.variables.remove(name);
                        if let Some(hint) = hint {
                            facts.shadowed_variables.remove(name);
                            facts.variables.insert(name.clone(), hint);
                        }
                    }
                }
                Declaration::Ftype {
                    type_specifier,
                    name,
                } => {
                    facts.shadowed_functions.insert(name.clone());
                    facts.functions.remove(name);
                    if let Some(hint) = function_result_hint(type_specifier.form()) {
                        facts.shadowed_functions.remove(name);
                        facts.functions.insert(name.clone(), hint);
                    }
                }
                Declaration::Optimize(qualities) => {
                    if let Some(speed) = qualities
                        .iter()
                        .rev()
                        .find(|quality| quality.quality == Quality::Speed)
                    {
                        facts.specialization_disabled = speed.value == 0;
                    }
                }
                // check-added-lines: allow(wildcard) unrelated declarations add no scalar hints or specialization policy.
                _ => {}
            }
        }
        self.lexical_facts.push(facts);
    }

    pub(super) fn pop_declarations(&mut self) {
        if self.lexical_facts.len() > 1 {
            self.lexical_facts.pop();
        }
    }

    pub(super) fn variable_hint(&self, name: &SymbolRef) -> Option<ScalarTypeHint> {
        for facts in self.lexical_facts.iter().rev() {
            if facts.shadowed_variables.contains(name) {
                return None;
            }
            if let Some(hint) = facts.variables.get(name) {
                return Some(*hint);
            }
        }
        None
    }

    pub(super) fn function_hint(&self, name: &SymbolRef) -> Option<ScalarTypeHint> {
        for facts in self.lexical_facts.iter().rev() {
            if facts.shadowed_functions.contains(name) {
                return None;
            }
            if let Some(hint) = facts.functions.get(name) {
                return Some(*hint);
            }
        }
        None
    }

    pub(super) fn specialization_disabled(&self) -> bool {
        self.lexical_facts
            .last()
            .is_some_and(|facts| facts.specialization_disabled)
    }

    fn block(&mut self, f: &mut FunctionLowerer, params: Vec<(Ty, ValueId)>) -> BlockId {
        let id = f.create_block(params);
        self.blocks.push(id);
        id
    }

    fn split_region_entry(&mut self, f: &mut FunctionLowerer) -> Result<BlockId, LowerError> {
        let source = f.current_block();
        let connect_source = !f.is_terminated();
        let entry = self.block(f, Vec::new());
        if connect_source {
            f.position(source)?;
            f.terminate(Terminator::Jump {
                target: entry,
                args: Vec::new(),
            })?;
        }
        f.position(entry)?;
        Ok(entry)
    }

    fn enter_current(
        &mut self,
        f: &mut FunctionLowerer,
        id: HandlerRegionId,
    ) -> Result<(), LowerError> {
        self.active.push(id);
        f.none(OpKind::EnterHandler { region: id })
    }

    fn enter(
        &mut self,
        f: &mut FunctionLowerer,
        id: HandlerRegionId,
    ) -> Result<BlockId, LowerError> {
        let entry = self.split_region_entry(f)?;
        self.enter_current(f, id)?;
        Ok(entry)
    }

    fn leave(&mut self, f: &mut FunctionLowerer, id: HandlerRegionId) -> Result<(), LowerError> {
        f.none(OpKind::LeaveHandler { region: id })?;
        if self.active.last().copied() == Some(id) {
            self.active.pop();
        }
        Ok(())
    }

    /// Number of handler regions currently open (entered but not yet left).
    const fn active_len(&self) -> usize {
        self.active.len()
    }

    /// Collect the blocks that can execute within a newly lowered handler
    /// body, including handlers established by nested regions. Unreachable
    /// normal-path merges are not protected by an enclosing region.
    fn protected_blocks(
        &self,
        f: &FunctionLowerer,
        start: BlockId,
        first_new_block: usize,
        first_new_region: usize,
    ) -> Vec<BlockId> {
        let candidates = std::iter::once(start)
            .chain(self.blocks.iter().skip(first_new_block).copied())
            .collect::<Vec<_>>();
        let candidate_set = candidates.iter().copied().collect::<HashSet<_>>();
        let nested_regions = self.regions.get(first_new_region..).unwrap_or(&[]);
        let mut reachable = HashSet::new();
        let mut pending = vec![start];

        while let Some(block) = pending.pop() {
            if !candidate_set.contains(&block) || !reachable.insert(block) {
                continue;
            }
            pending.extend(f.successors(block).iter().copied());
            for region in nested_regions {
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

    /// Leave every handler region opened since `depth`, innermost first.
    ///
    /// Used by a lexically-visible `return-from`/`go` fast path (jumping
    /// directly to the bound block rather than desugaring to `throw`) so
    /// `unwind-protect` cleanup and `progv` restores still run for regions
    /// nested between the jump and its target, even though no closure
    /// boundary is crossed.
    fn close_active_since(
        &mut self,
        f: &mut FunctionLowerer,
        depth: usize,
    ) -> Result<(), LowerError> {
        while self.active.len() > depth {
            let Some(id) = self.active.last().copied() else {
                break;
            };
            self.leave(f, id)?;
        }
        Ok(())
    }

    fn token(name: &SymbolRef, kind: &str, id: HandlerRegionId) -> SymbolRef {
        SymbolRef::interned("NCL-NLX", format!("{kind}:{}:{}", name.name, id.0))
    }

    fn target(&self, name: &SymbolRef) -> Option<NonLocalTarget> {
        self.targets
            .iter()
            .rev()
            .find(|target| target.name == *name)
            .cloned()
    }
}

fn scalar_hint(form: &Literal) -> Option<ScalarTypeHint> {
    let Literal::Symbol(symbol) = form else {
        let elements = form.list_elements()?;
        return elements.first().and_then(|name| scalar_hint(name));
    };
    Some(match symbol.name.to_ascii_uppercase().as_str() {
        "FIXNUM" => ScalarTypeHint::Fixnum,
        "INTEGER" => ScalarTypeHint::Integer,
        "NUMBER" | "REAL" | "RATIONAL" => ScalarTypeHint::Number,
        "BOOLEAN" => ScalarTypeHint::Boolean,
        "NON-NUMBER" | "SYMBOL" | "STRING" | "CHARACTER" | "CONS" | "LIST" | "NULL" => {
            ScalarTypeHint::NonNumber
        }
        // check-added-lines: allow(wildcard) unrecognized source types remain unclassified.
        _ => return None,
    })
}

fn function_result_hint(form: &Literal) -> Option<ScalarTypeHint> {
    let elements = form.list_elements()?;
    let [function_type, _arguments, result_type, ..] = elements.as_slice() else {
        return None;
    };
    if !matches!(function_type, Literal::Symbol(name) if name.name.eq_ignore_ascii_case("FUNCTION"))
    {
        return None;
    }
    if matches!(result_type, Literal::Symbol(name) if name.name.eq_ignore_ascii_case("VALUES")) {
        return elements.get(3).and_then(|ty| scalar_hint(ty));
    }
    scalar_hint(result_type)
}
