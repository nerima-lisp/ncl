use ncl_syntax::{read, FormKind, ReadErrorKind, Reader, MAX_NESTING_DEPTH};

#[test]
fn reads_lists_prefixes_and_literals() {
    let forms = read("(+ 1 2) '(a . b) #(\"x\" #\\Space #t)").unwrap();

    assert_eq!(forms.len(), 3);
    assert!(matches!(forms[0].kind, FormKind::List(_)));
    assert!(matches!(forms[1].kind, FormKind::List(_)));
    assert!(matches!(forms[2].kind, FormKind::Vector(_)));
}

#[test]
fn reads_complex_literals() {
    let forms = read("#C(1 -2) #c(3.0 4/5)").unwrap();

    assert_eq!(forms.len(), 2);
    assert!(matches!(
        &forms[0].kind,
        FormKind::Complex { real, imaginary }
            if real.to_string() == "1" && imaginary.to_string() == "-2"
    ));
    assert!(matches!(
        &forms[1].kind,
        FormKind::Complex { real, imaginary }
            if real.to_string() == "3.0" && imaginary.to_string() == "4/5"
    ));
    assert_eq!(forms[0].to_string(), "#C(1 -2)");
    assert_eq!(forms[1].to_string(), "#C(3.0 4/5)");
}

#[test]
fn unquote_requires_a_quasiquote_context() {
    for source in [",value", ",@values", "' ,value"] {
        let error = read(source).unwrap_err();

        assert!(matches!(
            error.kind,
            ReadErrorKind::UnquoteOutsideQuasiquote
        ));
    }

    assert!(read("`(outer ,value ,@(list value))").is_ok());
    assert!(read("`(outer `((inner ,value)))").is_ok());
}

#[test]
fn reads_bit_vectors() {
    let forms = read("#*1010 #*").unwrap();

    assert_eq!(forms.len(), 2);
    assert!(matches!(
        &forms[0].kind,
        FormKind::BitVector(bits) if bits == &vec![1, 0, 1, 0]
    ));
    assert!(matches!(&forms[1].kind, FormKind::BitVector(bits) if bits.is_empty()));
    assert_eq!(forms[0].to_string(), "#*1010");
    assert!(matches!(
        read("#*102").unwrap_err().kind,
        ReadErrorKind::InvalidDispatch
    ));
}

#[test]
fn reads_read_time_evaluation() {
    let forms = read("#.(+ 1 2)").unwrap();

    assert_eq!(forms.len(), 1);
    let FormKind::ReadTimeEval(form) = &forms[0].kind else {
        panic!("expected a read-time evaluation form");
    };
    assert_eq!(form.to_string(), "(+ 1 2)");
    assert_eq!(forms[0].to_string(), "#.(+ 1 2)");
    assert!(matches!(
        read("#.").unwrap_err().kind,
        ReadErrorKind::UnexpectedEnd {
            context: "read-time evaluation"
        }
    ));
}

#[test]
fn reads_fixed_radix_integer_dispatch() {
    let forms = read("#b101 #O17 #x+Ab #x-10 #x1f").unwrap();

    assert_eq!(forms.len(), 5);
    assert!(matches!(&forms[0].kind, FormKind::Atom(atom) if atom == "#b101"));
    assert!(matches!(&forms[1].kind, FormKind::Atom(atom) if atom == "#O17"));
    assert!(matches!(&forms[2].kind, FormKind::Atom(atom) if atom == "#x+Ab"));
    assert!(matches!(&forms[3].kind, FormKind::Atom(atom) if atom == "#x-10"));
    assert!(matches!(&forms[4].kind, FormKind::Atom(atom) if atom == "#x1f"));

    for source in ["#b", "#x-", "#b2", "#o8", "#xg", "#x1f2gh"] {
        assert!(
            matches!(
                read(source).unwrap_err().kind,
                ReadErrorKind::InvalidDispatch
            ),
            "{source}"
        );
    }
}

#[test]
fn reads_general_radix_integer_dispatch() {
    let forms = read("#2r101 #10R42 #36rz #16r-ff").unwrap();

    assert_eq!(forms.len(), 4);
    assert!(matches!(&forms[0].kind, FormKind::Atom(atom) if atom == "#2r101"));
    assert!(matches!(&forms[1].kind, FormKind::Atom(atom) if atom == "#10R42"));
    assert!(matches!(&forms[2].kind, FormKind::Atom(atom) if atom == "#36rz"));
    assert!(matches!(&forms[3].kind, FormKind::Atom(atom) if atom == "#16r-ff"));

    for source in ["#r1", "#1r1", "#37r1", "#10r", "#2r102", "#10r1/2"] {
        assert!(
            matches!(
                read(source).unwrap_err().kind,
                ReadErrorKind::InvalidDispatch
            ),
            "{source}"
        );
    }
}

