use super::*;
use ncl_syntax::read;

#[test]
fn compiles_default_parameter_functions_into_separate_code() -> Result<(), String> {
    let form = read("(&optional (value 10) &key (limit 20) &aux (state 30))")
        .map_err(|error| error.to_string())?
        .remove(0);
    let lambda_list = CompileState::parameters(&form).map_err(|error| error.to_string())?;
    let mut state = CompileState::default();
    let optional = state
        .compile_optional_parameters(&lambda_list.optional)
        .map_err(|error| error.to_string())?;
    let keywords = state
        .compile_keyword_parameters(&lambda_list.keywords)
        .map_err(|error| error.to_string())?;
    let auxiliary = state
        .compile_auxiliary_parameters(&lambda_list.auxiliary)
        .map_err(|error| error.to_string())?;
    assert_eq!(optional.len(), 1);
    assert_eq!(keywords.len(), 1);
    assert_eq!(auxiliary.len(), 1);
    assert_eq!(state.functions.len(), 3);
    assert!(state.functions.iter().all(|function| matches!(
        function.instructions.as_slice(),
        [Instruction::Constant(_), Instruction::Return]
    )));
    Ok(())
}

fn expect_catch_arity(error: &CompileError) {
    match &error.kind {
        CompileErrorKind::Arity { operator, .. } => assert_eq!(operator, "CATCH"),
        other => panic!("expected the nested CATCH arity error to propagate, got {other:?}"),
    }
}

fn malformed_default(
    source: &str,
    compile: impl FnOnce(&mut CompileState, &OrdinaryLambdaList) -> Result<(), CompileError>,
) -> Result<(), String> {
    let form = read(source).map_err(|error| error.to_string())?.remove(0);
    let list = CompileState::parameters(&form).map_err(|error| error.to_string())?;
    let mut state = CompileState::default();
    let error = compile(&mut state, &list).map_or_else(
        |error| error,
        |()| panic!("malformed default unexpectedly compiled"),
    );
    expect_catch_arity(&error);
    Ok(())
}

#[test]
fn compile_optional_parameters_propagates_malformed_default_value() -> Result<(), String> {
    malformed_default("(&optional (value (catch)))", |state, list| {
        state
            .compile_optional_parameters(&list.optional)
            .map(|_| ())
    })
}

#[test]
fn compile_keyword_parameters_propagates_malformed_default_value() -> Result<(), String> {
    malformed_default("(&key (value (catch)))", |state, list| {
        state.compile_keyword_parameters(&list.keywords).map(|_| ())
    })
}

#[test]
fn compile_auxiliary_parameters_propagates_malformed_default_value() -> Result<(), String> {
    malformed_default("(&aux (value (catch)))", |state, list| {
        state
            .compile_auxiliary_parameters(&list.auxiliary)
            .map(|_| ())
    })
}
