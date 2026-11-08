use super::*;
use crate::PublishedFunction;

#[test]
fn function_entry_constants_resolve_pending_and_registered_functions() -> Result<(), String> {
    let mut runtime = Runtime::new().map_err(|error| format!("runtime: {error:?}"))?;
    let id = FunctionId(u32::MAX);
    let function = Function {
        id: FunctionId(0),
        name: "function-entry-constant".to_owned(),
        params: Vec::new(),
        return_types: Vec::new(),
        blocks: Vec::new(),
        locals: Vec::new(),
        constants: vec![Constant::FunctionEntry(id)],
        handler_regions: Vec::new(),
        debug: Vec::new(),
    };

    runtime
        .evaluator
        .state
        .functions
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(id.0, PublishedFunction { entry: 0x5678 });

    let mut pending = BTreeMap::new();
    pending.insert(id.0, PublishedFunction { entry: 0x1234 });
    let pending_table = runtime
        .make_constants(
            &function,
            &pending,
            &mut HashMap::new(),
            &mut HashMap::new(),
        )
        .map_err(|error| format!("pending function entry: {error:?}"))?;
    assert_eq!(
        simple_vector_ref(&runtime.context, pending_table, 0)
            .map_err(|error| format!("pending constant lookup: {error:?}"))?
            .bits(),
        0x1234
    );

    let registered_table = runtime
        .make_constants(
            &function,
            &BTreeMap::new(),
            &mut HashMap::new(),
            &mut HashMap::new(),
        )
        .map_err(|error| format!("registered function entry: {error:?}"))?;
    assert_eq!(
        simple_vector_ref(&runtime.context, registered_table, 0)
            .map_err(|error| format!("registered constant lookup: {error:?}"))?
            .bits(),
        0x5678
    );

    let missing = u32::MAX - 1;
    let missing_function = Function {
        constants: vec![Constant::FunctionEntry(FunctionId(missing))],
        ..function
    };
    assert!(matches!(
        runtime.make_constants(
            &missing_function,
            &BTreeMap::new(),
            &mut HashMap::new(),
            &mut HashMap::new(),
        ),
        Err(RuntimeError::Native(message)) if message == format!("function entry {missing} is unavailable")
    ));
    Ok(())
}
