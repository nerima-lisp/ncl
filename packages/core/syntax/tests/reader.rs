#![allow(clippy::expect_used, clippy::unwrap_used, missing_docs)]

use ncl_syntax::{FormKind, MAX_NESTING_DEPTH, ReadErrorKind, Span, read};

#[test]
fn reads_lists_prefixes_and_literals() {
    let forms = read("(+ 1 2) '(a . b) #(\"x\" #\\Space #t #f)").unwrap();

    assert_eq!(forms.len(), 3);
    assert!(matches!(forms[0].kind, FormKind::List(_)));
    assert!(matches!(forms[1].kind, FormKind::List(_)));
    assert!(matches!(forms[2].kind, FormKind::Vector(_)));
}

#[test]
fn expands_complex_dispatch_literal_to_constructor_form() {
    let forms = read("#C(2 3)").unwrap();

    assert_eq!(forms[0].to_string(), "(complex 2 3)");
}

#[test]
fn reader_conditionals_include_or_skip_one_form() {
    let forms = read("#+:ncl active #-:ncl skipped after").unwrap();

    assert_eq!(
        forms.iter().map(ToString::to_string).collect::<Vec<_>>(),
        ["active", "after"]
    );
}

#[test]
fn reader_conditionals_support_boolean_feature_expressions() {
    let forms = read("#+(and :ncl (not :other)) yes #- (or :ncl :other) no").unwrap();

    assert_eq!(forms.len(), 1);
    assert_eq!(forms[0].to_string(), "yes");
}

#[test]
fn reader_conditionals_accept_custom_features_case_insensitively() {
    let forms = ncl_syntax::Reader::with_features("#+:Custom yes", ["custom"])
        .read_all()
        .unwrap();

    assert_eq!(forms[0].to_string(), "yes");
}

#[test]
fn comments_are_ignored_and_spans_are_source_offsets() {
    let forms = read("; comment\n(+ 1 2)").unwrap();

    assert_eq!(forms[0].span.start, 10);
    assert_eq!(forms[0].span.end, 17);
}

#[test]
fn nested_block_comments_are_ignored() {
    let forms = read("#| outer #| nested |# outer |# (+ 1 2) #| tail |# 42").unwrap();

    assert_eq!(forms.len(), 2);
    assert!(matches!(forms[0].kind, FormKind::List(_)));
    assert!(matches!(
        &forms[1].kind,
        FormKind::Atom(atom) if atom == "42"
    ));
}

#[test]
fn unterminated_block_comments_report_eof() {
    let error = read("#| outer #| nested |#").unwrap_err();

    assert!(matches!(
        error.kind,
        ReadErrorKind::UnexpectedEnd {
            context: "block comment"
        }
    ));
    assert_eq!(error.span.start, 0);
}

#[test]
fn malformed_input_has_a_typed_error_and_span() {
    let error = read("(+ 1").unwrap_err();

    assert!(matches!(
        error.kind,
        ReadErrorKind::UnexpectedEnd { context: "list" }
    ));
    assert_eq!(error.span.start, 0);
}

#[test]
fn reports_eof_context_for_incomplete_list_forms() {
    for source in ["(item", "(item "] {
        assert_eq!(
            read(source).unwrap_err().kind,
            ReadErrorKind::UnexpectedEnd { context: "list" },
            "source: {source}"
        );
    }
    assert_eq!(
        read("(item .").unwrap_err().kind,
        ReadErrorKind::MissingDottedTail
    );
}

#[test]
fn rejects_unmatched_closing_delimiters() {
    let error = read(")").unwrap_err();

    assert!(matches!(
        error.kind,
        ReadErrorKind::UnexpectedClosingDelimiter { delimiter: ')' }
    ));
    assert_eq!(error.span, ncl_syntax::Span::new(0, 1));
}

#[test]
fn discarded_forms_are_not_returned() {
    let forms = read("#;(ignored form) 42").unwrap();

    assert_eq!(forms.len(), 1);
    assert!(matches!(
        &forms[0].kind,
        FormKind::Atom(atom) if atom == "42"
    ));
}

#[test]
fn dispatch_booleans_require_a_token_boundary() {
    let error = read("#true").unwrap_err();

    assert!(matches!(error.kind, ReadErrorKind::InvalidDispatch));
}

#[test]
fn malformed_prefix_and_dispatch_forms_report_typed_errors() {
    let cases = [
        ("'", "quote"),
        ("`", "quasiquote"),
        (",", "unquote"),
        (",@", "unquote-splicing"),
        ("#'", "function"),
        ("#", "dispatch macro"),
        ("#;", "discarded form"),
    ];

    for (source, context) in cases {
        assert_eq!(
            read(source).unwrap_err().kind,
            ReadErrorKind::UnexpectedEnd { context },
            "source: {source}"
        );
    }
    assert_eq!(read("#x").unwrap_err().kind, ReadErrorKind::InvalidDispatch);
}

#[test]
fn unrecognized_dispatch_characters_are_rejected() {
    let error = read("#g").unwrap_err();

    assert_eq!(error.kind, ReadErrorKind::InvalidDispatch);
    assert_eq!(error.span, Span::new(0, 1));
}

