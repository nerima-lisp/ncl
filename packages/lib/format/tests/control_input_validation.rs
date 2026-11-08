//! Edge cases for FORMAT selection and iteration controls.

#![expect(
    clippy::expect_used,
    reason = "Test setup uses expect and unwrap to keep fixture failures local."
)]

use ncl_lib_format::{
    ControlPart, ControlSegment, Directive, DirectiveBody, DirectiveKind, DirectiveModifiers,
    FormatControl, FormatErrorKind, Parameter, Terminator, TerminatorKind, execute, parse,
};
use ncl_object::{Runtime, ThreadContext, Word, make_bignum_from_i128, make_cons, make_string};
use ncl_printer::StringSink;

fn context() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().expect("runtime");
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).expect("register context");
    (runtime, ctx)
}

fn string(runtime: &Runtime, ctx: &mut ThreadContext, value: &str) -> Word {
    make_string(ctx, runtime, &value.chars().collect::<Vec<_>>()).expect("string")
}

fn list(runtime: &Runtime, ctx: &mut ThreadContext, values: &[Word]) -> Word {
    values.iter().rev().copied().fold(Word::NIL, |tail, value| {
        make_cons(ctx, runtime, value, tail).expect("cons")
    })
}

fn run(
    control: &str,
    arguments: &[Word],
    ctx: &mut ThreadContext,
    runtime: &Runtime,
) -> (String, usize) {
    let parsed = parse(control).expect("control");
    let mut sink = StringSink::new();
    let consumed = execute(&parsed, arguments, ctx, runtime, &mut sink).expect("execution");
    (sink.into_string(), consumed)
}

fn assert_error(
    control: &str,
    arguments: &[Word],
    expected: &FormatErrorKind,
    ctx: &mut ThreadContext,
    runtime: &Runtime,
) {
    let parsed = parse(control).expect("control");
    let mut sink = StringSink::new();
    let error = execute(&parsed, arguments, ctx, runtime, &mut sink).expect_err("execution");
    assert_eq!(&error.kind, expected, "{control}");
}

const fn directive(
    kind: DirectiveKind,
    parameters: Vec<Parameter>,
    colon: bool,
    at_sign: bool,
    body: DirectiveBody,
    terminator: Option<Terminator>,
) -> Directive {
    Directive {
        position: 0,
        parameters,
        colon,
        at_sign,
        kind,
        function_name: None,
        body,
        terminator,
    }
}

const fn terminator(kind: TerminatorKind, colon: bool, at_sign: bool) -> Terminator {
    Terminator {
        kind,
        colon,
        at_sign,
        position: 1,
    }
}

fn sequence() -> DirectiveBody {
    DirectiveBody::Sequence(Box::new(FormatControl {
        parts: vec![ControlPart::Literal("x".to_owned())],
    }))
}

const fn clause(separator: Option<DirectiveModifiers>) -> ControlSegment {
    ControlSegment {
        separator,
        control: FormatControl { parts: vec![] },
    }
}

fn assert_ast_error(
    directive: Directive,
    arguments: &[Word],
    expected: &FormatErrorKind,
    ctx: &mut ThreadContext,
    runtime: &Runtime,
) {
    let control = FormatControl {
        parts: vec![ControlPart::Directive(directive)],
    };
    let mut sink = StringSink::new();
    let error = execute(&control, arguments, ctx, runtime, &mut sink).expect_err("malformed AST");
    assert_eq!(&error.kind, expected);
}

