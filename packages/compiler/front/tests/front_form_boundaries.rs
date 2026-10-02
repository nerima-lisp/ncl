#![allow(
    clippy::unwrap_used,
    missing_docs,
    reason = "tests assert on public front-end errors"
)]

use ncl_compiler_front::FrontError;
use ncl_object::{Runtime, ThreadContext, Word};

#[test]
fn form_boundaries_preserve_object_errors_and_unsupported_literals() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let mut table = ncl_compiler_front::form::UninternedTable::new();

    assert!(matches!(
        ncl_compiler_front::form::word_string(&ctx, Word::fixnum(7)),
        Err(FrontError::Object(ncl_object::ObjectError::TypeError))
    ));
    assert!(matches!(
        ncl_compiler_front::form::symbol_ref(&ctx, &mut table, Word::fixnum(7)),
        Err(FrontError::Object(ncl_object::ObjectError::TypeError))
    ));
    assert_eq!(
        ncl_compiler_front::form::number_literal(&mut ctx, Word::TRUE),
        Err(FrontError::UnsupportedLiteral)
    );
    assert_eq!(
        ncl_compiler_front::form::literal(&mut ctx, &mut table, Word::TRUE),
        Err(FrontError::UnsupportedLiteral)
    );
}
