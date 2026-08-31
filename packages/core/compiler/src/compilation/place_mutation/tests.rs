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
fn compile_setf_uses_direct_assignment_for_symbol_places() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let items = parse_items("(setf x 1 |Mixed| 2 (car x) 3)");

    state
        .compile_setf(function, Span::new(0, 1), &items)
        .unwrap_or_else(|error| panic!("valid SETF places should compile: {error}"));

    let instructions = &state.functions[function].instructions;
    assert!(instructions.contains(&Instruction::Set("X".to_string())));
    assert!(instructions.contains(&Instruction::SetExact("Mixed".to_string())));
    assert!(instructions.contains(&Instruction::SetfList {
        operator: "CAR".to_string(),
        name: "X".to_string(),
        escaped: false,
    }));
}

#[test]
fn compile_setf_uses_native_nth_for_a_constant_index_and_symbol_place() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let items = parse_items("(setf (nth 2 xs) 9)");
    state
        .compile_setf(function, Span::new(0, 1), &items)
        .unwrap();
    assert!(
        state.functions[function]
            .instructions
            .contains(&Instruction::SetfNth {
                index: 2,
                name: "XS".to_string(),
                escaped: false,
            })
    );
}

#[test]
fn compile_setf_uses_native_nth_for_a_dynamic_index_and_symbol_place() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let items = parse_items("(setf (nth index xs) 9)");
    state
        .compile_setf(function, Span::new(0, 1), &items)
        .unwrap();
    assert!(
        state.functions[function]
            .instructions
            .contains(&Instruction::SetfNthDynamic {
                name: "XS".to_string(),
                escaped: false,
            })
    );
}

#[test]
fn compile_setf_uses_native_aref_for_a_symbol_place() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let items = parse_items("(setf (aref xs index) 9)");
    state
        .compile_setf(function, Span::new(0, 1), &items)
        .unwrap();
    assert!(
        state.functions[function]
            .instructions
            .contains(&Instruction::SetfArefDynamic {
                rank: 1,
                name: "XS".to_string(),
                escaped: false,
            })
    );
}

#[test]
fn compile_push_and_pop_use_native_list_instructions_for_symbol_places() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let push = parse_items("(push 1 xs)");
    let pop = parse_items("(pop |Mixed|)");

    state
        .compile_runtime_definition(function, Span::new(0, 1), &push)
        .unwrap();
    state
        .compile_runtime_definition(function, Span::new(0, 1), &pop)
        .unwrap();

    let instructions = &state.functions[function].instructions;
    assert!(instructions.contains(&Instruction::PushList {
        name: "XS".to_string(),
        escaped: false,
    }));
    assert!(instructions.contains(&Instruction::PopList {
        name: "Mixed".to_string(),
        escaped: true,
    }));
}

#[test]
fn compile_pushnew_uses_native_instruction_without_options() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let pushnew = parse_items("(pushnew 1 xs)");

    state
        .compile_runtime_definition(function, Span::new(0, 1), &pushnew)
        .unwrap();

    assert!(
        state.functions[function]
            .instructions
            .contains(&Instruction::PushNewList {
                name: "XS".to_string(),
                escaped: false,
            })
    );
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
