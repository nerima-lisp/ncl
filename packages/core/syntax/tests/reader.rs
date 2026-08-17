use ncl_syntax::{FormKind, MAX_NESTING_DEPTH, ReadErrorKind, read, read_with_features};

fn assert_single_form_display(source: &str, expected: &str) {
    let forms = read(source).unwrap_or_else(|error| panic!("{source:?}: {error}"));

    assert_eq!(forms.len(), 1, "{source:?}");
    assert_eq!(forms[0].to_string(), expected, "{source:?}");
}

#[test]
fn reads_lists_prefixes_and_literals() {
    let forms = read("(+ 1 2) '(a . b) #(\"x\" #\\Space #t)").unwrap();

    assert_eq!(forms.len(), 3);
    assert!(matches!(forms[0].kind, FormKind::List(_)));
    assert!(matches!(forms[1].kind, FormKind::List(_)));
    assert!(matches!(forms[2].kind, FormKind::Vector(_)));
}

#[test]
fn reads_dispatch_forms_from_table() {
    let cases = [
        ("#b1010", "10"),
        ("#o17", "15"),
        ("#xff", "255"),
        ("#C(1 2)", "(complex 1 2)"),
    ];

    for (source, expected) in cases {
        assert_single_form_display(source, expected);
    }
}

#[test]
fn reads_complex_dispatch_as_constructor_form() {
    let forms = read("#C(1 2) #c(-3 4)").unwrap();

    assert_eq!(forms.len(), 2);
    assert_eq!(forms[0].to_string(), "(complex 1 2)");
    assert_eq!(forms[1].to_string(), "(complex -3 4)");

    let error = read("#C(1)").unwrap_err();
    assert!(matches!(error.kind, ReadErrorKind::InvalidDispatch));
}

#[test]
fn reads_radix_integer_dispatches_as_decimal_atoms() {
    let forms = read("#b101101 #o55 #x2d #X+2D #b-101").unwrap();

    assert_eq!(
        forms.iter().map(ToString::to_string).collect::<Vec<_>>(),
        ["45", "45", "45", "45", "-5"]
    );
    assert_eq!(forms[0].span.start, 0);
    assert_eq!(forms[0].span.end, 8);
    assert_eq!(forms[4].span.start, 25);
    assert_eq!(forms[4].span.end, 31);

    for source in ["#b", "#b+", "#b102", "#xG"] {
        assert!(matches!(
            read(source).unwrap_err().kind,
            ReadErrorKind::InvalidDispatch
        ));
    }
}

