#![allow(clippy::unwrap_used, reason = "tests assert on printer output")]

//! The ownership gate: `ncl-printer` must register every symbol it owns.

#[test]
fn registers_every_owned_symbol() {
    let runtime = ncl_object::Runtime::new().unwrap();
    let mut ctx = ncl_object::ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_printer::register(&mut ctx, &runtime).unwrap();
    ncl_ownership::assert_crate_coverage(&runtime, &mut ctx, "ncl-printer").unwrap();
}
