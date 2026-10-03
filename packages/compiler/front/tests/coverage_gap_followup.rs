#![allow(clippy::expect_used, missing_docs)]

use ncl_compiler_front::{
    Expr, FunctionDesignator, LambdaExpr, LambdaList, LetBinding, Literal, LocalFunction, Operator,
    SymbolRef, lower_toplevel,
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
