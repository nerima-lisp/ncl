#![allow(clippy::unwrap_used, reason = "tests assert on coverage failures")]

//! Ownership coverage for the NCL-GC extension surface.

#[test]
fn registers_every_owned_symbol() {
    let runtime = ncl_object::Runtime::new().unwrap();
    let mut ctx = ncl_object::ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_object::register(&mut ctx, &runtime).unwrap();
    let table = include_str!("../ownership.tsv");
    ncl_ownership::assert_crate_coverage_from_table(&runtime, &mut ctx, table, "ncl-object")
        .unwrap();
}
