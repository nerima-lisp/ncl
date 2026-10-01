//! Fifteenth-wave coverage for remaining lower analysis and binding branches.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use ncl_compiler_front::{
    AuxParam, Expr, KeyParam, LambdaExpr, LambdaList, LetBinding, Literal, LocalFunction, Operator,
    OptionalParam, ParamName, SymbolRef, lower_toplevel,
};
use ncl_ir::{Constant, Function, HandlerKind, OpKind, Terminator, verify};

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

fn lambda(body: Expr, lambda_list: LambdaList) -> LambdaExpr {
    LambdaExpr {
        lambda_list,
        declarations: Vec::new(),
        docstring: None,
        body: vec![body],
    }
}

#[test]
fn analysis_walks_all_control_wrappers_before_an_escaping_return() {
    let exit = symbol("EXIT");
    let nested_return = Expr::Lambda(Box::new(lambda(
        Expr::Progn(vec![Expr::ReturnFrom {
            name: exit.clone(),
            value: Some(Box::new(Expr::Constant(Literal::fixnum(15)))),
        }]),
        LambdaList::new(),
    )));
    let escaping = lower_toplevel(&Expr::Block {
        name: exit.clone(),
        body: vec![Expr::Locally {
            declarations: Vec::new(),
            body: vec![nested_return],
        }],
    })
    .expect("nested return through locally lowers");
    assert_verifies(&escaping.entry);
    assert!(
        escaping
            .entry
            .handler_regions
            .iter()
            .any(|region| region.kind == HandlerKind::Catch)
    );
    assert!(any_op(&escaping.entry, |kind| matches!(
        kind,
        OpKind::MakeClosure { .. }
    )));

    let protected = lower_toplevel(&Expr::Block {
        name: symbol("DONE"),
        body: vec![Expr::UnwindProtect {
            protected: Box::new(Expr::ReturnFrom {
                name: symbol("DONE"),
                value: Some(Box::new(Expr::Constant(Literal::fixnum(1)))),
            }),
            cleanup: vec![Expr::MultipleValueProg1 {
                first: Box::new(Expr::Constant(Literal::Nil)),
                forms: vec![Expr::The {
                    type_specifier: ncl_compiler_front::TypeSpecifier::new(Literal::T),
                    value: Box::new(Expr::Constant(Literal::Nil)),
                }],
            }],
        }],
    })
    .expect("unwind-protect control path lowers");
    assert_verifies(&protected.entry);
    assert!(
        protected
            .entry
            .handler_regions
            .iter()
            .any(|region| { region.kind == HandlerKind::UnwindProtect })
    );
    assert!(any_op(&protected.entry, |kind| matches!(
        kind,
        OpKind::Safepoint
    )));

    let tag = symbol("TAG");
    let tagbody = lower_toplevel(&Expr::Tagbody(vec![
        ncl_compiler_front::TagbodyItem::Tag(tag.clone()),
        ncl_compiler_front::TagbodyItem::Form(Expr::Constant(Literal::Nil)),
        ncl_compiler_front::TagbodyItem::Form(Expr::Go { tag }),
    ]))
    .expect("tagbody control path lowers");
    assert_verifies(&tagbody.entry);
    assert!(tagbody.entry.handler_regions.is_empty());
    assert!(any_op(&tagbody.entry, |kind| matches!(
        kind,
        OpKind::Safepoint
    )));
}

#[test]
fn complex_lambda_list_emits_optional_keyword_rest_and_aux_contracts() {
    let list = LambdaList {
        required: vec![ParamName::Symbol(symbol("REQUIRED"))],
        optional: vec![OptionalParam {
            name: ParamName::Symbol(symbol("OPTIONAL")),
            default: Some(Expr::Constant(Literal::fixnum(7))),
            supplied_p: Some(ParamName::Symbol(symbol("OPTIONAL-P"))),
        }],
        rest: Some(ParamName::Symbol(symbol("REST"))),
        keys: vec![
            KeyParam {
                keyword: SymbolRef::interned("KEYWORD", "VALUE"),
                name: ParamName::Symbol(symbol("VALUE")),
                default: Some(Expr::Constant(Literal::fixnum(8))),
                supplied_p: Some(ParamName::Symbol(symbol("VALUE-P"))),
            },
            KeyParam {
                keyword: SymbolRef::interned("KEYWORD", "OTHER"),
                name: ParamName::Symbol(symbol("OTHER")),
                default: None,
                supplied_p: None,
            },
        ],
        allow_other_keys: true,
        aux: vec![
            AuxParam {
                name: ParamName::Symbol(symbol("AUX-VALUE")),
                default: Some(Expr::Constant(Literal::fixnum(9))),
            },
            AuxParam {
                name: ParamName::Symbol(symbol("AUX-NIL")),
                default: None,
            },
        ],
        ..LambdaList::new()
    };
    let lowered = lower_toplevel(&Expr::Lambda(Box::new(lambda(
        Expr::Variable(symbol("AUX-VALUE")),
        list,
    ))))
    .expect("complex lambda list lowers");
    assert_verifies(&lowered.entry);
    let nested = &lowered.nested[0];
    assert_verifies(nested);
    for name in [
        "make-rest-list",
        "check-keywords",
        "keyword-value",
        "keyword-supplied-p",
        "CONS",
    ] {
        assert!(
            any_op(
                nested,
                |kind| matches!(kind, OpKind::Builtin { name: op_name, .. } if op_name == name)
            ),
            "missing builtin {name}"
        );
    }
    assert!(any_op(nested, |kind| matches!(
        kind,
        OpKind::Compare {
            op: ncl_ir::Compare::Ge,
            ..
        }
    )));
    assert!(any_terminator(nested, |term| matches!(
        term,
        Terminator::Branch { .. }
    )));
    assert!(
        nested
            .constants
            .iter()
            .any(|constant| matches!(constant, Constant::Fixnum(9)))
    );
}

