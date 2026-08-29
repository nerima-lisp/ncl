#[allow(clippy::wildcard_imports)]
use super::*;

pub(super) fn format_iteration_end(
    characters: &[char],
    start: usize,
) -> Result<usize, RuntimeError> {
    format_directive_end(characters, start, '{', "format iteration is missing ~}")
}

pub(super) fn format_choice_end(characters: &[char], start: usize) -> Result<usize, RuntimeError> {
    format_directive_end(characters, start, '[', "format choice is missing ~]")
}

pub(super) fn format_justification_end(
    characters: &[char],
    start: usize,
) -> Result<usize, RuntimeError> {
    format_directive_end(characters, start, '<', "format justification is missing ~>")
}

pub(super) fn format_case_conversion_end(
    characters: &[char],
    start: usize,
) -> Result<usize, RuntimeError> {
    format_directive_end(
        characters,
        start,
        '(',
        "format case conversion is missing ~)",
    )
}

pub(super) fn format_directive_end(
    characters: &[char],
    start: usize,
    opening: char,
    missing_message: &str,
) -> Result<usize, RuntimeError> {
    let mut stack = vec![opening];
    let mut index = start;
    while index < characters.len() {
        if characters[index] != '~' {
            index += 1;
            continue;
        }
        let (directive_index, _, _) = format_directive_prefix(characters, index + 1)?;
        let Some(directive) = characters.get(directive_index).copied() else {
            break;
        };
        match directive.to_ascii_uppercase() {
            '{' | '[' | '<' | '(' => stack.push(directive.to_ascii_uppercase()),
            '}' | ']' | '>' | ')' => {
                let expected_opening = match directive {
                    '}' => '{',
                    ']' => '[',
                    '>' => '<',
                    ')' => '(',
                    _ => unreachable!(
                        "the outer match arm already narrowed directive to a closing bracket"
                    ),
                };
                if stack.last().copied() == Some(expected_opening) {
                    stack.pop();
                    if stack.is_empty() {
                        return Ok(index);
                    }
                }
            }
            _ => {}
        }
        index = directive_index + 1;
    }
    Err(RuntimeError::InvalidForm {
        message: missing_message.to_string(),
        span: None,
    })
}

pub(super) fn format_choice_clauses(body: &[char]) -> Result<Vec<(&[char], bool)>, RuntimeError> {
    let mut clauses = Vec::new();
    let mut clause_start = 0;
    let mut default_clause = false;
    let mut stack = Vec::new();
    let mut index = 0;
    while index < body.len() {
        if body[index] != '~' {
            index += 1;
            continue;
        }
        let (directive_index, colon_modifier, at_sign_modifier) =
            format_directive_prefix(body, index + 1)?;
        let Some(directive) = body.get(directive_index).copied() else {
            return Err(RuntimeError::InvalidForm {
                message: "format choice clause ends after a tilde".to_string(),
                span: None,
            });
        };
        let directive = directive.to_ascii_uppercase();
        match directive {
            '{' | '[' | '<' | '(' => stack.push(directive),
            '}' | ']' | '>' | ')' => {
                let expected_opening = match directive {
                    '}' => '{',
                    ']' => '[',
                    '>' => '<',
                    ')' => '(',
                    _ => unreachable!(
                        "the outer match arm already narrowed directive to a closing bracket"
                    ),
                };
                if stack.last().copied() == Some(expected_opening) {
                    stack.pop();
                } else if stack.is_empty() {
                    return Err(RuntimeError::InvalidForm {
                        message: format!("unexpected format choice terminator ~{directive}"),
                        span: None,
                    });
                }
            }
            ';' if stack.is_empty() => {
                if at_sign_modifier {
                    return Err(RuntimeError::InvalidForm {
                        message: "at-sign modifier is not supported on a format choice clause"
                            .to_string(),
                        span: None,
                    });
                }
                clauses.push((&body[clause_start..index], default_clause));
                clause_start = directive_index + 1;
                default_clause = colon_modifier;
            }
            _ => {}
        }
        index = directive_index + 1;
    }
    if !stack.is_empty() {
        return Err(RuntimeError::InvalidForm {
            message: "format choice contains an unclosed nested directive".to_string(),
            span: None,
        });
    }
    clauses.push((&body[clause_start..], default_clause));
    Ok(clauses)
}
