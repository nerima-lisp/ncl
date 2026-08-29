#![allow(clippy::wildcard_imports)]

use super::*;

pub(super) fn replacement_mask(
    matched: Vec<usize>,
    count: Option<usize>,
    from_end: bool,
    item_count: usize,
) -> Vec<bool> {
    let limit = count.unwrap_or(matched.len()).min(matched.len());
    let mut replace = vec![false; item_count];
    if from_end {
        for index in matched.into_iter().rev().take(limit) {
            replace[index] = true;
        }
    } else {
        for index in matched.into_iter().take(limit) {
            replace[index] = true;
        }
    }
    replace
}

pub(super) fn result(
    kind: SequenceKind,
    items: Vec<Value>,
    replace: &[bool],
    new_item: &Value,
    span: Span,
) -> Result<Value, RuntimeError> {
    let result = items
        .into_iter()
        .enumerate()
        .map(|(index, value)| {
            if replace[index] {
                new_item.clone()
            } else {
                value
            }
        })
        .collect::<Vec<_>>();
    match kind {
        SequenceKind::List => Ok(Value::list(result)),
        SequenceKind::Vector => Ok(Value::vector(result)),
        SequenceKind::String => result
            .into_iter()
            .map(|item| match item {
                Value::Character(character) => Ok(character),
                item => Err(RuntimeError::Type {
                    expected: "CHARACTER".to_string(),
                    actual: item.type_name().to_string(),
                    span: Some(span),
                }),
            })
            .collect::<Result<String, _>>()
            .map(Value::string),
    }
}
