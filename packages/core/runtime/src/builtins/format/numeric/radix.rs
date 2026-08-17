fn format_radix_directive(
    value: i64,
    parameters: &[FormatParameter],
    colon_modifier: bool,
    at_sign_modifier: bool,
) -> Result<String, RuntimeError> {
    if let Some(parameter) = parameters.first().copied()
        && !matches!(parameter, FormatParameter::Missing)
    {
        let radix = match parameter {
            FormatParameter::Number(value) => {
                u32::try_from(value).map_err(|_| RuntimeError::InvalidForm {
                    message: "format radix must be between 2 and 36".to_string(),
                    span: None,
                })?
            }
            FormatParameter::Missing => unreachable!(),
            FormatParameter::Character(_) => {
                return Err(RuntimeError::InvalidForm {
                    message: "format radix must be numeric".to_string(),
                    span: None,
                });
            }
        };
        if !(2..=36).contains(&radix) {
            return Err(RuntimeError::InvalidForm {
                message: "format radix must be between 2 and 36".to_string(),
                span: None,
            });
        }
        return format_integer_directive(value, radix, &parameters[1..], false, at_sign_modifier);
    }
    if at_sign_modifier {
        Ok(format_roman_number(value, colon_modifier))
    } else {
        Ok(format_english_number(value, colon_modifier))
    }
}

fn format_english_number(value: i64, ordinal: bool) -> String {
    if value < 0 {
        if value == i64::MIN {
            return format!(
                "minus {}",
                format_unsigned_integer(value.unsigned_abs(), 10)
            );
        }
        return format!(
            "minus {}",
            format_english_number(value.wrapping_neg(), ordinal)
        );
    }
    let magnitude = value as u64;
    if magnitude == 0 {
        return if ordinal {
            "zeroth".to_string()
        } else {
            "zero".to_string()
        };
    }
    const GROUPS: &[&str] = &[
        "",
        "thousand",
        "million",
        "billion",
        "trillion",
        "quadrillion",
    ];
    let mut chunks = Vec::new();
    let mut remainder = magnitude;
    while remainder != 0 {
        chunks.push(remainder % 1000);
        remainder /= 1000;
    }
    if chunks.len() > GROUPS.len() {
        return format_integer_radix(value, 10);
    }
    let ordinal_group = if ordinal {
        chunks.iter().position(|chunk| *chunk != 0)
    } else {
        None
    };
    let mut parts = Vec::new();
    for index in (0..chunks.len()).rev() {
        let chunk = chunks[index];
        if chunk == 0 {
            continue;
        }
        let group_is_ordinal = ordinal_group == Some(index);
        let mut part = if group_is_ordinal && index == 0 {
            english_under_thousand(chunk, true)
        } else {
            english_under_thousand(chunk, false)
        };
        if index != 0 {
            part.push(' ');
            part.push_str(GROUPS[index]);
            if group_is_ordinal {
                part.push_str("th");
            }
        }
        parts.push(part);
    }
    parts.join(" ")
}

fn english_under_thousand(value: u64, ordinal: bool) -> String {
    const CARDINALS: &[&str] = &[
        "zero",
        "one",
        "two",
        "three",
        "four",
        "five",
        "six",
        "seven",
        "eight",
        "nine",
        "ten",
        "eleven",
        "twelve",
        "thirteen",
        "fourteen",
        "fifteen",
        "sixteen",
        "seventeen",
        "eighteen",
        "nineteen",
    ];
    const ORDINALS: &[&str] = &[
        "zeroth",
        "first",
        "second",
        "third",
        "fourth",
        "fifth",
        "sixth",
        "seventh",
        "eighth",
        "ninth",
        "tenth",
        "eleventh",
        "twelfth",
        "thirteenth",
        "fourteenth",
        "fifteenth",
        "sixteenth",
        "seventeenth",
        "eighteenth",
        "nineteenth",
    ];
    const TENS: &[&str] = &[
        "",
        "",
        "twenty",
        "thirty",
        "forty",
        "fifty",
        "sixty",
        "seventy",
        "eighty",
        "ninety",
    ];
    const ORDINAL_TENS: &[&str] = &[
        "",
        "",
        "twentieth",
        "thirtieth",
        "fortieth",
        "fiftieth",
        "sixtieth",
        "seventieth",
        "eightieth",
        "ninetieth",
    ];
    if value < 20 {
        return if ordinal {
            ORDINALS[value as usize].to_string()
        } else {
            CARDINALS[value as usize].to_string()
        };
    }
    if value < 100 {
        let tens = value / 10;
        let ones = value % 10;
        if ones == 0 {
            return if ordinal {
                ORDINAL_TENS[tens as usize].to_string()
            } else {
                TENS[tens as usize].to_string()
            };
        }
        return format!(
            "{}-{}",
            TENS[tens as usize],
            english_under_thousand(ones, ordinal)
        );
    }
    let hundreds = value / 100;
    let remainder = value % 100;
    if remainder == 0 {
        if ordinal {
            format!("{} hundredth", CARDINALS[hundreds as usize])
        } else {
            format!("{} hundred", CARDINALS[hundreds as usize])
        }
    } else {
        format!(
            "{} hundred {}",
            CARDINALS[hundreds as usize],
            english_under_thousand(remainder, ordinal)
        )
    }
}

fn format_roman_number(value: i64, old_style: bool) -> String {
    if value == 0 {
        return "N".to_string();
    }
    let negative = value < 0;
    let magnitude = value.unsigned_abs();
    if !old_style && magnitude > 3999 {
        return format_integer_radix(value, 10);
    }
    let numerals = [
        (1000_u64, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut remainder = magnitude;
    let mut result = String::new();
    if negative {
        result.push('-');
    }
    for (unit, numeral) in numerals {
        while remainder >= unit {
            result.push_str(numeral);
            remainder -= unit;
        }
    }
    result
}
