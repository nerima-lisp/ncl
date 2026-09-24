//! Macroexpansion through `MacroCaller`, and local macro shadowing.
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "integration tests assert on results"
)]

#[path = "support/forms.rs"]
mod forms;

use forms::{Fixture, intern, list};

use ncl_compiler_front::{Expr, FrontError, Literal, MacroCaller, SymbolRef};
use ncl_object::{Runtime, ThreadContext, Word, set_symbol_macro};

/// A macro caller that returns `:GLOBAL` from a global macro and `:LOCAL` from
/// a local one, so a test can tell which path the expander took.
struct Marker;

impl Marker {
    fn marker(ctx: &mut ThreadContext, runtime: &Runtime, name: &str) -> Word {
        let quote = intern(ctx, runtime, "COMMON-LISP", "QUOTE");
        let marker = intern(ctx, runtime, "KEYWORD", name);
        list(ctx, runtime, &[quote, marker])
    }
}

impl MacroCaller for Marker {
    fn call_macro(
        &mut self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        _name: &SymbolRef,
        _form: Word,
    ) -> Result<Word, FrontError> {
        Ok(Self::marker(ctx, runtime, "GLOBAL"))
    }

    fn call_local_macro(
        &mut self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        _definition: &ncl_compiler_front::LocalMacro,
        _form: Word,
    ) -> Result<Word, FrontError> {
        Ok(Self::marker(ctx, runtime, "LOCAL"))
    }
}

/// Assert that an expression is the constant keyword `name`.
fn assert_marker(expr: Expr, name: &str) {
    match expr {
        Expr::Constant(Literal::Symbol(symbol)) => {
            assert_eq!(symbol.name, name);
            assert!(symbol.is_keyword());
        }
        other => panic!("expected the {name} marker, got {other:?}"),
    }
}

#[test]
fn a_global_macro_is_expanded_through_the_caller() {
    let mut f = Fixture::new();
    let macro_name = f.intern("COMMON-LISP-USER", "MY-MACRO");
    set_symbol_macro(&mut f.ctx, macro_name, true).unwrap();
    let argument = Word::fixnum(1);
    let form = f.list(&[macro_name, argument]);
    let mut caller = Marker;
    let expanded = f.expand_with(form, &mut caller).unwrap();
    assert_marker(expanded, "GLOBAL");
}

#[test]
fn a_macro_without_a_caller_reports_a_failure() {
    let mut f = Fixture::new();
    let macro_name = f.intern("COMMON-LISP-USER", "MY-MACRO");
    set_symbol_macro(&mut f.ctx, macro_name, true).unwrap();
    let form = f.list(&[macro_name]);
    assert!(matches!(
        f.expand(form),
        Err(FrontError::MacroExpansion { .. })
    ));
}

#[test]
fn a_local_macro_shadows_a_global_macro() {
    let mut f = Fixture::new();
    let macro_name = f.intern("COMMON-LISP-USER", "M");
    set_symbol_macro(&mut f.ctx, macro_name, true).unwrap();
    let parameter = f.user("X");
    let lambda_list = f.list(&[parameter]);
    let definition = f.list(&[macro_name, lambda_list, parameter]);
    let definitions = f.list(&[definition]);
    let call = f.list(&[macro_name, Word::fixnum(1)]);
    let form = f.form("MACROLET", &[definitions, call]);
    let mut caller = Marker;
    let expanded = f.expand_with(form, &mut caller).unwrap();
    match expanded {
        Expr::Macrolet { body, .. } => assert_marker(body.into_iter().next().unwrap(), "LOCAL"),
        other => panic!("expected Macrolet, got {other:?}"),
    }
}

#[test]
fn a_symbol_macro_replaces_a_reference() {
    let mut f = Fixture::new();
    let name = f.user("X");
    let expansion = Word::fixnum(1);
    let definition = f.list(&[name, expansion]);
    let definitions = f.list(&[definition]);
    let body = f.user("X");
    let form = f.form("SYMBOL-MACROLET", &[definitions, body]);
    match f.expand(form).unwrap() {
        Expr::SymbolMacrolet { body, .. } => {
            assert_eq!(body, vec![Expr::Constant(Literal::fixnum(1))]);
        }
        other => panic!("expected SymbolMacrolet, got {other:?}"),
    }
}

#[test]
fn a_symbol_macro_shadows_a_let_binding() {
    let mut f = Fixture::new();
    let name = f.user("X");
    let binding = f.list(&[name, Word::fixnum(2)]);
    let bindings = f.list(&[binding]);
    let expansion = Word::fixnum(1);
    let definition = f.list(&[name, expansion]);
    let definitions = f.list(&[definition]);
    let body = f.user("X");
    let inner = f.form("SYMBOL-MACROLET", &[definitions, body]);
    let form = f.form("LET", &[bindings, inner]);
    match f.expand(form).unwrap() {
        Expr::Let { body, .. } => match &body[0] {
            Expr::SymbolMacrolet { body, .. } => {
                assert_eq!(body, &vec![Expr::Constant(Literal::fixnum(1))]);
            }
            other => panic!("expected SymbolMacrolet, got {other:?}"),
        },
        other => panic!("expected Let, got {other:?}"),
    }
}

#[test]
fn a_chain_of_symbol_macros_resolves() {
    let mut f = Fixture::new();
    let x = f.user("X");
    let y = f.user("Y");
    let first = f.list(&[x, y]);
    let second = f.list(&[y, Word::fixnum(1)]);
    let definitions = f.list(&[first, second]);
    let body = f.user("X");
    let form = f.form("SYMBOL-MACROLET", &[definitions, body]);
    match f.expand(form).unwrap() {
        Expr::SymbolMacrolet { body, .. } => {
            assert_eq!(body, vec![Expr::Constant(Literal::fixnum(1))]);
        }
        other => panic!("expected SymbolMacrolet, got {other:?}"),
    }
}

#[test]
fn setq_of_a_symbol_macro_is_rejected() {
    let mut f = Fixture::new();
    let name = f.user("X");
    let definition = f.list(&[name, Word::fixnum(1)]);
    let definitions = f.list(&[definition]);
    let assignment = f.form("SETQ", &[name, Word::fixnum(2)]);
    let form = f.form("SYMBOL-MACROLET", &[definitions, assignment]);
    assert!(matches!(
        f.expand(form),
        Err(FrontError::MalformedForm { .. })
    ));
}
