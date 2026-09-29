//! Closure capture analysis shared by the IR v2 lowering path.

use std::collections::BTreeSet;

use crate::ast::LambdaExpr;
use crate::symbols::SymbolRef;
use ncl_ir::ValueId;

use super::env::Slot;
use super::function::FunctionLowerer;

/// A captured variable and how its value is obtained in the enclosing function.
pub(super) type Capture = (SymbolRef, Slot);
pub(super) type FunctionCapture = (SymbolRef, ValueId);

/// Find lexical variables from the enclosing function that a lambda reads or writes.
pub(super) fn collect_captures(f: &mut FunctionLowerer, lambda: &LambdaExpr) -> Vec<Capture> {
    let mut captures = Vec::new();
    for name in super::capture::free_variables(lambda) {
        if let Some(slot) = f.env().lookup_variable(&name) {
            captures.push((name, slot));
        }
    }
    captures
}

pub(super) fn collect_function_captures(
    f: &mut FunctionLowerer,
    lambda: &LambdaExpr,
) -> Vec<FunctionCapture> {
    let names = f
        .env()
        .function_names()
        .into_iter()
        .collect::<BTreeSet<_>>();
    super::function_refs::local_function_references(lambda, &names)
        .into_iter()
        .filter_map(|name| {
            f.env()
                .lookup_function(&name)
                .map(|entry| (name, entry.callee))
        })
        .collect()
}
