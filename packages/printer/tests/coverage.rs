#![allow(clippy::unwrap_used, reason = "tests assert on printer output")]

//! The ownership gate: `ncl-printer` must register every symbol it owns.

#[test]
fn registers_every_owned_symbol() {
    let runtime = ncl_object::Runtime::new().unwrap();
    let mut ctx = ncl_object::ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_printer::register(&mut ctx, &runtime).unwrap();
    let table = include_str!("../ownership.tsv");
    ncl_ownership::assert_crate_coverage_from_table(&runtime, &mut ctx, table, "ncl-printer")
        .unwrap();
}

#[test]
fn registers_ncl_ext_printer_extensions() {
    let runtime = ncl_object::Runtime::new().unwrap();
    let mut ctx = ncl_object::ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_printer::register(&mut ctx, &runtime).unwrap();

    for name in ["PRINT-SYMBOL-WITH-PREFIX", "PRINT-UNREADABLY"] {
        assert!(
            runtime.function(&mut ctx, "NCL-EXT", name).is_some(),
            "{name}"
        );
    }
    let package = runtime.find_package(&ctx, "NCL-EXT").unwrap();
    for name in ["*PRINT-CIRCLE-NOT-SHARED*", "*PRINT-VECTOR-LENGTH*"] {
        let symbol = ncl_object::Package::from_word(package)
            .intern(&mut ctx, &runtime, name)
            .unwrap()
            .0;
        assert!(
            ncl_object::symbol_is_special(&ctx, symbol).unwrap(),
            "{name}"
        );
    }
}
