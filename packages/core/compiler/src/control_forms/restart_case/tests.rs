mod invalid_function_id;

use super::*;
use ncl_syntax::read;

fn bad_catch(span: Span) -> Form {
    Form::list(vec![Form::atom("CATCH", span)], span)
}

fn empty_clause(span: Span) -> Form {
    Form::list(
        vec![Form::atom("R", span), Form::list(Vec::new(), span)],
        span,
    )
}

fn expect_internal(error: &CompileError) {
    assert!(
        matches!(error.kind, CompileErrorKind::Internal { .. }),
        "expected an internal error, got {:?}",
        error.kind
    );
}

fn expect_catch_arity(error: &CompileError) {
    match &error.kind {
        CompileErrorKind::Arity { operator, .. } => assert_eq!(operator, "CATCH"),
        other => panic!("expected the nested CATCH arity error to propagate, got {other:?}"),
    }
}

#[test]
fn compile_restart_case_propagates_malformed_protected_form() {
    let span = Span::new(0, 1);
    let items = vec![
        Form::atom("RESTART-CASE", span),
        bad_catch(span),
        empty_clause(span),
    ];
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());

    let error = state
        .compile_restart_case(function, span, &items)
        .map_or_else(
            |error| error,
            |value| {
                panic!(
                    "a malformed protected form must propagate its own compile error, got {value:?}"
                )
            },
        );
    expect_catch_arity(&error);
}

#[test]
fn compile_restart_case_propagates_malformed_lambda_list() {
    let span = Span::new(0, 1);
    let clause = Form::list(vec![Form::atom("R", span), Form::atom("VALUE", span)], span);
    let items = vec![
        Form::atom("RESTART-CASE", span),
        Form::atom("1", span),
        clause,
    ];
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());

    let error = state
        .compile_restart_case(function, span, &items)
        .map_or_else(
            |error| error,
            |value| panic!("a non-list restart lambda list must be rejected, got {value:?}"),
        );
    assert!(matches!(error.kind, CompileErrorKind::ExpectedList { .. }));
}

#[test]
fn compile_restart_case_propagates_malformed_optional_default() -> Result<(), String> {
    let span = Span::new(0, 1);
    let lambda_list = read("(&optional (x (catch)))")
        .map_err(|error| error.to_string())?
        .remove(0);
    let clause = Form::list(vec![Form::atom("R", span), lambda_list], span);
    let items = vec![
        Form::atom("RESTART-CASE", span),
        Form::atom("1", span),
        clause,
    ];
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());

    let error = state
        .compile_restart_case(function, span, &items)
        .map_or_else(|error| error, |value| panic!("a malformed &optional default value must propagate its own compile error, got {value:?}"));
    expect_catch_arity(&error);
    Ok(())
}

#[test]
fn compile_restart_case_propagates_malformed_keyword_default() -> Result<(), String> {
    let span = Span::new(0, 1);
    let lambda_list = read("(&key (x (catch)))")
        .map_err(|error| error.to_string())?
        .remove(0);
    let clause = Form::list(vec![Form::atom("R", span), lambda_list], span);
    let items = vec![
        Form::atom("RESTART-CASE", span),
        Form::atom("1", span),
        clause,
    ];
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());

    let error = state
        .compile_restart_case(function, span, &items)
        .map_or_else(|error| error, |value| panic!("a malformed &key default value must propagate its own compile error, got {value:?}"));
    expect_catch_arity(&error);
    Ok(())
}

#[test]
fn compile_restart_case_propagates_malformed_auxiliary_default() -> Result<(), String> {
    let span = Span::new(0, 1);
    let lambda_list = read("(&aux (x (catch)))")
        .map_err(|error| error.to_string())?
        .remove(0);
    let clause = Form::list(vec![Form::atom("R", span), lambda_list], span);
    let items = vec![
        Form::atom("RESTART-CASE", span),
        Form::atom("1", span),
        clause,
    ];
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());

    let error = state
        .compile_restart_case(function, span, &items)
        .map_or_else(|error| error, |value| panic!("a malformed &aux default value must propagate its own compile error, got {value:?}"));
    expect_catch_arity(&error);
    Ok(())
}

mod malformed_clause_body;
mod with_condition_restarts_tests;
