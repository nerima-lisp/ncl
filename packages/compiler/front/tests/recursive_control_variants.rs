//! Ninth-wave coverage for reachable recursive walkers and control variants.
#![allow(clippy::expect_used, clippy::unwrap_used)]

#[path = "support/forms.rs"]
mod forms;

use forms::Fixture;
use ncl_compiler_front::{
    Expr, LambdaExpr, LambdaList, LetBinding, Literal, LocalMacro, Operator, OptionalParam,
    ParamName, SpecialForm, SymbolMacro, SymbolRef, TagbodyItem, lower_toplevel,
};
use ncl_ir::{Constant, Function, HandlerKind, OpKind, Terminator, verify};

fn user(name: &str) -> SymbolRef {
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

const fn lambda(body: Vec<Expr>, list: LambdaList) -> LambdaExpr {
    LambdaExpr {
        lambda_list: list,
        declarations: Vec::new(),
        docstring: None,
        body,
    }
}

#[test]
fn symbol_and_macrolet_walkers_preserve_nested_values_and_captures() {
    let variable = user("VALUE");
    let symbol_macro = Expr::SymbolMacrolet {
        definitions: vec![SymbolMacro {
            name: user("ALIAS"),
            expansion: Expr::Variable(variable.clone()),
        }],
        declarations: Vec::new(),
        body: vec![Expr::Lambda(Box::new(lambda(
            vec![Expr::Progn(vec![
                Expr::Variable(variable.clone()),
                Expr::Setq(vec![(variable.clone(), Expr::Constant(Literal::fixnum(2)))]),
            ])],
            LambdaList::new(),
        )))],
    };
    let local_macro = LocalMacro {
        name: user("LOCAL-MACRO"),
        lambda_list: LambdaList::new(),
        declarations: Vec::new(),
        docstring: None,
        body: vec![Expr::Constant(Literal::T)],
    };
    let expression = Expr::Let {
        sequential: false,
        bindings: vec![LetBinding {
            name: variable,
            value: Some(Expr::Constant(Literal::Nil)),
        }],
        declarations: Vec::new(),
        body: vec![
            symbol_macro,
            Expr::Macrolet {
                definitions: vec![local_macro],
                declarations: Vec::new(),
                body: vec![Expr::Constant(Literal::fixnum(3))],
            },
        ],
    };
    let lowered = lower_toplevel(&expression).expect("macrolet capture walk lowers");
    assert_verifies(&lowered.entry);
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::MakeValueCell { .. }
    )));
    assert!(lowered.nested.iter().any(|nested| {
        assert_verifies(nested);
        any_op(nested, |kind| matches!(kind, OpKind::LoadCapture { .. }))
    }));
    assert!(
        lowered
            .entry
            .constants
            .iter()
            .any(|constant| matches!(constant, Constant::Fixnum(3)))
    );
}

#[test]
fn normal_block_return_and_tagbody_paths_use_jumps_without_handlers() {
    let name = user("BLOCK-NAME");
    let block = Expr::Block {
        name: name.clone(),
        body: vec![Expr::ReturnFrom {
            name,
            value: Some(Box::new(Expr::Constant(Literal::fixnum(42)))),
        }],
    };
    let lowered = lower_toplevel(&block).expect("local block return lowers");
    assert_verifies(&lowered.entry);
    assert!(lowered.entry.handler_regions.is_empty());
    assert!(any_terminator(&lowered.entry, |term| matches!(
        term,
        Terminator::Jump { .. }
    )));

    let tag = user("NEXT");
    let tagbody = Expr::Tagbody(vec![
        TagbodyItem::Tag(tag.clone()),
        TagbodyItem::Form(Expr::Constant(Literal::Nil)),
        TagbodyItem::Form(Expr::Go { tag }),
    ]);
    let tagbody_lowered = lower_toplevel(&tagbody).expect("local tagbody lowers");
    assert_verifies(&tagbody_lowered.entry);
    assert!(tagbody_lowered.entry.handler_regions.is_empty());
    assert!(any_terminator(&tagbody_lowered.entry, |term| matches!(
        term,
        Terminator::Jump { .. }
    )));
}

