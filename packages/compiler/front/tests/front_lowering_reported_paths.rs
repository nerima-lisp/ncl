//! Twelfth-wave coverage selected from the crate-local llvm-cov region report.
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
fn analysis_wrapper_matrix_reaches_nonlocal_return_with_valid_ir() {
    let exit = symbol("EXIT");
    let nested_return = || {
        Expr::Lambda(Box::new(lambda(
            Expr::Progn(vec![Expr::ReturnFrom {
                name: exit.clone(),
                value: Some(Box::new(Expr::Constant(Literal::fixnum(3)))),
            }]),
            LambdaList::new(),
        )))
    };
    let wrappers = vec![
        Expr::Progn(vec![Expr::Constant(Literal::Nil)]),
        Expr::If {
            test: Box::new(Expr::Constant(Literal::T)),
            then: Box::new(Expr::Constant(Literal::T)),
            otherwise: Some(Box::new(Expr::Constant(Literal::Nil))),
        },
        Expr::Let {
            sequential: false,
            bindings: vec![LetBinding {
                name: symbol("VALUE"),
                value: Some(Expr::Constant(Literal::fixnum(1))),
            }],
            declarations: Vec::new(),
            body: vec![Expr::Variable(symbol("VALUE"))],
        },
    ];
    for wrapper in wrappers {
        let lowered = lower_toplevel(&Expr::Block {
            name: exit.clone(),
            body: vec![wrapper, nested_return()],
        })
        .expect("wrapper matrix lowers");
        assert_verifies(&lowered.entry);
        assert!(
            lowered
                .entry
                .handler_regions
                .iter()
                .any(|region| region.kind == HandlerKind::Catch)
        );
        assert!(any_op(&lowered.entry, |kind| matches!(
            kind,
            OpKind::MakeClosure { .. }
        )));
    }
}

#[test]
fn analysis_contains_return_walks_nested_value_and_optional_branches() {
    let exit = symbol("EXIT");
    let nested = Expr::Lambda(Box::new(lambda(
        Expr::If {
            test: Box::new(Expr::Constant(Literal::T)),
            then: Box::new(Expr::Let {
                sequential: true,
                bindings: vec![LetBinding {
                    name: symbol("LOCAL"),
                    value: Some(Expr::Constant(Literal::Nil)),
                }],
                declarations: Vec::new(),
                body: vec![Expr::ReturnFrom {
                    name: exit.clone(),
                    value: None,
                }],
            }),
            otherwise: Some(Box::new(Expr::Constant(Literal::Nil))),
        },
        LambdaList::new(),
    )));
    let lowered = lower_toplevel(&Expr::Block {
        name: exit,
        body: vec![nested],
    })
    .expect("nested contains-return path lowers");
    assert_verifies(&lowered.entry);
    assert!(
        lowered
            .entry
            .handler_regions
            .iter()
            .any(|region| region.kind == HandlerKind::Catch)
    );
    assert!(any_op(&lowered.nested[0], |kind| matches!(
        kind,
        OpKind::LoadCapture { .. }
    )));
}

