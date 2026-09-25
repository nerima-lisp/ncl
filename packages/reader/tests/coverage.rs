#![allow(clippy::unwrap_used, reason = "tests assert on coverage failures")]

//! Acceptance test for the ownership gate: `ncl-reader` must register every
//! Phase 1 symbol the reader ownership table assigns to it.

use ncl_object::{Runtime, ThreadContext};

#[test]
fn registers_every_owned_symbol() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_reader::register(&runtime).unwrap();
    let table = include_str!("../ownership.tsv");
    ncl_ownership::assert_crate_coverage_from_table(&runtime, &mut ctx, table, "ncl-reader")
        .unwrap();
}
