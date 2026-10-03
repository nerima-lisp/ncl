//! Seventh-wave coverage for recursive analysis, lowering adapters, and parser edges.
#![allow(clippy::expect_used, clippy::unwrap_used)]

#[path = "support/forms.rs"]
mod forms;

use forms::Fixture;
use ncl_compiler_front::{
    Expr, FunctionDesignator, LambdaExpr, LambdaList, LetBinding, Literal, LowerError, Operator,
    SpecialForm, SymbolRef, TagbodyItem, lower_toplevel,
};
use ncl_ir::{Constant, Function, HandlerKind, OpKind, Terminator, verify};

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

fn empty_lambda(body: Expr) -> LambdaExpr {
    LambdaExpr {
        lambda_list: LambdaList::new(),
        declarations: Vec::new(),
        docstring: None,
        body: vec![body],
    }
}

#[test]
fn nested_analysis_handles_function_designator_and_tagbody_return_paths() {
    let exit = symbol("COMMON-LISP-USER", "EXIT");
    let nested_return = Expr::ReturnFrom {
        name: exit.clone(),
        value: Some(Box::new(Expr::Constant(Literal::fixnum(10)))),
    };
    let function_form = Expr::Function(FunctionDesignator::Lambda(Box::new(empty_lambda(
        Expr::Progn(vec![nested_return]),
    ))));
    let block = Expr::Block {
        name: exit,
        body: vec![Expr::Tagbody(vec![
            TagbodyItem::Tag(symbol("COMMON-LISP-USER", "START")),
            TagbodyItem::Form(function_form),
        ])],
    };
    let lowered = lower_toplevel(&block).expect("nested return analysis lowers");
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
fn lexical_let_star_and_global_setq_keep_distinct_storage_behavior() {
    let x = symbol("COMMON-LISP-USER", "X");
    let y = symbol("COMMON-LISP-USER", "Y");
    let expression = Expr::Let {
        sequential: true,
        bindings: vec![
            LetBinding {
                name: x.clone(),
                value: Some(Expr::Constant(Literal::fixnum(2))),
            },
            LetBinding {
                name: y.clone(),
                value: Some(Expr::Variable(x.clone())),
            },
        ],
        declarations: Vec::new(),
        body: vec![
            Expr::Setq(vec![(x.clone(), Expr::Variable(y))]),
            Expr::Variable(x),
        ],
    };
    let lowered = lower_toplevel(&expression).expect("let* lowering");
    assert_verifies(&lowered.entry);
    assert!(!any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::StoreField { .. }
    )));
    assert!(
        lowered
            .entry
            .constants
            .iter()
            .any(|constant| matches!(constant, Constant::Fixnum(2)))
    );

    let global = Expr::Setq(vec![(
        symbol("COMMON-LISP-USER", "GLOBAL"),
        Expr::Constant(Literal::fixnum(3)),
    )]);
    let global_lowered = lower_toplevel(&global).expect("global setq lowering");
    assert_verifies(&global_lowered.entry);
    assert!(any_op(
        &global_lowered.entry,
        |kind| matches!(kind, OpKind::StoreField { field, .. } if *field == u32::try_from(ncl_object::symbol_offset::VALUE).unwrap())
    ));
}

#[test]
fn control_normal_paths_close_handlers_and_preserve_multiple_values() {
    let unwind = Expr::UnwindProtect {
        protected: Box::new(Expr::Constant(Literal::fixnum(4))),
        cleanup: vec![Expr::Constant(Literal::fixnum(5))],
    };
    let lowered = lower_toplevel(&unwind).expect("normal unwind path lowers");
    assert_verifies(&lowered.entry);
    assert_eq!(
        lowered.entry.handler_regions[0].kind,
        HandlerKind::UnwindProtect
    );
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::EnterHandler { .. }
    )));
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::LeaveHandler { .. }
    )));
    assert!(any_terminator(&lowered.entry, |term| matches!(
        term,
        Terminator::Jump { .. }
    )));

    let values = Expr::MultipleValueProg1 {
        first: Box::new(Expr::Constant(Literal::T)),
        forms: vec![
            Expr::Constant(Literal::Nil),
            Expr::Constant(Literal::fixnum(8)),
        ],
    };
    let values_lowered = lower_toplevel(&values).expect("multiple values lower");
    assert_verifies(&values_lowered.entry);
    assert!(any_op(
        &values_lowered.entry,
        |kind| matches!(kind, OpKind::SetMultipleValues { values } if values.len() == 1)
    ));
}

