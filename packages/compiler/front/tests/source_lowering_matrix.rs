//! Twentieth-wave coverage for source-to-IR lowering boundaries.
#![allow(clippy::expect_used, clippy::unwrap_used)]

#[path = "support/forms.rs"]
mod forms;

use forms::Fixture;
use ncl_compiler_front::{
    Expr, LambdaExpr, LambdaList, Literal, LowerError, ParamName, lower_toplevel,
};
use ncl_ir::{Compare, Constant, Function, OpKind, Terminator, verify};
use ncl_object::Word;

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

fn count_ops(function: &Function, predicate: impl Fn(&OpKind) -> bool) -> usize {
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
fn source_global_setq_and_read_emit_distinct_value_cell_paths() {
    let mut fixture = Fixture::new();
    let global = fixture.user("GLOBAL-VALUE");
    let setq = fixture.form("SETQ", &[global, Word::fixnum(17)]);
    let source = fixture.form("PROGN", &[setq, global]);
    let expression = fixture.expand(source).expect("expand global setq and read");
    let lowered = lower_toplevel(&expression).expect("lower global setq and read");

    assert_verifies(&lowered.entry);
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::StoreField { field, .. }
            if *field == u32::try_from(ncl_object::symbol_offset::VALUE).unwrap()
    )));
    assert!(
        lowered
            .entry
            .constants
            .iter()
            .any(|constant| matches!(constant, Constant::Fixnum(17)))
    );
    assert!(
        lowered
            .entry
            .constants
            .iter()
            .any(|constant| matches!(constant, Constant::Unbound))
    );
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::Compare {
            op: Compare::Eq,
            ..
        }
    )));
    assert!(any_terminator(&lowered.entry, |term| matches!(
        term,
        Terminator::Branch { .. }
    )));
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::CallClosure {
            named_symbol: Some(_),
            args,
            ..
        } if args.len() == 2
    )));
}

#[test]
fn source_assigned_capture_shares_one_cell_across_closures() {
    let mut fixture = Fixture::new();
    let x = fixture.user("X");
    let lambda = fixture.cl("LAMBDA");
    let function = fixture.cl("FUNCTION");
    let call = fixture.cl("FUNCALL");

    let first_body = fixture.form("SETQ", &[x, Word::fixnum(1)]);
    let first_lambda = fixture.list(&[lambda, Word::NIL, first_body]);
    let first_designator = fixture.list(&[function, first_lambda]);
    let first_call = fixture.list(&[call, first_designator]);

    let second_body = fixture.form("SETQ", &[x, Word::fixnum(2)]);
    let second_lambda = fixture.list(&[lambda, Word::NIL, second_body]);
    let second_designator = fixture.list(&[function, second_lambda]);
    let second_call = fixture.list(&[call, second_designator]);

    let binding = fixture.list(&[x, Word::fixnum(0)]);
    let bindings = fixture.list(&[binding]);
    let let_symbol = fixture.cl("LET");
    let source = fixture.list(&[let_symbol, bindings, first_call, second_call, x]);
    let expression = fixture.expand(source).expect("expand shared capture");
    let lowered = lower_toplevel(&expression).expect("lower shared capture");

    assert_verifies(&lowered.entry);
    assert_eq!(lowered.nested.len(), 2);
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::MakeValueCell { .. }
    )));
    for nested in &lowered.nested {
        assert_verifies(nested);
        assert!(any_op(nested, |kind| matches!(
            kind,
            OpKind::StoreField { field: 0, .. }
        )));
    }
}

