use std::error::Error;

use ncl_syntax::{Form, FormKind, ReadError, ReadErrorKind, Span};

#[test]
fn spans_report_lengths_and_empty_ranges() {
    let cases = [
        (Span::new(2, 7), 5, false),
        (Span::new(4, 4), 0, true),
        (Span::new(8, 3), 0, true),
    ];

    for (span, length, is_empty) in cases {
        assert_eq!(span.len(), length);
        assert_eq!(span.is_empty(), is_empty);
    }
}

#[test]
fn forms_display_each_form_kind() {
    let atom = Form::atom("name", Span::new(0, 4));
    let cases = [
        (atom.clone(), "name"),
        (
            Form::new(
                FormKind::String("line\nvalue".to_string()),
                Span::new(0, 12),
            ),
            "\"line\\nvalue\"",
        ),
        (Form::new(FormKind::Character(' '), Span::new(0, 3)), "#\\ "),
        (
            Form::list(
                vec![atom.clone(), Form::atom("42", Span::new(5, 7))],
                Span::new(0, 8),
            ),
            "(name 42)",
        ),
        (
            Form::dotted_list(
                vec![atom.clone()],
                Form::atom("tail", Span::new(8, 12)),
                Span::new(0, 13),
            ),
            "(name . tail)",
        ),
        (
            Form::new(
                FormKind::Vector(vec![atom, Form::atom("42", Span::new(5, 7))]),
                Span::new(0, 9),
            ),
            "#(name 42)",
        ),
    ];

    for (form, expected) in cases {
        assert_eq!(form.to_string(), expected);
    }
}

#[test]
fn read_error_kinds_and_spans_are_human_readable() {
    let cases = [
        (
            ReadErrorKind::UnexpectedEnd { context: "list" },
            "unexpected end of input while reading list",
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
            ReadErrorKind::InvalidRadix { radix: 2 },
            "invalid base-2 integer",
        ),
        (
            ReadErrorKind::NestingTooDeep { limit: 128 },
            "reader nesting exceeds limit 128",
        ),
    ];

    for (kind, expected_kind) in cases {
        let error = ReadError::new(kind, Span::new(3, 9));
        assert_eq!(error.to_string(), format!("{expected_kind} at byte 3..9"));
        assert!(error.source().is_none());
    }
}
