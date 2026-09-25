#![allow(clippy::unwrap_used, reason = "tests assert on coverage failures")]

//! Acceptance tests for the ownership gate against the embedded table.

use ncl_object::{Package, Runtime, ThreadContext, Word};
use ncl_ownership::{
    Kind, Missing, OwnershipError, assert_crate_coverage, assert_crate_function_bindings_from_table,
    rows, rows_for_crate,
};

/// The table the crate embeds, read again to derive expected counts.
const TABLE: &str = include_str!("../../../conformance/ownership/symbols.tsv");

#[test]
fn every_ownership_row_parses() {
    assert_eq!(rows().unwrap().len(), TABLE.lines().count() - 1);
}

#[test]
fn rows_for_crate_matches_the_table() {
    let expected = TABLE
        .lines()
        .filter(|line| {
            let mut columns = line.split('\t');
            columns.nth(3) == Some("ncl-types") && columns.next() == Some("1")
        })
        .count();
    assert_eq!(rows_for_crate("ncl-types", 1).unwrap().len(), expected);
    assert!(expected > 0);
}

#[test]
fn empty_runtime_reports_every_phase_one_row() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let expected = rows_for_crate("ncl-types", 1).unwrap().len();
    let error = assert_crate_coverage(&runtime, &mut ctx, "ncl-types").unwrap_err();
    match error {
        OwnershipError::Missing(missing) => assert_eq!(missing.len(), expected),
        other => panic!("expected Missing, got {other}"),
    }
}

#[test]
fn failure_report_lists_one_line_per_missing_symbol() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let error = assert_crate_coverage(&runtime, &mut ctx, "ncl-types").unwrap_err();
    let report = error.to_string();
    assert_eq!(
        report.lines().count(),
        rows_for_crate("ncl-types", 1).unwrap().len()
    );
    assert!(report.contains("COMMON-LISP::"));
    assert_eq!(format!("{error:?}"), report);
}

#[test]
fn unknown_crate_reports_no_rows() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let error = assert_crate_coverage(&runtime, &mut ctx, "ncl-not-a-crate").unwrap_err();
    assert!(matches!(error, OwnershipError::NoRows { .. }));
    assert!(
        error
            .to_string()
            .contains("no Phase 1 rows for crate ncl-not-a-crate")
    );
}

#[test]
fn fully_registered_crate_passes() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let rows = rows_for_crate("ncl-lib-format", 1).unwrap();
    assert!(!rows.is_empty());
    for row in &rows {
        let package = Package::from(runtime.find_package(&ctx, &row.package).unwrap());
        package.intern(&mut ctx, &runtime, &row.symbol).unwrap();
        if row.kind.contains(&Kind::Function) {
            runtime
                .define_function(&mut ctx, &row.package, &row.symbol, Word::fixnum(1))
                .unwrap();
        }
    }
    assert_crate_coverage(&runtime, &mut ctx, "ncl-lib-format").unwrap();
}

#[test]
fn interned_symbol_reports_the_missing_function() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let rows = rows_for_crate("ncl-lib-format", 1).unwrap();
    let row = &rows[0];
    let package = Package::from(runtime.find_package(&ctx, &row.package).unwrap());
    package.intern(&mut ctx, &runtime, &row.symbol).unwrap();
    let error = assert_crate_coverage(&runtime, &mut ctx, "ncl-lib-format").unwrap_err();
    match error {
        OwnershipError::Missing(missing) => {
            assert_eq!(missing.len(), 1);
            assert_eq!(missing[0].symbol, row.symbol);
            assert_eq!(missing[0].reason, "function not registered");
        }
        other => panic!("expected Missing, got {other}"),
    }
}

#[test]
fn interned_class_reports_the_missing_class() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let rows = rows_for_crate("ncl-types", 1).unwrap();
    let row = rows
        .iter()
        .find(|row| row.package == "COMMON-LISP" && row.kind.contains(&Kind::Class))
        .unwrap();
    let package = Package::from(runtime.find_package(&ctx, &row.package).unwrap());
    package.intern(&mut ctx, &runtime, &row.symbol).unwrap();
    let error = assert_crate_coverage(&runtime, &mut ctx, "ncl-types").unwrap_err();
    let missing = match error {
        OwnershipError::Missing(missing) => missing,
        other => panic!("expected Missing, got {other}"),
    };
    assert!(
        missing
            .iter()
            .any(|entry| entry.symbol == row.symbol && entry.reason == "class not registered")
    );
}

