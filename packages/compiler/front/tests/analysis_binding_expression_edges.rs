//! Sixth-wave coverage for remaining analysis, binding, expression, and control paths.
#![allow(clippy::expect_used, clippy::unwrap_used)]

#[path = "support/forms.rs"]
mod forms;

use forms::Fixture;
use ncl_compiler_front::{
    EvalSituation, Expr, FunctionDesignator, LambdaExpr, LambdaList, Literal, LowerError, Operator,
    SpecialForm, SymbolRef, lower_toplevel,
};
use ncl_ir::{Function, HandlerKind, OpKind, Terminator, verify};

fn symbol(package: &str, name: &str) -> SymbolRef {
    SymbolRef::interned(package, name)
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

const fn lambda(body: Vec<Expr>) -> LambdaExpr {
    LambdaExpr {
        lambda_list: LambdaList::new(),
        declarations: Vec::new(),
        docstring: None,
        body,
    }
}

#[test]
fn analysis_recurses_through_catch_progv_and_tagbody_before_escaping() {
    let exit = symbol("COMMON-LISP-USER", "EXIT");
    let return_from = || Expr::ReturnFrom {
        name: exit.clone(),
        value: Some(Box::new(Expr::Constant(Literal::fixnum(6)))),
    };
    let body = vec![
        Expr::Catch {
            tag: Box::new(Expr::Constant(Literal::Symbol(symbol(
                "COMMON-LISP-USER",
                "TAG",
            )))),
            body: vec![return_from()],
        },
        Expr::Progv {
            symbols: Box::new(Expr::Constant(Literal::Nil)),
            values: Box::new(Expr::Constant(Literal::Nil)),
            body: vec![Expr::Tagbody(vec![
                ncl_compiler_front::TagbodyItem::Tag(symbol("COMMON-LISP-USER", "START")),
                ncl_compiler_front::TagbodyItem::Form(Expr::Constant(Literal::Nil)),
            ])],
        },
        Expr::UnwindProtect {
            protected: Box::new(Expr::Constant(Literal::Nil)),
            cleanup: vec![Expr::Constant(Literal::T)],
        },
    ];
    let lowered = lower_toplevel(&Expr::Block { name: exit, body }).expect("analysis lowers");
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
}

#[test]
fn progv_and_empty_multiple_value_call_emit_real_runtime_adapters() {
    let progv = Expr::Progv {
        symbols: Box::new(Expr::Constant(Literal::Nil)),
        values: Box::new(Expr::Constant(Literal::Nil)),
        body: vec![Expr::Constant(Literal::fixnum(12))],
    };
    let lowered = lower_toplevel(&progv).expect("progv lowers");
    assert_verifies(&lowered.entry);
    assert_eq!(lowered.entry.handler_regions.len(), 1);
    assert_eq!(lowered.entry.handler_regions[0].kind, HandlerKind::Progv);
    assert_eq!(lowered.entry.handler_regions[0].binding_targets.len(), 2);
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::EnterHandler { .. }
    )));
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::LeaveHandler { .. }
    )));

    let multiple = Expr::MultipleValueCall {
        function: Box::new(Expr::Function(FunctionDesignator::Name(symbol(
            "COMMON-LISP",
            "LIST",
        )))),
        arguments: Vec::new(),
    };
    let multiple_lowered = lower_toplevel(&multiple).expect("empty multiple-value-call lowers");
    assert_verifies(&multiple_lowered.entry);
    assert!(any_op(
        &multiple_lowered.entry,
        |kind| matches!(kind, OpKind::CallClosure { args, .. } if args.len() == 2)
    ));
    assert!(any_op(
        &multiple_lowered.entry,
        |kind| matches!(kind, OpKind::LoadField { field, .. } if *field == u32::try_from(ncl_object::symbol_offset::FUNCTION).unwrap())
    ));
}

#[test]
fn terminated_call_argument_does_not_lower_unreachable_siblings() {
    let block_name = symbol("COMMON-LISP-USER", "DONE");
    let expression = Expr::Block {
        name: block_name.clone(),
        body: vec![Expr::Call {
            operator: Operator::Name(symbol("COMMON-LISP", "LIST")),
            arguments: vec![
                Expr::ReturnFrom {
                    name: block_name,
                    value: None,
                },
                Expr::Constant(Literal::Array {
                    dimensions: vec![1],
                    element_type: ncl_object::ArrayElementType::T,
                    elements: vec![Literal::fixnum(1)],
                }),
            ],
        }],
    };
    let lowered = lower_toplevel(&expression).expect("terminated argument lowers");
    assert_verifies(&lowered.entry);
    assert!(any_terminator(&lowered.entry, |term| matches!(
        term,
        Terminator::Jump { .. }
    )));
    assert!(
        !lowered
            .entry
            .constants
            .iter()
            .any(|constant| { matches!(constant, ncl_ir::Constant::StringBytes(_)) })
    );
}

