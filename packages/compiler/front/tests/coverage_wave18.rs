//! Eighteenth-wave coverage for reachable lower branches.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use ncl_compiler_front::{
    AuxParam, Expr, FunctionDesignator, KeyParam, LambdaExpr, LambdaList, LetBinding, Literal,
    LocalFunction, Operator, OptionalParam, ParamName, SymbolRef, lower_toplevel,
};
use ncl_ir::{Constant, Function, OpKind, Terminator, verify};

fn symbol(name: &str) -> SymbolRef {
    SymbolRef::interned("COMMON-LISP-USER", name)
}

fn lambda(body: Expr, list: LambdaList) -> LambdaExpr {
    LambdaExpr {
        lambda_list: list,
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

fn count_ops(function: &Function, predicate: impl Fn(&OpKind) -> bool) -> usize {
    function
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .filter(|op| predicate(&op.kind))
        .count()
}

#[test]
fn analysis_paths_distinguish_local_exit_from_nested_escape() {
    let exit = symbol("EXIT");
    let local = LocalFunction {
        name: symbol("LOCAL"),
        lambda: lambda(Expr::Constant(Literal::Nil), LambdaList::new()),
    };
    let nested = Expr::Lambda(Box::new(lambda(
        Expr::ReturnFrom {
            name: exit.clone(),
            value: Some(Box::new(Expr::Constant(Literal::fixnum(9)))),
        },
        LambdaList::new(),
    )));
    let escaped = lower_toplevel(&Expr::Block {
        name: exit,
        body: vec![Expr::Flet {
            definitions: vec![local],
            declarations: Vec::new(),
            body: vec![nested],
        }],
    })
    .expect("nested block exit lowers");
    assert_verifies(&escaped.entry);
    assert!(
        escaped
            .entry
            .handler_regions
            .iter()
            .any(|region| { region.kind == ncl_ir::HandlerKind::Catch })
    );
    assert!(any_op(&escaped.entry, |kind| matches!(
        kind,
        OpKind::MakeClosure { .. }
    )));

    let normal = lower_toplevel(&Expr::Block {
        name: symbol("LOCAL-BLOCK"),
        body: vec![Expr::If {
            test: Box::new(Expr::Constant(Literal::T)),
            then: Box::new(Expr::Constant(Literal::fixnum(1))),
            otherwise: Some(Box::new(Expr::Constant(Literal::fixnum(2)))),
        }],
    })
    .expect("local block fast path lowers");
    assert_verifies(&normal.entry);
    assert!(normal.entry.handler_regions.is_empty());
    assert!(
        normal
            .entry
            .constants
            .iter()
            .any(|constant| matches!(constant, Constant::Fixnum(1 | 2)))
    );
}

#[test]
fn capture_and_binding_paths_box_assigned_values_and_keep_free_functions() {
    let value = symbol("VALUE");
    let function = symbol("HELPER");
    let helper = LocalFunction {
        name: function.clone(),
        lambda: lambda(
            Expr::Call {
                operator: Operator::Name(symbol("EXTERNAL")),
                arguments: vec![Expr::Variable(value.clone())],
            },
            LambdaList::new(),
        ),
    };
    let expression = Expr::Let {
        sequential: false,
        bindings: vec![LetBinding {
            name: value.clone(),
            value: Some(Expr::Constant(Literal::fixnum(1))),
        }],
        declarations: Vec::new(),
        body: vec![
            Expr::Setq(vec![(value, Expr::Constant(Literal::fixnum(2)))]),
            Expr::Flet {
                definitions: vec![helper],
                declarations: Vec::new(),
                body: vec![Expr::Call {
                    operator: Operator::Name(function),
                    arguments: Vec::new(),
                }],
            },
        ],
    };
    let lowered = lower_toplevel(&expression).expect("assigned free value lowers");
    assert_verifies(&lowered.entry);
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::MakeValueCell { .. }
    )));
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::StoreField { field: 0, .. }
    )));
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::CallClosure { .. }
    )));
    assert!(any_op(&lowered.nested[0], |kind| matches!(
        kind,
        OpKind::LoadCapture { .. }
    )));
    assert!(any_op(&lowered.nested[0], |kind| matches!(
        kind,
        OpKind::CallClosure { .. }
    )));
}

