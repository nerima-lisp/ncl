//! Additional edge coverage for lower analysis and capture environments.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use ncl_compiler_front::{
    lower_toplevel, Expr, FunctionDesignator, LambdaExpr, LambdaList, LetBinding, Literal,
    SymbolRef,
};
use ncl_ir::{verify, Function, HandlerKind, OpKind, Terminator};

fn symbol(name: &str) -> SymbolRef {
    SymbolRef::interned("COMMON-LISP-USER", name)
}

fn lambda(body: Expr) -> LambdaExpr {
    LambdaExpr {
        lambda_list: LambdaList::new(),
        declarations: Vec::new(),
        docstring: None,
        body: vec![body],
    }
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
fn return_from_inside_unwind_protect_uses_escaping_block_analysis() {
    let exit = symbol("EXIT");
    let expression = Expr::Block {
        name: exit.clone(),
        body: vec![Expr::UnwindProtect {
            protected: Box::new(Expr::If {
                test: Box::new(Expr::Constant(Literal::T)),
                then: Box::new(Expr::ReturnFrom {
                    name: exit,
                    value: Some(Box::new(Expr::Constant(Literal::fixnum(7)))),
                }),
                otherwise: Some(Box::new(Expr::Constant(Literal::Nil))),
            }),
            cleanup: vec![Expr::Constant(Literal::T)],
        }],
    };

    let lowered = lower_toplevel(&expression).expect("unwind-protected return lowers");
    assert_verifies(&lowered.entry);
    assert!(lowered
        .entry
        .handler_regions
        .iter()
        .any(|region| region.kind == HandlerKind::Catch));
    assert!(lowered
        .entry
        .handler_regions
        .iter()
        .any(|region| region.kind == HandlerKind::UnwindProtect));
    assert!(any_terminator(&lowered.entry, |term| matches!(
        term,
        Terminator::Throw { .. }
    )));
}

#[test]
fn throw_and_multiple_value_prog1_preserve_three_free_variable_captures() {
    let tag = symbol("TAG");
    let first = symbol("FIRST");
    let second = symbol("SECOND");
    let expression = Expr::Let {
        sequential: false,
        bindings: vec![
            LetBinding {
                name: tag.clone(),
                value: Some(Expr::Constant(Literal::Nil)),
            },
            LetBinding {
                name: first.clone(),
                value: Some(Expr::Constant(Literal::fixnum(1))),
            },
            LetBinding {
                name: second.clone(),
                value: Some(Expr::Constant(Literal::fixnum(2))),
            },
        ],
        declarations: Vec::new(),
        body: vec![Expr::Lambda(Box::new(lambda(Expr::Throw {
            tag: Box::new(Expr::Variable(tag)),
            value: Box::new(Expr::MultipleValueProg1 {
                first: Box::new(Expr::Variable(first)),
                forms: vec![Expr::Variable(second)],
            }),
        })))],
    };

    let lowered = lower_toplevel(&expression).expect("wrapped captures lower");
    assert_verifies(&lowered.entry);
    assert_eq!(lowered.nested.len(), 1);
    let captures = lowered
        .entry
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .find_map(|op| match &op.kind {
            OpKind::MakeClosure { captures, .. } => Some(captures.len()),
            _ => None,
        })
        .expect("closure operation is emitted");
    assert_eq!(captures, 3);
    assert!(any_op(&lowered.nested[0], |kind| matches!(
        kind,
        OpKind::LoadCapture { index: 0 | 1 | 2 }
    )));
}

#[test]
fn function_lambda_designator_boxes_an_assigned_captured_value() {
    let value = symbol("VALUE");
    let expression = Expr::Let {
        sequential: false,
        bindings: vec![LetBinding {
            name: value.clone(),
            value: Some(Expr::Constant(Literal::Nil)),
        }],
        declarations: Vec::new(),
        body: vec![Expr::Function(FunctionDesignator::Lambda(Box::new(
            lambda(Expr::Progn(vec![
                Expr::Setq(vec![(value.clone(), Expr::Constant(Literal::fixnum(9)))]),
                Expr::Variable(value),
            ])),
        )))],
    };

    let lowered = lower_toplevel(&expression).expect("function lambda capture lowers");
    assert_verifies(&lowered.entry);
    assert_eq!(lowered.nested.len(), 1);
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::MakeValueCell { .. }
    )));
    assert!(any_op(&lowered.nested[0], |kind| matches!(
        kind,
        OpKind::LoadCapture { index: 0 }
    )));
    assert!(any_op(&lowered.nested[0], |kind| matches!(
        kind,
        OpKind::StoreField { field: 0, .. }
    )));
}
