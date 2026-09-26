#![allow(missing_docs, clippy::expect_used)]

use ncl_lib_format::{ControlPart, DirectiveKind, Parameter, parse};

#[test]
fn parses_literals_directives_and_parameters() {
    let control = parse("value=~10,'xD~%~A").expect("valid control string");
    assert_eq!(control.parts[0], ControlPart::Literal("value=".to_owned()));
    assert_eq!(
        control.parts[1],
        ControlPart::Directive(ncl_lib_format::Directive {
            parameters: vec![
                Parameter::Integer(10),
                Parameter::Unsupplied,
                Parameter::Character('x')
            ],
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
