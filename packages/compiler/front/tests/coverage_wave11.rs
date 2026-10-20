//! Eleventh-wave coverage for the remaining reachable front-lowering paths.
#![allow(clippy::expect_used, clippy::unwrap_used)]

#[path = "support/forms.rs"]
mod forms;

use forms::Fixture;
use ncl_compiler_front::{
    Expr, FunctionDesignator, LambdaExpr, LambdaList, LetBinding, Literal, LocalMacro, Operator,
    ParamName, SpecialForm, SymbolMacro, SymbolRef, lower_toplevel,
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

fn empty_lambda(body: Expr) -> Expr {
    Expr::Lambda(Box::new(LambdaExpr {
        lambda_list: LambdaList::new(),
        declarations: Vec::new(),
        docstring: None,
        body: vec![body],
    }))
}

#[test]
fn analysis_nested_return_crosses_locally_macrolet_and_symbol_macrolet() {
    let exit = symbol("EXIT");
    let nested_return = || Expr::ReturnFrom {
        name: exit.clone(),
        value: Some(Box::new(Expr::Constant(Literal::fixnum(44)))),
    };
    let locally = Expr::Locally {
        declarations: Vec::new(),
        body: vec![empty_lambda(Expr::Progn(vec![nested_return()]))],
    };
    let macrolet = Expr::Macrolet {
        definitions: vec![LocalMacro {
            name: symbol("M"),
            lambda_list: LambdaList::new(),
            declarations: Vec::new(),
            docstring: None,
            body: vec![Expr::Constant(Literal::T)],
        }],
        declarations: Vec::new(),
        body: vec![empty_lambda(Expr::Progn(vec![nested_return()]))],
    };
    let symbol_macrolet = Expr::SymbolMacrolet {
        definitions: vec![SymbolMacro {
            name: symbol("ALIAS"),
            expansion: Expr::Constant(Literal::Nil),
        }],
        declarations: Vec::new(),
        body: vec![empty_lambda(Expr::Progn(vec![nested_return()]))],
    };
    for body in [locally, macrolet, symbol_macrolet] {
        let lowered = lower_toplevel(&Expr::Block {
            name: exit.clone(),
            body: vec![body],
        })
        .expect("nested return wrapper lowers");
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
fn analysis_nested_return_in_call_argument_selects_escaping_block() {
    let exit = symbol("EXIT");
    let expression = Expr::Block {
        name: exit.clone(),
        body: vec![Expr::Call {
            operator: Operator::Name(symbol("LIST")),
            arguments: vec![empty_lambda(Expr::ReturnFrom {
                name: exit,
                value: Some(Box::new(Expr::Constant(Literal::fixnum(2)))),
            })],
        }],
    };
    let lowered = lower_toplevel(&expression).expect("call argument escape lowers");
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
        OpKind::CallClosure { .. }
    )));
}

#[test]
fn capture_shadowing_does_not_box_inner_binding_but_outer_capture_is_real() {
    let x = symbol("X");
    let expression = Expr::Let {
        sequential: false,
        bindings: vec![LetBinding {
            name: x.clone(),
            value: Some(Expr::Constant(Literal::fixnum(1))),
        }],
        declarations: Vec::new(),
        body: vec![
            Expr::Lambda(Box::new(LambdaExpr {
                lambda_list: LambdaList {
                    required: vec![ParamName::Symbol(x.clone())],
                    ..LambdaList::new()
                },
                declarations: Vec::new(),
                docstring: None,
                body: vec![Expr::Variable(x.clone())],
            })),
            empty_lambda(Expr::Variable(x)),
        ],
    };
    let lowered = lower_toplevel(&expression).expect("shadowed capture lowers");
    assert_verifies(&lowered.entry);
    assert_eq!(lowered.nested.len(), 2);
    assert!(any_op(&lowered.nested[0], |kind| matches!(
        kind,
        OpKind::LoadArg { index: 1 }
    )));
    assert!(any_op(&lowered.nested[1], |kind| matches!(
        kind,
        OpKind::LoadCapture { .. }
    )));
    assert!(!any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::MakeValueCell { .. }
    )));
}

#[test]
fn let_bindings_and_function_designator_loads_cover_value_and_function_namespaces() {
    let x = symbol("X");
    let expression = Expr::Let {
        sequential: true,
        bindings: vec![LetBinding {
            name: x.clone(),
            value: None,
        }],
        declarations: Vec::new(),
        body: vec![Expr::Progn(vec![
            Expr::Function(FunctionDesignator::Name(symbol("EXTERNAL"))),
            Expr::Variable(x),
        ])],
    };
    let lowered = lower_toplevel(&expression).expect("let namespace lowering");
    assert_verifies(&lowered.entry);
    assert!(
        lowered
            .entry
            .constants
            .iter()
            .any(|constant| matches!(constant, Constant::Nil))
    );
    assert!(any_op(
        &lowered.entry,
        |kind| matches!(kind, OpKind::LoadField { field, .. } if *field == ncl_object::symbol_offset::FUNCTION as u32)
    ));
}

