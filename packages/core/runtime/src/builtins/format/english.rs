#[allow(clippy::wildcard_imports)]
use super::*;

pub(crate) fn format_english_number(value: &ibig::IBig, ordinal: bool) -> String {
    let negative = value < &ibig::IBig::from(0);
    let magnitude = if negative {
        -value.clone()
    } else {
        value.clone()
    };
    if negative {
        return format!("negative {}", format_english_number(&magnitude, ordinal));
    }
    if magnitude == ibig::IBig::from(0) {
        return if ordinal {
            "zeroth".to_string()
        } else {
            "zero".to_string()
        };
    }
    let mut chunks = Vec::new();
    let group = ibig::IBig::from(1000);
    let mut remainder = magnitude;
    while remainder != ibig::IBig::from(0) {
        let chunk = &remainder % &group;
        chunks.push(u64::try_from(&chunk).unwrap_or_default());
        remainder = &remainder / &group;
    }
    if chunks.len() > 6 {
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

pub(crate) fn format_english_number_value(value: &ibig::IBig, ordinal: bool) -> String {
    if let Ok(value) = i64::try_from(value) {
        return format_english_number(&ibig::IBig::from(value), ordinal);
    }
    let negative = value < &ibig::IBig::from(0);
    let digits = value.to_string();
    let magnitude = digits.strip_prefix('-').unwrap_or(&digits);
    let first_width = magnitude.len() % 3;
    let first_width = if first_width == 0 { 3 } else { first_width };
    let mut chunks = Vec::new();
    let mut start = 0;
    while start < magnitude.len() {
        let width = if start == 0 { first_width } else { 3 };
        let chunk = magnitude[start..start + width].parse::<u64>().unwrap_or(0);
        chunks.push(chunk);
        start += width;
    }
    if chunks.len() > ENGLISH_NUMBER_GROUPS.len() {
        return digits;
    }
    let mut parts = Vec::new();
    let ordinal_group = if ordinal {
        chunks.iter().rposition(|chunk| *chunk != 0)
    } else {
        None
    };
    for (index, chunk) in chunks.iter().enumerate() {
        if *chunk == 0 {
            continue;
        }
        let group_index = chunks.len() - index - 1;
        let group_is_ordinal = ordinal_group == Some(index);
        let mut part = english_under_thousand(*chunk, group_is_ordinal && group_index == 0);
        if group_index != 0 {
            part.push(' ');
            part.push_str(ENGLISH_NUMBER_GROUPS[group_index]);
            if group_is_ordinal {
                part.push_str("th");
            }
        }
        parts.push(part);
    }
    if parts.is_empty() {
        return if ordinal { "zeroth" } else { "zero" }.to_string();
    }
    let result = parts.join(" ");
    if negative {
        format!("minus {result}")
    } else {
        result
    }
}

pub(crate) fn english_under_thousand(value: u64, ordinal: bool) -> String {
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
