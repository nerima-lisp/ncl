//! Seventeenth-wave coverage for remaining lower branches.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use ncl_compiler_front::{
    AuxParam, Expr, FunctionDesignator, KeyParam, LambdaExpr, LambdaList, LetBinding, Literal,
    LocalFunction, Operator, OptionalParam, ParamName, SymbolRef, lower_toplevel,
};
use ncl_ir::{Constant, Function, OpKind, Terminator, verify};

fn symbol(name: &str) -> SymbolRef {
    SymbolRef::interned("COMMON-LISP-USER", name)
}

fn lambda(body: Expr, lambda_list: LambdaList) -> LambdaExpr {
    LambdaExpr {
        lambda_list,
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

fn any_terminator(function: &Function, predicate: impl Fn(&Terminator) -> bool) -> bool {
    function
        .blocks
        .iter()
        .any(|block| predicate(&block.terminator))
}

#[test]
fn analysis_walks_matching_and_nonmatching_targets_through_all_wrappers() {
    let exit = symbol("EXIT");
    let local = LocalFunction {
        name: symbol("LOCAL"),
        lambda: lambda(Expr::Constant(Literal::Nil), LambdaList::new()),
    };
    let nested = Expr::Lambda(Box::new(lambda(
        Expr::Progn(vec![Expr::ReturnFrom {
            name: exit.clone(),
            value: Some(Box::new(Expr::Constant(Literal::fixnum(6)))),
        }]),
        LambdaList::new(),
    )));
    let escaping = lower_toplevel(&Expr::Block {
        name: exit,
        body: vec![Expr::Flet {
            definitions: vec![local],
            declarations: Vec::new(),
            body: vec![nested],
        }],
    })
    .expect("nested return matrix lowers");
    assert_verifies(&escaping.entry);
    assert!(
        escaping
            .entry
            .handler_regions
            .iter()
            .any(|region| { region.kind == ncl_ir::HandlerKind::Catch })
    );
    assert!(any_op(&escaping.entry, |kind| matches!(
        kind,
        OpKind::MakeClosure { .. }
    )));

    let control = lower_toplevel(&Expr::Progv {
        symbols: Box::new(Expr::Constant(Literal::Nil)),
        values: Box::new(Expr::Constant(Literal::Nil)),
        body: vec![Expr::UnwindProtect {
            protected: Box::new(Expr::If {
                test: Box::new(Expr::Constant(Literal::T)),
                then: Box::new(Expr::Constant(Literal::Nil)),
                otherwise: None,
            }),
            cleanup: vec![Expr::The {
                type_specifier: ncl_compiler_front::TypeSpecifier::new(Literal::T),
                value: Box::new(Expr::Constant(Literal::Nil)),
            }],
        }],
    })
    .expect("normal control wrappers lower");
    assert_verifies(&control.entry);
    assert!(
        control
            .entry
            .handler_regions
            .iter()
            .any(|region| { region.kind == ncl_ir::HandlerKind::Progv })
    );
    assert!(any_terminator(&control.entry, |term| matches!(
        term,
        Terminator::Jump { .. } | Terminator::Return { .. }
    )));

    let tag = symbol("TAG");
    let tagbody = lower_toplevel(&Expr::Tagbody(vec![
        ncl_compiler_front::TagbodyItem::Tag(tag.clone()),
        ncl_compiler_front::TagbodyItem::Form(Expr::Constant(Literal::Nil)),
        ncl_compiler_front::TagbodyItem::Form(Expr::Go { tag }),
    ]))
    .expect("tagbody analysis path lowers");
    assert_verifies(&tagbody.entry);
    assert!(any_op(&tagbody.entry, |kind| matches!(
        kind,
        OpKind::Safepoint
    )));
}

#[test]
fn free_name_analysis_propagates_value_and_function_namespaces() {
    let value = symbol("VALUE");
    let helper = symbol("HELPER");
    let local = LocalFunction {
        name: helper.clone(),
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
            value: Some(Expr::Constant(Literal::fixnum(8))),
        }],
        declarations: Vec::new(),
        body: vec![Expr::Labels {
            definitions: vec![local],
            declarations: Vec::new(),
            body: vec![Expr::Lambda(Box::new(lambda(
                Expr::Progn(vec![
                    Expr::Function(FunctionDesignator::Name(helper)),
                    Expr::Variable(value),
                ]),
                LambdaList::new(),
            )))],
        }],
    };
    let lowered = lower_toplevel(&expression).expect("value and function free names lower");
    assert_verifies(&lowered.entry);
    assert!(lowered.nested.len() >= 2);
    assert!(
        count_ops(&lowered.entry, |kind| matches!(
            kind,
            OpKind::MakeClosure { .. }
        )) >= 2
    );
    for nested in &lowered.nested {
        assert_verifies(nested);
        assert!(any_op(nested, |kind| matches!(
            kind,
            OpKind::LoadCapture { .. }
        )));
    }
}

