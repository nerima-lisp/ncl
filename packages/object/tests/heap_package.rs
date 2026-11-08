//! Integration test for the runtime package registry.

use ncl_object::{Runtime, ThreadContext};

#[test]
fn package_registry_survives_vector_reallocation_and_gc() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("Runtime::new failed: {error:?}"));
    let mut ctx = ThreadContext::new();
    assert!(ctx.register(&runtime).is_ok());
    for index in 0..24 {
        let name = format!("P{index}");
        assert!(runtime.ensure_package(&mut ctx, &name).is_ok());
    }
    assert!(ctx.collect(true).is_ok());
    for index in 0..24 {
        let name = format!("P{index}");
        assert!(runtime.find_package(&ctx, &name).is_some());
    }
}