#[test]
fn registered_class_is_not_reported() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let rows = rows_for_crate("ncl-types", 1).unwrap();
    let row = rows
        .iter()
        .find(|row| row.package == "COMMON-LISP" && row.kind == vec![Kind::Class])
        .unwrap();
    let package = Package::from(runtime.find_package(&ctx, &row.package).unwrap());
    package.intern(&mut ctx, &runtime, &row.symbol).unwrap();
    runtime
        .define_class(&mut ctx, row.symbol.as_str(), Word::fixnum(1))
        .unwrap();
    let error = assert_crate_coverage(&runtime, &mut ctx, "ncl-types").unwrap_err();
    let missing = match error {
        OwnershipError::Missing(missing) => missing,
        other => panic!("expected Missing, got {other}"),
    };
    assert!(!missing.iter().any(|entry| entry.symbol == row.symbol));
}

fn intern_symbol(runtime: &Runtime, ctx: &mut ThreadContext, package: &str, name: &str) -> Word {
    Package::from(runtime.find_package(ctx, package).unwrap())
        .intern(ctx, runtime, name)
        .unwrap()
        .0
}

fn missing_entries(runtime: &Runtime, ctx: &mut ThreadContext, crate_name: &str) -> Vec<Missing> {
    let error = assert_crate_coverage(runtime, ctx, crate_name).unwrap_err();
    match error {
        OwnershipError::Missing(missing) => missing,
        other => panic!("expected Missing, got {other}"),
    }
}

#[test]
fn macro_variable_and_constant_kinds_require_their_flag_bits() {
    let cases = [
        ("ncl-lib-macros", Kind::Macro, "macro bit not set"),
        ("ncl-lib-streams", Kind::Variable, "special bit not set"),
        (
            "ncl-lib-hash-arrays",
            Kind::Constant,
            "constant bit not set",
        ),
    ];
    for (crate_name, kind, reason) in cases {
        let runtime = Runtime::new().unwrap();
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime).unwrap();
        let row = rows_for_crate(crate_name, 1)
            .unwrap()
            .into_iter()
            .find(|row| row.kind == vec![kind])
            .unwrap();
        intern_symbol(&runtime, &mut ctx, &row.package, &row.symbol);
        let missing = missing_entries(&runtime, &mut ctx, crate_name);
        assert!(
            missing
                .iter()
                .any(|entry| entry.symbol == row.symbol && entry.reason == reason),
            "expected {reason:?} for {}::{}",
            row.package,
            row.symbol
        );
        let symbol = intern_symbol(&runtime, &mut ctx, &row.package, &row.symbol);
        match kind {
            Kind::Macro => ncl_object::set_symbol_macro(&mut ctx, symbol, true).unwrap(),
            Kind::Variable => ncl_object::set_symbol_special(&mut ctx, symbol, true).unwrap(),
            Kind::Constant => ncl_object::set_symbol_constant(&mut ctx, symbol, true).unwrap(),
            _ => unreachable!(),
        }
        let missing = missing_entries(&runtime, &mut ctx, crate_name);
        assert!(
            !missing
                .iter()
                .any(|entry| entry.symbol == row.symbol && entry.reason == reason),
            "{}::{} still missing after setting {kind:?}",
            row.package,
            row.symbol
        );
    }
}

#[test]
fn strict_function_check_rejects_an_unbound_symbol_cell() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let table = "package\tsymbol\tkind\tcrate\tphase\tdirect-expansion\tnotes\nTEST\tFOO\tfunction\ttest\t1\tno\t\n";
    runtime.ensure_package(&mut ctx, "TEST").unwrap();
    let symbol = intern_symbol(&runtime, &mut ctx, "TEST", "FOO");
    runtime
        .define_function(&mut ctx, "TEST", "FOO", Word::NIL)
        .unwrap();
    let error = assert_crate_function_bindings_from_table(&runtime, &mut ctx, table, "test")
        .unwrap_err();
    assert!(error.to_string().contains("symbol function cell unbound"));
    assert_eq!(ncl_object::symbol_function(&ctx, symbol).unwrap(), Word::UNBOUND);
}
