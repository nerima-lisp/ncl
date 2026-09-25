//! Ownership coverage gate: every Phase-1 `ncl-compiler-front` symbol is
//! registered.
#![allow(clippy::unwrap_used, reason = "tests assert on coverage failures")]

const TABLE: &str = include_str!("../ownership.tsv");

#[test]
fn registers_every_owned_symbol() {
    let runtime = ncl_object::Runtime::new().unwrap();
    let mut ctx = ncl_object::ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_compiler_front::register(&runtime).unwrap();
    ncl_ownership::assert_crate_coverage_from_table(&runtime, &mut ctx, TABLE, "ncl-compiler-front").unwrap();
}
