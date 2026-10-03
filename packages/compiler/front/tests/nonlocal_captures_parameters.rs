//! Fourth-wave coverage for non-local analysis, captures, parameters, and control.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use ncl_compiler_front::{
    Expr, FunctionDesignator, LambdaExpr, LambdaList, LetBinding, Literal, Operator, ParamName,
    SymbolRef, TagbodyItem, lower_toplevel,
};
use ncl_ir::{Function, HandlerKind, OpKind, Terminator, verify};

fn symbol(name: &str) -> SymbolRef {
    SymbolRef::interned("COMMON-LISP-USER", name)
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

fn empty_lambda(body: Expr) -> Expr {
    Expr::Lambda(Box::new(LambdaExpr {
        lambda_list: LambdaList::new(),
        declarations: Vec::new(),
        docstring: None,
        body: vec![body],
    }))
}

fn escaping_block(body: Expr) -> Function {
    lower_toplevel(&Expr::Block {
        name: symbol("EXIT"),
        body: vec![body],
    })
    .expect("escaping block lowers")
    .entry
}

#[test]
fn nested_return_analysis_selects_escaping_blocks_across_wrappers() {
    let target = symbol("EXIT");
    let nested_return = || Expr::ReturnFrom {
        name: target.clone(),
        value: Some(Box::new(Expr::Constant(Literal::fixnum(7)))),
    };

    let cases = [
        Expr::Progn(vec![empty_lambda(nested_return())]),
        Expr::Let {
            sequential: false,
            bindings: vec![LetBinding {
                name: symbol("VALUE"),
                value: Some(Expr::Constant(Literal::fixnum(1))),
            }],
            declarations: Vec::new(),
            body: vec![empty_lambda(nested_return())],
        },
        Expr::If {
            test: Box::new(Expr::Constant(Literal::T)),
            then: Box::new(empty_lambda(nested_return())),
            otherwise: Some(Box::new(Expr::Constant(Literal::Nil))),
        },
        Expr::Call {
            operator: Operator::Lambda(Box::new(match empty_lambda(nested_return()) {
                Expr::Lambda(lambda) => *lambda,
                _ => unreachable!(),
            })),
            arguments: Vec::new(),
        },
        Expr::Function(FunctionDesignator::Lambda(Box::new(
            match empty_lambda(nested_return()) {
                Expr::Lambda(lambda) => *lambda,
                _ => unreachable!(),
            },
        ))),
    ];

    for body in cases {
        let lowered = escaping_block(body);
        assert_verifies(&lowered);
        assert!(
            lowered
                .handler_regions
                .iter()
                .any(|region| region.kind == HandlerKind::Catch)
        );
        assert!(any_op(&lowered, |kind| matches!(
            kind,
            OpKind::MakeClosure { .. }
        )));
    }
}

#[test]
fn return_inside_unwind_protect_uses_cleanup_safe_escape_path() {
    let name = symbol("EXIT");
    let expression = Expr::Block {
        name: name.clone(),
        body: vec![Expr::UnwindProtect {
            protected: Box::new(Expr::ReturnFrom {
                name,
                value: Some(Box::new(Expr::Constant(Literal::fixnum(9)))),
            }),
            cleanup: vec![Expr::Constant(Literal::fixnum(1))],
        }],
    };
    let lowered = lower_toplevel(&expression).expect("unwind-protected block lowers");
    assert_verifies(&lowered.entry);
    assert!(
        lowered
            .entry
            .handler_regions
            .iter()
            .any(|region| region.kind == HandlerKind::Catch)
    );
    assert!(
        lowered
            .entry
            .handler_regions
            .iter()
            .any(|region| region.kind == HandlerKind::UnwindProtect)
    );
    assert!(any_op(
        &lowered.entry,
        |kind| matches!(kind, OpKind::Builtin { name, .. } if name == "throw")
    ));
}

#[test]
fn direct_go_analysis_keeps_local_tagbody_on_jump_path() {
    let tag = symbol("RETRY");
    let expression = Expr::Tagbody(vec![
        TagbodyItem::Form(Expr::Go { tag: tag.clone() }),
        TagbodyItem::Tag(tag),
        TagbodyItem::Form(Expr::Constant(Literal::fixnum(1))),
    ]);
    let lowered = lower_toplevel(&expression).expect("local tagbody lowers");
    assert_verifies(&lowered.entry);
    assert!(lowered.entry.handler_regions.is_empty());
    assert!(any_terminator(&lowered.entry, |term| matches!(
        term,
        Terminator::Jump { .. }
    )));
}

#[test]
fn capture_analysis_boxes_assigned_lexical_values_and_loads_captures() {
    let x = symbol("X");
    let closure_body = Expr::Progn(vec![
        Expr::Setq(vec![(x.clone(), Expr::Constant(Literal::fixnum(2)))]),
        Expr::Variable(x.clone()),
    ]);
    let first = empty_lambda(closure_body.clone());
    let second = Expr::Function(FunctionDesignator::Lambda(Box::new(
        match empty_lambda(closure_body) {
            Expr::Lambda(lambda) => *lambda,
            _ => unreachable!(),
        },
    )));
    let expression = Expr::Let {
        sequential: false,
        bindings: vec![LetBinding {
            name: x.clone(),
            value: Some(Expr::Constant(Literal::Nil)),
        }],
        declarations: Vec::new(),
        body: vec![first, second, Expr::Variable(x)],
    };
    let lowered = lower_toplevel(&expression).expect("capturing let lowers");
    assert_verifies(&lowered.entry);
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::MakeValueCell { .. }
    )));
    assert_eq!(lowered.nested.len(), 2);
    for nested in &lowered.nested {
        assert_verifies(nested);
        assert!(any_op(nested, |kind| matches!(
            kind,
            OpKind::LoadCapture { .. }
        )));
        assert!(any_op(nested, |kind| matches!(
            kind,
            OpKind::StoreField { field: 0, .. }
        )));
    }
}

