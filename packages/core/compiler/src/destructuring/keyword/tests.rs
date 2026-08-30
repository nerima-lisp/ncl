use super::*;

fn bad_form(span: Span) -> Form {
    Form::new(FormKind::String("bad".to_string()), span)
}

#[test]
fn compile_destructuring_keyword_name_rejects_a_non_atom_form() {
    let span = Span::new(0, 1);
    let error = CompileState::compile_destructuring_keyword_name(&Form::list(Vec::new(), span))
        .map_or_else(
            |error| error,
            |value| panic!("a list cannot be a keyword designator, got {value:?}"),
        );
    assert!(matches!(
        error.kind,
        CompileErrorKind::ExpectedSymbol { .. }
    ));
}

#[test]
fn compile_destructuring_keyword_parameter_rejects_a_non_symbol_non_list_form() {
    let mut state = CompileState::default();
    let mut seen = HashSet::new();
    let span = Span::new(0, 1);
    let error = state
        .compile_destructuring_keyword_parameter(&bad_form(span), &mut seen)
        .map_or_else(
            |error| error,
            |value| panic!("a string literal cannot be a keyword parameter, got {value:?}"),
        );
    assert!(matches!(error.kind, CompileErrorKind::InvalidForm { .. }));
}

#[test]
fn compile_destructuring_keyword_parameter_rejects_an_empty_list_instead_of_panicking() {
    let mut state = CompileState::default();
    let mut seen = HashSet::new();
    let span = Span::new(0, 1);
    let error = state
        .compile_destructuring_keyword_parameter(&Form::list(Vec::new(), span), &mut seen)
        .map_or_else(
            |error| error,
            |value| panic!("an empty list cannot be a keyword parameter, got {value:?}"),
        );
    assert!(
        matches!(&error.kind, CompileErrorKind::InvalidForm { message } if message.contains("must not be empty"))
    );
}

#[test]
fn compile_destructuring_keyword_parameter_propagates_a_key_specification_pattern_error() {
    let mut state = CompileState::default();
    let mut seen = HashSet::new();
    let span = Span::new(0, 1);
    let form = Form::list(
        vec![Form::list(
            vec![Form::atom(":name", span), bad_form(span)],
            span,
        )],
        span,
    );
    let error = state
        .compile_destructuring_keyword_parameter(&form, &mut seen)
        .map_or_else(
            |error| error,
            |value| {
                panic!(
                    "a malformed key-specification pattern should fail to compile, got {value:?}"
                )
            },
        );
    assert!(matches!(error.kind, CompileErrorKind::InvalidForm { .. }));
}

#[test]
fn compile_destructuring_keyword_parameter_propagates_a_keyword_variable_pattern_error() {
    let mut state = CompileState::default();
    let mut seen = HashSet::new();
    let span = Span::new(0, 1);
    let form = Form::list(vec![Form::atom(":name", span), bad_form(span)], span);
    let error = state
        .compile_destructuring_keyword_parameter(&form, &mut seen)
        .map_or_else(
            |error| error,
            |value| {
                panic!("a malformed keyword-variable pattern should fail to compile, got {value:?}")
            },
        );
    assert!(matches!(error.kind, CompileErrorKind::InvalidForm { .. }));
}

#[test]
fn compile_destructuring_keyword_parameter_propagates_a_bare_pattern_error() {
    let mut state = CompileState::default();
    let mut seen = HashSet::new();
    let span = Span::new(0, 1);
    let form = Form::list(vec![bad_form(span)], span);
    let error = state
        .compile_destructuring_keyword_parameter(&form, &mut seen)
        .map_or_else(
            |error| error,
            |value| panic!("a malformed bare pattern should fail to compile, got {value:?}"),
        );
    assert!(matches!(error.kind, CompileErrorKind::InvalidForm { .. }));
}

#[test]
fn compile_destructuring_keyword_parameter_propagates_a_supplied_p_error() {
    let mut state = CompileState::default();
    let mut seen = HashSet::new();
    let span = Span::new(0, 1);
    let form = Form::list(
        vec![
            Form::atom(":name", span),
            Form::atom("X", span),
            Form::atom("1", span),
            Form::atom(":sp", span),
        ],
        span,
    );
    let error = state
        .compile_destructuring_keyword_parameter(&form, &mut seen)
        .map_or_else(
            |error| error,
            |value| panic!("a keyword cannot name a supplied-p variable, got {value:?}"),
        );
    assert!(matches!(
        error.kind,
        CompileErrorKind::ExpectedSymbol { .. }
    ));
}

#[test]
fn compile_destructuring_keyword_parameter_propagates_a_default_error() {
    let mut state = CompileState::default();
    let mut seen = HashSet::new();
    let span = Span::new(0, 1);
    let dotted = Form::dotted_list(vec![Form::atom("a", span)], Form::atom("b", span), span);
    let form = Form::list(
        vec![Form::atom(":name", span), Form::atom("X", span), dotted],
        span,
    );
    let error = state
        .compile_destructuring_keyword_parameter(&form, &mut seen)
        .map_or_else(
            |error| error,
            |value| panic!("a malformed default value should fail to compile, got {value:?}"),
        );
    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedForm { .. }
    ));
}
