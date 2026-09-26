#![allow(
    missing_docs,
    clippy::unwrap_used,
    reason = "tests assert on registration failures"
)]

use ncl_object::{Runtime, ThreadContext};

#[test]
fn register_all_registers_clos() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();

    ncl_stdlib::register_all(&mut ctx, &runtime).unwrap();

    assert!(runtime.class(&mut ctx, "CLASS").is_some());
    assert!(runtime.function(&mut ctx, "COMMON-LISP", "FLOOR").is_some());
}
