use ncl_syntax::{parse_ordinary_lambda_list, read, FormKind, LambdaListErrorKind};

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
