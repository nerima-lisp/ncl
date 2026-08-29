#[allow(clippy::wildcard_imports)]
use super::*;

pub(super) fn format_justification_clauses(body: &[char]) -> Result<Vec<&[char]>, RuntimeError> {
    let mut clauses = Vec::new();
    let mut clause_start = 0;
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
                message: "format justification clause ends after a tilde".to_string(),
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
                        message: format!("unexpected format justification terminator ~{directive}"),
                        span: None,
                    });
                } else {
                    return Err(RuntimeError::InvalidForm {
                        message: format!("mismatched format justification terminator ~{directive}"),
                        span: None,
                    });
                }
            }
            ';' if stack.is_empty() => {
                if colon_modifier || at_sign_modifier {
                    return Err(RuntimeError::InvalidForm {
                        message: "format justification does not support modifiers on ~;"
                            .to_string(),
                        span: None,
                    });
                }
                clauses.push(&body[clause_start..index]);
                clause_start = directive_index + 1;
            }
            _ => {}
        }
        index = directive_index + 1;
    }
    if !stack.is_empty() {
        return Err(RuntimeError::InvalidForm {
            message: "format justification contains an unclosed nested directive".to_string(),
            span: None,
        });
    }
    clauses.push(&body[clause_start..]);
    Ok(clauses)
}