#[test]
fn primitive_call_families_emit_builtin_ops_with_their_actual_arity_rules() {
    for (name, args) in [
        ("+", 2_usize),
        ("-", 2),
        ("*", 2),
        ("<", 2),
        ("CAR", 1),
        ("CONS", 2),
    ] {
        let expression = Expr::Call {
            operator: Operator::Name(SymbolRef::interned("COMMON-LISP", name)),
            arguments: (0..args)
                .map(|value| Expr::Constant(Literal::fixnum(value as i64)))
                .collect(),
        };
        let lowered = lower_toplevel(&expression).expect("primitive call lowers");
        assert_verifies(&lowered.entry);
        assert!(any_op(
            &lowered.entry,
            |kind| matches!(kind, OpKind::Builtin { name: actual, args: values } if actual == name && values.len() == args)
        ));
    }
}

#[test]
fn required_optional_and_aux_parameter_slots_have_concrete_prologue_values() {
    let required = symbol("REQUIRED");
    let optional = symbol("OPTIONAL");
    let aux = symbol("AUX");
    let list = LambdaList {
        required: vec![ParamName::Symbol(required)],
        optional: vec![ncl_compiler_front::OptionalParam {
            name: ParamName::Symbol(optional),
            default: Some(Expr::Constant(Literal::fixnum(6))),
            supplied_p: None,
        }],
        aux: vec![ncl_compiler_front::AuxParam {
            name: ParamName::Symbol(aux),
            default: Some(Expr::Constant(Literal::fixnum(7))),
        }],
        ..LambdaList::new()
    };
    let expression = Expr::Lambda(Box::new(LambdaExpr {
        lambda_list: list,
        declarations: Vec::new(),
        docstring: None,
        body: vec![Expr::Constant(Literal::T)],
    }));
    let lowered = lower_toplevel(&expression).expect("parameter prologue lowers");
    assert_verifies(&lowered.entry);
    let nested = &lowered.nested[0];
    assert_verifies(nested);
    assert!(any_op(nested, |kind| matches!(
        kind,
        OpKind::LoadArg { index: 1 }
    )));
    assert!(any_op(nested, |kind| matches!(
        kind,
        OpKind::Compare { .. }
    )));
    assert!(any_terminator(nested, |term| matches!(
        term,
        Terminator::Branch { .. }
    )));
    assert!(
        nested
            .constants
            .iter()
            .any(|constant| matches!(constant, Constant::Fixnum(6)))
    );
    assert!(
        nested
            .constants
            .iter()
            .any(|constant| matches!(constant, Constant::Fixnum(7)))
    );
}

#[test]
fn caught_throw_and_normal_unwind_have_distinct_control_effects() {
    let tag = Expr::Constant(Literal::Symbol(symbol("TAG")));
    let caught = Expr::Catch {
        tag: Box::new(tag.clone()),
        body: vec![Expr::Throw {
            tag: Box::new(tag),
            value: Box::new(Expr::Constant(Literal::fixnum(8))),
        }],
    };
    let caught = lower_toplevel(&caught).expect("caught throw lowers");
    assert_verifies(&caught.entry);
    assert!(any_op(
        &caught.entry,
        |kind| matches!(kind, OpKind::Builtin { name, .. } if name == "throw")
    ));
    assert!(any_terminator(&caught.entry, |term| matches!(
        term,
        Terminator::Throw { .. }
    )));
    assert!(
        caught
            .entry
            .handler_regions
            .iter()
            .any(|region| region.kind == HandlerKind::Catch)
    );

    let unwind = Expr::UnwindProtect {
        protected: Box::new(Expr::Constant(Literal::fixnum(9))),
        cleanup: vec![Expr::Setq(vec![(
            symbol("CLEAN"),
            Expr::Constant(Literal::T),
        )])],
    };
    let unwind = lower_toplevel(&unwind).expect("unwind cleanup lowers");
    assert_verifies(&unwind.entry);
    assert!(
        unwind
            .entry
            .handler_regions
            .iter()
            .any(|region| region.kind == HandlerKind::UnwindProtect)
    );
    assert!(any_op(&unwind.entry, |kind| matches!(
        kind,
        OpKind::StoreField { .. }
    )));
}

#[test]
fn special_parser_valid_forms_and_malformed_function_are_distinct() {
    let mut fixture = Fixture::new();
    let true_symbol = fixture.cl("T");
    let if_form = fixture.form("IF", &[true_symbol, ncl_object::Word::NIL]);
    let expanded = fixture.expand(if_form).expect("if expands");
    assert!(matches!(
        expanded,
        Expr::If {
            otherwise: None,
            ..
        }
    ));

    let bad_function = fixture.form("FUNCTION", &[ncl_object::Word::NIL, ncl_object::Word::NIL]);
    let error = fixture.expand(bad_function).unwrap_err();
    assert!(matches!(
        error,
        ncl_compiler_front::FrontError::WrongNumberOfForms { .. }
            | ncl_compiler_front::FrontError::MalformedForm { .. }
    ));
    assert!(error.to_string().contains("function"));
    assert_eq!(
        SpecialForm::from_symbol(&SymbolRef::interned("COMMON-LISP", "IF")),
        Some(SpecialForm::If)
    );
}
