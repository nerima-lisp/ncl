use std::error::Error as _;

use ncl_compiler::{CompileError, CompileErrorKind};
use ncl_syntax::{ReadError, ReadErrorKind, Span};

use crate::error::RuntimeError;

#[test]
fn source_delegates_to_wrapped_read_and_compile_errors() {
    let read_error = ReadError::new(ReadErrorKind::MissingDottedTail, Span::new(0, 1));
    let wrapped_read = RuntimeError::Read(Box::new(read_error.clone()));
    let source = wrapped_read
        .source()
        .unwrap_or_else(|| panic!("read error has a source"));
    assert_eq!(source.to_string(), read_error.to_string());

    let compile_error = CompileError::new(
        CompileErrorKind::Internal {
            message: "bad".to_owned(),
        },
        Span::new(0, 1),
    );
    let wrapped_compile = RuntimeError::Compile(Box::new(compile_error.clone()));
    let source = wrapped_compile
        .source()
        .unwrap_or_else(|| panic!("compile error has a source"));
    assert_eq!(source.to_string(), compile_error.to_string());
}

#[test]
fn source_is_none_for_non_wrapping_variants() {
    assert!(RuntimeError::DivisionByZero.source().is_none());
    assert!(RuntimeError::NumericOverflow.source().is_none());
}
