//! Public error contracts for the runtime facade.

#![allow(clippy::unwrap_used, reason = "tests assert on error values")]

use std::io;

use ncl_reader::ReadError;
use ncl_runtime::RuntimeError;

#[test]
fn runtime_errors_preserve_categories_and_display_text() {
    let io_error = RuntimeError::from(io::Error::new(io::ErrorKind::NotFound, "missing"));
    assert!(matches!(io_error, RuntimeError::Io { ref path, .. } if path == "<source>"));
    assert!(
        io_error
            .to_string()
            .contains("cannot read <source>: missing")
    );

    let native_error = RuntimeError::Native("bad entry".to_owned());
    assert_eq!(native_error.to_string(), "native error: bad entry");

    let undefined = RuntimeError::UndefinedFunction {
        name: "NO-SUCH-FUNCTION".to_owned(),
    };
    assert_eq!(
        undefined.to_string(),
        "undefined function UNDEFINED-FUNCTION: NO-SUCH-FUNCTION"
    );

    let object_error = RuntimeError::from(ncl_object::ObjectError::TypeError);
    assert_eq!(object_error.to_string(), "object error: TypeError");
}

#[test]
fn incomplete_reader_errors_are_distinguished_from_complete_errors() {
    let incomplete = RuntimeError::from(ReadError::UnexpectedEof);
    assert!(incomplete.is_incomplete_read());
    assert_eq!(incomplete.to_string(), "read error: UnexpectedEof");

    let complete = RuntimeError::from(ReadError::UnmatchedRightParen);
    assert!(!complete.is_incomplete_read());
}
