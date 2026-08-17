use std::error::Error;

use ncl_syntax::{ReadError, ReadErrorKind, Span};

fn assert_error_display_cases(cases: impl IntoIterator<Item = (ReadErrorKind, &'static str)>) {
    for (kind, expected) in cases {
        assert_eq!(kind.to_string(), expected, "kind display: {kind:?}");

        let error = ReadError::new(kind.clone(), Span::new(3, 8));
        assert_eq!(
            error.to_string(),
            format!("{expected} at byte 3..8"),
            "error display: {kind:?}"
        );
        assert_eq!(error.kind, kind);
        assert_eq!(error.span, Span::new(3, 8));
        assert!(error.source().is_none());
    }
}

#[test]
fn read_error_kinds_have_human_readable_displays() {
    assert_error_display_cases([
        (
            ReadErrorKind::UnexpectedEnd { context: "string" },
            "unexpected end of input while reading string",
        ),
        (
            ReadErrorKind::UnexpectedClosingDelimiter { delimiter: ')' },
            "unexpected closing delimiter )",
        ),
        (
            ReadErrorKind::MismatchedDelimiter {
                expected: ')',
                found: ']',
            },
            "expected ), found ]",
        ),
        (
            ReadErrorKind::MissingDottedTail,
            "dotted list is missing its tail",
        ),
        (
            ReadErrorKind::MultipleDottedTails,
            "dotted list has more than one dot",
        ),
        (ReadErrorKind::InvalidEscape, "invalid string escape"),
        (
            ReadErrorKind::InvalidCharacterName,
            "invalid character name",
        ),
        (ReadErrorKind::InvalidDispatch, "invalid reader dispatch"),
        (
            ReadErrorKind::NestingTooDeep { limit: 128 },
            "reader nesting exceeds limit 128",
        ),
    ]);
}