#[test]
fn expression_paths_cover_symbol_cells_designators_and_call_arity_fallback() {
    let forms = vec![
        Expr::Call {
            operator: Operator::Name(symbol("symbol-value")),
            arguments: vec![Expr::Constant(Literal::Symbol(symbol("CELL")))],
        },
        Expr::Call {
            operator: Operator::Name(symbol("set-symbol-value")),
            arguments: vec![
                Expr::Constant(Literal::Symbol(symbol("CELL"))),
                Expr::Constant(Literal::fixnum(4)),
            ],
        },
        Expr::Call {
            operator: Operator::Name(symbol("CAR")),
            arguments: vec![Expr::Constant(Literal::Nil), Expr::Constant(Literal::Nil)],
        },
        Expr::Function(FunctionDesignator::Name(symbol("EXTERNAL"))),
    ];
    let lowered = lower_toplevel(&Expr::Progn(forms)).expect("cell and fallback calls lower");
    assert_verifies(&lowered.entry);
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::LoadField { field, .. } if *field == u32::try_from(ncl_object::symbol_offset::VALUE).unwrap()
    )));
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::StoreField { field, .. } if *field == u32::try_from(ncl_object::symbol_offset::VALUE).unwrap()
    )));
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::LoadField { field, .. } if *field == u32::try_from(ncl_object::symbol_offset::FUNCTION).unwrap()
    )));
    assert!(any_op(
        &lowered.entry,
        |kind| matches!(kind, OpKind::CallClosure { args, .. } if args.len() == 3)
    ));
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::Safepoint
    )));
}

#[test]
fn parameter_paths_emit_optional_keyword_rest_and_aux_runtime_values() {
    let list = LambdaList {
        required: vec![ParamName::Symbol(symbol("REQUIRED"))],
        optional: vec![OptionalParam {
            name: ParamName::Symbol(symbol("OPTIONAL")),
            default: None,
            supplied_p: Some(ParamName::Symbol(symbol("OPTIONAL-P"))),
        }],
        rest: Some(ParamName::Symbol(symbol("REST"))),
        keys: vec![KeyParam {
            keyword: SymbolRef::interned("KEYWORD", "VALUE"),
            name: ParamName::Symbol(symbol("VALUE")),
            default: Some(Expr::Constant(Literal::fixnum(7))),
            supplied_p: Some(ParamName::Symbol(symbol("VALUE-P"))),
        }],
        aux: vec![AuxParam {
            name: ParamName::Symbol(symbol("AUX")),
            default: None,
        }],
        allow_other_keys: true,
        ..LambdaList::new()
    };
    let lowered = lower_toplevel(&Expr::Lambda(Box::new(lambda(
        Expr::Progn(vec![
            Expr::Variable(symbol("OPTIONAL-P")),
            Expr::Variable(symbol("VALUE-P")),
            Expr::Variable(symbol("AUX")),
        ]),
        list,
    ))))
    .expect("parameter runtime matrix lowers");
    assert_verifies(&lowered.entry);
    let nested = &lowered.nested[0];
    assert_verifies(nested);
    assert_eq!(
        count_ops(nested, |kind| matches!(kind, OpKind::Compare { .. })),
        2
    );
    for builtin in [
        "make-rest-list",
        "check-keywords",
        "keyword-value",
        "keyword-supplied-p",
    ] {
        assert!(
            any_op(nested, |kind| matches!(
                kind,
                OpKind::Builtin { name, .. } if name == builtin
            )),
            "missing builtin {builtin}"
        );
    }
    assert!(any_op(
        nested,
        |kind| matches!(kind, OpKind::Builtin { name, .. } if name == "CONS")
    ));
    assert!(any_op(nested, |kind| matches!(
        kind,
        OpKind::Convert { .. }
    )));
    assert!(
        nested
            .constants
            .iter()
            .any(|constant| matches!(constant, Constant::Nil))
    );
    assert!(
        nested
            .blocks
            .iter()
            .any(|block| matches!(block.terminator, Terminator::Branch { .. }))
    );
}
