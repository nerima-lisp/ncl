use crate::Value;
use crate::builtins::format::model::FormatParameter;
use crate::builtins::format::parameters::{
    format_parameter_character, format_parameter_count, format_parameter_number,
};
use crate::builtins::format::parser::{
    format_directive_prefix, parse_format_directive, parse_format_parameters,
};

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one table-driven test keeps the shared parser boundary matrix together"
)]
fn parses_format_parameters_through_shared_boundary_cases() {
    let parse = |control: &str, arguments: &[Value]| {
        let characters = control.chars().collect::<Vec<_>>();
        let mut character_index = 0;
        let mut argument_index = 0;
        parse_format_parameters(
            &characters,
            &mut character_index,
            arguments,
            &mut argument_index,
        )
    };

    let parser_cases = [
        (
            ",#,'xA",
            vec![Value::Integer(1), Value::Nil],
            &["missing", "number:2", "character:x"][..],
        ),
        (
            ",vA",
            vec![Value::Integer(-3)],
            &["missing", "number:-3"][..],
        ),
        ("12A", vec![], &["number:12"][..]),
        (",A", vec![], &["missing", "missing"][..]),
    ];
    for (control, arguments, expected) in parser_cases {
        let actual = parse(control, &arguments)
            .unwrap_or_else(|error| panic!("{control} should parse: {error}"))
            .into_iter()
            .map(|parameter| match parameter {
                FormatParameter::Missing => "missing".to_string(),
                FormatParameter::Number(value) => format!("number:{value}"),
                FormatParameter::Character(value) => format!("character:{value}"),
            })
            .collect::<Vec<_>>();
        assert_eq!(actual, expected, "{control}");
    }
    for (control, arguments) in [
        ("'", vec![]),
        ("-A", vec![]),
        ("999999999999999999999999A", vec![]),
        ("vA", vec![]),
        ("vA", vec![Value::string("not integer")]),
    ] {
        assert!(parse(control, &arguments).is_err(), "{control}");
    }

    let directive_cases = [("~:A", true), ("~@D", true), ("~:@D", true)];
    for (control, expected) in directive_cases {
        let characters = control.chars().collect::<Vec<_>>();
        let mut character_index = 1;
        let mut argument_index = 0;
        let directive =
            parse_format_directive(&characters, &mut character_index, &[], &mut argument_index)
                .unwrap_or_else(|error| panic!("{control} should parse: {error}"));
        assert_eq!(
            directive.colon_modifier || directive.at_sign_modifier,
            expected
        );
    }
    for control in ["~:Q", "~:"] {
        let characters = control.chars().collect::<Vec<_>>();
        let mut character_index = 1;
        let mut argument_index = 0;
        assert!(
            parse_format_directive(&characters, &mut character_index, &[], &mut argument_index,)
                .is_err(),
            "{control}"
        );
    }

    let numeric_cases = [
        (&[][..], 7, Ok(7_i64)),
        (&[FormatParameter::Number(11)][..], 7, Ok(11_i64)),
    ];
    for (parameters, default, expected) in numeric_cases {
        assert_eq!(format_parameter_number(parameters, 0, default), expected);
    }
    assert!(format_parameter_number(&[FormatParameter::Character('x')], 0, 7).is_err());

    let count_cases = [
        (&[][..], 3, Ok(3_usize)),
        (&[FormatParameter::Number(5)][..], 3, Ok(5_usize)),
    ];
    for (parameters, default, expected) in count_cases {
        assert_eq!(format_parameter_count(parameters, 0, default), expected);
    }
    assert!(format_parameter_count(&[FormatParameter::Number(-1)], 0, 3).is_err());

    let character_cases = [
        (&[][..], 'd', Ok('d')),
        (&[FormatParameter::Character('x')][..], 'd', Ok('x')),
    ];
    for (parameters, default, expected) in character_cases {
        assert_eq!(format_parameter_character(parameters, 0, default), expected);
    }
    assert!(format_parameter_character(&[FormatParameter::Number(1)], 0, 'd').is_err());

    let prefix_cases = [
        ("~,'xA", 1, Ok((4, false, false))),
        ("~,-12A", 1, Ok((5, false, false))),
        ("~12A", 2, Ok((3, false, false))),
    ];
    for (control, start, expected) in prefix_cases {
        let characters = control.chars().collect::<Vec<_>>();
        assert_eq!(format_directive_prefix(&characters, start), expected);
    }
    for control in ["~,'", "~,-A"] {
        let characters = control.chars().collect::<Vec<_>>();
        assert!(
            format_directive_prefix(&characters, 1).is_err(),
            "{control}"
        );
    }
}
