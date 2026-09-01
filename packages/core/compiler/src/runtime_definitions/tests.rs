use super::super::*;

fn parse_items(source: &str) -> Vec<Form> {
    let mut forms = ncl_syntax::read(source).expect("test source should parse");
    match forms.remove(0).kind {
        ncl_syntax::FormKind::List(items) => items,
        form => panic!("expected list form, got {form:?}"),
    }
}

#[test]
fn compile_runtime_mutation_fallback_uses_native_instruction() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let items = parse_items("(push 1 (unknown-place))");

    state
        .compile_runtime_definition(function, Span::new(0, 1), &items)
        .expect("runtime mutation should compile");

    assert!(matches!(
        state.functions[function].instructions.as_slice(),
        [Instruction::RuntimeMutation(_)]
    ));
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
fn compile_defstruct_uses_native_instruction() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let items = parse_items("(defstruct point x)");

    state
        .compile_defstruct(function, Span::new(0, 1), &items)
        .expect("DEFSTRUCT should compile");

    assert!(matches!(
        state.functions[function].instructions.as_slice(),
        [Instruction::Defstruct(_)]
    ));
}

#[test]
fn compile_defclass_uses_native_instruction() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let items = parse_items("(defclass point () ())");

    state
        .compile_defclass(function, Span::new(0, 1), &items)
        .expect("DEFCLASS should compile");

    assert!(matches!(
        state.functions[function].instructions.as_slice(),
        [Instruction::Defclass(_)]
    ));
}

#[test]
fn compile_defgeneric_uses_native_instruction() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let items = parse_items("(defgeneric point-total (object))");

    state
        .compile_defgeneric(function, Span::new(0, 1), &items)
        .expect("DEFGENERIC should compile");

    assert!(matches!(
        state.functions[function].instructions.as_slice(),
        [Instruction::Defgeneric(_)]
    ));
}

#[test]
fn compile_defmethod_uses_native_instruction() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let items = parse_items("(defmethod point-total ((object t)) object)");

    state
        .compile_defmethod(function, Span::new(0, 1), &items)
        .expect("DEFMETHOD should compile");

    assert!(matches!(
        state.functions[function].instructions.as_slice(),
        [Instruction::Defmethod(_)]
    ));
}

#[test]
fn compile_defsetf_uses_native_instruction() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let items = parse_items("(defsetf access writer)");

    state
        .compile_defsetf(function, Span::new(0, 1), &items)
        .expect("DEFSETF should compile");

    assert!(matches!(
        state.functions[function].instructions.as_slice(),
        [Instruction::Defsetf(_)]
    ));
}

#[test]
fn compile_defconstant_uses_native_instruction() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let items = parse_items("(defconstant +answer+ 42)");

    state
        .compile_defconstant(function, Span::new(0, 1), &items)
        .expect("DEFCONSTANT should compile");

    assert!(matches!(
        state.functions[function].instructions.as_slice(),
        [Instruction::Defconstant(_)]
    ));
}

#[test]
fn compile_define_symbol_macro_uses_native_instruction() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let items = parse_items("(define-symbol-macro answer 42)");

    state
        .compile_define_symbol_macro(function, Span::new(0, 1), &items)
        .expect("DEFINE-SYMBOL-MACRO should compile");

    assert!(matches!(
        state.functions[function].instructions.as_slice(),
        [Instruction::DefineSymbolMacro(_)]
    ));
}

#[test]
fn compile_define_modify_macro_uses_native_instruction() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let items = parse_items("(define-modify-macro adjust (delta) +)");

    state
        .compile_define_modify_macro(function, Span::new(0, 1), &items)
        .expect("DEFINE-MODIFY-MACRO should compile");

    assert!(matches!(
        state.functions[function].instructions.as_slice(),
        [Instruction::DefineModifyMacro(_)]
    ));
}

#[test]
fn compile_define_setf_expander_uses_native_instruction() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let items = parse_items("(define-setf-expander access (place) place)");

    state
        .compile_define_setf_expander(function, Span::new(0, 1), &items)
        .expect("DEFINE-SETF-EXPANDER should compile");

    assert!(matches!(
        state.functions[function].instructions.as_slice(),
        [Instruction::DefineSetfExpander(_)]
    ));
}

#[test]
fn compile_get_setf_expansion_uses_native_instruction() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let items = parse_items("(get-setf-expansion place)");

    state
        .compile_get_setf_expansion(function, Span::new(0, 1), &items)
        .expect("GET-SETF-EXPANSION should compile");

    assert!(matches!(
        state.functions[function].instructions.as_slice(),
        [Instruction::GetSetfExpansion(_)]
    ));
}

#[test]
fn compile_psetf_uses_native_instruction() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let items = parse_items("(psetf first 1 second 2)");

    state
        .compile_psetf(function, Span::new(0, 1), &items)
        .expect("PSETF should compile");

    assert!(matches!(
        state.functions[function].instructions.as_slice(),
        [Instruction::Psetf(_)]
    ));
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
fn compile_load_time_value_uses_native_instruction() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let items = parse_items("(load-time-value (+ 1 2) nil)");

    state
        .compile_load_time_value(function, Span::new(0, 1), &items)
        .expect("LOAD-TIME-VALUE should compile");

    assert!(matches!(
        state.functions[function].instructions.as_slice(),
        [Instruction::LoadTimeValue(_)]
    ));
}

#[test]
fn compile_runtime_definition_uses_native_rotate_and_shift_for_symbol_places() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let rotatef = parse_items("(rotatef a |B| c)");
    state
        .compile_runtime_definition(function, Span::new(0, 1), &rotatef)
        .expect("ROTATEF symbol places should compile");
    assert!(state.functions[function]
        .instructions
        .contains(&Instruction::RotatefSymbols(vec![
            ("A".to_string(), false),
            ("B".to_string(), true),
            ("C".to_string(), false),
        ])));

    let shiftf = parse_items("(shiftf a b 9)");
    state
        .compile_runtime_definition(function, Span::new(0, 1), &shiftf)
        .expect("SHIFTF symbol places should compile");
    assert!(state.functions[function]
        .instructions
        .contains(&Instruction::ShiftfSymbols(vec![
            ("A".to_string(), false),
            ("B".to_string(), false),
        ])));
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
        assert!(state.functions[function]
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::Eval(_))));
        assert!(!state.functions[function]
            .instructions
            .iter()
            .any(|instruction| {
                matches!(
                    instruction,
                    Instruction::RotatefSymbols(_) | Instruction::ShiftfSymbols(_)
                )
            }));
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

    assert!(state.functions[function]
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
        }));
}

#[test]
fn compile_runtime_definition_preserves_pushnew_key_before_test_order() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let items = parse_items("(pushnew 1 xs :key #'identity :test #'equal)");

    state
        .compile_runtime_definition(function, Span::new(0, 1), &items)
        .expect("PUSHNEW options should compile");

    assert!(state.functions[function]
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
        }));
}
