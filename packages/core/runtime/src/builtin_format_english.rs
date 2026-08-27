#![allow(clippy::redundant_pub_crate)]
#[allow(clippy::wildcard_imports)]
use super::*;

pub(super) fn format_english_number(value: i64, ordinal: bool) -> String {
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
    let magnitude = value.unsigned_abs();
    if magnitude == 0 {
        return if ordinal {
            "zeroth".to_string()
        } else {
            "zero".to_string()
        };
    }
    let mut chunks = Vec::new();
    let mut remainder = magnitude;
    while remainder != 0 {
        chunks.push(remainder % 1000);
        remainder /= 1000;
    }
    if chunks.len() > ENGLISH_NUMBER_GROUPS.len() {
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
            part.push_str(ENGLISH_NUMBER_GROUPS[index]);
            if group_is_ordinal {
                part.push_str("th");
            }
        }
        parts.push(part);
    }
    parts.join(" ")
}

pub(super) fn english_under_thousand(value: u64, ordinal: bool) -> String {
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
        "", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
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
        let index = usize::from(u8::try_from(value).unwrap_or_default());
        return if ordinal {
            ORDINALS[index].to_string()
        } else {
            CARDINALS[index].to_string()
        };
    }
    if value < 100 {
        let tens = usize::from(u8::try_from(value / 10).unwrap_or_default());
        let ones = value % 10;
        if ones == 0 {
            return if ordinal {
                ORDINAL_TENS[tens].to_string()
            } else {
                TENS[tens].to_string()
            };
        }
        return format!("{}-{}", TENS[tens], english_under_thousand(ones, ordinal));
    }
    let hundreds = usize::from(u8::try_from(value / 100).unwrap_or_default());
    let remainder = value % 100;
    if remainder == 0 {
        if ordinal {
            format!("{} hundredth", CARDINALS[hundreds])
        } else {
            format!("{} hundred", CARDINALS[hundreds])
        }
    } else {
        format!(
            "{} hundred {}",
            CARDINALS[hundreds],
            english_under_thousand(remainder, ordinal)
        )
    }
}
