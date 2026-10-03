#![allow(clippy::expect_used, missing_docs)]

use ncl_compiler_front::{
    AuxParam, Expr, FunctionDesignator, KeyParam, LambdaExpr, LambdaList, LetBinding, Literal,
    LocalFunction, Operator, OptionalParam, ParamName, SymbolRef, TypeSpecifier, lower_toplevel,
};
use ncl_ir::{Constant, Function, HandlerKind, OpKind, verify};

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
        .flat_map(|block| &block.ops)
        .any(|op| predicate(&op.kind))
}

#[test]
fn normal_catch_and_unwind_paths_preserve_values_and_cleanup() {
    let catch = lower_toplevel(&Expr::Catch {
        tag: Box::new(Expr::Constant(Literal::Symbol(symbol("TAG")))),
        body: vec![Expr::Constant(Literal::fixnum(11))],
    })
    .expect("normal catch lowers");
    assert_verifies(&catch.entry);
    assert_eq!(catch.entry.handler_regions[0].kind, HandlerKind::Catch);
    assert!(catch.entry.constants.contains(&Constant::Fixnum(11)));

    let unwind = lower_toplevel(&Expr::UnwindProtect {
        protected: Box::new(Expr::Constant(Literal::fixnum(13))),
        cleanup: vec![Expr::Constant(Literal::fixnum(29))],
    })
    .expect("normal unwind-protect lowers");
    assert_verifies(&unwind.entry);
    assert_eq!(
        unwind.entry.handler_regions[0].kind,
        HandlerKind::UnwindProtect
    );
    assert!(unwind.entry.blocks.iter().any(|block| {
        block
            .ops
            .iter()
            .any(|op| matches!(&op.kind, OpKind::SetMultipleValues { values } if values.len() == 1))
    }));
    assert!(unwind.entry.constants.contains(&Constant::Fixnum(13)));
    assert!(unwind.entry.constants.contains(&Constant::Fixnum(29)));
}

#[test]
fn recursive_labels_propagate_outer_variable_and_function_captures() {
    let value = symbol("VALUE");
    let first = symbol("FIRST");
    let second = symbol("SECOND");
    let helper = symbol("HELPER");
    let expression = Expr::Let {
        sequential: false,
        bindings: vec![LetBinding {
            name: value.clone(),
            value: Some(Expr::Constant(Literal::fixnum(7))),
        }],
        declarations: Vec::new(),
        body: vec![Expr::Flet {
            definitions: vec![LocalFunction {
                name: helper.clone(),
                lambda: lambda(Expr::Constant(Literal::fixnum(9))),
            }],
            declarations: Vec::new(),
            body: vec![Expr::Labels {
                definitions: vec![
                    LocalFunction {
                        name: first.clone(),
                        lambda: lambda(Expr::Call {
                            operator: Operator::Name(second.clone()),
                            arguments: Vec::new(),
                        }),
                    },
                    LocalFunction {
                        name: second,
                        lambda: lambda(Expr::Progn(vec![
                            Expr::Variable(value),
                            Expr::Call {
                                operator: Operator::Name(helper),
                                arguments: Vec::new(),
                            },
                        ])),
                    },
                ],
                declarations: Vec::new(),
                body: vec![Expr::Call {
                    operator: Operator::Name(first),
                    arguments: Vec::new(),
                }],
            }],
        }],
    };
    let lowered = lower_toplevel(&expression).expect("recursive captures lower");
    assert_verifies(&lowered.entry);
    assert!(lowered.nested.len() >= 3);
    assert!(
        lowered.nested.iter().any(|function| {
            any_op(function, |kind| matches!(kind, OpKind::LoadCapture { .. }))
        })
    );
    assert!(
        lowered.nested.iter().any(|function| {
            any_op(function, |kind| matches!(kind, OpKind::CallClosure { .. }))
        })
    );
    assert!(lowered.entry.constants.contains(&Constant::Fixnum(7)));
}

#[test]
fn function_designator_wrappers_still_lower_to_a_closure_call() {
    let expression = Expr::Call {
        operator: Operator::Lambda(Box::new(lambda(Expr::Constant(Literal::fixnum(23))))),
        arguments: Vec::new(),
    };
    let lowered = lower_toplevel(&expression).expect("lambda designator lowers");
    assert_verifies(&lowered.entry);
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::CallClosure {
            named_symbol: None,
            ..
        }
    )));
    assert!(
        lowered
            .nested
            .iter()
            .any(|function| { function.constants.contains(&Constant::Fixnum(23)) })
    );
}

#[test]
fn function_name_designator_and_multiple_value_call_keep_real_arguments() {
    let expression = Expr::MultipleValueCall {
        function: Box::new(Expr::Function(FunctionDesignator::Name(symbol("LIST")))),
        arguments: vec![
            Expr::Constant(Literal::fixnum(31)),
            Expr::Constant(Literal::fixnum(37)),
        ],
    };
    let lowered = lower_toplevel(&expression).expect("multiple value call lowers");
    assert_verifies(&lowered.entry);
    assert!(lowered.entry.constants.contains(&Constant::Fixnum(31)));
    assert!(lowered.entry.constants.contains(&Constant::Fixnum(37)));
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::CallClosure { args, .. } if args.len() == 4
    )));
}

