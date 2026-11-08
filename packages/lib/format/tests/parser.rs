#![allow(missing_docs, clippy::expect_used, clippy::unwrap_used)]

use ncl_lib_format::{
    ControlPart, DirectiveBody, DirectiveKind, FormatControl, Parameter, ParseErrorKind,
    TerminatorKind, parse,
};

fn directive(control: &FormatControl, index: usize) -> Option<&ncl_lib_format::Directive> {
    match &control.parts[index] {
        ControlPart::Directive(directive) => Some(directive),
        ControlPart::Literal(_) => None,
    }
}

#[test] // CLHS 22.3
fn parses_literals_directives_and_parameters() {
    let control = parse("value=~10,'xD~%~A").expect("valid control string");
    assert_eq!(control.parts[0], ControlPart::Literal("value=".to_owned()));
    assert_eq!(
        control.parts[1],
        ControlPart::Directive(ncl_lib_format::Directive {
            position: 6,
            parameters: vec![Parameter::Integer(10), Parameter::Character('x')],
            colon: false,
            at_sign: false,
            kind: DirectiveKind::D,
            function_name: None,
            body: DirectiveBody::None,
            terminator: None,
        })
    );
    assert_eq!(
        control.parts[2],
        ControlPart::Directive(ncl_lib_format::Directive {
            position: 13,
            parameters: Vec::new(),
            colon: false,
            at_sign: false,
            kind: DirectiveKind::Percent,
            function_name: None,
            body: DirectiveBody::None,
            terminator: None,
        })
    );
}

#[test] // CLHS 22.3
fn parses_literals_parameters_and_modifiers_with_source_offsets() {
    let control = parse("value=~v,,#,'x:@D~%~~").expect("valid control");
    assert_eq!(control.parts[0], ControlPart::Literal("value=".to_owned()));
    let first = directive(&control, 1).expect("directive");
    assert_eq!(first.position, 6);
    assert_eq!(first.kind, DirectiveKind::D);
    assert!(first.colon);
    assert!(first.at_sign);
    assert_eq!(
        first.parameters,
        vec![
            Parameter::Relative,
            Parameter::Unsupplied,
            Parameter::Remaining,
            Parameter::Character('x'),
        ]
    );
    assert_eq!(
        directive(&control, 2).expect("percent").kind,
        DirectiveKind::Percent
    );
    assert_eq!(
        directive(&control, 3).expect("tilde").kind,
        DirectiveKind::Tilde
    );
}

#[test] // CLHS 22.3.7.2/.4/.5
fn parses_nested_sequences_and_clause_modifiers() {
    let control = parse("~[zero~;one~:;other~]").expect("conditional");
    let conditional = directive(&control, 0).expect("conditional");
    assert_eq!(conditional.kind, DirectiveKind::Conditional);
    let DirectiveBody::Clauses(clauses) = &conditional.body else {
        panic!("conditional clauses");
    };
    assert_eq!(clauses.len(), 3);
    assert_eq!(
        clauses[0].control.parts,
        vec![ControlPart::Literal("zero".to_owned())]
    );
    assert_eq!(
        clauses[1].separator.as_ref().map(|item| item.colon),
        Some(false)
    );
    assert_eq!(
        clauses[1].control.parts,
        vec![ControlPart::Literal("one".to_owned())]
    );
    assert_eq!(
        clauses[2].separator.as_ref().map(|item| item.colon),
        Some(true)
    );
    assert_eq!(
        clauses[2].control.parts,
        vec![ControlPart::Literal("other".to_owned())]
    );

    let iteration = parse("~{prefix~A~}").expect("iteration");
    let item = directive(&iteration, 0).expect("iteration directive");
    assert_eq!(item.kind, DirectiveKind::Iteration);
    assert_eq!(
        item.terminator.map(|term| term.kind),
        Some(TerminatorKind::RightBrace)
    );
    let DirectiveBody::Sequence(body) = &item.body else {
        panic!("iteration sequence");
    };
    assert_eq!(body.parts[0], ControlPart::Literal("prefix".to_owned()));
    assert_eq!(
        directive(body, 1).expect("body directive").kind,
        DirectiveKind::A
    );
}

#[test] // CLHS 22.3.5.2, 22.3.6.2
fn distinguishes_justification_from_logical_blocks() {
    let justified = parse("~<~A~;~A~>").expect("justification");
    assert_eq!(
        directive(&justified, 0).expect("justification").kind,
        DirectiveKind::Justification
    );
    assert_eq!(
        directive(&justified, 0)
            .expect("terminator")
            .terminator
            .map(|term| term.colon),
        Some(false)
    );

    let logical = parse("~:<~A~;~A~:>").expect("logical block");
    let item = directive(&logical, 0).expect("logical block");
    assert_eq!(item.kind, DirectiveKind::LogicalBlock);
    assert!(item.colon);
    assert_eq!(item.terminator.map(|term| term.colon), Some(true));
}

