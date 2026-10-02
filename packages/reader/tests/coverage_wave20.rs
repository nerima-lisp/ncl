#![allow(
    clippy::unwrap_used,
    missing_docs,
    reason = "coverage tests assert on reader behavior"
)]

use ncl_object::{
    ObjectRef, Package, Runtime, ThreadContext, Word, classify_object, symbol_package,
};
use ncl_reader::{ReadError, ReadOptions, parse_integer, read_from_string};

fn setup() -> (Runtime, ThreadContext, ReadOptions) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let opts = ReadOptions::standard(&mut ctx, &runtime).unwrap();
    (runtime, ctx, opts)
}

#[test]
fn package_markers_distinguish_keywords_internal_symbols_and_packages() {
    let (runtime, mut ctx, opts) = setup();

    let keyword = read_from_string(&mut ctx, &runtime, ":tag", &opts)
        .unwrap()
        .unwrap();
    let keyword_package = symbol_package(&ctx, keyword).unwrap();
    assert_ne!(keyword_package, Word::NIL);
    let keyword_name = Package::from_word(keyword_package).name(&ctx).unwrap();
    assert_eq!(ncl_object::string_length(&ctx, keyword_name).unwrap(), 7);

    let internal = read_from_string(&mut ctx, &runtime, "COMMON-LISP::CAR", &opts)
        .unwrap()
        .unwrap();
    assert!(matches!(
        classify_object(&ctx, internal),
        ObjectRef::Symbol(_)
    ));
    assert_eq!(
        symbol_package(&ctx, internal).unwrap(),
        runtime.find_package(&ctx, "COMMON-LISP").unwrap()
    );

    let package = read_from_string(&mut ctx, &runtime, "COMMON-LISP:", &opts)
        .unwrap()
        .unwrap();
    assert!(matches!(
        classify_object(&ctx, package),
        ObjectRef::Package(_)
    ));

    for input in [":", "A:B:C", "MISSING:NAME", "MISSING:"] {
        assert!(
            matches!(
                read_from_string(&mut ctx, &runtime, input, &opts),
                Err(ReadError::InvalidSymbolToken(_)) | Err(ReadError::PackageNotFound(_))
            ),
            "input: {input}"
        );
    }
}

#[test]
fn reader_entry_points_report_empty_and_malformed_boundary_forms() {
    let (runtime, mut ctx, opts) = setup();
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "", &opts).unwrap(),
        None
    );
    assert_eq!(
        read_from_string(&mut ctx, &runtime, ")", &opts).unwrap_err(),
        ReadError::UnmatchedRightParen
    );
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#c(", &opts).unwrap_err(),
        ReadError::UnexpectedEof
    );
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#|unterminated", &opts).unwrap_err(),
        ReadError::UnexpectedEof
    );

    let (negative, end) =
        parse_integer(&mut ctx, &runtime, "  -17xyz", None, Some(0), Some(5)).unwrap();
    assert_eq!(negative, Word::fixnum(-17));
    assert_eq!(end, 5);
    assert_eq!(
        parse_integer(&mut ctx, &runtime, "123", None, Some(2), Some(2)).unwrap_err(),
        ReadError::InvalidNumber("123".to_owned())
    );
}