#[test]
fn capture_and_bindings_cover_local_function_calls_and_multiple_values() {
    let variable = symbol("VALUE");
    let function = LocalFunction {
        name: symbol("LOCAL"),
        lambda: lambda(
            Expr::Call {
                operator: Operator::Name(symbol("EXTERNAL")),
                arguments: vec![Expr::Variable(variable.clone())],
            },
            LambdaList::new(),
        ),
    };
    let local = Expr::Let {
        sequential: false,
        bindings: vec![LetBinding {
            name: variable,
            value: Some(Expr::Constant(Literal::fixnum(5))),
        }],
        declarations: Vec::new(),
        body: vec![Expr::Flet {
            definitions: vec![function],
            declarations: Vec::new(),
            body: vec![Expr::Call {
                operator: Operator::Name(symbol("LOCAL")),
                arguments: Vec::new(),
            }],
        }],
    };
    let lowered = lower_toplevel(&local).expect("captured local function lowers");
    assert_verifies(&lowered.entry);
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::MakeClosure { .. }
    )));
    assert!(any_op(&lowered.nested[0], |kind| matches!(
        kind,
        OpKind::LoadCapture { .. }
    )));
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::CallClosure { .. }
    )));

    let multiple = Expr::MultipleValueCall {
        function: Box::new(Expr::Function(FunctionDesignator::Name(symbol("LIST")))),
        arguments: vec![
            Expr::Constant(Literal::fixnum(1)),
            Expr::Constant(Literal::fixnum(2)),
        ],
    };
    let values = lower_toplevel(&multiple).expect("multiple values lower");
    assert_verifies(&values.entry);
    assert!(any_op(
        &values.entry,
        |kind| matches!(kind, OpKind::CallClosure { args, .. } if args.len() == 4)
    ));
}

#[test]
fn expression_forms_cover_function_cells_and_control_terminators() {
    let function = Expr::Function(FunctionDesignator::Name(symbol("EXTERNAL")));
    let lowered = lower_toplevel(&function).expect("function designator lowers");
    assert_verifies(&lowered.entry);
    assert!(any_op(
        &lowered.entry,
        |kind| matches!(kind, OpKind::LoadField { field, .. } if *field == u32::try_from(ncl_object::symbol_offset::FUNCTION).unwrap())
    ));

    let block_name = symbol("DONE");
    let call = Expr::Call {
        operator: Operator::Name(symbol("LIST")),
        arguments: vec![Expr::ReturnFrom {
            name: block_name.clone(),
            value: Some(Box::new(Expr::Constant(Literal::fixnum(8)))),
        }],
    };
    let block = lower_toplevel(&Expr::Block {
        name: block_name,
        body: vec![call],
    })
    .expect("terminated call lowers");
    assert_verifies(&block.entry);
    assert!(any_terminator(&block.entry, |term| matches!(
        term,
        Terminator::Jump { .. }
    )));
}

#[test]
fn parameter_keyword_and_rest_paths_assert_runtime_prologue_operations() {
    let list = LambdaList {
        rest: Some(ParamName::Symbol(symbol("ARGS"))),
        keys: vec![ncl_compiler_front::KeyParam {
            keyword: SymbolRef::interned("KEYWORD", "VALUE"),
            name: ParamName::Symbol(symbol("VALUE")),
            default: None,
            supplied_p: Some(ParamName::Symbol(symbol("VALUE-P"))),
        }],
        allow_other_keys: false,
        ..LambdaList::new()
    };
    let expression = Expr::Lambda(Box::new(lambda(Expr::Constant(Literal::T), list)));
    let lowered = lower_toplevel(&expression).expect("key rest prologue lowers");
    assert_verifies(&lowered.entry);
    let nested = &lowered.nested[0];
    assert_verifies(nested);
    assert!(any_op(
        nested,
        |kind| matches!(kind, OpKind::Builtin { name, .. } if name == "make-rest-list")
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
fn empty_and_unbound_forms_return_concrete_values_or_typed_errors() {
    let empty_call = Expr::Call {
        operator: Operator::Name(symbol("LIST")),
        arguments: Vec::new(),
    };
    let lowered = lower_toplevel(&empty_call).expect("empty call lowers");
    assert_verifies(&lowered.entry);
    assert!(any_op(
        &lowered.entry,
        |kind| matches!(kind, OpKind::CallClosure { args, .. } if args.len() == 1)
    ));

    let unbound = lower_toplevel(&Expr::ReturnFrom {
        name: symbol("MISSING"),
        value: None,
    })
    .unwrap_err();
    assert!(matches!(
        unbound,
        ncl_compiler_front::LowerError::EscapingControl { .. }
    ));
    assert!(unbound.to_string().contains("MISSING"));
}
