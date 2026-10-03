//! Eighth-wave coverage for additional reachable lowering and parser branches.
#![allow(clippy::expect_used, clippy::unwrap_used)]

#[path = "support/forms.rs"]
mod forms;

use forms::Fixture;
use ncl_compiler_front::{
    EvalSituation, Expr, KeyParam, LambdaExpr, LambdaList, LetBinding, Literal, LowerError,
    Operator, OptionalParam, ParamName, SpecialForm, SymbolRef, TagbodyItem, lower_toplevel,
};
use ncl_ir::{Constant, Function, HandlerKind, OpKind, Terminator, verify};

fn user(name: &str) -> SymbolRef {
    SymbolRef::interned("COMMON-LISP-USER", name)
}

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

#[test]
fn non_escaping_block_carries_assigned_live_values_through_exit() {
    let x = user("X");
    let expression = Expr::Let {
        sequential: false,
        bindings: vec![LetBinding {
            name: x.clone(),
            value: Some(Expr::Constant(Literal::fixnum(1))),
        }],
        declarations: Vec::new(),
        body: vec![Expr::Block {
            name: user("DONE"),
            body: vec![
                Expr::Setq(vec![(x.clone(), Expr::Constant(Literal::fixnum(2)))]),
                Expr::Variable(x),
            ],
        }],
    };
    let lowered = lower_toplevel(&expression).expect("live block lowers");
    assert_verifies(&lowered.entry);
    assert!(lowered.entry.blocks.iter().any(|block| {
        matches!(block.terminator, Terminator::Jump { ref args, .. } if args.len() == 2)
    }));
    assert!(
        lowered
            .entry
            .constants
            .iter()
            .any(|constant| matches!(constant, Constant::Fixnum(2)))
    );
    assert!(lowered.entry.handler_regions.is_empty());
}

#[test]
fn tagbody_fast_path_handles_assignment_and_back_edge() {
    let x = user("X");
    let tag = user("LOOP");
    let expression = Expr::Let {
        sequential: false,
        bindings: vec![LetBinding {
            name: x.clone(),
            value: Some(Expr::Constant(Literal::Nil)),
        }],
        declarations: Vec::new(),
        body: vec![Expr::Tagbody(vec![
            TagbodyItem::Tag(tag.clone()),
            TagbodyItem::Form(Expr::Setq(vec![(x, Expr::Constant(Literal::T))])),
            TagbodyItem::Form(Expr::Go { tag }),
        ])],
    };
    let lowered = lower_toplevel(&expression).expect("assigned tagbody lowers");
    assert_verifies(&lowered.entry);
    assert!(lowered.entry.handler_regions.is_empty());
    assert!(any_terminator(&lowered.entry, |term| matches!(
        term,
        Terminator::Jump { .. }
    )));
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::Safepoint
    )));
}

#[test]
fn symbol_and_function_cells_cover_load_and_store_variants() {
    let value = Expr::Call {
        operator: Operator::Name(SymbolRef::interned("COMMON-LISP", "symbol-value")),
        arguments: vec![Expr::Constant(Literal::Symbol(user("X")))],
    };
    let loaded = lower_toplevel(&value).expect("symbol-value lowers");
    assert_verifies(&loaded.entry);
    assert!(any_op(
        &loaded.entry,
        |kind| matches!(kind, OpKind::LoadField { field, .. } if *field == u32::try_from(ncl_object::symbol_offset::VALUE).unwrap())
    ));

    let stored = Expr::Call {
        operator: Operator::Name(SymbolRef::interned("COMMON-LISP", "set-symbol-value")),
        arguments: vec![
            Expr::Constant(Literal::Symbol(user("X"))),
            Expr::Constant(Literal::fixnum(7)),
        ],
    };
    let stored = lower_toplevel(&stored).expect("set-symbol-value lowers");
    assert_verifies(&stored.entry);
    assert!(any_op(
        &stored.entry,
        |kind| matches!(kind, OpKind::StoreField { field, .. } if *field == u32::try_from(ncl_object::symbol_offset::VALUE).unwrap())
    ));

    let function_store = Expr::Call {
        operator: Operator::Name(SymbolRef::interned("NCL", "FDEFINITION-SET")),
        arguments: vec![
            Expr::Constant(Literal::Symbol(user("F"))),
            Expr::Constant(Literal::Symbol(user("G"))),
        ],
    };
    let function_store = lower_toplevel(&function_store).expect("function-cell store lowers");
    assert_verifies(&function_store.entry);
    assert!(any_op(
        &function_store.entry,
        |kind| matches!(kind, OpKind::StoreField { field, .. } if *field == u32::try_from(ncl_object::symbol_offset::FUNCTION).unwrap())
    ));
}