#[test]
fn symbol_and_function_cell_forms_preserve_load_store_and_funcall_shapes() {
    let symbol_value = Expr::Call {
        operator: Operator::Name(symbol("symbol-value")),
        arguments: vec![Expr::Constant(Literal::Symbol(symbol("VALUE")))],
    };
    let set_symbol_value = Expr::Call {
        operator: Operator::Name(symbol("set-symbol-value")),
        arguments: vec![
            Expr::Constant(Literal::Symbol(symbol("VALUE"))),
            Expr::Constant(Literal::fixnum(4)),
        ],
    };
    let function_store = Expr::Call {
        operator: Operator::Name(SymbolRef::interned("NCL-EXT", "FDEFINITION-SET")),
        arguments: vec![
            Expr::Constant(Literal::Symbol(symbol("FUNCTION"))),
            Expr::Constant(Literal::Symbol(symbol("TARGET"))),
        ],
    };
    let lowered = lower_toplevel(&Expr::Progn(vec![
        symbol_value,
        set_symbol_value,
        function_store,
    ]))
    .expect("symbol cell forms lower");
    assert_verifies(&lowered.entry);
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::LoadField { field, .. } if *field == ncl_object::symbol_offset::VALUE as u32
    )));
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::StoreField { field, .. } if *field == ncl_object::symbol_offset::VALUE as u32
    )));
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::StoreField { field, .. } if *field == ncl_object::symbol_offset::FUNCTION as u32
    )));

    let closure = Expr::The {
        type_specifier: ncl_compiler_front::TypeSpecifier::new(Literal::T),
        value: Box::new(Expr::Lambda(Box::new(lambda(
            Expr::Constant(Literal::fixnum(3)),
            LambdaList::new(),
        )))),
    };
    let funcall = Expr::Call {
        operator: Operator::Name(symbol("FUNCALL")),
        arguments: vec![closure, Expr::Constant(Literal::fixnum(1))],
    };
    let called = lower_toplevel(&funcall).expect("funcall closure designator lowers");
    assert_verifies(&called.entry);
    assert!(any_op(&called.entry, |kind| matches!(
        kind,
        OpKind::CallClosure { args, .. } if args.len() == 2
    )));
}

#[test]
fn recursive_functions_propagate_variable_and_function_captures() {
    let value = symbol("VALUE");
    let first_name = symbol("FIRST");
    let second_name = symbol("SECOND");
    let first = LocalFunction {
        name: first_name.clone(),
        lambda: lambda(
            Expr::Call {
                operator: Operator::Name(second_name.clone()),
                arguments: vec![Expr::Variable(value.clone())],
            },
            LambdaList::new(),
        ),
    };
    let second = LocalFunction {
        name: second_name.clone(),
        lambda: lambda(
            Expr::Call {
                operator: Operator::Name(first_name.clone()),
                arguments: vec![Expr::Variable(value.clone())],
            },
            LambdaList::new(),
        ),
    };
    let expression = Expr::Let {
        sequential: false,
        bindings: vec![LetBinding {
            name: value,
            value: Some(Expr::Constant(Literal::fixnum(10))),
        }],
        declarations: Vec::new(),
        body: vec![Expr::Labels {
            definitions: vec![first, second],
            declarations: Vec::new(),
            body: vec![Expr::Call {
                operator: Operator::Name(first_name),
                arguments: Vec::new(),
            }],
        }],
    };
    let lowered = lower_toplevel(&expression).expect("recursive captures lower");
    assert_verifies(&lowered.entry);
    assert_eq!(lowered.nested.len(), 2);
    let entry_closures = lowered
        .entry
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .filter(|op| matches!(op.kind, OpKind::MakeClosure { .. }));
    assert_eq!(entry_closures.count(), 2);
    for nested in &lowered.nested {
        assert_verifies(nested);
        assert!(any_op(nested, |kind| matches!(
            kind,
            OpKind::MakeClosure { .. }
        )));
        assert!(any_op(nested, |kind| matches!(
            kind,
            OpKind::LoadCapture { .. }
        )));
        assert!(any_op(nested, |kind| matches!(
            kind,
            OpKind::CallClosure { .. }
        )));
    }
}

#[test]
fn lexical_setq_and_unbound_setq_keep_distinct_side_effects() {
    let local = symbol("LOCAL");
    let lexical = Expr::Let {
        sequential: false,
        bindings: vec![LetBinding {
            name: local.clone(),
            value: Some(Expr::Constant(Literal::fixnum(1))),
        }],
        declarations: Vec::new(),
        body: vec![Expr::Setq(vec![(
            local,
            Expr::Constant(Literal::fixnum(2)),
        )])],
    };
    let lexical_lowered = lower_toplevel(&lexical).expect("lexical setq lowers");
    assert_verifies(&lexical_lowered.entry);
    assert!(!any_op(&lexical_lowered.entry, |kind| matches!(
        kind,
        OpKind::StoreField { .. }
    )));
    assert!(
        lexical_lowered
            .entry
            .constants
            .iter()
            .any(|constant| matches!(constant, Constant::Fixnum(2)))
    );

    let global_lowered = lower_toplevel(&Expr::Setq(vec![(
        symbol("GLOBAL"),
        Expr::Constant(Literal::fixnum(3)),
    )]))
    .expect("unbound setq lowers");
    assert_verifies(&global_lowered.entry);
    assert!(any_op(&global_lowered.entry, |kind| matches!(
        kind,
        OpKind::StoreField { field, .. } if *field == ncl_object::symbol_offset::VALUE as u32
    )));
}