#[test]
fn closure_designator_detection_covers_progn_and_eval_wrappers() {
    let inner = Expr::Lambda(Box::new(empty_lambda(Expr::Constant(Literal::fixnum(1)))));
    let designator = Expr::Progn(vec![
        Expr::Constant(Literal::Nil),
        Expr::The {
            type_specifier: ncl_compiler_front::TypeSpecifier::new(Literal::T),
            value: Box::new(Expr::LoadTimeValue {
                form: Box::new(inner),
                read_only: true,
            }),
        },
    ]);
    let expression = Expr::Call {
        operator: Operator::Name(symbol("COMMON-LISP", "FUNCALL")),
        arguments: vec![designator],
    };
    let lowered = lower_toplevel(&expression).expect("wrapped funcall lowers");
    assert_verifies(&lowered.entry);
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::MakeClosure { .. }
    )));
    assert!(any_op(
        &lowered.entry,
        |kind| matches!(kind, OpKind::CallClosure { args, .. } if args.len() == 1)
    ));
}

#[test]
fn control_errors_report_unbound_go_throw_and_return_targets() {
    for expression in [
        Expr::Go {
            tag: symbol("COMMON-LISP-USER", "MISSING-TAG"),
        },
        Expr::ReturnFrom {
            name: symbol("COMMON-LISP-USER", "MISSING-BLOCK"),
            value: None,
        },
    ] {
        let error = lower_toplevel(&expression).unwrap_err();
        assert!(matches!(error, LowerError::EscapingControl { .. }));
        assert!(!error.to_string().is_empty());
    }

    let tag = Expr::Constant(Literal::Symbol(symbol("COMMON-LISP-USER", "TAG")));
    let throw = Expr::Throw {
        tag: Box::new(tag.clone()),
        value: Box::new(Expr::Constant(Literal::fixnum(1))),
    };
    let lowered = lower_toplevel(&Expr::Catch {
        tag: Box::new(tag),
        body: vec![throw],
    })
    .expect("caught throw lowers to IR");
    assert_verifies(&lowered.entry);
    assert!(
        lowered
            .entry
            .handler_regions
            .iter()
            .any(|region| region.kind == HandlerKind::Catch)
    );
    assert!(any_op(
        &lowered.entry,
        |kind| matches!(kind, OpKind::Builtin { name, .. } if name == "throw")
    ));
    assert!(any_terminator(&lowered.entry, |term| matches!(
        term,
        Terminator::Throw { .. }
    )));
}

#[test]
fn special_parser_rejects_empty_required_shapes_with_typed_errors() {
    let mut fixture = Fixture::new();
    for (operator, expected_name) in [
        ("BLOCK", "block"),
        ("RETURN-FROM", "return-from"),
        ("GO", "go"),
        ("CATCH", "catch"),
        ("THROW", "throw"),
        ("UNWIND-PROTECT", "unwind-protect"),
        ("IF", "if"),
        ("LET", "let"),
        ("PROGV", "progv"),
        ("FUNCTION", "function"),
        ("QUOTE", "quote"),
        ("THE", "the"),
        ("SETQ", "setq"),
    ] {
        let form = fixture.form(operator, &[]);
        let error = fixture.expand(form).unwrap_err();
        assert!(matches!(
            error,
            ncl_compiler_front::FrontError::WrongNumberOfForms { .. }
                | ncl_compiler_front::FrontError::MalformedForm { .. }
        ));
        assert!(
            error
                .to_string()
                .to_ascii_lowercase()
                .contains(expected_name)
        );
    }
}

#[test]
fn special_form_resolution_separates_standard_extensions_and_unknowns() {
    let standard = [
        ("BLOCK", SpecialForm::Block),
        ("LABELS", SpecialForm::Labels),
        ("UNWIND-PROTECT", SpecialForm::UnwindProtect),
    ];
    for (name, expected) in standard {
        assert_eq!(
            SpecialForm::from_symbol(&symbol("COMMON-LISP", name)),
            Some(expected)
        );
        assert_eq!(expected.name(), name.to_ascii_lowercase());
    }
    assert_eq!(
        SpecialForm::from_symbol(&symbol("NCL-SYS", "%PRIMITIVE")),
        Some(SpecialForm::Primitive)
    );
    assert_eq!(
        SpecialForm::from_symbol(&symbol("NCL-SYS", "NLX-PROTECT")),
        Some(SpecialForm::NlxProtect)
    );
    assert_eq!(
        SpecialForm::from_symbol(&symbol("NCL-EXT", "TRULY-THE")),
        Some(SpecialForm::TrulyThe)
    );
    assert_eq!(SpecialForm::from_symbol(&symbol("OTHER", "BLOCK")), None);
}
