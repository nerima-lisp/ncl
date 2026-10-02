//! Edge coverage for escaping tagbody and its closure-safe control transfer.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use ncl_compiler_front::{
    Expr, FunctionDesignator, Function, LambdaExpr, LambdaList, LetBinding, Literal,
    SymbolRef, TagbodyItem, lower_toplevel,
};
use ncl_ir::{HandlerKind, OpKind, Terminator, verify};

fn symbol(name: &str) -> SymbolRef {
    SymbolRef::interned("COMMON-LISP-USER", name)
}

fn empty_lambda(body: Expr) -> Expr {
    Expr::Lambda(Box::new(LambdaExpr {
        lambda_list: LambdaList::new(),
        declarations: Vec::new(),
        docstring: None,
        body: vec![body],
    }))
}

fn assert_verifies(function: &Function) {
    if let Err(errors) = verify(function) {
        panic!("{} does not verify: {errors:?}\n{function}", function.name);
    }
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
    assert_verifies(&lowered.entry);
    assert_eq!(lowered.nested.len(), 1);
    assert_eq!(lowered.entry.handler_regions.len(), 1);
    assert_eq!(lowered.entry.handler_regions[0].kind, HandlerKind::Catch);
    assert!(lowered.entry.handler_regions[0].catch_tag.is_some());
    assert!(!lowered.entry.handler_regions[0].protected.is_empty());

    let nested = &lowered.nested[0];
    assert_verifies(nested);
    assert!(any_op(nested, |kind| matches!(
        kind,
        OpKind::Builtin { name, args } if name == "throw" && args.len() == 2
    )));
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
    assert_verifies(&lowered.entry);
    assert_eq!(lowered.entry.handler_regions.len(), 1);
    assert_eq!(lowered.entry.handler_regions[0].kind, HandlerKind::Catch);
    assert_eq!(lowered.entry.handler_regions[0].binding_targets.len(), 1);
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::MakeValueCell { .. }
    )));

    assert_eq!(lowered.nested.len(), 1);
    let nested = &lowered.nested[0];
    assert_verifies(nested);
    assert!(any_op(nested, |kind| matches!(
        kind,
        OpKind::StoreField { field: 0, .. }
    )));
    assert!(any_op(nested, |kind| matches!(
        kind,
        OpKind::Builtin { name, args } if name == "throw" && args.len() == 2
    )));
    assert!(any_terminator(nested, |term| matches!(
        term,
        Terminator::Throw { .. }
    )));
}
