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

#[test]
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

#[test]
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

#[test]
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

#[test]
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

#[test]
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

#[test]
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
    assert_eq!(
        parse("~1٢D").expect_err("ASCII decimal only").kind,
        ParseErrorKind::InvalidParameter
    );
}
