use super::{
    english_under_thousand, format_character_directive, format_english_number,
    format_grouped_digits, format_integer_directive, format_radix_directive, format_roman_number,
    FormatParameter,
};

#[test]
fn grouped_digits_follow_a_table_of_boundaries() {
    let cases = [
        ("", ',', 3, ""),
        ("12", ',', 3, "12"),
        ("1234", ',', 3, "1,234"),
        ("1234567", '_', 3, "1_234_567"),
        ("123456", ' ', 2, "12 34 56"),
    ];

    for (digits, separator, interval, expected) in cases {
        assert_eq!(format_grouped_digits(digits, separator, interval), expected);
    }
}

#[test]
fn character_directives_cover_named_and_literal_forms() {
    let cases = [
        ('\0', false, false, "\0"),
        ('\n', true, false, "Newline"),
        (' ', false, true, "#\\Space"),
        ('x', true, false, "x"),
        ('x', false, true, "#\\x"),
    ];

    for (character, colon_modifier, at_sign_modifier, expected) in cases {
        assert_eq!(
            format_character_directive(character, colon_modifier, at_sign_modifier),
            expected
        );
    }
}

#[test]
fn english_number_helpers_cover_cardinal_ordinal_and_fallbacks() {
    let under_thousand_cases = [
        (0, false, "zero"),
        (19, false, "nineteen"),
        (20, false, "twenty"),
        (21, false, "twenty-one"),
        (90, true, "ninetieth"),
        (100, false, "one hundred"),
        (100, true, "one hundredth"),
        (101, true, "one hundred first"),
    ];
    for (value, ordinal, expected) in under_thousand_cases {
        assert_eq!(english_under_thousand(value, ordinal), expected);
    }

    let number_cases = [
        (0, false, "zero"),
        (42, false, "forty-two"),
        (1001, true, "one thousand first"),
        (-42, true, "minus forty-second"),
        (i64::MIN, false, "minus 9223372036854775808"),
        (1_000_000_000_000_000_000, false, "1000000000000000000"),
    ];
    for (value, ordinal, expected) in number_cases {
        assert_eq!(format_english_number(value, ordinal), expected);
    }
}

#[test]
fn radix_and_roman_directives_share_the_integer_boundary() {
    assert_eq!(
        format_integer_directive(42, 16, &[], false, false).unwrap(),
        "2A"
    );
    assert_eq!(
        format_radix_directive(42, &[FormatParameter::Number(16)], false, false).unwrap(),
        "2A"
    );

    let roman_cases = [
        (0, false, "N"),
        (42, false, "XLII"),
        (-42, false, "-XLII"),
        (3999, false, "MMMCMXCIX"),
        (4000, false, "4000"),
        (4000, true, "MMMM"),
    ];
    for (value, old_style, expected) in roman_cases {
        assert_eq!(format_roman_number(value, old_style), expected);
    }
}
