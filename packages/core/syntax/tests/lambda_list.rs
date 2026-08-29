#![allow(clippy::expect_used, clippy::unwrap_used, missing_docs)]

use ncl_syntax::{FormKind, LambdaListErrorKind, parse_ordinary_lambda_list, read};

fn parse(source: &str) -> ncl_syntax::OrdinaryLambdaList {
    let form = &read(source).expect("lambda list should parse")[0];
    parse_ordinary_lambda_list(form).expect("lambda list should be valid")
}

#[test]
fn parses_required_optional_supplied_p_and_rest_parameters() {
    let lambda_list = parse("(first &optional (second (+ first 1) second-p) third &rest rest)");

    assert_eq!(lambda_list.required, vec!["FIRST"]);
    assert_eq!(lambda_list.optional.len(), 2);
    assert_eq!(lambda_list.optional[0].name, "SECOND");
    assert_eq!(
        lambda_list.optional[0].supplied_p.as_deref(),
        Some("SECOND-P")
    );
    assert!(matches!(
        lambda_list.optional[0].init_form.kind,
        FormKind::List(_)
    ));
    assert_eq!(lambda_list.optional[1].name, "THIRD");
    assert_eq!(lambda_list.rest.as_deref(), Some("REST"));
}

#[test]
fn optional_parameters_default_to_nil_when_no_init_form_is_given() {
    let lambda_list = parse("(&optional value)");

    assert_eq!(lambda_list.optional[0].name, "VALUE");
    assert!(matches!(
        &lambda_list.optional[0].init_form.kind,
        FormKind::Atom(name) if name == "NIL"
    ));
}

#[test]
fn parses_auxiliary_parameters_after_optional_and_rest_parameters() {
    let lambda_list =
        parse("(first &optional (second (+ first 1)) &rest rest &aux (sum (+ first second)) next)");

    assert_eq!(lambda_list.required, vec!["FIRST"]);
    assert_eq!(lambda_list.optional.len(), 1);
    assert_eq!(lambda_list.rest.as_deref(), Some("REST"));
    assert_eq!(lambda_list.auxiliary.len(), 2);
    assert_eq!(lambda_list.auxiliary[0].name, "SUM");
    assert!(matches!(
        lambda_list.auxiliary[0].init_form.kind,
        FormKind::List(_)
    ));
    assert_eq!(lambda_list.auxiliary[1].name, "NEXT");
    assert!(matches!(
        &lambda_list.auxiliary[1].init_form.kind,
        FormKind::Atom(name) if name == "NIL"
    ));
}

#[test]
fn parses_keyword_parameters_and_allow_other_keys() {
    let lambda_list = parse(
        "(required &optional (optional 10 optional-p) &rest rest
          &key first (second (+ first 1) second-p)
               ((:third third-value) (+ second 1) third-p)
          &allow-other-keys
          &aux (result (+ first second)) copy)",
    );

    assert_eq!(lambda_list.required, vec!["REQUIRED"]);
    assert_eq!(lambda_list.optional[0].name, "OPTIONAL");
    assert_eq!(lambda_list.rest.as_deref(), Some("REST"));
    assert!(lambda_list.has_keyword_section);
    assert!(lambda_list.allow_other_keys);
    assert_eq!(lambda_list.keywords.len(), 3);
    assert_eq!(lambda_list.keywords[0].keyword_name, "FIRST");
    assert_eq!(lambda_list.keywords[0].name, "FIRST");
    assert_eq!(lambda_list.keywords[1].keyword_name, "SECOND");
    assert_eq!(lambda_list.keywords[1].name, "SECOND");
    assert_eq!(
        lambda_list.keywords[1].supplied_p.as_deref(),
        Some("SECOND-P")
    );
    assert_eq!(lambda_list.keywords[2].keyword_name, "THIRD");
    assert_eq!(lambda_list.keywords[2].name, "THIRD-VALUE");
    assert_eq!(lambda_list.auxiliary.len(), 2);
    assert_eq!(lambda_list.auxiliary[0].name, "RESULT");
    assert_eq!(lambda_list.auxiliary[1].name, "COPY");
}

