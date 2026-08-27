#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::Value;
    use crate::builtins::format::boundaries;

    fn render(control: &str, arguments: impl AsRef<[Value]>) -> String {
        match format_control(control, arguments.as_ref()) {
            Ok(value) => value,
            Err(error) => panic!("format control should be valid: {error}"),
        }
    }

    fn assert_value(result: Result<Value, crate::RuntimeError>, expected: impl std::fmt::Display) {
        assert_eq!(
            match result {
                Ok(value) => value.to_string(),
                Err(error) => panic!("builtin should succeed: {error}"),
            },
            expected.to_string()
        );
    }

    #[test]
    fn renders_text_and_common_value_directives() {
        assert_eq!(
            render(
                "hello ~~ ~A ~S",
                vec![Value::symbol("x"), Value::Integer(7)]
            ),
            "hello ~ X 7"
        );
        assert_eq!(render("~:A", vec![Value::Nil]), "()");
        assert_eq!(
            render(
                "~C ~:C ~@C",
                vec![
                    Value::Character('a'),
                    Value::Character(' '),
                    Value::Character('z')
                ]
            ),
            "a Space #\\z"
        );
    }

    #[test]
    fn renders_aesthetic_sequences_and_nested_values() {
        let cases = [
            (Value::string("text"), "text"),
            (Value::Character('x'), "x"),
            (
                Value::list(vec![Value::Integer(1), Value::string("two")]),
                "(1 two)",
            ),
            (
                Value::dotted_list(vec![Value::Integer(1)], Value::symbol("tail")),
                "(1 . TAIL)",
            ),
            (
                Value::vector(vec![Value::Integer(1), Value::Character('x')]),
                "#(1 x)",
            ),
        ];

        for (value, expected) in cases {
            assert_eq!(render("~A", vec![value]), expected);
        }
    }

    #[test]
    fn renders_integer_radix_and_punctuation_directives() {
        assert_eq!(
            render(
                "~D ~B ~O ~X ~R",
                vec![
                    Value::Integer(42),
                    Value::Integer(5),
                    Value::Integer(8),
                    Value::Integer(15),
                    Value::Integer(4)
                ]
            ),
            "42 101 10 F four"
        );
        assert_eq!(
            render(
                "~P ~@P ~:P",
                vec![Value::Integer(2), Value::Integer(1), Value::Integer(2)]
            ),
            "s y "
        );
        assert_eq!(render("~%~&~|~~~_", vec![]), "\x0c~");
    }

    #[test]
    fn formats_numeric_helpers_at_boundary_values() {
        assert_eq!(format_integer_radix(0, 10), "0");
        assert_eq!(format_integer_radix(-42, 16), "-2A");
        assert_eq!(format_integer_radix(i64::MIN, 10), "-9223372036854775808");
        assert_eq!(format_unsigned_integer(255, 2), "11111111");
        assert_eq!(format_grouped_digits("1234567", ',', 3), "1,234,567");
        assert_eq!(format_grouped_digits("1234", ',', 0), "1234");
        assert_eq!(format_english_number(0, false), "zero");
        assert_eq!(format_english_number(0, true), "zeroth");
        assert_eq!(format_english_number(-42, false), "minus forty-two");
        assert_eq!(
            format_english_number(i64::MIN, false),
            "minus 9223372036854775808"
        );
        assert_eq!(
            format_english_number(i64::MAX, false),
            format_integer_radix(i64::MAX, 10)
        );
        assert_eq!(format_english_number(21, false), "twenty-one");
        assert_eq!(format_english_number(42, true), "forty-second");
        assert_eq!(format_roman_number(4, false), "IV");
        assert_eq!(format_roman_number(4, true), "IV");
        assert_eq!(format_roman_number(0, false), "N");
    }

    #[test]
    fn formats_english_numbers_from_table_cases() {
        let cases = [
            (1, false, "one"),
            (19, true, "nineteenth"),
            (20, false, "twenty"),
            (30, true, "thirtieth"),
            (100, false, "one hundred"),
            (100, true, "one hundredth"),
            (105, false, "one hundred five"),
            (101, true, "one hundred first"),
            (999, true, "nine hundred ninety-ninth"),
            (1_001, false, "one thousand one"),
            (1_000_000, true, "one millionth"),
        ];

        for (value, ordinal, expected) in cases {
            assert_eq!(format_english_number(value, ordinal), expected, "{value}");
        }
    }

    #[test]
    fn parses_format_choice_boundaries_from_table_cases() {
        let cases = [
            ("~A~:;~A", 2, vec![false, true]),
            ("~A~[~A~]", 1, vec![false]),
        ];

        for (control, expected_count, expected_defaults) in cases {
            let body = control.chars().collect::<Vec<_>>();
            let clauses = boundaries::format_choice_clauses(&body)
                .unwrap_or_else(|error| panic!("format choice should parse: {error}"));
            assert_eq!(clauses.len(), expected_count, "{control}");
            assert_eq!(
                clauses
                    .iter()
                    .map(|(_, default)| *default)
                    .collect::<Vec<_>>(),
                expected_defaults,
                "{control}"
            );
        }
    }

    #[test]
    fn rejects_malformed_format_boundaries_from_table_cases() {
        let cases = [
            (
                "~A~[~A",
                "format choice contains an unclosed nested directive",
            ),
            ("~A~]", "unexpected format choice terminator ~]"),
            (
                "~A~@;",
                "at-sign modifier is not supported on a format choice clause",
            ),
        ];

        for (control, expected_message) in cases {
            let body = control.chars().collect::<Vec<_>>();
            let Err(error) = boundaries::format_choice_clauses(&body) else {
                panic!("malformed format choice should fail: {control}");
            };
            assert!(
                error.to_string().contains(expected_message),
                "{control}: {error}"
            );
        }
    }

    #[test]
    fn parses_nested_format_boundaries_from_table_cases() {
        type BoundaryFinder = fn(&[char], usize) -> Result<usize, RuntimeError>;
        let cases: [(&str, BoundaryFinder, usize); 4] = [
            ("~{item ~[choice~]~}", boundaries::format_iteration_end, 17),
            ("~[item ~<justified~>~]", boundaries::format_choice_end, 20),
            (
                "~<item ~(case~)~>",
                boundaries::format_justification_end,
                15,
            ),
            (
                "~(item ~{iteration~}~)",
                boundaries::format_case_conversion_end,
                20,
            ),
        ];

        for (control, find_end, expected) in cases {
            let characters = control.chars().collect::<Vec<_>>();
            assert_eq!(find_end(&characters, 1), Ok(expected), "{control}");
        }
    }

    #[test]
    fn rejects_missing_format_boundaries_from_table_cases() {
        type BoundaryFinder = fn(&[char], usize) -> Result<usize, RuntimeError>;
        let cases: [(BoundaryFinder, &str); 4] = [
            (
                boundaries::format_iteration_end,
                "format iteration is missing ~}",
            ),
            (boundaries::format_choice_end, "format choice is missing ~]"),
            (
                boundaries::format_justification_end,
                "format justification is missing ~>",
            ),
            (
                boundaries::format_case_conversion_end,
                "format case conversion is missing ~)",
            ),
        ];

        for (find_end, expected_message) in cases {
            let characters = "~A".chars().collect::<Vec<_>>();
            let Err(error) = find_end(&characters, 1) else {
                panic!("missing format boundary should fail");
            };
            assert!(error.to_string().contains(expected_message));
        }
    }

    #[test]
    fn formats_character_and_radix_helper_variants() {
        assert_eq!(format_character_directive('\0', true, false), "Null");
        assert_eq!(format_character_directive('\n', false, true), "#\\Newline");
        assert_eq!(format_character_directive('?', true, true), "#\\?");
        assert_eq!(format_character_directive('a', false, false), "a");
        assert_eq!(format_grouped_digits("", ',', 3), "");
        assert_eq!(format_grouped_digits("1234", ',', 3), "1,234");
        assert_eq!(
            format_radix_directive(42, &[FormatParameter::Number(16)], false, false,)
                .unwrap_or_else(|error| panic!("hexadecimal radix should format: {error}")),
            "2A"
        );
        assert_eq!(
            format_radix_directive(4, &[], true, true)
                .unwrap_or_else(|error| panic!("roman number should format: {error}")),
            "IV"
        );
        assert_eq!(
            format_radix_directive(42, &[], false, false)
                .unwrap_or_else(|error| panic!("english number should format: {error}")),
            "forty-two"
        );
    }

    #[test]
    fn rejects_invalid_radix_parameters_and_missing_format_arguments() {
        for parameter in [
            FormatParameter::Number(1),
            FormatParameter::Number(-1),
            FormatParameter::Number(37),
            FormatParameter::Character('x'),
        ] {
            assert!(format_radix_directive(1, &[parameter], false, false).is_err());
        }

        let mut argument_index = 0;
        assert!(format_argument("~A", &[], &mut argument_index).is_err());
        assert_eq!(argument_index, 0);

        let arguments = [Value::Integer(7)];
        let argument = format_argument("~A", &arguments, &mut argument_index)
            .unwrap_or_else(|error| panic!("argument should be available: {error}"));
        assert_eq!(argument.to_string(), "7");
        assert_eq!(argument_index, 1);
    }

    #[test]
    fn renders_case_iteration_choice_and_nested_controls() {
        assert_eq!(render("~( ~A ~)", vec![Value::string("MiXeD")]), " mixed ");
        assert_eq!(
            render(
                "~{~A, ~}",
                vec![Value::list(vec![Value::Integer(1), Value::Integer(2)])]
            ),
            "1, 2, "
        );
        assert_eq!(render("~[zero~;one~;two~]", vec![Value::Integer(1)]), "one");
        assert_eq!(
            render(
                "~?",
                vec![Value::string("~A"), Value::list(vec![Value::Integer(3)])]
            ),
            "3"
        );
    }

    #[test]
    fn formats_tab_directive_from_table_cases() {
        let cases = [
            ("~T", "abc", "abc "),
            ("~5,4T", "", "     "),
            ("~5,4T", "abc", "abc  "),
            ("~5,4@T", "abc", "abc     "),
            ("~5,4T", "abcdefgh", "abcdefgh "),
            ("~0,0T", "abcdefgh", "abcdefgh"),
            ("~:T", "abc", "abc"),
            ("~5,4T", "ab\ncd", "ab\ncd   "),
        ];

        for (control, prefix, expected) in cases {
            assert_eq!(render(&format!("{prefix}{control}"), vec![]), expected);
        }
    }

    #[test]
    fn rounds_justification_width_to_the_column_increment() {
        assert_eq!(
            render(
                "~10,2,1<~A~;~A~>",
                vec![Value::string("a"), Value::string("b")]
            ),
            "a        b"
        );
        assert_eq!(
            render(
                "~10,3,1<~A~;~A~>",
                vec![Value::string("a"), Value::string("b")]
            ),
            "a        b"
        );
    }

    #[test]
    fn formats_general_float_non_finite_values_and_rejects_colon_modifier() {
        assert_eq!(
            format_general_float_directive(f64::INFINITY, &[], false, false)
                .unwrap_or_else(|error| panic!("infinity should format: {error}")),
            "Inf"
        );
        assert_eq!(
            format_general_float_directive(f64::NEG_INFINITY, &[], false, true)
                .unwrap_or_else(|error| panic!("negative infinity should format: {error}")),
            "-Inf"
        );
        assert_eq!(
            format_general_float_directive(f64::NAN, &[], false, false)
                .unwrap_or_else(|error| panic!("NaN should format: {error}")),
            "NaN"
        );
        let formatted = format_general_float_directive(
            f64::INFINITY,
            &[
                FormatParameter::Number(8),
                FormatParameter::Missing,
                FormatParameter::Missing,
                FormatParameter::Number(0),
                FormatParameter::Character('f'),
                FormatParameter::Character('g'),
                FormatParameter::Character('d'),
            ],
            false,
            false,
        )
        .unwrap_or_else(|error| panic!("parameterized infinity should format: {error}"));
        assert!(
            formatted.contains("Inf"),
            "unexpected output: {formatted:?}"
        );
        assert!(format_general_float_directive(1.0, &[], true, false).is_err());
    }

    #[test]
    fn formats_general_float_fixed_and_exponential_forms_and_validates_parameters() {
        let fixed = format_general_float_directive(
            12.5,
            &[
                FormatParameter::Number(0),
                FormatParameter::Number(2),
                FormatParameter::Number(0),
            ],
            false,
            false,
        )
        .unwrap_or_else(|error| panic!("fixed form should format: {error}"));
        assert!(fixed.contains("12"), "unexpected fixed output: {fixed:?}");

        let exponential = format_general_float_directive(
            1.25e20,
            &[
                FormatParameter::Number(0),
                FormatParameter::Number(2),
                FormatParameter::Number(0),
            ],
            false,
            false,
        )
        .unwrap_or_else(|error| panic!("exponential form should format: {error}"));
        assert!(
            exponential.contains('e'),
            "unexpected exponential output: {exponential:?}"
        );

        assert!(
            parse_general_float_parameters(&[
                FormatParameter::Missing,
                FormatParameter::Number(-1),
            ])
            .is_err()
        );
        assert!(
            parse_general_float_parameters(&[
                FormatParameter::Missing,
                FormatParameter::Character('x'),
            ])
            .is_err()
        );
        assert!(
            parse_general_float_parameters(&[
                FormatParameter::Missing,
                FormatParameter::Missing,
                FormatParameter::Character('x'),
            ])
            .is_err()
        );
    }

    #[test]
    fn formats_exponential_float_boundary_cases_from_table() {
        let cases = [
            (
                12.5,
                vec![FormatParameter::Number(0), FormatParameter::Number(2)],
                false,
                false,
                "1.25E+1",
            ),
            (
                -12.5,
                vec![FormatParameter::Number(0), FormatParameter::Number(2)],
                false,
                true,
                "-1.25E+1",
            ),
            (
                f64::INFINITY,
                vec![FormatParameter::Number(6)],
                false,
                true,
                "  +Inf",
            ),
            (
                f64::NEG_INFINITY,
                vec![FormatParameter::Number(6)],
                false,
                false,
                "  -Inf",
            ),
        ];

        for (value, parameters, colon, at_sign, expected) in cases {
            let actual = format_exponential_float_directive(value, &parameters, colon, at_sign)
                .unwrap_or_else(|error| panic!("exponential case should format: {error}"));
            assert_eq!(actual, expected, "value={value}");
        }

        assert!(
            format_exponential_float_directive(
                1.0,
                &[FormatParameter::Character('x')],
                false,
                false,
            )
            .is_err()
        );
        assert!(
            format_exponential_float_directive(
                1.0,
                &[FormatParameter::Number(0), FormatParameter::Number(-1)],
                false,
                false,
            )
            .is_err()
        );
    }

    #[test]
    fn formats_dollar_float_and_calculates_general_float_defaults_from_table() {
        let dollar_cases = [
            (12.5, vec![], false, false, "12.50"),
            (-12.5, vec![], false, true, "-12.50"),
            (12.5, vec![FormatParameter::Number(0)], true, false, "12."),
        ];
        for (value, parameters, colon, at_sign, expected) in dollar_cases {
            let actual = format_dollar_float_directive(value, &parameters, colon, at_sign)
                .unwrap_or_else(|error| panic!("dollar case should format: {error}"));
            assert_eq!(actual, expected, "value={value}");
        }

        assert_eq!(general_float_decimal_exponent(0.0), 1);
        assert_eq!(general_float_decimal_exponent(12.5), 2);
        assert_eq!(general_float_default_fractional_digits(0.0125, -2), 5);
        assert_eq!(general_float_default_fractional_digits(100.0, 3), 3);
    }

    #[test]
    fn parses_general_float_parameters_from_table_cases() {
        let valid_cases = [
            vec![],
            vec![
                FormatParameter::Number(12),
                FormatParameter::Number(3),
                FormatParameter::Number(5),
                FormatParameter::Character('f'),
                FormatParameter::Number(1),
                FormatParameter::Number(2),
                FormatParameter::Character('d'),
            ],
        ];
        for parameters in valid_cases {
            assert!(parse_general_float_parameters(&parameters).is_ok());
        }

        let invalid_cases = [
            vec![FormatParameter::Character('x')],
            vec![FormatParameter::Character('x'), FormatParameter::Number(1)],
            vec![FormatParameter::Number(1), FormatParameter::Character('x')],
            vec![
                FormatParameter::Number(1),
                FormatParameter::Number(1),
                FormatParameter::Number(-3),
            ],
            vec![FormatParameter::Number(-1)],
        ];
        for (index, parameters) in invalid_cases.into_iter().enumerate() {
            assert!(
                parse_general_float_parameters(&parameters).is_err(),
                "invalid general-float parameter case {index}"
            );
        }
    }

    #[test]
    fn formats_general_float_from_table_boundary_cases() {
        let cases = [
            (0.0, vec![], false, false),
            (
                12.5,
                vec![FormatParameter::Number(8), FormatParameter::Number(2)],
                false,
                false,
            ),
            (
                0.00125,
                vec![FormatParameter::Number(8), FormatParameter::Number(2)],
                false,
                true,
            ),
            (
                f64::INFINITY,
                vec![FormatParameter::Number(8)],
                false,
                false,
            ),
        ];
        for (value, parameters, colon, at_sign) in cases {
            assert!(
                format_general_float_directive(value, &parameters, colon, at_sign).is_ok(),
                "value={value}"
            );
        }
    }

    #[test]
    fn rejects_malformed_or_incompatible_directives() {
        assert!(format_control("~", &[]).is_err());
        assert!(format_control("~A", &[]).is_err());
        assert!(format_control("~A", &[Value::Nil]).is_ok());
        assert!(format_control("~D", &[Value::string("not integer")]).is_err());
        assert!(format_control("~@?", &[Value::string("~A"), Value::Integer(1)]).is_ok());
        assert!(format_control("~?", &[Value::Integer(1)]).is_err());
        assert!(format_control("~[a~;b~]", &[Value::string("not integer")]).is_err());
    }

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
                parse_format_directive(
                    &characters,
                    &mut character_index,
                    &[],
                    &mut argument_index,
                )
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

    #[test]
    fn formats_text_fields_from_table_cases() {
        let cases = [
            ("text", &[][..], false, "text"),
            ("text", &[FormatParameter::Number(8)][..], false, "text    "),
            (
                "text",
                &[FormatParameter::Number(8), FormatParameter::Number(1)][..],
                true,
                "    text",
            ),
            (
                "text",
                &[
                    FormatParameter::Number(4),
                    FormatParameter::Number(1),
                    FormatParameter::Number(3),
                    FormatParameter::Character('.'),
                ][..],
                false,
                "text...",
            ),
        ];
        for (text, parameters, at_sign, expected) in cases {
            assert_eq!(
                format_text_field(text, parameters, at_sign)
                    .unwrap_or_else(|error| panic!("text field should format: {error}")),
                expected
            );
        }
        assert!(
            format_text_field(
                "text",
                &[FormatParameter::Number(8), FormatParameter::Number(0)],
                false
            )
            .is_err()
        );
    }

    #[test]
    fn handles_core_list_operations_through_table_cases() {
        let cases = [
            (
                "list",
                list(&[Value::Integer(1), Value::Integer(2)]),
                Value::list(vec![Value::Integer(1), Value::Integer(2)]),
            ),
            (
                "list*",
                list_star(&[Value::Integer(1), Value::list(vec![Value::Integer(2)])]),
                Value::list(vec![Value::Integer(1), Value::Integer(2)]),
            ),
            (
                "make-list",
                make_list(&[Value::Integer(2)]),
                Value::list(vec![Value::Nil, Value::Nil]),
            ),
            (
                "values-list",
                values_list(&[Value::list(vec![Value::Integer(3)])]),
                Value::values(vec![Value::Integer(3)]),
            ),
        ];
        for (name, result, expected) in cases {
            let actual = result.unwrap_or_else(|error| panic!("{name}: {error}"));
            assert_eq!(actual.to_string(), expected.to_string());
        }
        assert!(list_star(&[]).is_err());
        assert!(make_list(&[Value::Integer(1), Value::keyword("unknown"), Value::Nil]).is_err());
        assert!(values_list(&[Value::Integer(1)]).is_err());
        assert!(make_list(&[]).is_err());
        assert!(make_list(&[Value::Integer(1), Value::keyword("initial-element")]).is_err());
        assert!(make_list(&[Value::string("not size")]).is_err());
        assert_value(
            make_list(&[
                Value::Integer(2),
                Value::keyword("initial-element"),
                Value::Integer(9),
            ]),
            Value::list(vec![Value::Integer(9), Value::Integer(9)]),
        );
        assert_value(list_length(&[Value::Nil]), Value::Integer(0));
        assert!(list_length(&[Value::Integer(1)]).is_err());
    }

    #[test]
    fn handles_sequence_transforms_and_bounds() {
        let string = Value::string("AbC");
        assert_value(
            string_upcase(std::slice::from_ref(&string)),
            Value::string("ABC"),
        );
        assert_value(
            string_downcase(std::slice::from_ref(&string)),
            Value::string("abc"),
        );
        assert_value(
            string_capitalize(&[Value::string("hello WORLD")]),
            Value::string("Hello World"),
        );
        assert_value(
            subseq(&[string.clone(), Value::Integer(1), Value::Integer(3)]),
            Value::string("bC"),
        );
        assert!(subseq(&[string, Value::Integer(3), Value::Integer(1)]).is_err());
        assert_value(
            length(&[Value::list(vec![Value::Nil, Value::Nil])]),
            Value::Integer(2),
        );
        assert!(elt(&[Value::string("a"), Value::Integer(2)]).is_err());
        assert_value(
            elt(&[Value::list(vec![Value::Integer(4)]), Value::Integer(0)]),
            Value::Integer(4),
        );
        assert_value(
            elt(&[Value::vector(vec![Value::Integer(5)]), Value::Integer(0)]),
            Value::Integer(5),
        );
        assert!(elt(&[Value::Nil, Value::Integer(0)]).is_err());
        assert!(elt(&[Value::Integer(1), Value::Integer(0)]).is_err());
    }

    #[test]
    fn handles_string_comparisons_and_type_predicates() {
        assert_value(
            string_equal(&[Value::string("abc"), Value::string("abc")]),
            Value::boolean(true),
        );
        assert_value(
            string_case_equal(&[Value::string("AbC"), Value::string("aBc")]),
            Value::boolean(true),
        );
        assert_value(
            string_less_than(&[Value::string("a"), Value::string("b")]),
            Value::Integer(0),
        );
        assert_value(
            string_greater_than(&[Value::string("b"), Value::string("a")]),
            Value::Integer(0),
        );
        assert_value(
            string_less_equal(&[Value::string("a"), Value::string("a")]),
            Value::Integer(1),
        );
        assert_value(
            string_greater_equal(&[Value::string("b"), Value::string("a")]),
            Value::Integer(0),
        );
        assert_value(
            string_less_equal(&[Value::string("b"), Value::string("a")]),
            Value::Nil,
        );
        assert_value(
            string_greater_equal(&[Value::string("a"), Value::string("b")]),
            Value::Nil,
        );
        assert_value(characterp(&[Value::Character('x')]), Value::boolean(true));
        assert_value(keywordp(&[Value::keyword("name")]), Value::boolean(true));
        assert_value(
            vectorp(&[Value::vector(vec![Value::Nil])]),
            Value::boolean(true),
        );
        assert_value(endp(&[Value::Nil]), Value::boolean(true));
        assert!(characterp(&[Value::Integer(1), Value::Integer(2)]).is_err());
        assert!(string_equal(&[Value::string("a"), Value::Integer(1)]).is_err());
    }

    #[test]
    fn handles_cons_property_and_list_access_operations() {
        let Ok(pair) = cons(&[Value::Integer(1), Value::list(vec![Value::Integer(2)])]) else {
            panic!("cons test input must be valid");
        };
        assert_value(car(std::slice::from_ref(&pair)), Value::Integer(1));
        assert_value(
            cdr(std::slice::from_ref(&pair)),
            Value::list(vec![Value::Integer(2)]),
        );
        assert_value(nth(&[Value::Integer(0), pair]), Value::Integer(1));
        assert_value(
            list_length(&[Value::list(vec![Value::Nil, Value::Nil])]),
            Value::Integer(2),
        );
        assert_value(
            acons(&[Value::symbol("key"), Value::Integer(3), Value::Nil]),
            Value::list(vec![Value::dotted_list(
                vec![Value::symbol("KEY")],
                Value::Integer(3),
            )]),
        );
        assert!(car(&[Value::Integer(1)]).is_err());
        assert_value(
            nth(&[Value::Integer(4), Value::list(vec![Value::Nil])]),
            Value::Nil,
        );

        let dotted = Value::dotted_list(
            vec![Value::Integer(2), Value::Integer(3)],
            Value::Integer(4),
        );
        assert_value(
            list_star(&[Value::Integer(1), dotted.clone()]),
            Value::dotted_list(
                vec![Value::Integer(1), Value::Integer(2), Value::Integer(3)],
                Value::Integer(4),
            ),
        );
        assert_value(
            nthcdr(&[Value::Integer(1), dotted.clone()]),
            Value::dotted_list(vec![Value::Integer(3)], Value::Integer(4)),
        );
        assert_value(
            nthcdr(&[Value::Integer(2), dotted.clone()]),
            Value::Integer(4),
        );
        assert!(nthcdr(&[Value::Integer(3), dotted.clone()]).is_err());
        assert_value(
            cdr(std::slice::from_ref(&dotted)),
            Value::dotted_list(vec![Value::Integer(3)], Value::Integer(4)),
        );
        assert_value(
            cons(&[Value::Integer(1), dotted.clone()]),
            Value::dotted_list(
                vec![Value::Integer(1), Value::Integer(2), Value::Integer(3)],
                Value::Integer(4),
            ),
        );
        assert_value(
            append(&[Value::list(vec![Value::Integer(1)]), dotted]),
            Value::dotted_list(
                vec![Value::Integer(1), Value::Integer(2), Value::Integer(3)],
                Value::Integer(4),
            ),
        );
        assert!(append(&[Value::Integer(1), Value::Nil]).is_err());
        assert_value(nthcdr(&[Value::Integer(0), Value::Nil]), Value::Nil);
        assert!(nthcdr(&[Value::Integer(0), Value::Integer(1)]).is_err());
        assert_value(
            pairlis(&[
                Value::list(vec![Value::symbol("a")]),
                Value::list(vec![Value::Integer(7)]),
            ]),
            Value::list(vec![Value::dotted_list(
                vec![Value::symbol("A")],
                Value::Integer(7),
            )]),
        );
        assert!(pairlis(&[Value::Nil]).is_err());
        assert!(pairlis(&[Value::list(vec![Value::Nil]), Value::Nil, Value::Integer(1)]).is_err());
        assert!(pairlis(&[Value::list(vec![Value::Nil]), Value::list(vec![])]).is_err());
    }

    #[test]
    fn renders_simple_directives_from_table_cases() {
        let cases = [
            ("x~%", vec![], "x\n"),
            ("x~2%", vec![], "x\n\n"),
            ("x~&", vec![], "x\n"),
            ("x\n~&", vec![], "x\n"),
            ("~~", vec![], "~"),
            ("~|", vec![], "\x0c"),
            ("~C", vec![Value::Character('\n')], "\n"),
            ("~:C", vec![Value::Character(' ')], "Space"),
            ("~_", vec![], ""),
            ("~*~A", vec![Value::Integer(1), Value::Integer(2)], "2"),
            ("~P", vec![Value::Integer(2)], "s"),
            ("~@P", vec![Value::Integer(2)], "ies"),
        ];

        for (control, arguments, expected) in cases {
            assert_eq!(render(control, arguments), expected, "control: {control}");
        }
    }

    #[test]
    fn renders_format_parameter_variants_from_table_cases() {
        let cases = [
            ("~,'0D", vec![Value::Integer(7)], "7"),
            ("~V,'0D", vec![Value::Integer(3), Value::Integer(7)], "007"),
            ("~#D", vec![Value::Integer(1), Value::Integer(2)], " 1"),
            ("~3D", vec![Value::Integer(7)], "  7"),
            ("~10,1,0,'_A", vec![Value::string("x")], "x_________"),
        ];

        for (control, arguments, expected) in cases {
            assert_eq!(render(control, arguments), expected, "control: {control}");
        }
    }

    #[test]
    fn rejects_malformed_format_controls_from_table_cases() {
        let cases = [
            ("~", vec![]),
            ("~'", vec![]),
            ("~-", vec![]),
            ("~V", vec![]),
            ("~V", vec![Value::string("not an integer")]),
            ("~1,", vec![]),
            ("~:Z", vec![]),
            ("~A", vec![]),
            ("~}", vec![]),
            ("~[", vec![Value::Integer(0)]),
            ("~[a~;b", vec![Value::Integer(0)]),
            ("~{", vec![Value::list(vec![Value::Integer(1)])]),
            ("~<", vec![Value::Integer(1)]),
            ("~(", vec![Value::Integer(1)]),
        ];

        for (control, arguments) in cases {
            assert!(
                format_control(control, &arguments).is_err(),
                "malformed control should fail: {control}"
            );
        }
    }

    #[test]
    fn rejects_incompatible_format_modifiers_from_table_cases() {
        let cases = [
            ("~:P", vec![]),
            ("~@I", vec![]),
            ("~1W", vec![Value::Integer(1)]),
            ("~1_", vec![]),
            ("~:?[ignored]", vec![Value::string("~A"), Value::Integer(1)]),
            ("~:C", vec![Value::Integer(1)]),
            ("~:[one~;two~;three~]", vec![Value::Nil]),
            ("~@[one~;two~]", vec![Value::Nil]),
            ("~1,2,3,4,5<~A~>", vec![Value::Integer(1)]),
        ];

        for (control, arguments) in cases {
            assert!(
                format_control(control, &arguments).is_err(),
                "incompatible format control should fail: {control}"
            );
        }
    }

    #[test]
    fn rejects_invalid_format_invocation_shapes_from_table_cases() {
        let cases = [
            vec![],
            vec![Value::Nil],
            vec![Value::Nil, Value::Integer(1)],
            vec![Value::Integer(1), Value::string("~A"), Value::Integer(1)],
        ];

        for arguments in cases {
            assert!(
                format_value(&arguments).is_err(),
                "invalid FORMAT invocation should fail: {arguments:?}"
            );
        }
    }
}
