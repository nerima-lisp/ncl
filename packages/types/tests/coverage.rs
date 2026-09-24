#![allow(clippy::unwrap_used, reason = "tests assert on coverage failures")]

//! Acceptance test for the ownership gate against the embedded table.

use ncl_object::{Runtime, ThreadContext};

#[test]
fn registers_every_owned_symbol() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_types::register(&runtime).unwrap();
    ncl_ownership::assert_crate_coverage(&runtime, &mut ctx, "ncl-types").unwrap();
}