#[test]
fn parameter_pattern_lowering_emits_argument_decomposition() {
    let x = symbol("X");
    let y = symbol("Y");
    let pattern = LambdaList {
        required: vec![ParamName::Pattern(Box::new(LambdaList {
            required: vec![ParamName::Symbol(x), ParamName::Symbol(y)],
            ..LambdaList::new()
        }))],
        ..LambdaList::new()
    };
    let expression = Expr::Lambda(Box::new(LambdaExpr {
        lambda_list: pattern,
        declarations: Vec::new(),
        docstring: None,
        body: vec![Expr::Constant(Literal::Nil)],
    }));
    let lowered = lower_toplevel(&expression).expect("destructuring parameter lowers");
    assert_verifies(&lowered.entry);
    let nested = lowered
        .nested
        .first()
        .expect("destructuring lambda is lowered as a nested function");
    assert_verifies(nested);
    assert!(any_op(nested, |kind| matches!(
        kind,
        OpKind::LoadArg { index: 1 }
    )));
    assert!(any_op(nested, |kind| matches!(
        kind,
        OpKind::Builtin { name, args } if name == "CAR" && args.len() == 1
    )));
    assert!(any_op(nested, |kind| matches!(
        kind,
        OpKind::Builtin { name, args } if name == "CDR" && args.len() == 1
    )));
}

#[test]
fn direct_function_and_variable_calls_keep_real_closure_side_effects() {
    let x = symbol("X");
    let lambda = LambdaExpr {
        lambda_list: LambdaList {
            required: vec![ParamName::Symbol(x.clone())],
            ..LambdaList::new()
        },
        declarations: Vec::new(),
        docstring: None,
        body: vec![Expr::Variable(x)],
    };
    let expression = Expr::Call {
        operator: Operator::Lambda(Box::new(lambda)),
        arguments: vec![Expr::Constant(Literal::fixnum(11))],
    };
    let lowered = lower_toplevel(&expression).expect("lambda call lowers");
    assert_verifies(&lowered.entry);
    assert_eq!(lowered.nested.len(), 1);
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::MakeClosure { .. }
    )));
    assert!(any_op(
        &lowered.entry,
        |kind| matches!(kind, OpKind::CallClosure { args, .. } if args.len() == 2)
    ));
    assert!(any_op(&lowered.nested[0], |kind| matches!(
        kind,
        OpKind::LoadArg { index: 1 }
    )));
}