#[test]
fn normal_catch_and_unwind_paths_leave_handlers_and_return_values() {
    let tag = Expr::Constant(Literal::Symbol(user("TAG")));
    let catch = Expr::Catch {
        tag: Box::new(tag),
        body: vec![Expr::Constant(Literal::fixnum(17))],
    };
    let caught = lower_toplevel(&catch).expect("normal catch lowers");
    assert_verifies(&caught.entry);
    assert_eq!(caught.entry.handler_regions[0].kind, HandlerKind::Catch);
    assert!(any_op(&caught.entry, |kind| matches!(
        kind,
        OpKind::EnterHandler { .. }
    )));
    assert!(any_op(&caught.entry, |kind| matches!(
        kind,
        OpKind::LeaveHandler { .. }
    )));

    let unwind = Expr::UnwindProtect {
        protected: Box::new(Expr::Constant(Literal::fixnum(18))),
        cleanup: vec![Expr::Constant(Literal::fixnum(19))],
    };
    let protected = lower_toplevel(&unwind).expect("normal unwind lowers");
    assert_verifies(&protected.entry);
    assert_eq!(
        protected.entry.handler_regions[0].kind,
        HandlerKind::UnwindProtect
    );
    assert!(
        protected
            .entry
            .constants
            .iter()
            .any(|constant| matches!(constant, Constant::Fixnum(19)))
    );
}

#[test]
fn function_designators_and_wrappers_select_the_expected_call_operation() {
    let named = Expr::Call {
        operator: Operator::Name(user("EXTERNAL")),
        arguments: vec![Expr::Constant(Literal::fixnum(1))],
    };
    let named_lowered = lower_toplevel(&named).expect("named call lowers");
    assert_verifies(&named_lowered.entry);
    assert!(any_op(
        &named_lowered.entry,
        |kind| matches!(kind, OpKind::LoadField { field, .. } if *field == u32::try_from(ncl_object::symbol_offset::FUNCTION).unwrap())
    ));
    assert!(any_op(
        &named_lowered.entry,
        |kind| matches!(kind, OpKind::CallClosure { args, .. } if args.len() == 2)
    ));

    let lambda_call = Expr::Call {
        operator: Operator::Lambda(Box::new(lambda(
            vec![Expr::Constant(Literal::fixnum(21))],
            LambdaList::new(),
        ))),
        arguments: Vec::new(),
    };
    let lambda_lowered = lower_toplevel(&lambda_call).expect("lambda call lowers");
    assert_verifies(&lambda_lowered.entry);
    assert!(any_op(&lambda_lowered.entry, |kind| matches!(
        kind,
        OpKind::MakeClosure { .. }
    )));
    assert!(any_op(
        &lambda_lowered.entry,
        |kind| matches!(kind, OpKind::CallClosure { args, .. } if args.len() == 1)
    ));
}

#[test]
fn optional_without_default_and_empty_aux_values_are_lowered_to_runtime_state() {
    let optional = user("OPTIONAL");
    let supplied = user("SUPPLIED");
    let aux = user("AUX");
    let list = LambdaList {
        optional: vec![OptionalParam {
            name: ParamName::Symbol(optional),
            default: None,
            supplied_p: Some(ParamName::Symbol(supplied)),
        }],
        aux: vec![ncl_compiler_front::AuxParam {
            name: ParamName::Symbol(aux),
            default: None,
        }],
        ..LambdaList::new()
    };
    let expression = Expr::Lambda(Box::new(lambda(vec![Expr::Constant(Literal::T)], list)));
    let lowered = lower_toplevel(&expression).expect("optional and aux lowering");
    assert_verifies(&lowered.entry);
    let nested = &lowered.nested[0];
    assert_verifies(nested);
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
            .any(|constant| matches!(constant, Constant::Nil))
    );
}