#[test] // CLHS 22.3.5.4, 22.3.8.1
fn parses_named_format_functions_and_case_conversion() {
    let control = parse("~/prin1/~@(hello ~A~)").expect("functions and case");
    let function = directive(&control, 0).expect("named function");
    assert_eq!(function.kind, DirectiveKind::Slash);
    assert_eq!(function.function_name.as_deref(), Some("PRIN1"));
    let case = directive(&control, 1).expect("case directive");
    assert_eq!(case.kind, DirectiveKind::CaseConversion);
    assert!(case.at_sign);
    assert_eq!(
        case.terminator.map(|term| term.kind),
        Some(TerminatorKind::RightParen)
    );
}

#[test] // CLHS 22.3.9.3
fn newline_directive_discards_following_whitespace_unless_colon_is_present() {
    let suppressed = parse("one~\n  \t two").expect("newline directive");
    assert_eq!(
        suppressed.parts[1],
        ControlPart::Directive(ncl_lib_format::Directive {
            position: 3,
            parameters: Vec::new(),
            colon: false,
            at_sign: false,
            kind: DirectiveKind::Newline,
            function_name: None,
            body: DirectiveBody::None,
            terminator: None,
        })
    );
    assert_eq!(suppressed.parts[2], ControlPart::Literal("two".to_owned()));

    let kept = parse("one~:\n  two").expect("colon newline directive");
    assert_eq!(kept.parts[2], ControlPart::Literal("  two".to_owned()));
}

#[test] // CLHS 22.3
fn reports_unknown_incomplete_and_unbalanced_controls() {
    assert_eq!(
        parse("abc~"),
        Err(ncl_lib_format::ParseError {
            offset: 3,
            directive: None,
            kind: ParseErrorKind::UnterminatedDirective,
        })
    );
    assert_eq!(
        parse("~z").expect_err("unknown").kind,
        ParseErrorKind::UnknownDirective
    );
    assert_eq!(
        parse("~]").expect_err("unexpected close").kind,
        ParseErrorKind::UnexpectedTerminator
    );
    assert_eq!(
        parse("~{~A").expect_err("unclosed iteration").kind,
        ParseErrorKind::UnterminatedConstruct { expected: '}' }
    );
    let nested_unknown = parse("~{~z~}").expect_err("unknown directive inside iteration");
    assert_eq!(nested_unknown.offset, 2);
    assert_eq!(nested_unknown.directive, Some('z'));
    assert_eq!(nested_unknown.kind, ParseErrorKind::UnknownDirective);
    assert_eq!(
        parse("~1٢D").expect_err("ASCII decimal only").kind,
        ParseErrorKind::InvalidParameter
    );
    assert_eq!(
        parse("~;").expect_err("top-level clause separator"),
        ncl_lib_format::ParseError {
            offset: 0,
            directive: Some(';'),
            kind: ParseErrorKind::UnexpectedTerminator,
        }
    );
}

#[test] // CLHS 22.3
fn parses_each_directive_kind_and_its_canonical_character() {
    let cases = [
        ("~A", DirectiveKind::A, 'A'),
        ("~S", DirectiveKind::S, 'S'),
        ("~W", DirectiveKind::W, 'W'),
        ("~C", DirectiveKind::C, 'C'),
        ("~R", DirectiveKind::R, 'R'),
        ("~D", DirectiveKind::D, 'D'),
        ("~B", DirectiveKind::B, 'B'),
        ("~O", DirectiveKind::O, 'O'),
        ("~X", DirectiveKind::X, 'X'),
        ("~F", DirectiveKind::F, 'F'),
        ("~E", DirectiveKind::E, 'E'),
        ("~G", DirectiveKind::G, 'G'),
        ("~$", DirectiveKind::Dollar, '$'),
        ("~%", DirectiveKind::Percent, '%'),
        ("~&", DirectiveKind::Ampersand, '&'),
        ("~|", DirectiveKind::Bar, '|'),
        ("~~", DirectiveKind::Tilde, '~'),
        ("~\n", DirectiveKind::Newline, '\n'),
        ("~_", DirectiveKind::Underscore, '_'),
        ("~:<~:>", DirectiveKind::LogicalBlock, '<'),
        ("~I", DirectiveKind::Indent, 'I'),
        ("~//", DirectiveKind::Slash, '/'),
        ("~T", DirectiveKind::Tab, 'T'),
        ("~<~>", DirectiveKind::Justification, '<'),
        ("~*", DirectiveKind::Star, '*'),
        ("~[~]", DirectiveKind::Conditional, '['),
        ("~{~}", DirectiveKind::Iteration, '{'),
        ("~?", DirectiveKind::Recursive, '?'),
        ("~(~)", DirectiveKind::CaseConversion, '('),
        ("~P", DirectiveKind::Plural, 'P'),
        ("~^", DirectiveKind::UpArrow, '^'),
    ];

    for (control_text, expected_kind, canonical_character) in cases {
        let control = parse(control_text).expect("directive control");
        let actual = directive(&control, 0).expect("directive");
        assert_eq!(actual.kind, expected_kind, "control {control_text:?}");
        assert_eq!(actual.kind.character(), canonical_character);

        if control_text.len() == 2 && canonical_character.is_ascii_alphabetic() {
            let lowercase = format!("~{}", canonical_character.to_ascii_lowercase());
            let control = parse(&lowercase).expect("lowercase directive");
            assert_eq!(
                directive(&control, 0).expect("lowercase directive").kind,
                expected_kind,
                "control {lowercase:?}"
            );
        }
    }
}

