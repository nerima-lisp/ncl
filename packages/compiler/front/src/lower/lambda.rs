//! Closure capture analysis shared by the IR v2 lowering path.

use crate::ast::LambdaExpr;
use crate::symbols::SymbolRef;

use super::env::Slot;
use super::function::FunctionLowerer;

/// A captured variable and how its value is obtained in the enclosing function.
pub(super) type Capture = (SymbolRef, Slot);

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
