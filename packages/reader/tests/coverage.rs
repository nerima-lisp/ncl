#![allow(clippy::unwrap_used, reason = "tests assert on coverage failures")]

//! Acceptance test for the ownership gate: `ncl-reader` must register every
//! Phase 1 symbol the reader ownership table assigns to it.

use ncl_object::{Package, Runtime, ThreadContext, make_string, pop_root, push_root};

#[test]
fn registers_every_owned_symbol() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_reader::register(&runtime).unwrap();
    let table = include_str!("../ownership.tsv");
    let rows = ncl_ownership::rows_for_crate_from_str(table, "ncl-reader", 1).unwrap();
    let implemented_table = table
        .lines()
        .filter(|line| {
            let mut fields = line.split('\t');
            let _package = fields.next();
            let _symbol = fields.next();
            !matches!(fields.next(), Some("function" | "class"))
        })
        .collect::<Vec<_>>()
        .join("\n");
    ncl_ownership::assert_crate_coverage_from_table(
        &runtime,
        &mut ctx,
        &implemented_table,
        "ncl-reader",
    )
    .unwrap();
    for row in rows {
        let mut package = runtime.find_package(&ctx, &row.package).unwrap();
        let package_token = push_root(&mut ctx, &mut package);
        let mut name =
            make_string(&mut ctx, &runtime, &row.symbol.chars().collect::<Vec<_>>()).unwrap();
        let name_token = push_root(&mut ctx, &mut name);
        assert!(
            Package::from_word(package)
                .find_symbol(&mut ctx, name)
                .unwrap()
                .is_some(),
            "{}::{} was not interned",
            row.package,
            row.symbol
        );
        assert!(pop_root(&mut ctx, name_token));
        assert!(pop_root(&mut ctx, package_token));
    }
}

#[test]
fn registration_survives_gc_and_does_not_install_unbound_builtins() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);

    ncl_reader::register(&runtime).unwrap();

    for name in [
        "COPY-READTABLE",
        "GET-DISPATCH-MACRO-CHARACTER",
        "GET-MACRO-CHARACTER",
        "MAKE-DISPATCH-MACRO-CHARACTER",
        "PARSE-INTEGER",
        "READ",
        "READ-DELIMITED-LIST",
        "READ-FROM-STRING",
        "READ-PRESERVING-WHITESPACE",
        "READTABLE-CASE",
        "READTABLEP",
        "SET-DISPATCH-MACRO-CHARACTER",
        "SET-MACRO-CHARACTER",
        "SET-SYNTAX-FROM-CHAR",
    ] {
        assert_eq!(runtime.function(&mut ctx, "COMMON-LISP", name), None);
    }
    assert_eq!(runtime.class(&mut ctx, "READTABLE"), None);
}
