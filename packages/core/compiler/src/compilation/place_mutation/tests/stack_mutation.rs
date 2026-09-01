use super::super::*;
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
fn compile_push_and_pop_with_car_places_use_native_instructions() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let push = parse_items("(push 1 (car xs))");
    let pop = parse_items("(pop (cdr xs))");

    state
        .compile_runtime_definition(function, Span::new(0, 1), &push)
        .expect("generalized PUSH should use a native list-place instruction");
    state
        .compile_runtime_definition(function, Span::new(0, 1), &pop)
        .expect("generalized POP should use a native list-place instruction");

    let instructions = &state.functions[function].instructions;
    assert!(instructions.iter().any(|instruction| matches!(
        instruction,
        Instruction::ListPlaceMutation { operator, accessor, name, .. }
            if operator == "PUSH" && accessor == "CAR" && name == "XS"
    )));
    assert!(instructions.iter().any(|instruction| matches!(
        instruction,
        Instruction::ListPlaceMutation { operator, accessor, name, .. }
            if operator == "POP" && accessor == "CDR" && name == "XS"
    )));
}

#[test]
fn compile_push_and_pop_use_native_gethash_instructions() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let push = parse_items("(push 1 (gethash key table))");
    let pop = parse_items("(pop (gethash key table))");

    state
        .compile_runtime_definition(function, Span::new(0, 1), &push)
        .unwrap();
    state
        .compile_runtime_definition(function, Span::new(0, 1), &pop)
        .unwrap();

    let instructions = &state.functions[function].instructions;
    assert_eq!(
        instructions
            .iter()
            .filter(|instruction| matches!(instruction, Instruction::PushGethash))
            .count(),
        1
    );
    assert_eq!(
        instructions
            .iter()
            .filter(|instruction| matches!(instruction, Instruction::PopGethash))
            .count(),
        1
    );
}

#[test]
fn compile_setf_property_places_evaluate_nested_targets_natively() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let setf = parse_items("(setf (get (car objects) key) value)");

    state
        .compile_setf(function, setf[0].span, &setf)
        .expect("SETF GET should compile through the native property instruction");

    let instructions = &state.functions[function].instructions;
    assert!(instructions.iter().any(|instruction| matches!(
        instruction,
        Instruction::SetfGetDynamic
    )));
    assert!(!instructions
        .iter()
        .any(|instruction| matches!(instruction, Instruction::Setf(_))));
}

#[test]
fn compile_pushnew_uses_native_gethash_instruction() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let items = parse_items("(pushnew 1 (gethash key table))");

    state
        .compile_runtime_definition(function, Span::new(0, 1), &items)
        .unwrap();

    assert!(
        state.functions[function]
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::PushNewGethash))
    );
}

#[test]
fn compile_pushnew_with_a_generalized_place_uses_native_instruction_without_options() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let pushnew = parse_items("(pushnew 1 (car xs))");

    state
        .compile_runtime_definition(function, Span::new(0, 1), &pushnew)
        .unwrap();

    assert!(
        state.functions[function]
            .instructions
            .iter()
            .any(|instruction| matches!(
                instruction,
                Instruction::ListPlaceMutation { operator, accessor, name, .. }
                    if operator == "PUSHNEW" && accessor == "CAR" && name == "XS"
            ))
    );
}

#[test]
fn compile_pushnew_with_options_and_a_generalized_place_uses_native_instruction() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let pushnew = parse_items("(pushnew 1 (car xs) :test #'equal)");

    state
        .compile_runtime_definition(function, Span::new(0, 1), &pushnew)
        .unwrap();

    assert!(
        state.functions[function]
            .instructions
            .iter()
            .any(|instruction| matches!(
                instruction,
                Instruction::ListPlacePushNewOptions { accessor, name, test_not, has_key, .. }
                    if accessor == "CAR" && name == "XS" && !test_not && !has_key
            ))
    );
}

#[test]
fn compile_pushnew_gethash_options_uses_native_instruction() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let pushnew = parse_items("(pushnew 1 (gethash key table) :test #'equal)");

    state
        .compile_runtime_definition(function, Span::new(0, 1), &pushnew)
        .unwrap();

    assert!(
        state.functions[function]
            .instructions
            .iter()
            .any(|instruction| matches!(
                instruction,
                Instruction::PushNewGethashOptions {
                    test_not: false,
                    has_key: false,
                    key_before_test: false,
                }
            ))
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
fn compile_modify_uses_native_nested_list_instruction() {
    let form = read("(incf (car (car xs)) 2)")
        .unwrap_or_else(|error| panic!("test source should parse: {error}"))
        .remove(0);

    let program = Compiler::compile_form(&form)
        .unwrap_or_else(|error| panic!("nested list INCF should compile: {error}"));
    let instructions = &program.functions[program.entry].instructions;

    assert!(instructions.iter().any(|instruction| matches!(
        instruction,
        Instruction::SetfNestedList { accessors, name, escaped }
            if accessors == &["CAR".to_string(), "CAR".to_string()]
                && name == "XS"
                && !escaped
    )));
}

#[test]
fn compile_modify_normalizes_constant_nth_in_nested_list_place() {
    let form = read("(incf (nth 1 (car xs)) 2)")
        .unwrap_or_else(|error| panic!("test source should parse: {error}"))
        .remove(0);

    let program = Compiler::compile_form(&form)
        .unwrap_or_else(|error| panic!("nested NTH should compile: {error}"));
    let instructions = &program.functions[program.entry].instructions;

    assert!(instructions.iter().any(|instruction| matches!(
        instruction,
        Instruction::SetfNestedList { accessors, name, escaped }
            if accessors == &["CAR".to_string(), "SECOND".to_string()]
                && name == "XS"
                && !escaped
    )));
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
