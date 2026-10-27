//! Non-local return analysis for IR v2 control lowering.

#[path = "analysis/go.rs"]
mod go;
#[path = "analysis/returns.rs"]
mod returns;
#[path = "analysis/unwind.rs"]
mod unwind;

use crate::ast::Expr;
use crate::symbols::SymbolRef;

pub(super) fn body_has_nested_return(forms: &[Expr], name: &SymbolRef) -> bool {
    forms.iter().any(|form| returns::nested_return(form, name))
        || (forms
            .iter()
            .any(|form| unwind::mentions_return_from(form, name))
            && forms.iter().any(unwind::has_unwind_protect))
}

pub(super) fn body_requires_escaping_go(forms: &[Expr], name: &SymbolRef) -> bool {
    forms.iter().any(|form| go::nested_go(form, name))
        || forms
            .iter()
            .any(|form| unwind::go_exits_unwind_protect(form, name))
}

/// Whether a lambda must carry the hidden tag for a lexical exit. This is
/// intentionally conservative: carrying an unused tag is cheaper than
/// turning a valid closure exit into a stale symbol lookup.
pub(super) fn mentions_exit(forms: &[Expr], name: &SymbolRef) -> bool {
    forms.iter().any(|form| {
        returns::contains_return(form, name)
            || returns::nested_return(form, name)
            || go::contains_go(form, name)
    })
}

pub(super) fn lambda_mentions_exit(lambda: &crate::ast::LambdaExpr, name: &SymbolRef) -> bool {
    returns::lambda_forms(lambda).any(|form| mentions_exit(std::slice::from_ref(form), name))
}
