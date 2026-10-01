//! Fourteenth-wave coverage for remaining analysis, binding, capture, and expression paths.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use ncl_compiler_front::{
    Expr, FunctionDesignator, LambdaExpr, LambdaList, LetBinding, Literal, LocalFunction, Operator,
    ParamName, SymbolRef, lower_toplevel,
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

fn lambda(body: Expr, list: LambdaList) -> LambdaExpr {
    LambdaExpr {
        lambda_list: list,
        declarations: Vec::new(),
        docstring: None,
        body: vec![body],
    }
}

#[test]
fn analysis_walks_lambda_call_function_flet_and_labels_forms() {
    let exit = symbol("EXIT");
    let returned = || Expr::ReturnFrom {
        name: exit.clone(),
        value: Some(Box::new(Expr::Constant(Literal::fixnum(12)))),
    };
    let local = LocalFunction {
        name: symbol("LOCAL"),
        lambda: lambda(Expr::Progn(vec![returned()]), LambdaList::new()),
    };
    let recursive = LocalFunction {
        name: symbol("RECURSIVE"),
        lambda: lambda(
            Expr::Call {
                operator: Operator::Name(symbol("RECURSIVE")),
                arguments: Vec::new(),
            },
            LambdaList::new(),
        ),
    };
    let body = vec![
        Expr::Call {
            operator: Operator::Lambda(Box::new(lambda(returned(), LambdaList::new()))),
            arguments: Vec::new(),
        },
        Expr::Function(FunctionDesignator::Lambda(Box::new(lambda(
            Expr::Progn(vec![returned()]),
            LambdaList::new(),
        )))),
        Expr::Flet {
            definitions: vec![local],
            declarations: Vec::new(),
            body: vec![Expr::Call {
                operator: Operator::Name(symbol("LOCAL")),
                arguments: Vec::new(),
            }],
        },
        Expr::Labels {
            definitions: vec![recursive],
            declarations: Vec::new(),
            body: vec![Expr::Call {
                operator: Operator::Name(symbol("RECURSIVE")),
                arguments: Vec::new(),
            }],
        },
    ];
    let lowered =
        lower_toplevel(&Expr::Block { name: exit, body }).expect("analysis matrix lowers");
    assert_verifies(&lowered.entry);
    assert!(
        lowered
            .entry
            .handler_regions
            .iter()
            .any(|region| region.kind == HandlerKind::Catch)
    );
    assert!(lowered.nested.len() >= 4);
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::MakeClosure { .. }
    )));
}

#[test]
fn assigned_captured_value_uses_a_cell_for_multiple_nested_closures() {
    let x = symbol("X");
    let closure = |value| {
        Expr::Lambda(Box::new(lambda(
            Expr::Progn(vec![
                Expr::Setq(vec![(x.clone(), Expr::Constant(Literal::fixnum(value)))]),
                Expr::Variable(x.clone()),
            ]),
            LambdaList::new(),
        )))
    };
    let expression = Expr::Let {
        sequential: false,
        bindings: vec![LetBinding {
            name: x.clone(),
            value: Some(Expr::Constant(Literal::Nil)),
        }],
        declarations: Vec::new(),
        body: vec![closure(1), closure(2), Expr::Variable(x)],
    };
    let lowered = lower_toplevel(&expression).expect("assigned capture lowers");
    assert_verifies(&lowered.entry);
    assert_eq!(lowered.nested.len(), 2);
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::MakeValueCell { .. }
    )));
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
fn builtin_arity_mismatch_falls_back_to_a_real_function_cell_call() {
    let expression = Expr::Call {
        operator: Operator::Name(SymbolRef::interned("COMMON-LISP", "CAR")),
        arguments: vec![
            Expr::Constant(Literal::fixnum(1)),
            Expr::Constant(Literal::fixnum(2)),
        ],
    };
    let lowered = lower_toplevel(&expression).expect("arity mismatch call lowers");
    assert_verifies(&lowered.entry);
    assert!(!any_op(
        &lowered.entry,
        |kind| matches!(kind, OpKind::Builtin { name, .. } if name == "CAR")
    ));
    assert!(any_op(
        &lowered.entry,
        |kind| matches!(kind, OpKind::LoadField { field, .. } if *field == ncl_object::symbol_offset::FUNCTION as u32)
    ));
    assert!(any_op(
        &lowered.entry,
        |kind| matches!(kind, OpKind::CallClosure { args, .. } if args.len() == 3)
    ));
}

