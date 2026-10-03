#![allow(missing_docs, clippy::expect_used)]

use ncl_lib_format::{ControlPart, DirectiveKind, Parameter, parse};

#[test]
fn parses_literals_directives_and_parameters() {
    let control = parse("value=~10,'xD~%~A").expect("valid control string");
    assert_eq!(control.parts[0], ControlPart::Literal("value=".to_owned()));
    assert_eq!(
        control.parts[1],
        ControlPart::Directive(ncl_lib_format::Directive {
            parameters: vec![Parameter::Integer(10), Parameter::Character('x')],
            colon: false,
            at_sign: false,
            kind: DirectiveKind::D,
        })
    );
    assert_eq!(
        control.parts[2],
        ControlPart::Directive(ncl_lib_format::Directive {
            parameters: Vec::new(),
            colon: false,
            at_sign: false,
            kind: DirectiveKind::Percent,
        })
    );
}

#[test]
fn parses_modifiers_and_relative_parameters() {
    let control = parse("~v,,'A:@S").expect("valid control string");
    assert_eq!(
        control.parts[0],
        ControlPart::Directive(ncl_lib_format::Directive {
            parameters: vec![
                Parameter::Relative,
                Parameter::Unsupplied,
                Parameter::Character('A')
            ],
            colon: true,
            at_sign: true,
            kind: DirectiveKind::S,
        })
    );
}

#[test]
fn parses_simple_directives_with_parameters_and_modifiers() {
    let control = parse("~10,,'x:D~v@S~A").expect("valid control string");
    assert_eq!(
        control.parts,
        vec![
            ControlPart::Directive(ncl_lib_format::Directive {
                parameters: vec![
                    Parameter::Integer(10),
                    Parameter::Unsupplied,
                    Parameter::Character('x')
                ],
                colon: true,
                at_sign: false,
                kind: DirectiveKind::D,
            }),
            ControlPart::Directive(ncl_lib_format::Directive {
                parameters: vec![Parameter::Relative],
                colon: false,
                at_sign: true,
                kind: DirectiveKind::S,
            }),
            ControlPart::Directive(ncl_lib_format::Directive {
                parameters: Vec::new(),
                colon: false,
                at_sign: false,
                kind: DirectiveKind::A,
            }),
        ]
    );
}

#[test]
fn parses_all_clhs_directives() {
    for character in "RPCFEG$|<>*?()[]{}^;/_ITW".chars() {
        assert!(parse(&format!("~{character}")).is_ok(), "~{character}");
    }
}

#[test]
fn rejects_incomplete_and_unknown_directives() {
    assert_eq!(
        parse("abc~"),
        Err(ncl_lib_format::ParseError {
            offset: 3,
            kind: ncl_lib_format::ParseErrorKind::UnterminatedDirective
        })
    );
    assert_eq!(
        parse("~z"),
        Err(ncl_lib_format::ParseError {
            offset: 1,
            kind: ncl_lib_format::ParseErrorKind::UnknownDirective
        })
    );
}

#[test]
fn rejects_truncated_parameters_without_panicking() {
    for input in ["~'", "~+", "~-", "~10,"] {
        let result = std::panic::catch_unwind(|| ncl_lib_format::parse(input));
        let result = result.expect("parser must return an error instead of panicking");
        assert!(result.is_err(), "{input:?} should be rejected");
    }
}
