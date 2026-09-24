//! Ownership coverage gate: every Phase-1 `ncl-compiler-front` symbol is
//! registered.
#![allow(clippy::unwrap_used, reason = "tests assert on coverage failures")]

#[test]
fn registers_every_owned_symbol() {
    let runtime = ncl_object::Runtime::new().unwrap();
    let mut ctx = ncl_object::ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_compiler_front::register(&runtime).unwrap();
    ncl_ownership::assert_crate_coverage(&runtime, &mut ctx, "ncl-compiler-front").unwrap();
}
