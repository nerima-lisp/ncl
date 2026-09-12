use super::*;

fn parse_items(source: &str) -> Vec<Form> {
    let forms = ncl_syntax::read(source)
        .unwrap_or_else(|error| panic!("test source should parse: {error}"));
    let FormKind::List(items) = &forms[0].kind else {
        panic!("expected invocation");
    };
    items.clone()
}

#[test]
fn setf_keeps_invocation_and_compiles_each_rhs_in_a_child() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let items = parse_items("(setf (aref xs index) (setq index 1) (car ys) 9)");
    state
        .compile_setf(function, span, &items)
        .unwrap_or_else(|error| panic!("SETF should compile: {error}"));
    assert_eq!(
        state.functions[function].instructions,
        [Instruction::SetfPlaces {
            invocation: Form::list(items, span),
            values: vec![function + 1, function + 2],
        }]
    );
    assert_eq!(
        state.functions[function + 1].instructions,
        [
            Instruction::Constant(Constant::Integer(1)),
            Instruction::Set("INDEX".to_string()),
            Instruction::Return,
        ]
    );
    assert_eq!(
        state.functions[function + 2].instructions,
        [
            Instruction::Constant(Constant::Integer(9)),
            Instruction::Return,
        ]
    );
}

#[test]
fn intrinsic_store_is_terminal_but_its_nested_setf_is_not() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let items = parse_items("(%setf-intrinsic-store (car xs) (setf (car ys) 9))");
    state
        .compile_setf_intrinsic_store(function, span, &items)
        .unwrap_or_else(|error| panic!("intrinsic store should compile: {error}"));
    assert!(matches!(&state.functions[function].instructions[..], [
        Instruction::SetfPlaces { .. }, Instruction::Setf(place)
    ] if place == &items[1]));
}

#[test]
fn setf_rejects_an_unpaired_place() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let items = parse_items("(setf x 1 y)");
    let error = state
        .compile_setf(function, Span::new(0, 1), &items)
        .map_or_else(|error| error, |()| panic!("unpaired place must fail"));
    assert!(matches!(error.kind, CompileErrorKind::InvalidForm { .. }));
}

#[test]
fn intrinsic_store_rejects_wrong_arity() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let items = parse_items("(%setf-intrinsic-store x)");
    let error = state
        .compile_setf_intrinsic_store(function, Span::new(0, 1), &items)
        .map_or_else(
            |error| error,
            |()| panic!("intrinsic store needs two operands"),
        );
    assert!(matches!(error.kind, CompileErrorKind::Arity { .. }));
}