#[test]
fn multiple_value_call_captures_values_and_calls_the_list_adapter() {
    let expression = Expr::MultipleValueCall {
        function: Box::new(Expr::Function(FunctionDesignator::Name(symbol("LIST")))),
        arguments: vec![
            Expr::Constant(Literal::fixnum(1)),
            Expr::Constant(Literal::fixnum(2)),
        ],
    };
    let lowered = lower_toplevel(&expression).expect("multiple value call lowers");
    assert_verifies(&lowered.entry);
    let value_captures = lowered
        .entry
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .filter(|op| matches!(op.kind, OpKind::CallClosure { ref args, .. } if args.len() == 1));
    assert_eq!(value_captures.count(), 2);
    assert!(any_op(
        &lowered.entry,
        |kind| matches!(kind, OpKind::CallClosure { args, .. } if args.len() == 4)
    ));
}

#[test]
fn key_parameter_without_default_and_supplied_p_has_all_keyword_ops() {
    let list = LambdaList {
        keys: vec![ncl_compiler_front::KeyParam {
            keyword: SymbolRef::interned("KEYWORD", "VALUE"),
            name: ParamName::Symbol(symbol("VALUE")),
            default: None,
            supplied_p: Some(ParamName::Symbol(symbol("VALUE-P"))),
        }],
        ..LambdaList::new()
    };
    let expression = Expr::Lambda(Box::new(lambda(Expr::Constant(Literal::T), list)));
    let lowered = lower_toplevel(&expression).expect("key parameter lowers");
    assert_verifies(&lowered.entry);
    let nested = &lowered.nested[0];
    assert_verifies(nested);
    assert!(any_op(
        nested,
        |kind| matches!(kind, OpKind::Builtin { name, .. } if name == "check-keywords")
    ));
    assert!(any_op(
        nested,
        |kind| matches!(kind, OpKind::Builtin { name, .. } if name == "keyword-value")
    ));
    assert!(any_op(
        nested,
        |kind| matches!(kind, OpKind::Builtin { name, .. } if name == "keyword-supplied-p")
    ));
    assert!(any_terminator(nested, |term| matches!(
        term,
        Terminator::Branch { .. }
    )));
}

#[test]
fn block_live_value_and_tagbody_back_edge_preserve_verified_control_flow() {
    let x = symbol("X");
    let block = Expr::Let {
        sequential: false,
        bindings: vec![LetBinding {
            name: x.clone(),
            value: Some(Expr::Constant(Literal::fixnum(0))),
        }],
        declarations: Vec::new(),
        body: vec![Expr::Block {
            name: symbol("DONE"),
            body: vec![
                Expr::Setq(vec![(x.clone(), Expr::Constant(Literal::fixnum(1)))]),
                Expr::Variable(x),
            ],
        }],
    };
    let lowered = lower_toplevel(&block).expect("live block lowers");
    assert_verifies(&lowered.entry);
    assert!(any_terminator(
        &lowered.entry,
        |term| matches!(term, Terminator::Jump { args, .. } if args.len() == 2)
    ));

    let tag = symbol("LOOP");
    let tagbody = Expr::Tagbody(vec![
        ncl_compiler_front::TagbodyItem::Tag(tag.clone()),
        ncl_compiler_front::TagbodyItem::Form(Expr::Constant(Literal::Nil)),
        ncl_compiler_front::TagbodyItem::Form(Expr::Go { tag }),
    ]);
    let tagbody = lower_toplevel(&tagbody).expect("tagbody back edge lowers");
    assert_verifies(&tagbody.entry);
    assert!(tagbody.entry.handler_regions.is_empty());
    assert!(any_op(&tagbody.entry, |kind| matches!(
        kind,
        OpKind::Safepoint
    )));
}

#[test]
fn empty_body_and_unbound_control_keep_return_and_error_contracts() {
    let empty = lower_toplevel(&Expr::Locally {
        declarations: Vec::new(),
        body: Vec::new(),
    })
    .expect("empty locally lowers");
    assert_verifies(&empty.entry);
    assert!(
        empty
            .entry
            .constants
            .iter()
            .any(|constant| matches!(constant, ncl_ir::Constant::Nil))
    );

    let error = lower_toplevel(&Expr::Go {
        tag: symbol("MISSING-TAG"),
    })
    .unwrap_err();
    assert!(matches!(
        error,
        ncl_compiler_front::LowerError::EscapingControl { .. }
    ));
    assert!(error.to_string().contains("MISSING-TAG"));
}
