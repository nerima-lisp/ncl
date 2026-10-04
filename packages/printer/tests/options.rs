//! Tests for the typed printer option value objects.

#![allow(clippy::unwrap_used, reason = "tests assert on option setup")]

use ncl_object::{Package, Runtime, ThreadContext, Word, set_symbol_value};
use ncl_printer::{PrintBase, PrintCase, PrintOptions};

#[test]
fn print_base_rejects_values_outside_the_cl_range() {
    assert_eq!(PrintBase::new(1), None);
    assert_eq!(PrintBase::new(2).map(PrintBase::get), Some(2));
    assert_eq!(PrintBase::new(36).map(PrintBase::get), Some(36));
    assert_eq!(PrintBase::new(37), None);
}

#[test]
fn options_are_read_through_typed_accessors() {
    let Some(base) = PrintBase::new(16) else {
        return;
    };
    let options = PrintOptions::new()
        .with_case(PrintCase::Downcase)
        .with_escape_mode(ncl_printer::EscapeMode::Raw)
        .with_readability_mode(ncl_printer::ReadabilityMode::Readable)
        .with_print_base(base);

    assert_eq!(options.case(), PrintCase::Downcase);
    assert!(!options.escape());
    assert!(options.readably());
    assert_eq!(options.print_base().get(), 16);
}

#[test]
fn options_read_pretty_layout_specials() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_printer::register(&mut ctx, &runtime).unwrap();
    for (name, value) in [
        ("*PRINT-PRETTY*", Word::TRUE),
        ("*PRINT-RIGHT-MARGIN*", Word::fixnum(32)),
        ("*PRINT-MISER-WIDTH*", Word::fixnum(4)),
        ("*PRINT-LINES*", Word::fixnum(2)),
    ] {
        let package = runtime.ensure_package(&mut ctx, "COMMON-LISP").unwrap();
        let symbol = Package::from_word(package)
            .intern(&mut ctx, &runtime, name)
            .unwrap()
            .0;
        set_symbol_value(&mut ctx, symbol, value).unwrap();
    }
    let options = PrintOptions::from_specials(&mut ctx, &runtime);
    assert!(options.pretty());
    assert_eq!(options.right_margin(), 32);
    assert_eq!(options.miser_width(), 4);
    assert_eq!(
        options.print_lines().map(ncl_printer::NonNegative::get),
        Some(2)
    );
}
