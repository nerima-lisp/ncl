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
        // Any variable assigned in either arm (transitively, through nested
        // ifs/blocks/tagbodies) needs its post-if value merged through a block
        // parameter, the same mechanism `lower_block`/`lower_tagbody` use.
        // Only `Slot::Value` bindings need this: `Slot::Cell` bindings (boxed
        // for closure capture) are mutated in place and already observe
        // writes from either arm without a merge.
        let mut assigned: std::collections::BTreeSet<SymbolRef> =
            capture::analyze(std::slice::from_ref(then))
                .assigned_names()
                .into_iter()
                .collect();
        if let Some(form) = otherwise {
            assigned.extend(capture::analyze(std::slice::from_ref(form)).assigned_names());
        }
        // Snapshot each live variable's value as of just before the branch:
        // both arms start from this same value, since they are mutually
        // exclusive alternatives over the same lexical env (there is no
        // per-branch env to fork, unlike a real interpreter).
        let live = assigned
            .into_iter()
            .filter_map(|name| match f.env().lookup_variable(&name) {
                Some(Slot::Value(value)) => Some((name, value)),
                // check-added-lines: allow(wildcard) cells need no merge.
                _ => None,
            })
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
                        Some(Slot::Value(value)) => Some(value),
                        // check-added-lines: allow(wildcard) only value slots are live here.
                        _ => None,
                    }),
            );
            f.terminate(Terminator::Jump {
                target: merge,
                args,
            })?;
        }
        // Undo whatever the `then` arm did to the live variables before
        // lowering `else`: the two arms are alternatives, not a sequence, so
        // `else` must see the pre-branch values, not `then`'s writes.
        for (name, value) in &live {
            f.env().rebind_variable(name, Slot::Value(*value));
        }
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
                        Some(Slot::Value(value)) => Some(value),
                        // check-added-lines: allow(wildcard) only value slots are live here.
                        _ => None,
                    }),
            );
            f.terminate(Terminator::Jump {
                target: merge,
                args,
            })?;
        }
        f.position(merge)?;
        for ((name, _), value) in live.into_iter().zip(live_values) {
            f.env().rebind_variable(&name, Slot::Value(value));
        }
        Ok(result)
    }
}
