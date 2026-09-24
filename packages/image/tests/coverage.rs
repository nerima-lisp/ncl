#![allow(clippy::unwrap_used, reason = "tests assert on coverage failures")]

//! Ownership coverage gate: every Phase-1 `ncl-image` symbol is registered.

use ncl_object::{Runtime, ThreadContext};

#[test]
fn registers_every_owned_symbol() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_image::register(&runtime).unwrap();
    ncl_ownership::assert_crate_coverage(&runtime, &mut ctx, "ncl-image").unwrap();
}