#[test]
fn reads_explicit_radix_integer_dispatches() {
    let forms = read("#2r101 #36rZ #36r-z #10r+42 #10r-42").unwrap();

    assert_eq!(
        forms.iter().map(ToString::to_string).collect::<Vec<_>>(),
        ["5", "35", "-35", "42", "-42"]
    );
    assert_eq!(forms[0].span.start, 0);
    assert_eq!(forms[0].span.end, 6);
    assert_eq!(forms[4].span.start, 28);
    assert_eq!(forms[4].span.end, 35);

    for source in ["#2r", "#2r102", "#1r0", "#37r0", "#2q1"] {
        assert!(matches!(
            read(source).unwrap_err().kind,
            ReadErrorKind::InvalidDispatch
        ));
    }
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
fn reader_conditionals_select_and_discard_forms() {
    let enabled = read_with_features("#+:ncl 1 #-:ncl 2", &["ncl"]).unwrap();
    assert_eq!(enabled.len(), 1);
    assert!(matches!(&enabled[0].kind, FormKind::Atom(value) if value == "1"));

    let disabled = read_with_features("#+:ncl 1 #-:ncl 2", &[]).unwrap();
    assert_eq!(disabled.len(), 1);
    assert!(matches!(&disabled[0].kind, FormKind::Atom(value) if value == "2"));
}

#[test]
fn default_reader_features_enable_ncl() {
    let forms = read("#+:ncl 1 #-:ncl 2").unwrap();

    assert_eq!(forms.len(), 1);
    assert!(matches!(&forms[0].kind, FormKind::Atom(value) if value == "1"));
}

#[test]
fn reader_conditionals_support_logical_feature_expressions() {
    let forms =
        read_with_features("#+(and :ncl (not :sbcl)) 11 #+(or :sbcl :ncl) 22", &["ncl"]).unwrap();

    assert_eq!(forms.len(), 2);
    assert!(matches!(&forms[0].kind, FormKind::Atom(value) if value == "11"));
    assert!(matches!(&forms[1].kind, FormKind::Atom(value) if value == "22"));
}

#[test]
fn invalid_reader_conditionals_report_typed_errors() {
    let invalid = read_with_features("#+(xor :ncl) 1", &["ncl"]).unwrap_err();
    assert!(matches!(invalid.kind, ReadErrorKind::InvalidDispatch));

    let missing = read_with_features("#+:ncl", &["ncl"]).unwrap_err();
    assert!(matches!(
        missing.kind,
        ReadErrorKind::UnexpectedEnd {
            context: "conditional form"
        }
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
fn reads_bit_vector_dispatch() {
    let forms = read("#*101 #*").unwrap();

    assert!(matches!(&forms[0].kind, FormKind::Vector(items) if items.len() == 3));
    assert!(matches!(&forms[1].kind, FormKind::Vector(items) if items.is_empty()));
    assert!(matches!(
        read("#*102").unwrap_err().kind,
        ReadErrorKind::InvalidDispatch
    ));
}

#[test]
fn reads_radix_integer_dispatch() {
    let forms = read("#b1010 #o17 #xff #b-11").unwrap();

    assert!(matches!(&forms[0].kind, FormKind::Atom(atom) if atom == "10"));
    assert!(matches!(&forms[1].kind, FormKind::Atom(atom) if atom == "15"));
    assert!(matches!(&forms[2].kind, FormKind::Atom(atom) if atom == "255"));
    assert!(matches!(&forms[3].kind, FormKind::Atom(atom) if atom == "-3"));
    assert!(matches!(
        read("#b").unwrap_err().kind,
        ReadErrorKind::InvalidDispatch
    ));
    assert!(matches!(
        read("#xfg").unwrap_err().kind,
        ReadErrorKind::InvalidDispatch
    ));
}

#[test]
fn reads_complex_literal_dispatch() {
    let forms = read("#C(1 2)").unwrap();

    assert_eq!(forms.len(), 1);
    assert!(matches!(&forms[0].kind, FormKind::List(items)
        if matches!(items.first(), Some(form) if matches!(&form.kind, FormKind::Atom(atom) if atom == "complex"))
        && matches!(items.get(1), Some(form) if matches!(&form.kind, FormKind::Atom(atom) if atom == "1"))
        && matches!(items.get(2), Some(form) if matches!(&form.kind, FormKind::Atom(atom) if atom == "2"))));
    assert!(matches!(
        read("#C1").unwrap_err().kind,
        ReadErrorKind::InvalidDispatch
    ));
    assert!(matches!(
        read("#C(1)").unwrap_err().kind,
        ReadErrorKind::InvalidDispatch
    ));
    assert!(matches!(
        read("#C(1 2 3)").unwrap_err().kind,
        ReadErrorKind::InvalidDispatch
    ));
}

#[test]
fn reads_structure_literal_dispatch() {
    let forms = read("#S(person :name \"Ada\" :age 21)").unwrap();

    assert_eq!(forms.len(), 1);
    assert!(matches!(&forms[0].kind, FormKind::List(items)
        if matches!(items.first(), Some(form) if matches!(&form.kind, FormKind::Atom(atom) if atom == "MAKE-person"))
        && matches!(items.get(1), Some(form) if matches!(&form.kind, FormKind::Atom(atom) if atom == ":name"))
        && matches!(items.get(2), Some(form) if matches!(&form.kind, FormKind::String(value) if value == "Ada"))
        && matches!(items.get(3), Some(form) if matches!(&form.kind, FormKind::Atom(atom) if atom == ":age"))
        && matches!(items.get(4), Some(form) if matches!(&form.kind, FormKind::Atom(atom) if atom == "21"))));
    assert!(matches!(
        read("#S()").unwrap_err().kind,
        ReadErrorKind::InvalidDispatch
    ));
    assert!(matches!(
        read("#S(person :name)").unwrap_err().kind,
        ReadErrorKind::InvalidDispatch
    ));
}

#[test]
fn reads_pathname_literal_dispatch() {
    let forms = read("#P\"/tmp/demo.txt\"").unwrap();

    assert_eq!(forms.len(), 1);
    assert!(matches!(
        &forms[0].kind,
        FormKind::String(value) if value == "/tmp/demo.txt"
    ));
    assert!(matches!(
        read("#Pdemo").unwrap_err().kind,
        ReadErrorKind::InvalidDispatch
    ));
    assert!(matches!(
        read("#P123").unwrap_err().kind,
        ReadErrorKind::InvalidDispatch
    ));
}

#[test]
fn reads_array_literal_dispatch() {
    let forms = read("#2A((1 2) (3 4))").unwrap();

    assert_eq!(forms.len(), 1);
    assert!(matches!(&forms[0].kind, FormKind::List(items)
        if matches!(items.first(), Some(form) if matches!(&form.kind, FormKind::Atom(atom) if atom == "make-array"))
        && matches!(items.get(1), Some(form) if matches!(&form.kind, FormKind::List(quoted)
            if matches!(quoted.first(), Some(head) if matches!(&head.kind, FormKind::Atom(atom) if atom == "quote"))
            && matches!(quoted.get(1), Some(dims) if matches!(&dims.kind, FormKind::List(dims)
                if dims.len() == 2
                && matches!(&dims[0].kind, FormKind::Atom(atom) if atom == "2")
                && matches!(&dims[1].kind, FormKind::Atom(atom) if atom == "2")))))
        && matches!(items.get(2), Some(form) if matches!(&form.kind, FormKind::Atom(atom) if atom == ":initial-contents"))
        && matches!(items.get(3), Some(form) if matches!(&form.kind, FormKind::List(quoted)
            if matches!(quoted.first(), Some(head) if matches!(&head.kind, FormKind::Atom(atom) if atom == "quote"))
            && matches!(quoted.get(1), Some(contents) if matches!(&contents.kind, FormKind::List(rows) if rows.len() == 2))))));
    assert!(matches!(
        read("#2A(1 2)").unwrap_err().kind,
        ReadErrorKind::InvalidDispatch
    ));
    assert!(matches!(
        read("#2A((1 2) (3))").unwrap_err().kind,
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
