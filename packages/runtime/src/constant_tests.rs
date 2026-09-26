use super::Runtime;
use ncl_ir::{Constant, ConstantIndex, Function, FunctionId};
use ncl_object::{Word, simple_vector_ref};
#[test]
fn constant_object_aliases_survive_gc_stress_and_strict_forwarding() -> Result<(), String> {
    let mut runtime = Runtime::new().map_err(|error| format!("runtime: {error:?}"))?;
    runtime.context.set_gc_stress(true);
    runtime.context.set_strict_forwarding(true);
    let function = Function {
        id: FunctionId(0),
        name: "constant-alias".to_owned(),
        params: Vec::new(),
        return_types: Vec::new(),
        blocks: Vec::new(),
        locals: Vec::new(),
        constants: vec![
            Constant::StringBytes(b"stable".to_vec()),
            Constant::Object(ConstantIndex(0)),
            Constant::StringBytes(b"allocation".to_vec()),
            Constant::Object(ConstantIndex(0)),
        ],
        handler_regions: Vec::new(),
        debug: Vec::new(),
    };
    let table = runtime
        .make_constants(&function)
        .map_err(|error| format!("constants: {error:?}"))?;
    let first = simple_vector_ref(&runtime.context, table, 0)
        .map_err(|error| format!("first constant: {error:?}"))?;
    let first_alias = simple_vector_ref(&runtime.context, table, 1)
        .map_err(|error| format!("first alias: {error:?}"))?;
    let second_alias = simple_vector_ref(&runtime.context, table, 3)
        .map_err(|error| format!("second alias: {error:?}"))?;
    // check-added-lines: allow(panic) test-only assertions
    assert_eq!(first, first_alias);
    // check-added-lines: allow(panic) test-only assertions
    assert_eq!(first, second_alias);
    // check-added-lines: allow(panic) test-only assertions
    assert_ne!(first, Word::NIL);
    Ok(())
}
