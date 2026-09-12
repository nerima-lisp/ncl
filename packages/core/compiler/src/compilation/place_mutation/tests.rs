use super::*;
use ncl_syntax::read;

fn parse_items(source: &str) -> Vec<Form> {
    let mut forms =
        read(source).unwrap_or_else(|error| panic!("test source should parse: {error}"));
    let form = forms.remove(0);
    let FormKind::List(items) = form.kind else {
        panic!("expected a list form, got {form:?}");
    };
    items
}

#[test]
fn compile_empty_setf_emits_nil() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let items = parse_items("(setf)");

    state
        .compile_setf(function, span, &items)
        .unwrap_or_else(|error| panic!("empty SETF should compile: {error}"));

    assert_eq!(
        state.functions[function].instructions,
        [Instruction::Constant(Constant::Nil)]
    );
}

#[test]
fn compile_setf_propagates_a_malformed_value_form_error() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let items = parse_items("(setf x (function))");

    let error = state.compile_setf(function, span, &items).map_or_else(
        |error| error,
        |value| {
            panic!("a value form that fails to compile must propagate its own error, got {value:?}")
        },
    );

    match error.kind {
        CompileErrorKind::Arity { operator, .. } => assert_eq!(operator, "FUNCTION"),
        other => panic!("expected the nested FUNCTION arity error to propagate, got {other:?}"),
    }
}

#[test]
fn compile_modify_symbol_evaluates_delta_before_reading_the_place() {
    for (operator, arithmetic) in [("INCF", "+"), ("DECF", "-")] {
        let mut state = CompileState::default();
        let function = state.reserve_function(None, Vec::new());
        let span = Span::new(0, 1);
        let items = parse_items(&format!("({operator} x (setq x 5))"));

        state
            .compile_modify_symbol(function, span, &items, operator, arithmetic)
            .unwrap_or_else(|error| panic!("{operator} should compile: {error}"));

        let instructions = &state.functions[function].instructions;
        let temporary = instructions
            .iter()
            .find_map(|instruction| match instruction {
                Instruction::Define(name) => Some(name.clone()),
                _ => None,
            })
            .unwrap_or_else(|| panic!("{operator} must save the delta: {instructions:?}"));
        assert_ne!(temporary, "X");
        assert_eq!(
            instructions,
            &[
                Instruction::EnterScope,
                Instruction::Constant(Constant::Integer(5)),
                Instruction::Set("X".to_string()),
                Instruction::Define(temporary.clone()),
                Instruction::Pop,
                Instruction::FunctionLoad(arithmetic.to_string()),
                Instruction::Load("X".to_string()),
                Instruction::Load(temporary),
                Instruction::Call(2),
                Instruction::Set("X".to_string()),
                Instruction::ExitScope,
            ],
            "{operator} must evaluate the delta once before reading the place"
        );
    }
}

#[test]
fn compile_modify_symbol_rejects_too_many_operands() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let items = parse_items("(incf x 1 2)");

    let error = state
        .compile_modify_symbol(function, span, &items, "INCF", "+")
        .map_or_else(
            |error| error,
            |value| panic!("INCF with more than one delta form must be rejected, got {value:?}"),
        );

    match error.kind {
        CompileErrorKind::Arity {
            operator,
            expected,
            actual,
        } => {
            assert_eq!(operator, "INCF");
            assert_eq!(expected, "one or two");
            assert_eq!(actual, 3);
        }
        other => panic!("expected an arity error, got {other:?}"),
    }
}

#[test]
fn compile_modify_symbol_rejects_a_non_symbol_place() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let items = parse_items("(incf 5)");

    let error = state
        .compile_modify_symbol(function, span, &items, "INCF", "+")
        .map_or_else(
            |error| error,
            |value| panic!("a numeric literal is not a valid modifying place, got {value:?}"),
        );

    assert!(matches!(
        error.kind,
        CompileErrorKind::ExpectedSymbol { context } if context == "INCF target"
    ));
}

#[test]
fn compile_modify_symbol_propagates_a_malformed_delta_form_error() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let items = parse_items("(incf x (function))");

    let error = state
        .compile_modify_symbol(function, span, &items, "INCF", "+")
        .map_or_else(
            |error| error,
            |value| {
                panic!(
                    "a delta form that fails to compile must propagate its own error, got {value:?}"
                )
            },
        );

    match error.kind {
        CompileErrorKind::Arity { operator, .. } => assert_eq!(operator, "FUNCTION"),
        other => panic!("expected the nested FUNCTION arity error to propagate, got {other:?}"),
    }
}

#[test]
fn compile_modify_symbol_uses_set_exact_for_an_escaped_place() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let items = parse_items("(incf |Mixed|)");

    state
        .compile_modify_symbol(function, span, &items, "INCF", "+")
        .unwrap_or_else(|error| panic!("an escaped place should still compile: {error}"));

    assert!(
        state.functions[function]
            .instructions
            .contains(&Instruction::SetExact("Mixed".to_string())),
        "escaped INCF place should bind with SetExact, got {:?}",
        state.functions[function].instructions
    );
}

#[test]
fn compile_modify_symbol_uses_modify_place_for_a_generalized_place() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let items = parse_items("(incf (car xs) 2)");

    state
        .compile_modify_symbol(function, span, &items, "INCF", "+")
        .unwrap_or_else(|error| panic!("a generalized place should compile: {error}"));

    assert_eq!(
        state.functions[function].instructions,
        [Instruction::ModifyPlace {
            invocation: Form::list(items, span),
            delta: function + 1,
            arithmetic: "+".to_string(),
        }]
    );
    assert_eq!(
        state.functions[function + 1].instructions,
        [
            Instruction::Constant(Constant::Integer(2)),
            Instruction::Return
        ]
    );
}

#[test]
fn compile_modify_symbol_reports_an_internal_error_for_an_invalid_function_id() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let items = parse_items("(incf x)");

    let error = state
        .compile_modify_symbol(function + 1, span, &items, "INCF", "+")
        .map_or_else(
            |error| error,
            |value| panic!("emitting into an invalid function id must fail, got {value:?}"),
        );

    assert!(matches!(error.kind, CompileErrorKind::Internal { .. }));
}
