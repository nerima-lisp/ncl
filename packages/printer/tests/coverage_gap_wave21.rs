#![allow(
    clippy::unwrap_used,
    missing_docs,
    reason = "coverage tests assert on printer error behavior"
)]

use ncl_object::ObjectError;
use ncl_printer::PrintError;

#[test]
fn print_error_variants_keep_display_and_source_contracts() {
    let object = PrintError::from(ObjectError::TypeError);
    assert_eq!(object.to_string(), "print: object error: TypeError");
    assert_eq!(
        std::error::Error::source(&object)
            .expect("object errors expose their source")
            .to_string(),
        "TypeError"
    );

    for (error, expected) in [
        (
            PrintError::Sink("closed".to_owned()),
            "print: sink error: closed",
        ),
        (
            PrintError::NotReadable,
            "print: object is not readable",
        ),
        (
            PrintError::Circularity,
            "print: circular structure without *print-circle*",
        ),
    ] {
        assert_eq!(error.to_string(), expected);
        assert!(std::error::Error::source(&error).is_none());
    }
}