#[test]
fn expression_call_matrix_distinguishes_builtin_arity_and_general_fallback() {
    let builtin = Expr::Call {
        operator: Operator::Name(symbol("+")),
        arguments: vec![
            Expr::Constant(Literal::fixnum(1)),
            Expr::Constant(Literal::fixnum(2)),
        ],
    };
    let wrong_arity = Expr::Call {
        operator: Operator::Name(symbol("+")),
        arguments: vec![Expr::Constant(Literal::fixnum(1))],
    };
    let function_value = Expr::Function(FunctionDesignator::Name(symbol("EXTERNAL")));
    let expression = Expr::Progn(vec![builtin, wrong_arity, function_value]);
    let lowered = lower_toplevel(&expression).expect("builtin and fallback calls lower");
    assert_verifies(&lowered.entry);
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::Builtin { name, args } if name == "+" && args.len() == 2
    )));
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::LoadField { field, .. } if *field == ncl_object::symbol_offset::FUNCTION as u32
    )));
    assert!(any_op(
        &lowered.entry,
        |kind| matches!(kind, OpKind::CallClosure { args, .. } if args.len() == 2)
    ));
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::Safepoint
    )));
}

#[test]
fn bindings_keep_cell_store_separate_from_lexical_rebind_and_global_store() {
    let cell = symbol("CELL");
    let plain = symbol("PLAIN");
    let expression = Expr::Let {
        sequential: false,
        bindings: vec![
            LetBinding {
                name: cell.clone(),
                value: Some(Expr::Constant(Literal::fixnum(1))),
            },
            LetBinding {
                name: plain.clone(),
                value: Some(Expr::Constant(Literal::fixnum(2))),
            },
        ],
        declarations: Vec::new(),
        body: vec![
            Expr::Setq(vec![(cell.clone(), Expr::Constant(Literal::fixnum(3)))]),
            Expr::Lambda(Box::new(lambda(Expr::Variable(cell), LambdaList::new()))),
            Expr::Setq(vec![(plain, Expr::Constant(Literal::fixnum(4)))]),
            Expr::Setq(vec![(symbol("GLOBAL"), Expr::Constant(Literal::fixnum(5)))]),
        ],
    };
    let lowered = lower_toplevel(&expression).expect("binding storage matrix lowers");
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
        OpKind::StoreField { field, .. } if *field == ncl_object::symbol_offset::VALUE as u32
    )));
    assert!(any_op(&lowered.nested[0], |kind| matches!(
        kind,
        OpKind::LoadField { field: 0, .. }
    )));
}

#[test]
fn parameters_cover_supplied_flags_keyword_defaults_and_aux_initializers() {
    let list = LambdaList {
        optional: vec![OptionalParam {
            name: ParamName::Symbol(symbol("OPTIONAL")),
            default: Some(Expr::Constant(Literal::fixnum(12))),
            supplied_p: Some(ParamName::Symbol(symbol("OPTIONAL-P"))),
        }],
        keys: vec![KeyParam {
            keyword: SymbolRef::interned("KEYWORD", "VALUE"),
            name: ParamName::Symbol(symbol("VALUE")),
            default: Some(Expr::Constant(Literal::fixnum(13))),
            supplied_p: Some(ParamName::Symbol(symbol("VALUE-P"))),
        }],
        rest: Some(ParamName::Symbol(symbol("REST"))),
        allow_other_keys: false,
        aux: vec![AuxParam {
            name: ParamName::Symbol(symbol("AUX")),
            default: Some(Expr::Constant(Literal::fixnum(14))),
        }],
        ..LambdaList::new()
    };
    let lowered = lower_toplevel(&Expr::Lambda(Box::new(lambda(
        Expr::Variable(symbol("AUX")),
        list,
    ))))
    .expect("parameter matrix lowers");
    assert_verifies(&lowered.entry);
    let nested = &lowered.nested[0];
    assert_verifies(nested);
    assert_eq!(
        count_ops(nested, |kind| matches!(kind, OpKind::Compare { .. })),
        2
    );
    for expected in [12, 13, 14] {
        assert!(
            nested
                .constants
                .iter()
                .any(|constant| matches!(constant, Constant::Fixnum(value) if *value == expected)),
            "missing initializer {expected}"
        );
    }
    assert!(any_op(nested, |kind| matches!(
        kind,
        OpKind::Builtin { name, .. } if name == "check-keywords"
    )));
    assert!(any_op(nested, |kind| matches!(
        kind,
        OpKind::Builtin { name, .. } if name == "keyword-supplied-p"
    )));
    assert!(any_terminator(nested, |term| matches!(
        term,
        Terminator::Branch { .. }
    )));
}