#[test]
fn eval_when_and_load_time_wrappers_preserve_nil_and_runtime_values() {
    let compile_only = Expr::EvalWhen {
        situations: vec![EvalSituation::CompileToplevel],
        body: vec![Expr::Constant(Literal::fixnum(99))],
    };
    let compile_lowered = lower_toplevel(&compile_only).expect("compile-only eval-when lowers");
    assert_verifies(&compile_lowered.entry);
    assert!(
        compile_lowered
            .entry
            .constants
            .iter()
            .any(|constant| matches!(constant, ncl_ir::Constant::Nil))
    );

    let runtime = Expr::EvalWhen {
        situations: vec![EvalSituation::Eval],
        body: vec![Expr::LoadTimeValue {
            form: Box::new(Expr::Constant(Literal::fixnum(77))),
            read_only: false,
        }],
    };
    let runtime_lowered = lower_toplevel(&runtime).expect("runtime eval-when lowers");
    assert_verifies(&runtime_lowered.entry);
    assert!(
        runtime_lowered
            .entry
            .constants
            .iter()
            .any(|constant| matches!(constant, ncl_ir::Constant::Fixnum(77)))
    );
}

#[test]
fn labels_self_capture_and_function_designator_load_are_distinct() {
    let name = symbol("COMMON-LISP-USER", "RECURSIVE");
    let labels = Expr::Labels {
        definitions: vec![ncl_compiler_front::LocalFunction {
            name: name.clone(),
            lambda: lambda(vec![Expr::Call {
                operator: Operator::Name(name.clone()),
                arguments: Vec::new(),
            }]),
        }],
        declarations: Vec::new(),
        body: vec![Expr::Function(FunctionDesignator::Name(name))],
    };
    let lowered = lower_toplevel(&labels).expect("self-referential labels lowers");
    assert_verifies(&lowered.entry);
    assert_eq!(lowered.nested.len(), 1);
    assert!(any_op(&lowered.nested[0], |kind| matches!(
        kind,
        OpKind::MakeClosure { .. }
    )));
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::MakeClosure { .. }
    )));
}

#[test]
fn special_form_resolution_respects_packages_and_reports_arity() {
    assert_eq!(
        SpecialForm::from_symbol(&symbol("COMMON-LISP", "BLOCK")),
        Some(SpecialForm::Block)
    );
    assert_eq!(
        SpecialForm::from_symbol(&symbol("NCL-SYS", "NLX-PROTECT")),
        Some(SpecialForm::NlxProtect)
    );
    assert_eq!(
        SpecialForm::from_symbol(&symbol("NCL-EXT", "TRULY-THE")),
        Some(SpecialForm::TrulyThe)
    );
    assert_eq!(
        SpecialForm::from_symbol(&symbol("COMMON-LISP-USER", "BLOCK")),
        None
    );
    assert_eq!(SpecialForm::NlxProtect.name(), "nlx-protect");

    let mut fixture = Fixture::new();
    let malformed = fixture.form("IF", &[]);
    let error = fixture.expand(malformed).unwrap_err();
    assert!(matches!(
        error,
        ncl_compiler_front::FrontError::WrongNumberOfForms { .. }
    ));
    assert!(error.to_string().contains("if"));
}

#[test]
fn unbound_control_and_unsupported_literal_errors_keep_their_types() {
    let name = symbol("COMMON-LISP-USER", "MISSING");
    let return_error = lower_toplevel(&Expr::ReturnFrom {
        name: name.clone(),
        value: None,
    })
    .unwrap_err();
    assert_eq!(return_error, LowerError::EscapingControl { name });

    let literal = lower_toplevel(&Expr::Constant(Literal::BitVector(vec![true, false])))
        .expect("bit-vector literal lowers");
    assert!(literal.entry.constants.iter().any(|constant| matches!(
        constant,
        ncl_ir::Constant::Array {
            element_type: ncl_ir::ArrayElementType::Bit,
            ..
        }
    )));
}
