use super::*;

fn check_binding_instructions(sequential: bool, escaped: bool) {
    for name in ["NCL-SETF-TEMP-0", "NCL-SETF-TEMP-1", "Mixed"] {
        let span = Span::new(0, 1);
        let source_name = if escaped {
            format!("|{name}|")
        } else {
            name.to_string()
        };
        let items = vec![
            Form::atom(if sequential { "LET*" } else { "LET" }, span),
            Form::list(vec![binding(&source_name, None, span)], span),
            Form::atom(&source_name, span),
        ];
        let mut state = CompileState::default();
        let function = state.reserve_function(None, Vec::new());
        state
            .compile_let(function, span, &items, sequential)
            .unwrap_or_else(|error| panic!("binding control should compile: {error}"));
        let (define, load) = if escaped {
            (
                Instruction::DefineExact(name.to_string()),
                Instruction::LoadExact(name.to_string()),
            )
        } else {
            (
                Instruction::Define(name.to_uppercase()),
                Instruction::Load(name.to_uppercase()),
            )
        };
        let mut expected = Vec::new();
        if !sequential {
            expected.push(Instruction::EnterScope);
        }
        expected.push(Instruction::Constant(Constant::Nil));
        if sequential {
            expected.push(Instruction::EnterScope);
        }
        expected.extend([define, Instruction::Pop, load, Instruction::ExitScope]);
        assert_eq!(
            state.functions[function].instructions, expected,
            "binding {source_name}, sequential={sequential}"
        );
    }
}

#[test]
fn compile_let_escaped_binding_instructions() {
    check_binding_instructions(false, true);
}

#[test]
fn compile_let_star_escaped_binding_instructions() {
    check_binding_instructions(true, true);
}

#[test]
fn compile_let_ordinary_binding_instructions() {
    check_binding_instructions(false, false);
}

#[test]
fn compile_let_star_ordinary_binding_instructions() {
    check_binding_instructions(true, false);
}

#[test]
fn compile_let_bare_binding_instructions() {
    for sequential in [false, true] {
        let span = Span::new(0, 1);
        let items = vec![
            Form::atom(if sequential { "LET*" } else { "LET" }, span),
            Form::list(vec![Form::atom("X", span)], span),
            Form::atom("X", span),
        ];
        let mut state = CompileState::default();
        let function = state.reserve_function(None, Vec::new());
        state
            .compile_let(function, span, &items, sequential)
            .unwrap_or_else(|error| panic!("bare binding should compile: {error}"));
        let mut expected = Vec::new();
        if !sequential {
            expected.push(Instruction::EnterScope);
        }
        expected.push(Instruction::Constant(Constant::Nil));
        if sequential {
            expected.push(Instruction::EnterScope);
        }
        expected.extend([
            Instruction::Define("X".to_string()),
            Instruction::Pop,
            Instruction::Load("X".to_string()),
            Instruction::ExitScope,
        ]);
        assert_eq!(state.functions[function].instructions, expected);
    }
}

fn bad_catch(span: Span) -> Form {
    Form::list(vec![Form::atom("CATCH", span)], span)
}

fn binding(name: &str, value: Option<Form>, span: Span) -> Form {
    let mut items = vec![Form::atom(name, span)];
    items.extend(value);
    Form::list(items, span)
}

fn expect_catch_arity(error: &CompileError) {
    match &error.kind {
        CompileErrorKind::Arity { operator, .. } => assert_eq!(operator, "CATCH"),
        other => panic!("expected the nested CATCH arity error to propagate, got {other:?}"),
    }
}

#[test]
fn compile_let_rejects_invalid_function_id() {
    let span = Span::new(0, 1);
    let items = vec![
        Form::atom("LET", span),
        Form::list(vec![binding("X", Some(Form::atom("1", span)), span)], span),
        Form::atom("X", span),
    ];
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());

    let error = state
        .compile_let(function + 1, span, &items, false)
        .map_or_else(
            |error| error,
            |value| {
                panic!(
                    "an out-of-range target function must fail entering the scope, got {value:?}"
                )
            },
        );
    assert!(matches!(error.kind, CompileErrorKind::Internal { .. }));
}

