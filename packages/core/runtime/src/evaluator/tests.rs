use crate::{Runtime, Value};

#[test]
fn global_environment_exposes_the_shared_global_scope() {
    let runtime = Runtime::new();
    let environment = runtime.global_environment();
    runtime.define_in("evaluator-tests-marker", Value::Integer(42), &environment);

    let looked_up = runtime
        .lookup_in("evaluator-tests-marker", &runtime.global_environment())
        .unwrap_or_else(|| {
            panic!("marker should be visible through a fresh handle to the global environment")
        });
    assert_eq!(looked_up.to_string(), "42");
}

#[test]
fn default_runtime_installs_builtins_like_new() {
    let runtime = Runtime::default();
    let values = runtime
        .eval_source("(+ 1 2)")
        .unwrap_or_else(|error| panic!("builtin + must work: {error}"));
    assert_eq!(values[0].to_string(), "3");
}