#[test]
#[allow(clippy::too_many_lines)]
fn expression_lowering_covers_wrappers_symbol_cells_builtins_and_parameter_shapes() {
    let closure = || Expr::Lambda(Box::new(lambda(Expr::Constant(Literal::fixnum(41)))));
    let wrapped = [
        Expr::Progn(vec![Expr::Constant(Literal::Nil), closure()]),
        Expr::Locally {
            declarations: Vec::new(),
            body: vec![closure()],
        },
        Expr::The {
            type_specifier: TypeSpecifier::new(Literal::T),
            value: Box::new(closure()),
        },
        Expr::LoadTimeValue {
            form: Box::new(closure()),
            read_only: true,
        },
        Expr::Macrolet {
            definitions: Vec::new(),
            declarations: Vec::new(),
            body: vec![closure()],
        },
        Expr::SymbolMacrolet {
            definitions: Vec::new(),
            declarations: Vec::new(),
            body: vec![closure()],
        },
    ];
    for designator in wrapped {
        let lowered = lower_toplevel(&Expr::Call {
            operator: Operator::Name(symbol("FUNCALL")),
            arguments: vec![designator, Expr::Constant(Literal::fixnum(3))],
        })
        .expect("wrapped closure designator lowers");
        assert_verifies(&lowered.entry);
        assert!(any_op(&lowered.entry, |kind| matches!(
            kind,
            OpKind::CallClosure {
                named_symbol: None,
                args,
                ..
            } if args.len() == 2
        )));
    }

    let symbol_value = symbol("VALUE");
    for (name, arguments, expected_field, expected_store) in [
        (
            "symbol-value",
            vec![Expr::Constant(Literal::Symbol(symbol_value))],
            0,
            false,
        ),
        (
            "set-symbol-value",
            vec![
                Expr::Constant(Literal::Symbol(symbol("VALUE"))),
                Expr::Constant(Literal::fixnum(8)),
            ],
            0,
            true,
        ),
    ] {
        let lowered = lower_toplevel(&Expr::Call {
            operator: Operator::Name(symbol(name)),
            arguments,
        })
        .expect("symbol cell call lowers");
        assert_verifies(&lowered.entry);
        assert!(any_op(&lowered.entry, |kind| match kind {
            OpKind::LoadField { field, .. } => !expected_store && *field == expected_field,
            OpKind::StoreField { field, .. } => expected_store && *field == expected_field,
            _ => false,
        }));
    }
    let fdefinition_set = lower_toplevel(&Expr::Call {
        operator: Operator::Name(SymbolRef::interned("NCL", "FDEFINITION-SET")),
        arguments: vec![
            Expr::Constant(Literal::Symbol(symbol("FUNCTION"))),
            Expr::Constant(Literal::Symbol(symbol("TARGET"))),
        ],
    })
    .expect("fdefinition-set lowers");
    assert_verifies(&fdefinition_set.entry);
    assert!(any_op(&fdefinition_set.entry, |kind| matches!(
        kind,
        OpKind::StoreField { field: 1, .. }
    )));

    for (name, arity) in [
        ("+", 2),
        ("*", 2),
        ("-", 2),
        ("<", 2),
        ("CAR", 1),
        ("CONS", 2),
    ] {
        let lowered = lower_toplevel(&Expr::Call {
            operator: Operator::Name(symbol(name)),
            arguments: (0..arity)
                .map(|n| Expr::Constant(Literal::fixnum(i64::from(n))))
                .collect(),
        })
        .expect("builtin call lowers");
        assert_verifies(&lowered.entry);
        assert!(any_op(&lowered.entry, |kind| matches!(
            kind,
            OpKind::Builtin { name: actual, .. } if actual.eq_ignore_ascii_case(name)
        )));
    }

    let list = LambdaList {
        required: vec![ParamName::Symbol(symbol("required"))],
        optional: vec![OptionalParam {
            name: ParamName::Symbol(symbol("optional")),
            default: None,
            supplied_p: Some(ParamName::Symbol(symbol("optional-p"))),
        }],
        rest: Some(ParamName::Symbol(symbol("rest"))),
        keys: vec![KeyParam {
            keyword: symbol(":KEY"),
            name: ParamName::Symbol(symbol("key")),
            default: Some(Expr::Constant(Literal::fixnum(17))),
            supplied_p: Some(ParamName::Symbol(symbol("key-p"))),
        }],
        allow_other_keys: true,
        aux: vec![AuxParam {
            name: ParamName::Symbol(symbol("aux")),
            default: None,
        }],
        ..LambdaList::new()
    };
    let lowered = lower_toplevel(&Expr::Lambda(Box::new(LambdaExpr {
        lambda_list: list,
        declarations: Vec::new(),
        docstring: None,
        body: vec![Expr::Constant(Literal::T)],
    })))
    .expect("complete lambda list lowers");
    assert_verifies(&lowered.entry);
    assert_verifies(&lowered.nested[0]);
    assert!(any_op(&lowered.nested[0], |kind| matches!(
        kind,
        OpKind::Builtin { name, .. } if name == "check-keywords"
    )));
    assert!(any_op(&lowered.nested[0], |kind| matches!(
        kind,
        OpKind::Builtin { name, .. } if name == "make-rest-list"
    )));
}
