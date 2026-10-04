pub fn translate_wildcards(source: &str, target: &str, value: &str) -> Option<String> {
    wildcard_captures(source, value).map(|captures| substitute_wildcards(target, &captures))
}

fn wildcard_captures(pattern: &str, value: &str) -> Option<Vec<String>> {
    fn search(
        pattern: &[char],
        value: &[char],
        pattern_index: usize,
        value_index: usize,
        captures: &mut Vec<String>,
    ) -> bool {
        if pattern_index == pattern.len() {
            return value_index == value.len();
        }
        let Some(&token) = pattern.get(pattern_index) else {
            return false;
        };
        if token == '*' {
            for end in value_index..=value.len() {
                captures.push(
                    value
                        .iter()
                        .skip(value_index)
                        .take(end - value_index)
                        .collect(),
                );
                if search(pattern, value, pattern_index + 1, end, captures) {
                    return true;
                }
                let _ = captures.pop();
            }
            return false;
        }
        let Some(&value_token) = value.get(value_index) else {
            return false;
        };
        if token != '?' && token != value_token {
            return false;
        }
        search(pattern, value, pattern_index + 1, value_index + 1, captures)
    }

    let pattern = pattern.chars().collect::<Vec<_>>();
    let value = value.chars().collect::<Vec<_>>();
    let mut captures = Vec::new();
    search(&pattern, &value, 0, 0, &mut captures).then_some(captures)
}

fn substitute_wildcards(pattern: &str, captures: &[String]) -> String {
    let mut result = String::new();
    let mut capture = captures.iter();
    for token in pattern.chars() {
        if token == '*' {
            if let Some(value) = capture.next() {
                result.push_str(value);
            }
        } else {
            result.push(token);
        }
    }
    result
}

pub fn directory_match(
    ctx: &ThreadContext,
    pattern: Word,
    value: Word,
) -> Result<bool, ObjectError> {
    if pattern == Word::NIL {
        return Ok(true);
    }
    fn parts(ctx: &ThreadContext, value: Word) -> Result<Vec<Word>, ObjectError> {
        let mut result = Vec::new();
        let mut cursor = value;
        while cursor != Word::NIL {
            result.push(car(ctx, cursor)?);
            cursor = cdr(ctx, cursor)?;
        }
        Ok(result)
    }
    fn matches(ctx: &ThreadContext, pattern: &[Word], value: &[Word]) -> Result<bool, ObjectError> {
        if pattern.is_empty() {
            return Ok(value.is_empty());
        }
        if symbol_text(ctx, pattern[0])
            .is_ok_and(|name| name.eq_ignore_ascii_case("WILD-INFERIORS"))
        {
            for consumed in 0..=value.len() {
                // check-added-lines: allow(index) bounded by value length
                if matches(ctx, &pattern[1..], &value[consumed..])? {
                    return Ok(true);
                }
            }
            return Ok(false);
        }
        if value.is_empty() || !pathname_component_match(ctx, pattern[0], value[0])? {
            return Ok(false);
        }
        matches(ctx, &pattern[1..], &value[1..])
    }
    matches(ctx, &parts(ctx, pattern)?, &parts(ctx, value)?)
}
use super::operations::pathname_component_match;
use super::{ObjectError, ThreadContext, Word, car, cdr, symbol_text};