#[test]
fn parameter_paths_without_defaults_still_bind_supplied_and_keyword_values() {
    let required = user("REQUIRED");
    let optional = user("OPTIONAL");
    let supplied = user("SUPPLIED");
    let keyword_name = user("VALUE");
    let list = LambdaList {
        required: vec![ParamName::Symbol(required)],
        optional: vec![OptionalParam {
            name: ParamName::Symbol(optional),
            default: None,
            supplied_p: Some(ParamName::Symbol(supplied)),
        }],
        keys: vec![KeyParam {
            keyword: SymbolRef::interned("KEYWORD", "VALUE"),
            name: ParamName::Symbol(keyword_name.clone()),
            default: None,
            supplied_p: None,
        }],
        allow_other_keys: true,
        ..LambdaList::new()
    };
    let expression = Expr::Call {
        operator: Operator::Lambda(Box::new(LambdaExpr {
            lambda_list: list,
            declarations: Vec::new(),
            docstring: None,
            body: vec![Expr::Variable(keyword_name)],
        })),
        arguments: vec![Expr::Constant(Literal::fixnum(1))],
    };
    let lowered = lower_toplevel(&expression).expect("parameter paths lower");
    assert_verifies(&lowered.entry);
    let nested = &lowered.nested[0];
    assert_verifies(nested);
    assert!(any_op(
        nested,
        |kind| matches!(kind, OpKind::Builtin { name, .. } if name == "check-keywords")
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
fn eval_when_branches_and_wrappers_preserve_their_actual_constants() {
    let compile = Expr::EvalWhen {
        situations: vec![EvalSituation::CompileToplevel, EvalSituation::LoadToplevel],
        body: vec![Expr::Constant(Literal::fixnum(100))],
    };
    let compile_lowered = lower_toplevel(&compile).expect("non-runtime eval-when lowers");
    assert_verifies(&compile_lowered.entry);
    assert!(
        compile_lowered
            .entry
            .constants
            .iter()
            .any(|constant| matches!(constant, Constant::Nil))
    );
    assert!(
        !compile_lowered
            .entry
            .constants
            .iter()
            .any(|constant| matches!(constant, Constant::Fixnum(100)))
    );

    let execute = Expr::EvalWhen {
        situations: vec![EvalSituation::Execute],
        body: vec![Expr::The {
            type_specifier: ncl_compiler_front::TypeSpecifier::new(Literal::T),
            value: Box::new(Expr::Constant(Literal::fixnum(101))),
        }],
    };
    let execute_lowered = lower_toplevel(&execute).expect("execute eval-when lowers");
    assert_verifies(&execute_lowered.entry);
    assert!(
        execute_lowered
            .entry
            .constants
            .iter()
            .any(|constant| matches!(constant, Constant::Fixnum(101)))
    );
}

#[test]
fn control_error_variants_and_caught_throw_have_stable_results() {
    let missing = lower_toplevel(&Expr::Go {
        tag: user("NO-TAG"),
    })
    .unwrap_err();
    assert!(matches!(missing, LowerError::EscapingControl { .. }));
    assert!(missing.to_string().contains("NO-TAG"));

    let tag = Expr::Constant(Literal::Symbol(user("TAG")));
    let caught = Expr::Catch {
        tag: Box::new(tag.clone()),
        body: vec![Expr::Throw {
            tag: Box::new(tag),
            value: Box::new(Expr::Constant(Literal::fixnum(33))),
        }],
    };
    let lowered = lower_toplevel(&caught).expect("caught throw lowers");
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
fn special_forms_reject_unknown_eval_situations_and_resolve_extensions() {
    let mut fixture = Fixture::new();
    let unknown = fixture.keyword("NOT-A-SITUATION");
    let situations = fixture.list(&[unknown]);
    let form = fixture.form("EVAL-WHEN", &[situations, ncl_object::Word::NIL]);
    let error = fixture.expand(form).unwrap_err();
    assert!(matches!(
        error,
        ncl_compiler_front::FrontError::MalformedForm { .. }
    ));
    assert!(error.to_string().contains("eval-when"));

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
    assert_eq!(SpecialForm::from_symbol(&symbol("OTHER", "IF")), None);
}

#[test]
fn unsupported_parameter_pattern_remains_a_typed_lowering_error() {
    let expression = Expr::Lambda(Box::new(LambdaExpr {
        lambda_list: LambdaList {
            required: vec![ParamName::Pattern(Box::new(LambdaList::new()))],
            ..LambdaList::new()
        },
        declarations: Vec::new(),
        docstring: None,
        body: vec![Expr::Constant(Literal::Nil)],
    }));
    let error = lower_toplevel(&expression).unwrap_err();
    assert_eq!(
        error,
        LowerError::UnsupportedLambdaList {
            feature: "destructuring parameter"
        }
    );
    assert_eq!(
        error.to_string(),
        "unsupported lambda list feature: destructuring parameter"
    );
}