#[test] // CLHS 22.3.7.2
fn conditional_selection_handles_bignums_out_of_range_values_and_bad_inputs() {
    let (runtime, mut ctx) = context();
    let two = make_bignum_from_i128(&mut ctx, &runtime, 2)
        .map(Into::into)
        .expect("bignum selector");
    let control = "~[zero~;one~;two~;~:;other~]";
    assert_eq!(
        run(control, &[two], &mut ctx, &runtime),
        ("two".to_owned(), 1)
    );
    assert_eq!(
        run(control, &[Word::fixnum(-1)], &mut ctx, &runtime),
        ("other".into(), 1)
    );
    assert_eq!(
        run("~[zero~;one~]", &[Word::fixnum(9)], &mut ctx, &runtime),
        (String::new(), 1)
    );
    for (arguments, expected) in [
        (vec![], FormatErrorKind::MissingArgument),
        (
            vec![string(&runtime, &mut ctx, "not an index")],
            FormatErrorKind::WrongArgumentType,
        ),
    ] {
        assert_error("~[zero~;one~]", &arguments, &expected, &mut ctx, &runtime);
    }
    assert_error(
        "~[zero~:;one~;many~]",
        &[Word::fixnum(0)],
        &FormatErrorKind::InvalidDirectiveUse,
        &mut ctx,
        &runtime,
    );
}

#[test] // CLHS 22.3.7.2
fn conditional_selection_validates_dynamic_parameters_and_clause_separators() {
    let (runtime, mut ctx) = context();
    assert_eq!(
        run("~v[zero~;one~]", &[Word::fixnum(1)], &mut ctx, &runtime),
        ("one".to_owned(), 1)
    );
    for (control, arguments, expected) in [
        (
            "~v[zero~;one~]",
            vec![string(&runtime, &mut ctx, "not an integer")],
            FormatErrorKind::WrongArgumentType,
        ),
        (
            "~'x[zero~;one~]",
            vec![],
            FormatErrorKind::WrongArgumentType,
        ),
        (
            "~[zero~@;one~]",
            vec![Word::fixnum(0)],
            FormatErrorKind::InvalidDirectiveUse,
        ),
        (
            "~[zero~1;one~]",
            vec![Word::fixnum(0)],
            FormatErrorKind::InvalidDirectiveUse,
        ),
        (
            "~:@[zero~]",
            vec![Word::NIL],
            FormatErrorKind::InvalidDirectiveUse,
        ),
        (
            "~1@[zero~]",
            vec![Word::TRUE],
            FormatErrorKind::InvalidDirectiveUse,
        ),
        ("~@[zero~]", vec![], FormatErrorKind::MissingArgument),
    ] {
        assert_error(control, &arguments, &expected, &mut ctx, &runtime);
    }
}

#[test] // CLHS 22.3.7.4/.5
fn iteration_enforces_progress_limits_and_proper_lists() {
    let (runtime, mut ctx) = context();
    let a = string(&runtime, &mut ctx, "a");
    let values = list(&runtime, &mut ctx, &[a]);
    let improper = make_cons(&mut ctx, &runtime, a, Word::fixnum(9)).expect("improper list");
    for (control, arguments, expected) in [
        ("~{x~}", vec![values], FormatErrorKind::InvalidDirectiveUse),
        ("~@{x~}", vec![a], FormatErrorKind::InvalidDirectiveUse),
        ("~-1{~A~}", vec![values], FormatErrorKind::InvalidParameter),
        ("~1,2{~A~}", vec![values], FormatErrorKind::InvalidParameter),
        ("~{~A~}", vec![improper], FormatErrorKind::ImproperList),
        ("~:{~A~}", vec![values], FormatErrorKind::ImproperList),
    ] {
        assert_error(control, &arguments, &expected, &mut ctx, &runtime);
    }
    let rows = list(&runtime, &mut ctx, &[values, values]);
    assert_eq!(
        run("~1:{~A~}", &[rows], &mut ctx, &runtime),
        ("a".into(), 1)
    );
    assert_eq!(
        run("~0{~A~}", &[values], &mut ctx, &runtime),
        (String::new(), 1)
    );
    assert_eq!(
        run("~1:@{~A~}", &[values, values], &mut ctx, &runtime),
        ("a".to_owned(), 1)
    );
}

#[test] // CLHS 22.3.7.4/.5, 22.3
fn iteration_limits_reject_character_parameters_and_dynamic_wrong_types() {
    let (runtime, mut ctx) = context();
    let value = string(&runtime, &mut ctx, "x");
    for (control, arguments, expected) in [
        ("~'x{~A~}", vec![value], FormatErrorKind::InvalidParameter),
        (
            "~v{~A~}",
            vec![string(&runtime, &mut ctx, "not an integer"), value],
            FormatErrorKind::WrongArgumentType,
        ),
        (
            "~v{~A~}",
            vec![Word::character(u32::from('x')), value],
            FormatErrorKind::InvalidParameter,
        ),
    ] {
        assert_error(control, &arguments, &expected, &mut ctx, &runtime);
    }
}

