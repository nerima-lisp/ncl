//! Dynamic (special-variable) binding for `let`/`let*`.

use ncl_ir::{HandlerKind, HandlerRegion, HandlerRegionId, OpKind, Terminator, Ty, ValueId};

use crate::ast::Expr;
use crate::declaration::Declaration;
use crate::symbols::SymbolRef;

use super::super::capture;
use super::super::error::LowerError;
use super::super::function::FunctionLowerer;
use super::Context;
use super::params::bind_let;

/// The names a `let`/`let*`'s declarations mark special: those declared
/// `(special name*)` locally in the body, and those the front end folded in
/// from a prior global proclamation (`defvar`, `defparameter`, `declaim`, or
/// `proclaim`).
fn special_names(declarations: &[Declaration]) -> Vec<SymbolRef> {
    declarations
        .iter()
        .filter_map(|declaration| match declaration {
            Declaration::Special(names) => Some(names.iter().cloned()),
            _ => None, // check-added-lines: allow(wildcard) only special declarations mark a binding
        })
        .flatten()
        .collect()
}

impl Context<'_> {
    pub(super) fn lower_let(
        &mut self,
        f: &mut FunctionLowerer,
        sequential: bool,
        bindings: &[crate::ast::LetBinding],
        declarations: &[Declaration],
        body: &[Expr],
    ) -> Result<ValueId, LowerError> {
        let specials = special_names(declarations);
        let analysis = capture::analyze(body);
        f.env().push();
        let value = if sequential {
            self.lower_let_sequential(f, bindings, &specials, &analysis, body)?
        } else {
            let mut evaluated = Vec::with_capacity(bindings.len());
            for binding in bindings {
                let value = match binding.value.as_ref() {
                    Some(form) => self.lower_expr(f, form)?,
                    None => f.nil()?,
                };
                evaluated.push(value);
            }
            self.lower_let_parallel(f, bindings, &evaluated, &specials, &analysis, body)?
        };
        f.env().pop();
        Ok(value)
    }

    /// Lower a `let*`'s remaining bindings and body, evaluating each init
    /// form immediately before installing its binding (so a later binding's
    /// init form observes an earlier special's new dynamic value, matching
    /// sequential `let*` semantics) and dynamically rebinding every name
    /// `declarations` marks special rather than giving it a lexical slot.
    fn lower_let_sequential(
        &mut self,
        f: &mut FunctionLowerer,
        bindings: &[crate::ast::LetBinding],
        specials: &[SymbolRef],
        analysis: &capture::Analysis,
        body: &[Expr],
    ) -> Result<ValueId, LowerError> {
        let Some((binding, rest)) = bindings.split_first() else {
            return self.lower_body(f, body);
        };
        let value = match binding.value.as_ref() {
            Some(form) => self.lower_expr(f, form)?,
            None => f.nil()?,
        };
        if specials.contains(&binding.name) {
            self.lower_dynamic_binding(f, &binding.name, value, move |ctx, f| {
                ctx.lower_let_sequential(f, rest, specials, analysis, body)
            })
        } else {
            bind_let(f, &binding.name, value, analysis)?;
            self.lower_let_sequential(f, rest, specials, analysis, body)
        }
    }

    /// Lower a `let`'s remaining bindings and body from already-evaluated
    /// init values (`let` evaluates every init form before any binding takes
    /// effect), dynamically rebinding every name `declarations` marks special
    /// rather than giving it a lexical slot.
    fn lower_let_parallel(
        &mut self,
        f: &mut FunctionLowerer,
        bindings: &[crate::ast::LetBinding],
        values: &[ValueId],
        specials: &[SymbolRef],
        analysis: &capture::Analysis,
        body: &[Expr],
    ) -> Result<ValueId, LowerError> {
        let (Some((binding, rest_bindings)), Some((&value, rest_values))) =
            (bindings.split_first(), values.split_first())
        else {
            return self.lower_body(f, body);
        };
        if specials.contains(&binding.name) {
            self.lower_dynamic_binding(f, &binding.name, value, move |ctx, f| {
                ctx.lower_let_parallel(f, rest_bindings, rest_values, specials, analysis, body)
            })
        } else {
            bind_let(f, &binding.name, value, analysis)?;
            self.lower_let_parallel(f, rest_bindings, rest_values, specials, analysis, body)
        }
    }

    /// Dynamically rebind one special variable's global value cell around
    /// `continuation`, restoring the previous value (bound or unbound) when
    /// leaving normally and when a non-local exit unwinds past this point.
    ///
    /// This reuses exactly the `progv` handler-region machinery (the same
    /// GC-safe binding stack, established via [`HandlerKind::Progv`] and the
    /// `EnterProgv`/`LeaveProgv` runtime functions): a `let`/`let*` binding
    /// that names a special variable behaves precisely like a one-variable
    /// `progv` wrapped around the rest of the form.
    fn lower_dynamic_binding(
        &mut self,
        f: &mut FunctionLowerer,
        name: &SymbolRef,
        value: ValueId,
        continuation: impl FnOnce(&mut Self, &mut FunctionLowerer) -> Result<ValueId, LowerError>,
    ) -> Result<ValueId, LowerError> {
        let symbol = f.symbol(name)?;
        let nil = f.nil()?;
        f.safepoint()?;
        let symbols = f.one(
            OpKind::Builtin {
                name: "CONS".to_owned(),
                args: vec![symbol, nil],
            },
            Ty::Word,
        )?;
        f.safepoint()?;
        let values = f.one(
            OpKind::Builtin {
                name: "CONS".to_owned(),
                args: vec![value, nil],
            },
            Ty::Word,
        )?;
        let result = f.fresh_value();
        let live = f.env().visible_variables();
        let region_id = HandlerRegionId(self.next_region);
        self.next_region += 1;
        let start = f.current_block();
        self.enter(f, region_id)?;
        let inner = continuation(self, f)?;
        let protected = self
            .blocks
            .iter()
            .copied()
            .filter(|block| block.0 >= start.0)
            .collect::<Vec<_>>();
        let body_end = f.current_block();
        let normal_path = !f.is_terminated();
        let live_values = live.iter().map(|_| f.fresh_value()).collect::<Vec<_>>();
        let merge = self.block(
            f,
            std::iter::once((Ty::Word, result))
                .chain(live_values.iter().copied().map(|value| (Ty::Word, value)))
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
                    detail: "dynamic binding lost a live lexical binding".to_owned(),
                })?;
            f.terminate(Terminator::Jump {
                target: merge,
                args: std::iter::once(inner)
                    .chain(normal_live.iter().map(|(_, slot)| match slot {
                        super::super::env::Slot::Cell(value)
                        | super::super::env::Slot::Value(value) => *value,
                    }))
                    .collect(),
            })?;
        }
        let handler = self.block(f, Vec::new());
        self.leave(f, region_id)?;
        let restored_value = f.nil()?;
        f.terminate(Terminator::Return {
            values: vec![restored_value],
        })?;
        self.regions.push(HandlerRegion {
            id: region_id,
            kind: HandlerKind::Progv,
            protected,
            handler,
            cleanup: None,
            catch_tag: None,
            binding_targets: vec![symbols, values],
            depth: 0,
            parent: None,
        });
        f.position(merge)?;
        for ((name, slot), value) in live.into_iter().zip(live_values) {
            f.env().rebind_variable(
                &name,
                match slot {
                    super::super::env::Slot::Cell(_) => super::super::env::Slot::Cell(value),
                    super::super::env::Slot::Value(_) => super::super::env::Slot::Value(value),
                },
            );
        }
        Ok(result)
    }
}
