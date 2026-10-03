//! Third-wave coverage for lowering control flow, bindings, calls, and literals.
#![allow(clippy::expect_used, clippy::unwrap_used)]

#[path = "support/forms.rs"]
mod forms;

use forms::Fixture;
use ncl_compiler_front::{Expr, Literal, NumberLiteral, Operator, SymbolRef, lower_toplevel};
use ncl_ir::{Constant, Function, HandlerKind, OpKind, Terminator, verify};

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

fn symbol(package: &str, name: &str) -> SymbolRef {
    SymbolRef::interned(package, name)
}

#[test]
fn local_and_recursive_functions_make_real_closures_and_calls() {
    let mut fixture = Fixture::new();
    let local = fixture.user("LOCAL");
    let x = fixture.user("X");
    let local_list = fixture.list(&[x]);
    let local_body = fixture.form("+", &[x, ncl_object::Word::fixnum(1)]);
    let local_definition = fixture.list(&[local, local_list, local_body]);
    let definitions = fixture.list(&[local_definition]);
    let call = fixture.form("LOCAL", &[ncl_object::Word::fixnum(3)]);
    let form = fixture.form("FLET", &[definitions, call]);
    let lowered = lower_toplevel(&fixture.expand(form).unwrap()).unwrap();

    assert_verifies(&lowered.entry);
    assert_eq!(lowered.nested.len(), 1);
    assert_verifies(&lowered.nested[0]);
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::MakeClosure { .. }
    )));
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::CallClosure { .. }
    )));

    let mut recursive_fixture = Fixture::new();
    let rec = recursive_fixture.user("REC");
    let n = recursive_fixture.user("N");
    let recursive_list = recursive_fixture.list(&[n]);
    let recursive_call = recursive_fixture.form("REC", &[n]);
    let recursive_body = recursive_fixture.form("IF", &[n, recursive_call, ncl_object::Word::NIL]);
    let recursive_definition = recursive_fixture.list(&[rec, recursive_list, recursive_body]);
    let recursive_definitions = recursive_fixture.list(&[recursive_definition]);
    let true_symbol = recursive_fixture.cl("T");
    let recursive_call_top = recursive_fixture.form("REC", &[true_symbol]);
    let recursive_form =
        recursive_fixture.form("LABELS", &[recursive_definitions, recursive_call_top]);
    let recursive = lower_toplevel(&recursive_fixture.expand(recursive_form).unwrap()).unwrap();
    assert_verifies(&recursive.entry);
    assert!(!recursive.nested.is_empty());
    assert!(any_op(&recursive.entry, |kind| matches!(
        kind,
        OpKind::MakeClosure { .. }
    )));
}

#[test]
fn control_forms_emit_handlers_and_branching_terminators() {
    let mut fixture = Fixture::new();
    let tag = fixture.user("TAG");
    let quoted_tag = fixture.form("QUOTE", &[tag]);
    let throw_tag = fixture.form("QUOTE", &[tag]);
    let throw = fixture.form("THROW", &[throw_tag, ncl_object::Word::fixnum(8)]);
    let catch = fixture.form("CATCH", &[quoted_tag, throw]);
    let caught = lower_toplevel(&fixture.expand(catch).unwrap()).unwrap();
    assert_verifies(&caught.entry);
    assert_eq!(caught.entry.handler_regions[0].kind, HandlerKind::Catch);
    assert!(any_terminator(&caught.entry, |term| matches!(
        term,
        Terminator::Throw { .. }
    )));

    let mut unwind_fixture = Fixture::new();
    let block_name = unwind_fixture.user("DONE");
    let clean = unwind_fixture.user("CLEAN");
    let cleanup = unwind_fixture.form("SETQ", &[clean, ncl_object::Word::fixnum(1)]);
    let returned = unwind_fixture.form("RETURN-FROM", &[block_name, ncl_object::Word::fixnum(9)]);
    let protected = unwind_fixture.form("UNWIND-PROTECT", &[returned, cleanup]);
    let form = unwind_fixture.form("BLOCK", &[block_name, protected]);
    let lowered = lower_toplevel(&unwind_fixture.expand(form).unwrap()).unwrap();
    assert_verifies(&lowered.entry);
    assert!(
        lowered
            .entry
            .handler_regions
            .iter()
            .any(|region| region.kind == HandlerKind::UnwindProtect)
    );
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::LeaveHandler { .. }
    )));
}

#[test]
fn bindings_cover_lexical_global_and_multiple_value_paths() {
    let mut fixture = Fixture::new();
    let x = fixture.user("X");
    let binding = fixture.list(&[x, ncl_object::Word::fixnum(0)]);
    let bindings = fixture.list(&[binding]);
    let set = fixture.form("SETQ", &[x, ncl_object::Word::fixnum(4)]);
    let form = fixture.form("LET", &[bindings, set, x]);
    let lowered = lower_toplevel(&fixture.expand(form).unwrap()).unwrap();
    assert_verifies(&lowered.entry);
    assert!(
        lowered
            .entry
            .constants
            .iter()
            .any(|constant| matches!(constant, Constant::Fixnum(4)))
    );
    assert!(!any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::Store { .. }
    )));

    let global = Expr::Setq(vec![(
        symbol("COMMON-LISP-USER", "GLOBAL"),
        Expr::Constant(Literal::fixnum(4)),
    )]);
    let global_lowered = lower_toplevel(&global).unwrap();
    assert_verifies(&global_lowered.entry);
    assert!(any_op(
        &global_lowered.entry,
        |kind| matches!(kind, OpKind::StoreField { field, .. } if *field == u32::try_from(ncl_object::symbol_offset::VALUE).unwrap())
    ));

    let multiple = Expr::MultipleValueCall {
        function: Box::new(Expr::Function(
            ncl_compiler_front::FunctionDesignator::Name(symbol("COMMON-LISP", "+")),
        )),
        arguments: vec![
            Expr::Constant(Literal::fixnum(1)),
            Expr::Constant(Literal::fixnum(2)),
        ],
    };
    let multiple_lowered = lower_toplevel(&multiple).unwrap();
    assert_verifies(&multiple_lowered.entry);
    assert!(any_op(&multiple_lowered.entry, |kind| matches!(
        kind,
        OpKind::CallClosure { .. }
    )));
    assert!(multiple_lowered.entry.constants.iter().any(|constant| {
        matches!(constant, Constant::Symbol { package, name } if package == "NCL-EXT" && name == "MULTIPLE-VALUE-CALL-LIST")
    }));
}

