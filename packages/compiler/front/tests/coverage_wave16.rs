//! Sixteenth-wave coverage for lower analysis, captures, calls, and parameters.
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

fn op_count(function: &Function, predicate: impl Fn(&OpKind) -> bool) -> usize {
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
fn analysis_false_paths_walk_nested_control_without_creating_handlers() {
    let local = symbol("LOCAL");
    let tag = symbol("TAG");
    let function = LocalFunction {
        name: symbol("LOCAL-FUNCTION"),
        lambda: lambda(Expr::Constant(Literal::Nil), LambdaList::new()),
    };
    let expression = Expr::Block {
        name: local.clone(),
        body: vec![
            Expr::Let {
                sequential: false,
                bindings: vec![LetBinding {
                    name: local,
                    value: Some(Expr::If {
                        test: Box::new(Expr::Constant(Literal::T)),
                        then: Box::new(Expr::Constant(Literal::fixnum(1))),
                        otherwise: None,
                    }),
                }],
                declarations: Vec::new(),
                body: vec![Expr::Catch {
                    tag: Box::new(Expr::Constant(Literal::Nil)),
                    body: vec![Expr::Progv {
                        symbols: Box::new(Expr::Constant(Literal::Nil)),
                        values: Box::new(Expr::Constant(Literal::Nil)),
                        body: vec![Expr::Tagbody(vec![
                            ncl_compiler_front::TagbodyItem::Tag(tag.clone()),
                            ncl_compiler_front::TagbodyItem::Form(Expr::Constant(Literal::Nil)),
                        ])],
                    }],
                }],
            },
            Expr::Flet {
                definitions: vec![function],
                declarations: Vec::new(),
                body: vec![Expr::Constant(Literal::Nil)],
            },
            Expr::MultipleValueProg1 {
                first: Box::new(Expr::Constant(Literal::fixnum(2))),
                forms: vec![Expr::LoadTimeValue {
                    form: Box::new(Expr::The {
                        type_specifier: ncl_compiler_front::TypeSpecifier::new(Literal::T),
                        value: Box::new(Expr::Constant(Literal::Nil)),
                    }),
                    read_only: true,
                }],
            },
        ],
    };
    let lowered = lower_toplevel(&expression).expect("nonmatching control targets lower");
    assert_verifies(&lowered.entry);
    assert!(
        lowered
            .entry
            .handler_regions
            .iter()
            .any(|region| region.kind == ncl_ir::HandlerKind::Catch)
    );
    assert!(
        lowered
            .entry
            .handler_regions
            .iter()
            .any(|region| region.kind == ncl_ir::HandlerKind::Progv)
    );
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::SetMultipleValues { .. }
    )));
    assert!(any_terminator(&lowered.entry, |term| matches!(
        term,
        Terminator::Jump { .. }
    )));
}

#[test]
fn capture_binding_matrix_keeps_bound_names_out_and_free_names_in() {
    let outer = symbol("OUTER");
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
            default: None,
            supplied_p: Some(ParamName::Symbol(symbol("VALUE-P"))),
        }],
        aux: vec![AuxParam {
            name: ParamName::Symbol(symbol("AUX")),
            default: None,
        }],
        ..LambdaList::new()
    };
    let body = Expr::Progn(vec![
        Expr::Variable(outer.clone()),
        Expr::Variable(symbol("REQUIRED")),
        Expr::Variable(symbol("OPTIONAL")),
        Expr::Variable(symbol("OPTIONAL-P")),
        Expr::Variable(symbol("REST")),
        Expr::Variable(symbol("VALUE")),
        Expr::Variable(symbol("VALUE-P")),
        Expr::Variable(symbol("AUX")),
    ]);
    let expression = Expr::Let {
        sequential: false,
        bindings: vec![LetBinding {
            name: outer.clone(),
            value: Some(Expr::Constant(Literal::fixnum(11))),
        }],
        declarations: Vec::new(),
        body: vec![Expr::Lambda(Box::new(lambda(body, list)))],
    };
    let lowered = lower_toplevel(&expression).expect("capture binding matrix lowers");
    assert_verifies(&lowered.entry);
    assert_eq!(lowered.nested.len(), 1);
    let closure = lowered
        .entry
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .find_map(|op| match &op.kind {
            OpKind::MakeClosure { captures, .. } => Some(captures.len()),
            _ => None,
        })
        .expect("closure operation is emitted");
    assert_eq!(closure, 1);
    let nested = &lowered.nested[0];
    assert_verifies(nested);
    assert!(any_op(nested, |kind| matches!(
        kind,
        OpKind::LoadCapture { index: 0 }
    )));
    assert!(op_count(nested, |kind| matches!(kind, OpKind::LoadArg { .. })) >= 2);
}

