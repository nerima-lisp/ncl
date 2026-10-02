//! Edge coverage for escaping tagbody and its closure-safe control transfer.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use ncl_compiler_front::{
    Expr, FunctionDesignator, LambdaExpr, LambdaList, LetBinding, Literal, SymbolRef, TagbodyItem,
    lower_toplevel,
};
use ncl_ir::{Function, HandlerKind, OpKind, Terminator, VerifyError, verify};

fn symbol(name: &str) -> SymbolRef {
    SymbolRef::interned("COMMON-LISP-USER", name)
}
fn any_op(function: &Function, predicate: impl Fn(&OpKind) -> bool) -> bool {
    function
        .blocks
        .iter()
        .any(|block| block.ops.iter().any(|op| predicate(&op.kind)))
}
fn any_terminator(function: &Function, predicate: impl Fn(&Terminator) -> bool) -> bool {
    function
        .blocks
        .iter()
        .any(|block| predicate(&block.terminator))
}
fn assert_current_escaping_tagbody_verifier_errors(function: &Function) {
    let errors = verify(function).expect_err("current escaping tagbody IR is not verifier-clean");
    assert_eq!(errors.len(), 2);
    assert!(
        errors
            .iter()
            .any(|error| matches!(error, VerifyError::UndefinedValue(_)))
    );
    assert!(
        errors
            .iter()
            .any(|error| matches!(error, VerifyError::TypeMismatch(_)))
    );
}

#[test]
fn escaping_tagbody_from_a_lambda_uses_a_catch_token_and_throw() {
    let tag = symbol("ESCAPE");
    let expression = Expr::Tagbody(vec![
        TagbodyItem::Tag(tag.clone()),
        TagbodyItem::Form(Expr::Function(FunctionDesignator::Lambda(Box::new(
            LambdaExpr {
                lambda_list: LambdaList::new(),
                declarations: Vec::new(),
                docstring: None,
                body: vec![Expr::Go { tag }],
            },
        )))),
    ]);
    let lowered = lower_toplevel(&expression).expect("escaping tagbody lowers");
    assert_current_escaping_tagbody_verifier_errors(&lowered.entry);
    assert_eq!(lowered.nested.len(), 1);
    assert_eq!(lowered.entry.handler_regions.len(), 1);
    assert_eq!(lowered.entry.handler_regions[0].kind, HandlerKind::Catch);
    assert!(lowered.entry.handler_regions[0].catch_tag.is_some());
    assert!(!lowered.entry.handler_regions[0].protected.is_empty());
    let nested = &lowered.nested[0];
    assert!(verify(nested).is_ok(), "nested lambda IR must verify");
    assert!(any_op(
        nested,
        |kind| matches!(kind, OpKind::Builtin { name, args } if name == "throw" && args.len() == 2)
    ));
    assert!(any_terminator(nested, |term| matches!(
        term,
        Terminator::Throw { .. }
    )));
}

#[test]
fn escaping_tagbody_carries_an_assigned_captured_cell_to_its_handler() {
    let tag = symbol("ESCAPE");
    let cell = symbol("CELL");
    let expression = Expr::Let {
        sequential: false,
        bindings: vec![LetBinding {
            name: cell.clone(),
            value: Some(Expr::Constant(Literal::fixnum(1))),
        }],
        declarations: Vec::new(),
        body: vec![Expr::Tagbody(vec![
            TagbodyItem::Tag(tag.clone()),
            TagbodyItem::Form(Expr::Function(FunctionDesignator::Lambda(Box::new(
                LambdaExpr {
                    lambda_list: LambdaList::new(),
                    declarations: Vec::new(),
                    docstring: None,
                    body: vec![
                        Expr::Setq(vec![(cell, Expr::Constant(Literal::fixnum(2)))]),
                        Expr::Go { tag },
                    ],
                },
            )))),
        ])],
    };
    let lowered = lower_toplevel(&expression).expect("cell-capturing tagbody lowers");
    assert_current_escaping_tagbody_verifier_errors(&lowered.entry);
    assert_eq!(lowered.entry.handler_regions.len(), 1);
    assert_eq!(lowered.entry.handler_regions[0].kind, HandlerKind::Catch);
    assert_eq!(lowered.entry.handler_regions[0].binding_targets.len(), 1);
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::MakeValueCell { .. }
    )));
    assert_eq!(lowered.nested.len(), 1);
    let nested = &lowered.nested[0];
    assert!(verify(nested).is_ok(), "nested lambda IR must verify");
    assert!(any_op(nested, |kind| matches!(
        kind,
        OpKind::StoreField { field: 0, .. }
    )));
    assert!(any_op(
        nested,
        |kind| matches!(kind, OpKind::Builtin { name, args } if name == "throw" && args.len() == 2)
    ));
    assert!(any_terminator(nested, |term| matches!(
        term,
        Terminator::Throw { .. }
    )));
}

#[test]
fn throwing_out_of_unwind_protect_builds_a_cleanup_handler_before_catch() {
    let tag = symbol("CATCH-TAG");
    let tag_literal = Expr::Constant(Literal::Symbol(tag));
    let expression = Expr::Catch {
        tag: Box::new(tag_literal.clone()),
        body: vec![Expr::UnwindProtect {
            protected: Box::new(Expr::Throw {
                tag: Box::new(tag_literal),
                value: Box::new(Expr::Constant(Literal::fixnum(11))),
            }),
            cleanup: vec![Expr::Constant(Literal::fixnum(12))],
        }],
    };
    let lowered = lower_toplevel(&expression).expect("throwing unwind-protect lowers");
    if let Err(errors) = verify(&lowered.entry) {
        panic!(
            "{} does not verify: {errors:?}\n{}",
            lowered.entry.name, lowered.entry
        );
    }
    assert_eq!(lowered.entry.handler_regions.len(), 2);
    assert!(
        lowered
            .entry
            .handler_regions
            .iter()
            .any(|region| region.kind == HandlerKind::Catch)
    );
    let unwind = lowered
        .entry
        .handler_regions
        .iter()
        .find(|region| region.kind == HandlerKind::UnwindProtect)
        .expect("unwind-protect handler");
    assert_eq!(unwind.cleanup, Some(unwind.handler));
    assert!(any_terminator(&lowered.entry, |term| matches!(
        term,
        Terminator::Throw { .. }
    )));
}
