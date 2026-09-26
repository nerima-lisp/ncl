//! Strict ownership coverage for the numbers registration boundary.

use ncl_object::{Runtime, ThreadContext};

#[test]
fn numbers_registration_binds_every_owned_symbol() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut context = ThreadContext::new();
    context
        .register(&runtime)
        .unwrap_or_else(|error| panic!("context: {error:?}"));
    ncl_lib_numbers::register(&runtime).unwrap_or_else(|error| panic!("numbers: {error:?}"));
    ncl_ownership::assert_crate_coverage_from_table(
        &runtime,
        &mut context,
        include_str!("../ownership.tsv"),
        "ncl-lib-numbers",
    )
    .unwrap_or_else(|error| panic!("coverage: {error:?}"));
    ncl_ownership::assert_crate_function_bindings_from_table(
        &runtime,
        &mut context,
        include_str!("../ownership.tsv"),
        "ncl-lib-numbers",
    )
    .unwrap_or_else(|error| panic!("bindings: {error:?}"));
}