#[test]
fn compile_let_star_defaults_omitted_binding_to_nil() {
    let span = Span::new(0, 1);
    let items = vec![
        Form::atom("LET*", span),
        Form::list(vec![binding("X", None, span)], span),
        Form::atom("X", span),
    ];
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());

    state
        .compile_let(function, span, &items, true)
        .unwrap_or_else(|error| {
            panic!("a LET* binding without an initial value defaults to NIL: {error}")
        });

    assert!(
        state.functions[function]
            .instructions
            .contains(&Instruction::Constant(Constant::Nil)),
        "an omitted LET* binding value should compile to a NIL constant, got {:?}",
        state.functions[function].instructions
    );
}

#[test]
fn compile_let_uses_special_binding_for_a_leading_special_declaration() {
    let span = Span::new(0, 1);
    let declaration = Form::list(
        vec![
            Form::atom("DECLARE", span),
            Form::list(
                vec![Form::atom("SPECIAL", span), Form::atom("X", span)],
                span,
            ),
        ],
        span,
    );
    let items = vec![
        Form::atom("LET", span),
        Form::list(vec![binding("X", Some(Form::atom("1", span)), span)], span),
        declaration,
        Form::atom("X", span),
    ];
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());

    state
        .compile_let(function, span, &items, false)
        .unwrap_or_else(|error| panic!("SPECIAL declaration should compile: {error}"));

    assert_eq!(
        state.functions[function].instructions,
        vec![
            Instruction::EnterScope,
            Instruction::Constant(Constant::Integer(1.into())),
            Instruction::DeclareSpecial("X".to_string()),
            Instruction::Define("X".to_string()),
            Instruction::Pop,
            Instruction::DeclareSpecial("X".to_string()),
            Instruction::Constant(Constant::Nil),
            Instruction::Pop,
            Instruction::Load("X".to_string()),
            Instruction::ExitScope,
        ]
    );
}

#[test]
fn compile_let_keeps_star_named_variables_lexical_without_a_special_declaration() {
    let span = Span::new(0, 1);
    let items = vec![
        Form::atom("LET", span),
        Form::list(
            vec![binding("*X*", Some(Form::atom("1", span)), span)],
            span,
        ),
        Form::atom("*X*", span),
    ];
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());

    state
        .compile_let(function, span, &items, false)
        .unwrap_or_else(|error| panic!("a star-named lexical binding should compile: {error}"));

    assert!(
        state.functions[function]
            .instructions
            .contains(&Instruction::Define("*X*".to_string()))
    );
}

#[test]
fn compile_let_star_propagates_malformed_binding_value() {
    let span = Span::new(0, 1);
    let items = vec![
        Form::atom("LET*", span),
        Form::list(vec![binding("X", Some(bad_catch(span)), span)], span),
        Form::atom("X", span),
    ];
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());

    let error = state
        .compile_let(function, span, &items, true)
        .map_or_else(|error| error, |value| panic!("a malformed sequential binding value must propagate its own compile error, got {value:?}"));
    expect_catch_arity(&error);
}

#[test]
fn compile_let_propagates_malformed_binding_value() {
    let span = Span::new(0, 1);
    let items = vec![
        Form::atom("LET", span),
        Form::list(vec![binding("X", Some(bad_catch(span)), span)], span),
        Form::atom("X", span),
    ];
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());

    let error = state
        .compile_let(function, span, &items, false)
        .map_or_else(|error| error, |value| panic!("a malformed parallel binding value must propagate its own compile error, got {value:?}"));
    expect_catch_arity(&error);
}

#[test]
fn compile_let_propagates_malformed_body_form() {
    let span = Span::new(0, 1);
    let items = vec![
        Form::atom("LET", span),
        Form::list(vec![binding("X", Some(Form::atom("1", span)), span)], span),
        bad_catch(span),
    ];
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());

    let error = state
        .compile_let(function, span, &items, false)
        .map_or_else(
            |error| error,
            |value| {
                panic!("a malformed body form must propagate its own compile error, got {value:?}")
            },
        );
    expect_catch_arity(&error);
}