#[test]
fn a_discarded_form_ending_the_input_reports_a_missing_list_item() {
    let error = read("(#;1").unwrap_err();

    assert_eq!(
        error.kind,
        ReadErrorKind::UnexpectedEnd {
            context: "list item"
        }
    );
}

#[test]
fn reads_radix_integer_dispatch() {
    let forms = read("#xFF #b1010 #o777 #3r120").unwrap();

    let atoms = forms
        .iter()
        .map(|form| match &form.kind {
            FormKind::Atom(atom) => atom.as_str(),
            _ => panic!("expected atom form"),
        })
        .collect::<Vec<_>>();
    assert_eq!(atoms, ["#xFF", "#b1010", "#o777", "#3r120"]);

    for source in ["#b", "#x-", "#b2", "#o8", "#37r1"] {
        assert_eq!(
            read(source).unwrap_err().kind,
            ReadErrorKind::InvalidDispatch,
            "source: {source}"
        );
    }
}

#[test]
fn reads_uninterned_symbol_dispatch() {
    let forms = read("#:foo #:Bar").unwrap();

    assert!(matches!(&forms[0].kind, FormKind::Atom(atom) if atom == "#:foo"));
    assert!(matches!(&forms[1].kind, FormKind::Atom(atom) if atom == "#:Bar"));
    assert!(matches!(
        read("#:").unwrap_err().kind,
        ReadErrorKind::InvalidDispatch
    ));
}

#[test]
fn dotted_lists_require_a_tail() {
    let error = read("(a .)").unwrap_err();

    assert!(matches!(error.kind, ReadErrorKind::MissingDottedTail));
}

#[test]
fn unterminated_dotted_lists_report_eof() {
    let error = read("(a . b").unwrap_err();

    assert!(matches!(
        error.kind,
        ReadErrorKind::UnexpectedEnd {
            context: "dotted list"
        }
    ));
}

#[test]
fn dotted_lists_reject_extra_forms() {
    let error = read("(a . b c)").unwrap_err();

    assert_eq!(
        error.kind,
        ReadErrorKind::MismatchedDelimiter {
            expected: ')',
            found: 'c'
        }
    );
}

#[test]
fn reads_delimiter_characters() {
    let forms = read(r"#\) #\;").unwrap();

    assert!(matches!(forms[0].kind, FormKind::Character(')')));
    assert!(matches!(forms[1].kind, FormKind::Character(';')));
    assert!(matches!(
        read("#\\ ").unwrap()[0].kind,
        FormKind::Character(' ')
    ));
}

#[test]
fn reads_string_escapes_and_rejects_invalid_forms() {
    let forms = read(r#""line\n\r\t\\\"""#).unwrap();
    assert!(matches!(
        &forms[0].kind,
        FormKind::String(value) if value == "line\n\r\t\\\""
    ));

    let invalid_inputs = [
        (r#""bad\q""#, ReadErrorKind::InvalidEscape),
        (
            r#""unterminated"#,
            ReadErrorKind::UnexpectedEnd { context: "string" },
        ),
        (
            r#""trailing\"#,
            ReadErrorKind::UnexpectedEnd { context: "string" },
        ),
        (r"#\", ReadErrorKind::InvalidCharacterName),
    ];
    for (source, expected) in invalid_inputs {
        assert_eq!(read(source).unwrap_err().kind, expected);
    }
}

#[test]
fn malformed_symbols_report_eof() {
    for (source, context) in [("|unterminated", "symbol"), (r"foo\", "symbol")] {
        assert_eq!(
            read(source).unwrap_err().kind,
            ReadErrorKind::UnexpectedEnd { context },
            "source: {source}"
        );
    }
}

#[test]
fn symbol_escapes_consume_the_following_character() {
    let forms = read(r"foo\ bar").unwrap();

    assert!(matches!(&forms[0].kind, FormKind::Atom(atom) if atom == r"foo\ bar"));
    assert_eq!(forms[0].span, Span::new(0, 8));
}

#[test]
fn character_names_are_case_insensitive_and_strict() {
    let forms = read("#\\SPACE #\\NewLine #\\tab #\\return #\\x").unwrap();
    let characters = forms
        .iter()
        .map(|form| match form.kind {
            FormKind::Character(character) => character,
            _ => panic!("expected character form"),
        })
        .collect::<Vec<_>>();
    assert_eq!(characters, [' ', '\n', '\t', '\r', 'x']);

    for source in ["#\\", "#\\xy"] {
        assert!(matches!(
            read(source).unwrap_err().kind,
            ReadErrorKind::InvalidCharacterName
        ));
    }
}

#[test]
fn deeply_nested_input_has_a_typed_limit_error() {
    let source = format!(
        "{}1{}",
        "(".repeat(MAX_NESTING_DEPTH + 1),
        ")".repeat(MAX_NESTING_DEPTH + 1)
    );
    let error = read(&source).unwrap_err();

    assert!(matches!(
        error.kind,
        ReadErrorKind::NestingTooDeep {
            limit: MAX_NESTING_DEPTH
        }
    ));
}