#[test] // CLHS 22.3
fn parses_signed_integer_parameters_and_reports_out_of_range_values() {
    let control = parse("~+9223372036854775807D~-9223372036854775808D~,D~,,D~,2D~1,,D~1,:D~1,@D")
        .expect("parameters");
    assert_eq!(
        directive(&control, 0).expect("positive").parameters,
        vec![Parameter::Integer(i64::MAX)]
    );
    assert_eq!(
        directive(&control, 1).expect("negative").parameters,
        vec![Parameter::Integer(i64::MIN)]
    );
    assert_eq!(
        directive(&control, 2).expect("omitted").parameters,
        vec![Parameter::Unsupplied]
    );
    assert_eq!(
        directive(&control, 3).expect("two omitted").parameters,
        vec![Parameter::Unsupplied, Parameter::Unsupplied]
    );
    assert_eq!(
        directive(&control, 4)
            .expect("omitted before value")
            .parameters,
        vec![Parameter::Unsupplied, Parameter::Integer(2)]
    );
    assert_eq!(
        directive(&control, 5).expect("trailing omitted").parameters,
        vec![
            Parameter::Integer(1),
            Parameter::Unsupplied,
            Parameter::Unsupplied,
        ]
    );
    let colon_with_omitted = directive(&control, 6).expect("colon with omitted");
    assert_eq!(
        colon_with_omitted.parameters,
        vec![Parameter::Integer(1), Parameter::Unsupplied]
    );
    assert!(colon_with_omitted.colon);
    let at_sign_with_omitted = directive(&control, 7).expect("at-sign with omitted");
    assert_eq!(
        at_sign_with_omitted.parameters,
        vec![Parameter::Integer(1), Parameter::Unsupplied]
    );
    assert!(at_sign_with_omitted.at_sign);

    for control in [
        "~9223372036854775808D",
        "~170141183460469231731687303715884105728D",
    ] {
        let error = parse(control).expect_err("integer parameter must fit i64");
        assert_eq!(error.offset, 0, "control {control:?}");
        assert_eq!(error.directive, None, "control {control:?}");
        assert_eq!(
            error.kind,
            ParseErrorKind::InvalidParameter,
            "control {control:?}"
        );
    }
}

#[test] // CLHS 22.3, 22.3.5.4
fn rejects_conflicting_header_parameters_and_incomplete_function_names() {
    for control in [
        "~:,D", "~@,D", "~1vD", "~1#D", "~1'xD", "~'", "~v1D", "~+D", "~-D",
    ] {
        assert_eq!(
            parse(control).expect_err("invalid header"),
            ncl_lib_format::ParseError {
                offset: 0,
                directive: None,
                kind: ParseErrorKind::InvalidParameter,
            },
            "control {control:?}"
        );
    }

    let error = parse("a~/PRIN1").expect_err("unterminated function name");
    assert_eq!(error.offset, 1);
    assert_eq!(error.directive, Some('/'));
    assert_eq!(error.kind, ParseErrorKind::UnterminatedFunctionName);
}

#[test] // CLHS 22.3.7.2/.4/.5
fn empty_clauses_mismatched_terminators_and_character_offsets_are_reported() {
    let control = parse("~[~;middle~;~]").expect("empty clauses");
    let DirectiveBody::Clauses(clauses) = &directive(&control, 0).expect("conditional").body else {
        panic!("conditional clauses");
    };
    assert_eq!(clauses.len(), 3);
    assert!(clauses[0].control.parts.is_empty());
    assert_eq!(
        clauses[1].control.parts,
        vec![ControlPart::Literal("middle".to_owned())]
    );
    assert!(clauses[2].control.parts.is_empty());

    for (control, expected) in [("~<", '>'), ("~[", ']'), ("~{", '}'), ("~(", ')')] {
        let error = parse(control).expect_err("unclosed nested construct");
        assert_eq!(error.offset, 0);
        assert_eq!(
            error.kind,
            ParseErrorKind::UnterminatedConstruct { expected }
        );
    }
    assert_eq!(
        parse("~[x~}").expect_err("mismatched terminator").kind,
        ParseErrorKind::UnexpectedTerminator
    );

    let error = parse("λ~z").expect_err("unknown directive after a multibyte character");
    assert_eq!(error.offset, 1);
    assert_eq!(error.directive, Some('z'));
    assert_eq!(error.kind, ParseErrorKind::UnknownDirective);
}