#[test]
fn expression_designators_cover_local_lookup_lambda_operator_and_fallback_call() {
    let local_name = symbol("LOCAL");
    let local = LocalFunction {
        name: local_name.clone(),
        lambda: lambda(Expr::Constant(Literal::fixnum(3)), LambdaList::new()),
    };
    let local_designator = Expr::Function(FunctionDesignator::Name(local_name.clone()));
    let lambda_call = Expr::Call {
        operator: Operator::Lambda(Box::new(lambda(
            Expr::Constant(Literal::fixnum(4)),
            LambdaList::new(),
        ))),
        arguments: vec![Expr::Constant(Literal::Nil)],
    };
    let fallback = Expr::Call {
        operator: Operator::Name(symbol("UNKNOWN-FUNCTION")),
        arguments: vec![Expr::Constant(Literal::fixnum(1))],
    };
    let expression = Expr::Flet {
        definitions: vec![local],
        declarations: Vec::new(),
        body: vec![local_designator, lambda_call, fallback],
    };
    let lowered = lower_toplevel(&expression).expect("designator and call matrix lowers");
    assert_verifies(&lowered.entry);
    assert!(
        op_count(&lowered.entry, |kind| matches!(
            kind,
            OpKind::MakeClosure { .. }
        )) >= 2
    );
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::LoadField { field, .. } if *field == ncl_object::symbol_offset::FUNCTION as u32
    )));
    assert!(
        op_count(&lowered.entry, |kind| matches!(
            kind,
            OpKind::CallClosure { .. }
        )) >= 2
    );
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::Safepoint
    )));
}

#[test]
fn capture_analysis_boxes_only_the_assigned_value_seen_by_a_closure() {
    let captured = symbol("CAPTURED");
    let assigned = symbol("ASSIGNED");
    let expression = Expr::Let {
        sequential: false,
        bindings: vec![
            LetBinding {
                name: captured.clone(),
                value: Some(Expr::Constant(Literal::fixnum(1))),
            },
            LetBinding {
                name: assigned.clone(),
                value: Some(Expr::Constant(Literal::fixnum(2))),
            },
        ],
        declarations: Vec::new(),
        body: vec![
            Expr::Setq(vec![(captured.clone(), Expr::Constant(Literal::fixnum(4)))]),
            Expr::Setq(vec![(assigned.clone(), Expr::Constant(Literal::fixnum(3)))]),
            Expr::Lambda(Box::new(lambda(
                Expr::Variable(captured.clone()),
                LambdaList::new(),
            ))),
            Expr::Variable(assigned),
        ],
    };
    let lowered = lower_toplevel(&expression).expect("selective capture analysis lowers");
    assert_verifies(&lowered.entry);
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::MakeValueCell { .. }
    )));
    assert!(any_op(
        &lowered.entry,
        |kind| matches!(kind, OpKind::MakeClosure { captures, .. } if captures.len() == 1)
    ));
    assert!(any_op(&lowered.nested[0], |kind| matches!(
        kind,
        OpKind::LoadCapture { .. }
    )));
}

#[test]
fn optional_defaults_and_aux_nil_have_separate_prologue_values() {
    let list = LambdaList {
        required: vec![ParamName::Symbol(symbol("REQUIRED"))],
        optional: vec![
            OptionalParam {
                name: ParamName::Symbol(symbol("WITH-DEFAULT")),
                default: Some(Expr::Constant(Literal::fixnum(21))),
                supplied_p: None,
            },
            OptionalParam {
                name: ParamName::Symbol(symbol("WITHOUT-DEFAULT")),
                default: None,
                supplied_p: Some(ParamName::Symbol(symbol("WITHOUT-P"))),
            },
        ],
        aux: vec![AuxParam {
            name: ParamName::Symbol(symbol("AUX-NIL")),
            default: None,
        }],
        ..LambdaList::new()
    };
    let lowered = lower_toplevel(&Expr::Lambda(Box::new(lambda(
        Expr::Variable(symbol("WITHOUT-P")),
        list,
    ))))
    .expect("optional and aux prologue lowers");
    assert_verifies(&lowered.entry);
    let nested = &lowered.nested[0];
    assert_verifies(nested);
    assert_eq!(
        op_count(nested, |kind| matches!(kind, OpKind::Compare { .. })),
        2
    );
    assert_eq!(
        op_count(nested, |kind| matches!(kind, OpKind::LoadArg { index: 0 })),
        2
    );
    assert!(
        nested
            .constants
            .iter()
            .any(|constant| matches!(constant, Constant::Fixnum(21)))
    );
    assert!(any_op(nested, |kind| matches!(
        kind,
        OpKind::Convert { .. }
    )));
    assert!(any_terminator(nested, |term| matches!(
        term,
        Terminator::Branch { .. }
    )));
}