#[test]
fn eval_when_execute_and_compile_only_have_distinct_constant_results() {
    let compile = Expr::EvalWhen {
        situations: vec![ncl_compiler_front::EvalSituation::CompileToplevel],
        body: vec![Expr::Constant(Literal::fixnum(30))],
    };
    let compile_lowered = lower_toplevel(&compile).expect("compile eval-when");
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
            .any(|constant| matches!(constant, Constant::Fixnum(30)))
    );

    let execute = Expr::EvalWhen {
        situations: vec![ncl_compiler_front::EvalSituation::Execute],
        body: vec![Expr::Constant(Literal::fixnum(31))],
    };
    let execute_lowered = lower_toplevel(&execute).expect("execute eval-when");
    assert_verifies(&execute_lowered.entry);
    assert!(
        execute_lowered
            .entry
            .constants
            .iter()
            .any(|constant| matches!(constant, Constant::Fixnum(31)))
    );
}

#[test]
fn special_parser_reports_unknown_situations_and_invalid_function_forms() {
    let mut fixture = Fixture::new();
    let unknown = fixture.keyword("UNKNOWN");
    let situations = fixture.list(&[unknown]);
    let eval_when = fixture.form("EVAL-WHEN", &[situations, ncl_object::Word::NIL]);
    let eval_error = fixture.expand(eval_when).unwrap_err();
    assert!(matches!(
        eval_error,
        ncl_compiler_front::FrontError::MalformedForm { .. }
    ));
    assert!(eval_error.to_string().contains("eval-when"));

    let bad_function = fixture.form("FUNCTION", &[ncl_object::Word::NIL, ncl_object::Word::NIL]);
    let function_error = fixture.expand(bad_function).unwrap_err();
    assert!(matches!(
        function_error,
        ncl_compiler_front::FrontError::WrongNumberOfForms { .. }
            | ncl_compiler_front::FrontError::MalformedForm { .. }
    ));
    assert!(function_error.to_string().contains("function"));
}

#[test]
fn special_form_package_resolution_is_case_and_namespace_sensitive() {
    assert_eq!(
        SpecialForm::from_symbol(&SymbolRef::interned("COMMON-LISP", "IF")),
        Some(SpecialForm::If)
    );
    assert_eq!(
        SpecialForm::from_symbol(&SymbolRef::interned("NCL-SYS", "%PRIMITIVE")),
        Some(SpecialForm::Primitive)
    );
    assert_eq!(
        SpecialForm::from_symbol(&SymbolRef::interned("NCL-EXT", "TRULY-THE")),
        Some(SpecialForm::TrulyThe)
    );
    assert_eq!(
        SpecialForm::from_symbol(&SymbolRef::interned("COMMON-LISP-USER", "IF")),
        None
    );
    assert_eq!(SpecialForm::If.name(), "if");
}

#[test]
fn lowering_materializes_arrays_and_destructures_parameters() {
    let literal = lower_toplevel(&Expr::Constant(Literal::Array {
        dimensions: vec![1],
        element_type: ncl_object::ArrayElementType::T,
        elements: vec![Literal::fixnum(1)],
    }))
    .expect("array literal lowers");
    assert!(
        literal
            .entry
            .constants
            .iter()
            .any(|constant| matches!(constant, ncl_ir::Constant::Array { .. }))
    );

    let pattern = Expr::Lambda(Box::new(LambdaExpr {
        lambda_list: LambdaList {
            required: vec![ParamName::Pattern(Box::new(LambdaList {
                required: vec![ParamName::Symbol(user("X")), ParamName::Symbol(user("Y"))],
                ..LambdaList::new()
            }))],
            ..LambdaList::new()
        },
        declarations: Vec::new(),
        docstring: None,
        body: vec![Expr::Constant(Literal::Nil)],
    }));
    let lowered = lower_toplevel(&pattern).expect("destructuring parameter lowers");
    assert_verifies(&lowered.entry);
    let nested = lowered.nested.first().expect("nested lambda");
    assert!(any_op(nested, |kind| matches!(
        kind,
        OpKind::LoadArg { index: 1 }
    )));
    assert!(any_op(nested, |kind| matches!(
        kind,
        OpKind::Builtin { name, args } if name == "CAR" && args.len() == 1
    )));
    assert!(any_op(nested, |kind| matches!(
        kind,
        OpKind::Builtin { name, args } if name == "CDR" && args.len() == 1
    )));
}