#[test] // CLHS 22.3.7.4/.5
fn colon_iteration_closing_forces_an_empty_body_unless_the_limit_is_zero() {
    let (runtime, mut ctx) = context();
    assert_eq!(
        run("~:{x~:}", &[Word::NIL], &mut ctx, &runtime),
        ("x".into(), 1)
    );
    assert_eq!(run("~:@{x~:}", &[], &mut ctx, &runtime), ("x".into(), 0));
    assert_eq!(
        run("~0:{x~:}", &[Word::NIL], &mut ctx, &runtime),
        (String::new(), 1)
    );
    assert_eq!(
        run("~0:@{x~:}", &[], &mut ctx, &runtime),
        (String::new(), 0)
    );
}

#[test] // CLHS 22.3.7.1, 22.3.9.2
fn argument_pointer_and_up_arrow_parameters_are_range_checked() {
    let (runtime, mut ctx) = context();
    let a = string(&runtime, &mut ctx, "a");
    let b = string(&runtime, &mut ctx, "b");
    assert_eq!(
        run("~2@*~A", &[a, b, a], &mut ctx, &runtime),
        ("a".into(), 3)
    );
    for (control, arguments, expected) in [
        ("~*", vec![], FormatErrorKind::InvalidParameter),
        ("~:*", vec![], FormatErrorKind::InvalidParameter),
        ("~:@*", vec![a], FormatErrorKind::InvalidParameter),
        ("~-1*", vec![a], FormatErrorKind::InvalidParameter),
        ("~2*", vec![a], FormatErrorKind::InvalidParameter),
        ("~@^", vec![], FormatErrorKind::InvalidDirectiveUse),
        ("~1,2,3,4^", vec![], FormatErrorKind::InvalidParameter),
    ] {
        assert_error(control, &arguments, &expected, &mut ctx, &runtime);
    }
    assert_eq!(run("~0,1^x", &[], &mut ctx, &runtime), ("x".into(), 0));
    assert_eq!(run("~1,2,3^x", &[], &mut ctx, &runtime), (String::new(), 0));
    assert_eq!(run("~^x", &[a], &mut ctx, &runtime), ("x".into(), 0));
    assert_eq!(run("~1,3^x", &[], &mut ctx, &runtime), ("x".into(), 0));
    assert_eq!(run("~2,1,3^x", &[], &mut ctx, &runtime), ("x".into(), 0));
    assert_error(
        "~'x*",
        &[a],
        &FormatErrorKind::InvalidParameter,
        &mut ctx,
        &runtime,
    );
    assert_error(
        "~4@*",
        &[a],
        &FormatErrorKind::InvalidParameter,
        &mut ctx,
        &runtime,
    );
}

#[test] // CLHS 22.3.7.4/.5
fn iteration_rejects_malformed_public_control_metadata() {
    let (runtime, mut ctx) = context();
    let valid_close = Some(terminator(TerminatorKind::RightBrace, false, false));
    for (body, close, parameters) in [
        (sequence(), None, vec![]),
        (DirectiveBody::None, valid_close, vec![]),
        (
            sequence(),
            Some(terminator(TerminatorKind::RightBracket, false, false)),
            vec![],
        ),
        (
            sequence(),
            Some(terminator(TerminatorKind::RightBrace, false, true)),
            vec![],
        ),
    ] {
        assert_ast_error(
            directive(
                DirectiveKind::Iteration,
                parameters,
                false,
                false,
                body,
                close,
            ),
            &[],
            &FormatErrorKind::InvalidDirectiveUse,
            &mut ctx,
            &runtime,
        );
    }
    assert_ast_error(
        directive(
            DirectiveKind::Iteration,
            vec![Parameter::Integer(1), Parameter::Integer(2)],
            false,
            false,
            sequence(),
            valid_close,
        ),
        &[],
        &FormatErrorKind::InvalidParameter,
        &mut ctx,
        &runtime,
    );
}

