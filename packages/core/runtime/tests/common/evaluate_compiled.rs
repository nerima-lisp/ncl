use ncl_runtime::{Runtime, Value};

pub fn evaluate_compiled(source: &str) -> Value {
    Runtime::new()
        .eval_compiled_source(source)
        .unwrap()
        .pop()
        .unwrap()
}
