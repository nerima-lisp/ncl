use crate::Value;
use crate::builtins::format::english::format_english_number;
use crate::builtins::format::integer_helpers::{format_integer_radix, format_unsigned_integer};
use crate::builtins::format::model::FormatParameter;
use crate::builtins::format::output::{
    format_argument, format_character_directive, format_grouped_digits, format_radix_directive,
    format_roman_number,
};

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
