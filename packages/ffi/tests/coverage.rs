#![allow(clippy::unwrap_used, reason = "tests assert on coverage failures")]

//! Ownership coverage gate: every Phase-1 `NCL-FFI` symbol is registered.

const OWNERSHIP: &str = include_str!("../ownership.tsv");

#[test]
fn registers_every_owned_symbol() {
    let runtime = ncl_object::Runtime::new().unwrap();
    let mut ctx = ncl_object::ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_ffi::register(&runtime).unwrap();
    ncl_ownership::assert_crate_coverage_from_table(&runtime, &mut ctx, OWNERSHIP, "ncl-ffi")
        .unwrap();
}