#[test]
fn rejects_duplicate_names_and_unsupported_markers() {
    for source in [
        "(value &optional (other nil value))",
        "(value &optional (other nil) &whole keyword)",
        "(value &rest rest extra)",
    ] {
        let form = &read(source).expect("source should parse")[0];
        let error = parse_ordinary_lambda_list(form).unwrap_err();
        assert!(
            matches!(error.kind, LambdaListErrorKind::InvalidForm { .. }),
            "{source}"
        );
    }
}

#[test]
fn rejects_malformed_optional_specs_and_markers() {
    for source in [
        "(value &optional (other nil supplied extra))",
        "(value &optional item &optional other)",
        "(value &rest)",
    ] {
        let form = &read(source).expect("source should parse")[0];
        let error = parse_ordinary_lambda_list(form).unwrap_err();

        assert!(
            matches!(error.kind, LambdaListErrorKind::InvalidForm { .. }),
            "{source}"
        );
    }

    for source in [
        "(value &optional (1 nil))",
        "(value &optional (other nil :supplied))",
    ] {
        let form = &read(source).expect("source should parse")[0];
        let error = parse_ordinary_lambda_list(form).unwrap_err();

        assert!(
            matches!(error.kind, LambdaListErrorKind::ExpectedSymbol { .. }),
            "{source}"
        );
    }
}

#[test]
fn rejects_malformed_auxiliary_specs_and_ordering() {
    for source in [
        "(value &aux (other nil extra))",
        "(value &aux item &aux other)",
        "(value &aux &optional other)",
        "(value &rest rest extra)",
    ] {
        let form = &read(source).expect("source should parse")[0];
        let error = parse_ordinary_lambda_list(form).unwrap_err();

        assert!(
            matches!(error.kind, LambdaListErrorKind::InvalidForm { .. }),
            "{source}"
        );
    }

    let source = "(value &aux (1 nil))";
    let form = &read(source).expect("source should parse")[0];
    let error = parse_ordinary_lambda_list(form).unwrap_err();
    assert!(matches!(
        error.kind,
        LambdaListErrorKind::ExpectedSymbol { .. }
    ));
}

#[test]
fn rejects_malformed_keyword_name_specifications() {
    for source in ["(&key ((:name)))", "(&key ((\"name\")))"] {
        let form = &read(source).expect("source should parse")[0];
        let error = parse_ordinary_lambda_list(form).unwrap_err();
        assert!(
            matches!(error.kind, LambdaListErrorKind::InvalidForm { .. })
                || matches!(error.kind, LambdaListErrorKind::ExpectedSymbol { .. }),
            "{source}"
        );
    }
}

#[test]
fn rejects_literal_parameter_names() {
    for source in [
        "(&optional nil)",
        "(&optional t)",
        "(&optional 42)",
        "(&optional 3.14)",
        "(&optional :keyword)",
    ] {
        let form = &read(source).expect("source should parse")[0];
        let error = parse_ordinary_lambda_list(form).unwrap_err();
        assert!(
            matches!(error.kind, LambdaListErrorKind::ExpectedSymbol { .. }),
            "{source}"
        );
    }
}

#[test]
fn rejects_lambda_list_boundary_and_duplicate_cases_from_a_table() {
    let non_list = &read("value").expect("source should parse")[0];
    let error = parse_ordinary_lambda_list(non_list).unwrap_err();
    assert!(matches!(error.kind, LambdaListErrorKind::ExpectedList));
    assert_eq!(error.to_string(), "parameters must be a list at byte 0..5");

    for source in [
        "(&optional (value nil supplied) (other nil supplied))",
        "(&key first first)",
        "(&key first &allow-other-keys second)",
        "(&key first &allow-other-keys &allow-other-keys)",
    ] {
        let form = &read(source).expect("source should parse")[0];
        let error = parse_ordinary_lambda_list(form).unwrap_err();
        assert!(
            matches!(error.kind, LambdaListErrorKind::InvalidForm { .. }),
            "{source}"
        );
    }
}

