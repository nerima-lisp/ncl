//! Tenth-wave coverage for additional lower analysis and binding paths.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use ncl_compiler_front::{
    Expr, FunctionDesignator, LambdaExpr, LambdaList, Literal, LocalFunction, Operator, ParamName,
    SymbolRef, lower_toplevel,
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

fn empty_lambda(body: Expr) -> LambdaExpr {
    LambdaExpr {
        lambda_list: LambdaList::new(),
        declarations: Vec::new(),
        docstring: None,
        body: vec![body],
    }
}

#[test]
fn analysis_recurses_through_if_setq_and_load_wrappers_for_nonlocal_return() {
    let exit = symbol("EXIT");
    let return_from = || Expr::ReturnFrom {
        name: exit.clone(),
        value: Some(Box::new(Expr::Constant(Literal::fixnum(1)))),
    };
    let body = Expr::Progn(vec![
        Expr::Setq(vec![(
            symbol("VALUE"),
            Expr::If {
                test: Box::new(Expr::Constant(Literal::T)),
                then: Box::new(Expr::Constant(Literal::Nil)),
                otherwise: Some(Box::new(Expr::Constant(Literal::T))),
            },
        )]),
        Expr::LoadTimeValue {
            form: Box::new(Expr::The {
                type_specifier: ncl_compiler_front::TypeSpecifier::new(Literal::T),
                value: Box::new(Expr::Lambda(Box::new(empty_lambda(Expr::Progn(vec![
                    return_from(),
                ]))))),
            }),
            read_only: false,
        },
        Expr::Lambda(Box::new(empty_lambda(Expr::Progn(vec![return_from()])))),
    ]);
    let lowered = lower_toplevel(&Expr::Block {
        name: exit,
        body: vec![body],
    })
    .expect("wrapped nonlocal return lowers");
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

#[test]
fn analysis_fast_path_keeps_plain_function_and_constant_blocks_local() {
    let expression = Expr::Block {
        name: symbol("LOCAL"),
        body: vec![
            Expr::Function(FunctionDesignator::Name(symbol("EXTERNAL"))),
            Expr::Constant(Literal::fixnum(22)),
        ],
    };
    let lowered = lower_toplevel(&expression).expect("plain block lowers");
    assert_verifies(&lowered.entry);
    assert!(lowered.entry.handler_regions.is_empty());
    assert!(any_terminator(&lowered.entry, |term| matches!(
        term,
        Terminator::Jump { .. }
    )));
    assert!(any_op(
        &lowered.entry,
        |kind| matches!(kind, OpKind::LoadField { field, .. } if *field == ncl_object::symbol_offset::FUNCTION as u32)
    ));
    assert!(
        lowered
            .entry
            .constants
            .iter()
            .any(|constant| matches!(constant, Constant::Fixnum(22)))
    );
}

#[test]
fn mutually_recursive_labels_build_function_captures_for_each_definition() {
    let first_name = symbol("FIRST");
    let second_name = symbol("SECOND");
    let first = LocalFunction {
        name: first_name.clone(),
        lambda: empty_lambda(Expr::Call {
            operator: Operator::Name(second_name.clone()),
            arguments: Vec::new(),
        }),
    };
    let second = LocalFunction {
        name: second_name.clone(),
        lambda: empty_lambda(Expr::Call {
            operator: Operator::Name(first_name.clone()),
            arguments: Vec::new(),
        }),
    };
    let expression = Expr::Labels {
        definitions: vec![first, second],
        declarations: Vec::new(),
        body: vec![Expr::Call {
            operator: Operator::Name(first_name),
            arguments: Vec::new(),
        }],
    };
    let lowered = lower_toplevel(&expression).expect("mutually recursive labels lower");
    assert_verifies(&lowered.entry);
    assert_eq!(lowered.nested.len(), 2);
    for nested in &lowered.nested {
        assert_verifies(nested);
        assert!(any_op(nested, |kind| matches!(
            kind,
            OpKind::MakeClosure { .. }
        )));
    }
}

#[test]
fn multiple_value_call_with_multiple_forms_captures_each_value() {
    let expression = Expr::MultipleValueCall {
        function: Box::new(Expr::Function(FunctionDesignator::Name(symbol("LIST")))),
        arguments: vec![
            Expr::Constant(Literal::fixnum(1)),
            Expr::Constant(Literal::fixnum(2)),
            Expr::Constant(Literal::fixnum(3)),
        ],
    };
    let lowered = lower_toplevel(&expression).expect("multiple-value call lowers");
    assert_verifies(&lowered.entry);
    let capture_calls = lowered
        .entry
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .filter(|op| matches!(op.kind, OpKind::CallClosure { ref args, .. } if args.len() == 1));
    assert_eq!(capture_calls.count(), 3);
    assert!(any_op(
        &lowered.entry,
        |kind| matches!(kind, OpKind::CallClosure { args, .. } if args.len() == 5)
    ));
}

#[test]
fn function_cell_store_accepts_common_lisp_extension_spelling() {
    let expression = Expr::Call {
        operator: Operator::Name(SymbolRef::interned("COMMON-LISP", "NCL::FDEFINITION-SET")),
        arguments: vec![
            Expr::Constant(Literal::Symbol(symbol("FUNCTION"))),
            Expr::Constant(Literal::Symbol(symbol("TARGET"))),
        ],
    };
    let lowered = lower_toplevel(&expression).expect("common-lisp function-cell store lowers");
    assert_verifies(&lowered.entry);
    assert!(any_op(
        &lowered.entry,
        |kind| matches!(kind, OpKind::StoreField { field, .. } if *field == ncl_object::symbol_offset::FUNCTION as u32)
    ));
}

#[test]
fn rest_and_aux_parameter_initialization_emits_runtime_rest_list_and_value() {
    let list = LambdaList {
        rest: Some(ParamName::Symbol(symbol("ARGS"))),
        aux: vec![ncl_compiler_front::AuxParam {
            name: ParamName::Symbol(symbol("AUX")),
            default: Some(Expr::Constant(Literal::fixnum(9))),
        }],
        ..LambdaList::new()
    };
    let expression = Expr::Lambda(Box::new(LambdaExpr {
        lambda_list: list,
        declarations: Vec::new(),
        docstring: None,
        body: vec![Expr::Constant(Literal::T)],
    }));
    let lowered = lower_toplevel(&expression).expect("rest aux lambda lowers");
    assert_verifies(&lowered.entry);
    let nested = &lowered.nested[0];
    assert_verifies(nested);
    assert!(any_op(
        nested,
        |kind| matches!(kind, OpKind::Builtin { name, args } if name == "make-rest-list" && args.len() == 2)
    ));
    assert!(
        nested
            .constants
            .iter()
            .any(|constant| matches!(constant, Constant::Fixnum(9)))
    );
}

#[test]
fn empty_setq_returns_nil_and_empty_progn_keeps_a_real_constant() {
    let setq = lower_toplevel(&Expr::Setq(Vec::new())).expect("empty setq lowers");
    assert_verifies(&setq.entry);
    assert!(
        setq.entry
            .constants
            .iter()
            .any(|constant| matches!(constant, Constant::Nil))
    );

    let progn = lower_toplevel(&Expr::Progn(Vec::new())).expect("empty progn lowers");
    assert_verifies(&progn.entry);
    assert!(
        progn
            .entry
            .constants
            .iter()
            .any(|constant| matches!(constant, Constant::Nil))
    );
}

#[test]
fn control_errors_keep_unbound_targets_typed_and_distinct() {
    let go = lower_toplevel(&Expr::Go {
        tag: symbol("NO-TAG"),
    })
    .unwrap_err();
    assert!(matches!(
        go,
        ncl_compiler_front::LowerError::EscapingControl { .. }
    ));
    assert!(go.to_string().contains("NO-TAG"));

    let return_from = lower_toplevel(&Expr::ReturnFrom {
        name: symbol("NO-BLOCK"),
        value: None,
    })
    .unwrap_err();
    assert!(matches!(
        return_from,
        ncl_compiler_front::LowerError::EscapingControl { .. }
    ));
    assert!(return_from.to_string().contains("NO-BLOCK"));
}