#[test]
fn source_flet_and_labels_preserve_function_namespace_captures() {
    let mut fixture = Fixture::new();
    let x = fixture.user("VALUE");
    let helper = fixture.user("HELPER");
    let definition = fixture.list(&[helper, Word::NIL, x]);
    let definitions = fixture.list(&[definition]);
    let call = fixture.list(&[helper]);
    let flet = fixture.form("FLET", &[definitions, call]);
    let binding = fixture.list(&[x, Word::fixnum(4)]);
    let let_symbol = fixture.cl("LET");
    let binding_list = fixture.list(&[binding]);
    let source = fixture.list(&[let_symbol, binding_list, flet]);
    let expression = fixture.expand(source).expect("expand flet capture");
    let lowered = lower_toplevel(&expression).expect("lower flet capture");

    assert_verifies(&lowered.entry);
    assert_eq!(lowered.nested.len(), 1);
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::MakeClosure { captures, .. } if captures.len() == 1
    )));
    assert_verifies(&lowered.nested[0]);
    assert!(any_op(&lowered.nested[0], |kind| matches!(
        kind,
        OpKind::LoadCapture { .. }
    )));
    assert!(
        any_op(&lowered.entry, |kind| matches!(
            kind,
            OpKind::CallClosure {
                named_symbol: None,
                ..
            }
        )),
        "expected local call in entry: {}",
        lowered.entry
    );

    let mut recursive_fixture = Fixture::new();
    let first = recursive_fixture.user("FIRST");
    let second = recursive_fixture.user("SECOND");
    let first_body = recursive_fixture.list(&[second]);
    let second_body = recursive_fixture.list(&[first]);
    let first_definition = recursive_fixture.list(&[first, Word::NIL, first_body]);
    let second_definition = recursive_fixture.list(&[second, Word::NIL, second_body]);
    let recursive_definitions = recursive_fixture.list(&[first_definition, second_definition]);
    let recursive_call = recursive_fixture.list(&[first]);
    let labels = recursive_fixture.form("LABELS", &[recursive_definitions, recursive_call]);
    let expression = recursive_fixture
        .expand(labels)
        .expect("expand recursive labels");
    let lowered = lower_toplevel(&expression).expect("lower recursive labels");

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
fn source_lambda_list_prologue_covers_optional_keywords_rest_and_aux() {
    let mut fixture = Fixture::new();
    let required = fixture.user("REQUIRED");
    let optional = fixture.user("OPTIONAL");
    let optional_p = fixture.user("OPTIONAL-P");
    let rest = fixture.user("REST");
    let key_value = fixture.user("VALUE");
    let key_p = fixture.user("VALUE-P");
    let aux = fixture.user("AUX");
    let optional_spec = fixture.list(&[optional, Word::fixnum(12), optional_p]);
    let key_keyword = fixture.keyword("VALUE");
    let key_pair = fixture.list(&[key_keyword, key_value]);
    let key_spec = fixture.list(&[key_pair, Word::fixnum(13), key_p]);
    let aux_spec = fixture.list(&[aux, Word::fixnum(14)]);
    let optional_marker = fixture.cl("&OPTIONAL");
    let rest_marker = fixture.cl("&REST");
    let key_marker = fixture.cl("&KEY");
    let allow_other_keys = fixture.cl("&ALLOW-OTHER-KEYS");
    let aux_marker = fixture.cl("&AUX");
    let lambda_list = fixture.list(&[
        required,
        optional_marker,
        optional_spec,
        rest_marker,
        rest,
        key_marker,
        key_spec,
        allow_other_keys,
        aux_marker,
        aux_spec,
    ]);
    let body = fixture.form("PROGN", &[optional_p, key_p, aux]);
    let lambda_symbol = fixture.cl("LAMBDA");
    let lambda_form = fixture.list(&[lambda_symbol, lambda_list, body]);
    let source = fixture.list(&[lambda_form, Word::fixnum(1)]);
    let expression = fixture.expand(source).expect("expand parameter prologue");
    let lowered = lower_toplevel(&expression).expect("lower parameter prologue");

    assert_verifies(&lowered.entry);
    assert_eq!(lowered.nested.len(), 1);
    let nested = &lowered.nested[0];
    assert_verifies(nested);
    assert_eq!(nested.params.len(), 3, "argc, required, and optional");
    assert_eq!(
        count_ops(nested, |kind| matches!(kind, OpKind::Compare { .. })),
        2
    );
    assert_eq!(
        count_ops(
            nested,
            |kind| matches!(kind, OpKind::Builtin { name, .. } if name == "CONS")
        ),
        1
    );
    for builtin in [
        "make-rest-list",
        "check-keywords",
        "keyword-value",
        "keyword-supplied-p",
    ] {
        assert!(any_op(nested, |kind| matches!(
            kind,
            OpKind::Builtin { name, .. } if name == builtin
        )));
    }
    for expected in [12, 13, 14] {
        assert!(
            nested
                .constants
                .iter()
                .any(|constant| matches!(constant, Constant::Fixnum(value) if *value == expected))
        );
    }
    assert!(any_terminator(nested, |term| matches!(
        term,
        Terminator::Branch { .. }
    )));
}

#[test]
fn source_calls_choose_builtin_closure_designator_and_arity_fallbacks() {
    let mut fixture = Fixture::new();
    let builtin = fixture.form("+", &[Word::fixnum(1), Word::fixnum(2)]);
    let fallback = fixture.form("+", &[Word::fixnum(3)]);
    let lambda_symbol = fixture.cl("LAMBDA");
    let lambda = fixture.list(&[lambda_symbol, Word::NIL, Word::fixnum(9)]);
    let designator = fixture.form("FUNCTION", &[lambda]);
    let closure_call = fixture.form("FUNCALL", &[designator, Word::fixnum(4)]);
    let source = fixture.form("PROGN", &[builtin, fallback, closure_call]);
    let expression = fixture.expand(source).expect("expand call matrix");
    let lowered = lower_toplevel(&expression).expect("lower call matrix");

    assert_verifies(&lowered.entry);
    assert_eq!(lowered.nested.len(), 1);
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::Builtin { name, args } if name == "+" && args.len() == 2
    )));
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::CallClosure {
            named_symbol: Some(_),
            args,
            ..
        } if args.len() == 2
    )));
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::CallClosure {
            named_symbol: None,
            args,
            ..
        } if args.len() == 2
    )));
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::MakeClosure { .. }
    )));
    assert_verifies(&lowered.nested[0]);
}

#[test]
fn typed_lowering_edges_keep_unbound_control_and_destructuring_errors_distinct() {
    let mut fixture = Fixture::new();
    let missing_tag = fixture.user("MISSING-TAG");
    let source = fixture.form("GO", &[missing_tag]);
    let expression = fixture.expand(source).expect("expand escaping go");
    let error = lower_toplevel(&expression).unwrap_err();
    assert!(matches!(error, LowerError::EscapingControl { .. }));
    assert!(error.to_string().contains("MISSING-TAG"));

    let expression = Expr::Lambda(Box::new(LambdaExpr {
        lambda_list: LambdaList {
            required: vec![ParamName::Pattern(Box::new(LambdaList::new()))],
            ..LambdaList::new()
        },
        declarations: Vec::new(),
        docstring: None,
        body: vec![Expr::Constant(Literal::Nil)],
    }));
    let lowered = lower_toplevel(&expression).expect("destructuring parameter lowers");
    assert_verifies(&lowered.entry);
    assert_verifies(&lowered.nested[0]);
    assert!(any_op(&lowered.nested[0], |kind| matches!(
        kind,
        OpKind::LoadArg { index: 1 }
    )));
}