#[test]
fn rejects_keyword_parameter_forms_with_wrong_shapes() {
    // items[0] of the keyword-spec list is neither an atom nor a two-element
    // list naming the keyword/parameter pair.
    let form = &read("(&key (\"s\" 1))").expect("source should parse")[0];
    let error = parse_ordinary_lambda_list(form).unwrap_err();
    assert_eq!(
        error.kind,
        LambdaListErrorKind::ExpectedSymbol {
            context: "keyword parameter"
        }
    );

    // A keyword-spec list outside the one-to-three element grammar.
    let form = &read("(&key ())").expect("source should parse")[0];
    let error = parse_ordinary_lambda_list(form).unwrap_err();
    assert!(
        matches!(&error.kind, LambdaListErrorKind::InvalidForm { message } if message.contains("one to three elements"))
    );

    // The keyword-spec form itself is neither an atom nor a list.
    let form = &read("(&key \"s\")").expect("source should parse")[0];
    let error = parse_ordinary_lambda_list(form).unwrap_err();
    assert_eq!(
        error.kind,
        LambdaListErrorKind::ExpectedSymbol {
            context: "keyword parameter"
        }
    );
}

#[test]
fn rejects_explicit_keyword_names_that_are_not_plain_symbols() {
    // ((keyword-name parameter-name) init-form) with a non-atom keyword name.
    let form = &read("(&key ((#\\a name) 1))").expect("source should parse")[0];
    let error = parse_ordinary_lambda_list(form).unwrap_err();
    assert_eq!(
        error.kind,
        LambdaListErrorKind::ExpectedSymbol {
            context: "keyword name"
        }
    );

    // A keyword-name atom that fails symbol-token parsing (too many
    // unescaped package-qualifier separators).
    let form = &read("(&key ((a:b:c name) 1))").expect("source should parse")[0];
    let error = parse_ordinary_lambda_list(form).unwrap_err();
    assert_eq!(
        error.kind,
        LambdaListErrorKind::ExpectedSymbol {
            context: "keyword name"
        }
    );

    // A keyword-name atom that names a literal (e.g. NIL) or a lambda-list
    // marker prefix is rejected too.
    for source in ["(&key ((nil name) 1))", "(&key ((&foo name) 1))"] {
        let form = &read(source).expect("source should parse")[0];
        let error = parse_ordinary_lambda_list(form).unwrap_err();
        assert_eq!(
            error.kind,
            LambdaListErrorKind::ExpectedSymbol {
                context: "keyword name"
            },
            "{source}"
        );
    }
}

#[test]
fn parameter_names_accept_escapes_and_reject_unparseable_atoms() {
    let lambda_list = parse("(|foo|)");
    assert_eq!(lambda_list.required, vec!["foo"]);
    assert!(lambda_list.required_escaped[0]);

    let form = &read("(a:b:c)").expect("source should parse")[0];
    let error = parse_ordinary_lambda_list(form).unwrap_err();
    assert_eq!(
        error.kind,
        LambdaListErrorKind::ExpectedSymbol {
            context: "parameter"
        }
    );
}

#[test]
fn rejects_auxiliary_parameter_forms_that_are_not_atoms_or_lists() {
    let form = &read("(&aux \"s\")").expect("source should parse")[0];
    let error = parse_ordinary_lambda_list(form).unwrap_err();
    assert_eq!(
        error.kind,
        LambdaListErrorKind::ExpectedSymbol {
            context: "auxiliary parameter"
        }
    );
}

#[test]
fn rejects_section_markers_in_invalid_positions() {
    for source in [
        "(&rest first &rest second)",
        "(&key first &rest rest)",
        "(&aux first &rest rest)",
        "(&rest rest extra)",
        "(&allow-other-keys)",
        "(&key first &allow-other-keys second)",
        "(&key first &allow-other-keys &allow-other-keys)",
        "(&aux first &aux second)",
        "(&key first &key second)",
        "(&key first &aux second &key third)",
    ] {
        let form = &read(source).expect("source should parse")[0];
        let error = parse_ordinary_lambda_list(form).unwrap_err();
        assert!(
            matches!(error.kind, LambdaListErrorKind::InvalidForm { .. }),
            "{source}"
        );
    }
}
