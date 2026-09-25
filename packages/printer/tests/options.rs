//! Tests for the typed printer option value objects.

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