#[test]
fn symbol_cell_operations_have_distinct_load_and_store_side_effects() {
    let value = Expr::Call {
        operator: Operator::Name(symbol("COMMON-LISP", "SYMBOL-VALUE")),
        arguments: vec![Expr::Constant(Literal::Symbol(symbol(
            "COMMON-LISP-USER",
            "X",
        )))],
    };
    let loaded = lower_toplevel(&value).unwrap();
    assert_verifies(&loaded.entry);
    assert!(any_op(
        &loaded.entry,
        |kind| matches!(kind, OpKind::LoadField { field, .. } if *field == u32::try_from(ncl_object::symbol_offset::VALUE).unwrap())
    ));

    let stored = Expr::Call {
        operator: Operator::Name(symbol("COMMON-LISP", "set-symbol-value")),
        arguments: vec![
            Expr::Constant(Literal::Symbol(symbol("COMMON-LISP-USER", "X"))),
            Expr::Constant(Literal::fixnum(12)),
        ],
    };
    let stored = lower_toplevel(&stored).unwrap();
    assert_verifies(&stored.entry);
    assert!(any_op(
        &stored.entry,
        |kind| matches!(kind, OpKind::StoreField { field, .. } if *field == u32::try_from(ncl_object::symbol_offset::VALUE).unwrap())
    ));
}

#[test]
fn expression_wrappers_and_builtins_preserve_values_in_verified_ir() {
    let expressions = [
        Expr::The {
            type_specifier: ncl_compiler_front::TypeSpecifier::new(Literal::T),
            value: Box::new(Expr::Constant(Literal::fixnum(3))),
        },
        Expr::LoadTimeValue {
            form: Box::new(Expr::Constant(Literal::fixnum(4))),
            read_only: true,
        },
        Expr::EvalWhen {
            situations: vec![ncl_compiler_front::EvalSituation::Execute],
            body: vec![Expr::Constant(Literal::fixnum(5))],
        },
        Expr::Locally {
            declarations: Vec::new(),
            body: vec![Expr::Constant(Literal::T)],
        },
    ];
    for expression in expressions {
        let lowered = lower_toplevel(&expression).unwrap();
        assert_verifies(&lowered.entry);
        assert!(!lowered.entry.blocks.is_empty());
    }

    for name in ["+", "-", "*", "<", "CONS", "CAR"] {
        let argument_count = if name == "CAR" { 1 } else { 2 };
        let arguments = (0..argument_count)
            .map(|value| Expr::Constant(Literal::fixnum(value)))
            .collect();
        let expression = Expr::Call {
            operator: Operator::Name(symbol("COMMON-LISP", name)),
            arguments,
        };
        let lowered = lower_toplevel(&expression).unwrap();
        assert_verifies(&lowered.entry);
        assert!(any_op(
            &lowered.entry,
            |kind| matches!(kind, OpKind::Builtin { name: actual, .. } if actual == name)
        ));
    }
}

#[test]
fn literal_lowering_covers_scalar_numbers_and_rejects_unrepresentable_data() {
    let expressions = [
        Expr::Constant(Literal::Character('λ' as u32)),
        Expr::Constant(Literal::Number(NumberLiteral::SingleFloat(1.5))),
        Expr::Constant(Literal::Number(NumberLiteral::Bignum {
            negative: true,
            limbs: vec![1, 2],
        })),
        Expr::Constant(Literal::Number(NumberLiteral::Ratio {
            numerator: Box::new(NumberLiteral::Fixnum(3)),
            denominator: Box::new(NumberLiteral::Fixnum(7)),
        })),
    ];
    for expression in expressions {
        let lowered = lower_toplevel(&expression).unwrap();
        assert_verifies(&lowered.entry);
        assert!(lowered.entry.constants.iter().any(|constant| {
            matches!(
                constant,
                Constant::Character(_)
                    | Constant::SingleFloat(_)
                    | Constant::Bignum { .. }
                    | Constant::Ratio { .. }
            )
        }));
    }

    for literal in [
        Literal::Array {
            dimensions: vec![1],
            element_type: ncl_object::ArrayElementType::T,
            elements: vec![Literal::fixnum(1)],
        },
        Literal::BitVector(vec![true, false]),
    ] {
        let lowered = lower_toplevel(&Expr::Constant(literal)).expect("literal lowers");
        assert_verifies(&lowered.entry);
        assert!(
            lowered
                .entry
                .constants
                .iter()
                .any(|constant| matches!(constant, Constant::Array { .. }))
        );
    }
}
