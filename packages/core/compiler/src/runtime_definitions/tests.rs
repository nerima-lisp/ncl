use super::super::*;

fn parse_items(source: &str) -> Vec<Form> {
    let mut forms = ncl_syntax::read(source).expect("test source should parse");
    match forms.remove(0).kind {
        ncl_syntax::FormKind::List(items) => items,
        form => panic!("expected list form, got {form:?}"),
    }
}

#[test]
fn compile_defstruct_reports_an_internal_error_for_an_invalid_function_id() {
    let mut state = CompileState::default();
    let span = Span::new(0, 1);
    let items = vec![Form::atom("DEFSTRUCT", span), Form::atom("POINT", span)];

    let error = state.compile_defstruct(99, span, &items).map_or_else(
        |error| error,
        |value| panic!("an unknown function id cannot receive instructions, got {value:?}"),
    );

    assert!(matches!(error.kind, CompileErrorKind::Internal { .. }));
}

#[test]
fn compile_runtime_definition_reports_an_internal_error_for_an_invalid_function_id() {
    let mut state = CompileState::default();
    let span = Span::new(0, 1);
    let items = vec![Form::atom("DEFPACKAGE", span), Form::atom("FOO", span)];

    let error = state
        .compile_runtime_definition(99, span, &items)
        .map_or_else(
            |error| error,
            |value| panic!("an unknown function id cannot receive instructions, got {value:?}"),
        );

    assert!(matches!(error.kind, CompileErrorKind::Internal { .. }));
}

#[test]
fn compile_load_time_value_rejects_more_than_two_arguments() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let items = vec![
        Form::atom("LOAD-TIME-VALUE", span),
        Form::atom("1", span),
        Form::atom("NIL", span),
        Form::atom("NIL", span),
    ];

    let Err(error) = state.compile_load_time_value(function, span, &items) else {
        panic!("too many LOAD-TIME-VALUE arguments must fail during compilation")
    };

    assert!(matches!(error.kind, CompileErrorKind::Arity { .. }));
}

#[test]
fn compile_runtime_definition_uses_native_rotate_and_shift_for_symbol_places() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let rotatef = parse_items("(rotatef a |B| c)");
    state
        .compile_runtime_definition(function, Span::new(0, 1), &rotatef)
        .expect("ROTATEF symbol places should compile");
    assert!(
        state.functions[function]
            .instructions
            .contains(&Instruction::RotatefSymbols(vec![
                ("A".to_string(), false),
                ("B".to_string(), true),
                ("C".to_string(), false),
            ]))
    );

    let shiftf = parse_items("(shiftf a b 9)");
    state
        .compile_runtime_definition(function, Span::new(0, 1), &shiftf)
        .expect("SHIFTF symbol places should compile");
    assert!(
        state.functions[function]
            .instructions
            .contains(&Instruction::ShiftfSymbols(vec![
                ("A".to_string(), false),
                ("B".to_string(), false),
            ]))
    );
}

#[test]
fn compile_runtime_definition_falls_back_for_generalized_rotate_and_shift_places() {
    let mut state = CompileState::default();
    for source in ["(rotatef (car xs) y)", "(shiftf (car xs) y 9)"] {
        let function = state.reserve_function(None, Vec::new());
        let items = parse_items(source);
        state
            .compile_runtime_definition(function, Span::new(0, 1), &items)
            .expect("generalized places should use evaluator fallback");
        assert!(
            state.functions[function]
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, Instruction::Eval(_)))
        );
        assert!(
            !state.functions[function]
                .instructions
                .iter()
                .any(|instruction| {
                    matches!(
                        instruction,
                        Instruction::RotatefSymbols(_) | Instruction::ShiftfSymbols(_)
                    )
                })
        );
    }
}

#[test]
fn compile_runtime_definition_uses_native_nested_rotate_and_shift_places() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let rotatef = parse_items("(rotatef (car xs) (cdr ys))");
    state
        .compile_runtime_definition(function, Span::new(0, 1), &rotatef)
        .expect("nested ROTATEF should compile");
    assert!(state.functions[function].instructions.iter().any(|instruction| matches!(instruction, Instruction::RotatefNestedList(places) if places.len() == 2)));

    let shiftf = parse_items("(shiftf (car (car xs)) (cdr ys) 9)");
    state
        .compile_runtime_definition(function, Span::new(0, 1), &shiftf)
        .expect("nested SHIFTF should compile");
    assert!(state.functions[function].instructions.iter().any(|instruction| matches!(instruction, Instruction::ShiftfNestedList(places) if places.len() == 2)));
}

#[test]
fn compile_runtime_definition_uses_native_single_place_rotatef() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let items = parse_items("(rotatef a)");

    state
        .compile_runtime_definition(function, Span::new(0, 1), &items)
        .expect("single-place ROTATEF should compile natively");
    assert!(
        state.functions[function]
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::RotatefSymbols(places) if places.len() == 1))
    );
}

#[test]
fn compile_runtime_definition_uses_native_pushnew_options_for_symbol_places() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let items = parse_items("(pushnew 1 xs :test-not #'equal :key #'identity)");

    state
        .compile_runtime_definition(function, Span::new(0, 1), &items)
        .expect("PUSHNEW options should compile");

    assert!(
        state.functions[function]
            .instructions
            .iter()
            .any(|instruction| {
                matches!(
                    instruction,
                    Instruction::PushNewListOptions {
                        name,
                        escaped: false,
                        test_not: true,
                        has_key: true,
                        key_before_test: false,
                    } if name == "XS"
                )
            })
    );
}

#[test]
fn compile_runtime_definition_preserves_pushnew_key_before_test_order() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let items = parse_items("(pushnew 1 xs :key #'identity :test #'equal)");

    state
        .compile_runtime_definition(function, Span::new(0, 1), &items)
        .expect("PUSHNEW options should compile");

    assert!(
        state.functions[function]
            .instructions
            .iter()
            .any(|instruction| {
                matches!(
                    instruction,
                    Instruction::PushNewListOptions {
                        name,
                        escaped: false,
                        test_not: false,
                        has_key: true,
                        key_before_test: true,
                    } if name == "XS"
                )
            })
    );
}
