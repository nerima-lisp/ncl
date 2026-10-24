//! `if` lowering for the IR v2 path.

use ncl_ir::{Compare, OpKind, Terminator, Ty, ValueId};

use crate::ast::Expr;
use crate::symbols::SymbolRef;

use super::super::capture;
use super::super::env::Slot;
use super::super::error::LowerError;
use super::super::function::FunctionLowerer;
use super::Context;

impl Context<'_> {
    pub(super) fn lower_if(
        &mut self,
        f: &mut FunctionLowerer,
        test: &Expr,
        then: &Expr,
        otherwise: Option<&Expr>,
    ) -> Result<ValueId, LowerError> {
        let tested = self.lower_expr(f, test)?;
        let nil = f.nil()?;
        let condition = f.one(
            OpKind::Compare {
                op: Compare::Ne,
                left: tested,
                right: nil,
            },
            Ty::Bool,
        )?;
        let start = f.current_block();
        let result = f.fresh_value();
        let mut assigned: std::collections::BTreeSet<SymbolRef> =
            capture::analyze(std::slice::from_ref(then))
                .assigned_names()
                .into_iter()
                .collect();
        if let Some(form) = otherwise {
            assigned.extend(capture::analyze(std::slice::from_ref(form)).assigned_names());
        }
        // Escaping control forms can rebind a captured cell at their join point,
        // so its SSA value must be merged along with ordinary value slots.
        let live = assigned
            .into_iter()
            .filter_map(|name| f.env().lookup_variable(&name).map(|slot| (name, slot)))
            .collect::<Vec<_>>();
        let live_values = live.iter().map(|_| f.fresh_value()).collect::<Vec<_>>();
        let then_block = self.block(f, Vec::new());
        let else_block = self.block(f, Vec::new());
        let merge = self.block(
            f,
            std::iter::once((Ty::Word, result))
                .chain(live_values.iter().copied().map(|value| (Ty::Word, value)))
                .collect(),
        );
        // Nonlocal exits can close regions while lowering an arm, so each
        // sibling and the merge must resume from the branch-entry stack.
        let branch_active = self.active.clone();
        f.position(start)?;
        f.terminate(Terminator::Branch {
            condition,
            then_target: then_block,
            then_args: Vec::new(),
            else_target: else_block,
            else_args: Vec::new(),
        })?;
        f.position(then_block)?;
        let then_value = self.lower_expr(f, then)?;
        if !f.is_terminated() {
            let mut args = vec![then_value];
            args.extend(
                live.iter()
                    .filter_map(|(name, _)| match f.env().lookup_variable(name) {
                        Some(Slot::Value(value) | Slot::Cell(value)) => Some(value),
                        None => None,
                    }),
            );
            f.terminate(Terminator::Jump {
                target: merge,
                args,
            })?;
        }
        // Undo whatever the then arm did before lowering the mutually exclusive
        // else arm.
        for (name, slot) in &live {
            f.env().rebind_variable(name, *slot);
        }
        self.active.clone_from(&branch_active);
        f.position(else_block)?;
        let else_value = match otherwise {
            Some(form) => self.lower_expr(f, form)?,
            None => f.nil()?,
        };
        if !f.is_terminated() {
            let mut args = vec![else_value];
            args.extend(
                live.iter()
                    .filter_map(|(name, _)| match f.env().lookup_variable(name) {
                        Some(Slot::Value(value) | Slot::Cell(value)) => Some(value),
                        None => None,
                    }),
            );
            f.terminate(Terminator::Jump {
                target: merge,
                args,
            })?;
        }
        self.active.clone_from(&branch_active);
        f.position(merge)?;
        for ((name, slot), value) in live.into_iter().zip(live_values) {
            let slot = match slot {
                Slot::Value(_) => Slot::Value(value),
                Slot::Cell(_) => Slot::Cell(value),
            };
            f.env().rebind_variable(&name, slot);
        }
        Ok(result)
    }
}