#[test]
fn reads_general_radix_integer_dispatch_without_i64_overflow() {
    let forms = read("#10r9223372036854775808 #16r8000000000000000").unwrap();

    assert_eq!(forms.len(), 2);
    assert!(matches!(
        &forms[0].kind,
        FormKind::Atom(atom) if atom == "#10r9223372036854775808"
    ));
    assert!(matches!(
        &forms[1].kind,
        FormKind::Atom(atom) if atom == "#16r8000000000000000"
    ));
    assert!(matches!(
        read("#2r102").unwrap_err().kind,
        ReadErrorKind::InvalidDispatch
    ));
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
fn reader_conditionals_select_enabled_forms_and_continue() {
    let forms = Reader::with_features(
        "#+enabled kept #-enabled discarded #+unknown removed #-unknown retained",
        ["enabled"],
    )
    .read_all()
    .unwrap();

    assert_eq!(forms.len(), 2);
    assert!(matches!(&forms[0].kind, FormKind::Atom(atom) if atom == "kept"));
    assert!(matches!(&forms[1].kind, FormKind::Atom(atom) if atom == "retained"));
}

#[test]
fn reader_conditionals_consume_a_disabled_list_branch() {
    let forms = Reader::new("#+missing (discarded (nested form)) 42")
        .read_all()
        .unwrap();

    assert_eq!(forms.len(), 1);
    assert!(matches!(&forms[0].kind, FormKind::Atom(atom) if atom == "42"));
}

#[test]
fn reader_conditionals_work_inside_lists() {
    let forms = Reader::with_features(
        "(#-enabled discarded #+enabled kept #+missing removed tail)",
        ["enabled"],
    )
    .read_all()
    .unwrap();

    let FormKind::List(items) = &forms[0].kind else {
        panic!("expected a list");
    };
    assert_eq!(items.len(), 2);
    assert!(matches!(&items[0].kind, FormKind::Atom(atom) if atom == "kept"));
    assert!(matches!(&items[1].kind, FormKind::Atom(atom) if atom == "tail"));
}

#[test]
fn reader_conditionals_normalize_symbols_and_keywords() {
    let forms = Reader::with_features("#+MiXeD symbol #+:MiXeD keyword", ["mixed", ":mixed"])
        .read_all()
        .unwrap();

    assert_eq!(forms.len(), 2);
    assert!(matches!(&forms[0].kind, FormKind::Atom(atom) if atom == "symbol"));
    assert!(matches!(&forms[1].kind, FormKind::Atom(atom) if atom == "keyword"));
}

#[test]
fn reader_conditionals_support_recursive_feature_expressions() {
    let forms = Reader::with_features(
        "#+(and unix (not windows)) first #+(or missing (and unix)) second #+(and unix windows) removed",
        ["unix"],
    )
    .read_all()
    .unwrap();

    assert_eq!(forms.len(), 2);
    assert!(matches!(&forms[0].kind, FormKind::Atom(atom) if atom == "first"));
    assert!(matches!(&forms[1].kind, FormKind::Atom(atom) if atom == "second"));
}

#[test]
fn reader_conditionals_report_eof_and_invalid_feature_expressions() {
    let error = Reader::new("#+").read_all().unwrap_err();
    assert!(matches!(
        error.kind,
        ReadErrorKind::UnexpectedEnd {
            context: "reader conditional feature expression"
        }
    ));

    let error = Reader::new("#+feature").read_all().unwrap_err();
    assert!(matches!(
        error.kind,
        ReadErrorKind::UnexpectedEnd {
            context: "reader conditional form"
        }
    ));

    let error = Reader::new("#+(xor feature) value").read_all().unwrap_err();
    assert!(matches!(error.kind, ReadErrorKind::InvalidDispatch));
}

#[test]
fn reader_conditionals_have_no_default_features() {
    let forms = read("#+unknown skipped 42").unwrap();

    assert_eq!(forms.len(), 1);
    assert!(matches!(&forms[0].kind, FormKind::Atom(atom) if atom == "42"));
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
fn dotted_lists_require_a_head() {
    let error = read("(. a)").unwrap_err();

    assert!(matches!(error.kind, ReadErrorKind::MissingDottedHead));
}

#[test]
fn dotted_lists_reject_multiple_tails() {
    let error = read("(a . b . c)").unwrap_err();

    assert!(matches!(error.kind, ReadErrorKind::MultipleDottedTails));
}

#[test]
fn dotted_lists_reject_a_second_dot_as_tail() {
    let error = read("(a . .)").unwrap_err();

    assert!(matches!(error.kind, ReadErrorKind::MultipleDottedTails));
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
fn string_backslash_quotes_the_next_character() {
    let forms = read(r#""\n" "\q" "\\" "\"""#).unwrap();

    assert!(matches!(&forms[0].kind, FormKind::String(value) if value == "n"));
    assert!(matches!(&forms[1].kind, FormKind::String(value) if value == "q"));
    assert!(matches!(&forms[2].kind, FormKind::String(value) if value == "\\"));
    assert!(matches!(&forms[3].kind, FormKind::String(value) if value == "\""));
}

#[test]
fn reads_standard_named_characters() {
    let forms = read("#\\Backspace #\\Linefeed #\\Page #\\Rubout").unwrap();

    assert!(matches!(forms[0].kind, FormKind::Character('\u{0008}')));
    assert!(matches!(forms[1].kind, FormKind::Character('\n')));
    assert!(matches!(forms[2].kind, FormKind::Character('\u{000c}')));
    assert!(matches!(forms[3].kind, FormKind::Character('\u{007f}')));
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

#[test]
fn deeply_nested_prefix_input_has_a_typed_limit_error() {
    let source = format!("{}1", "'".repeat(MAX_NESTING_DEPTH + 1));
    let error = read(&source).unwrap_err();

    assert!(matches!(
        error.kind,
        ReadErrorKind::NestingTooDeep {
            limit: MAX_NESTING_DEPTH
        }
    ));
}