#[test] // CLHS 22.3.7.2
fn selection_rejects_malformed_public_control_metadata_and_empty_clauses() {
    let (runtime, mut ctx) = context();
    let right_bracket = Some(terminator(TerminatorKind::RightBracket, false, false));
    for (body, close, colon, at_sign, parameters) in [
        (DirectiveBody::None, right_bracket, false, false, vec![]),
        (
            DirectiveBody::Clauses(vec![clause(None)]),
            Some(terminator(TerminatorKind::RightBrace, false, false)),
            false,
            false,
            vec![],
        ),
        (
            DirectiveBody::Clauses(vec![clause(None), clause(None)]),
            right_bracket,
            false,
            true,
            vec![],
        ),
        (
            DirectiveBody::Clauses(vec![clause(None)]),
            right_bracket,
            true,
            false,
            vec![],
        ),
    ] {
        assert_ast_error(
            directive(
                DirectiveKind::Conditional,
                parameters,
                colon,
                at_sign,
                body,
                close,
            ),
            &[Word::fixnum(0)],
            &FormatErrorKind::InvalidDirectiveUse,
            &mut ctx,
            &runtime,
        );
    }

    let invalid_separator = DirectiveModifiers {
        parameters: vec![],
        colon: false,
        at_sign: true,
        position: 1,
    };
    assert_ast_error(
        directive(
            DirectiveKind::Conditional,
            vec![],
            false,
            false,
            DirectiveBody::Clauses(vec![clause(Some(invalid_separator)), clause(None)]),
            right_bracket,
        ),
        &[Word::fixnum(0)],
        &FormatErrorKind::InvalidDirectiveUse,
        &mut ctx,
        &runtime,
    );
    let invalid_parameter_separator = DirectiveModifiers {
        parameters: vec![Parameter::Integer(1)],
        colon: false,
        at_sign: false,
        position: 1,
    };
    assert_ast_error(
        directive(
            DirectiveKind::Conditional,
            vec![],
            false,
            false,
            DirectiveBody::Clauses(vec![
                clause(Some(invalid_parameter_separator)),
                clause(None),
            ]),
            right_bracket,
        ),
        &[Word::fixnum(0)],
        &FormatErrorKind::InvalidDirectiveUse,
        &mut ctx,
        &runtime,
    );
    let empty_clauses = FormatControl {
        parts: vec![ControlPart::Directive(directive(
            DirectiveKind::Conditional,
            vec![Parameter::Integer(0)],
            false,
            false,
            DirectiveBody::Clauses(vec![]),
            right_bracket,
        ))],
    };
    let mut sink = StringSink::new();
    assert_eq!(
        execute(&empty_clauses, &[], &mut ctx, &runtime, &mut sink),
        Ok(0)
    );
    assert_eq!(sink.into_string(), "");
}

#[test] // CLHS 22.3.8.1
fn case_conversion_rejects_malformed_public_control_metadata() {
    let (runtime, mut ctx) = context();
    let valid_close = Some(terminator(TerminatorKind::RightParen, false, false));
    for (body, close, parameters) in [
        (sequence(), None, vec![]),
        (DirectiveBody::None, valid_close, vec![]),
        (
            sequence(),
            Some(terminator(TerminatorKind::RightBracket, false, false)),
            vec![],
        ),
        (
            sequence(),
            Some(terminator(TerminatorKind::RightParen, true, false)),
            vec![],
        ),
        (
            sequence(),
            Some(terminator(TerminatorKind::RightParen, false, true)),
            vec![],
        ),
        (sequence(), valid_close, vec![Parameter::Integer(1)]),
    ] {
        assert_ast_error(
            directive(
                DirectiveKind::CaseConversion,
                parameters,
                false,
                false,
                body,
                close,
            ),
            &[],
            &FormatErrorKind::InvalidDirectiveUse,
            &mut ctx,
            &runtime,
        );
    }
}
