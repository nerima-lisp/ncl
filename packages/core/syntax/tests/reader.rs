use ncl_syntax::{FormKind, MAX_NESTING_DEPTH, ReadErrorKind, read};

#[test]
fn reads_lists_prefixes_and_literals() {
    let forms = read("(+ 1 2) '(a . b) #(\"x\" #\\Space #t)").unwrap();

    assert_eq!(forms.len(), 3);
    assert!(matches!(forms[0].kind, FormKind::List(_)));
    assert!(matches!(forms[1].kind, FormKind::List(_)));
    assert!(matches!(forms[2].kind, FormKind::Vector(_)));
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
fn reads_delimiter_characters() {
    let forms = read(r#"#\) #\;"#).unwrap();

    assert!(matches!(forms[0].kind, FormKind::Character(')')));
    assert!(matches!(forms[1].kind, FormKind::Character(';')));
    assert!(matches!(
        read("#\\ ").unwrap()[0].kind,
        FormKind::Character(' ')
    ));
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
