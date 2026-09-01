use super::{
    arity, index_argument, rebuild_sequence, replace_bounds, sequence_bounds, sequence_elements,
    sequence_length, type_error,
};
use crate::{RuntimeError, Value};

pub fn subseq(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(2..=3).contains(&arguments.len()) {
        return Err(arity("subseq", "2 or 3", arguments.len()));
    }
    let length = sequence_length(&arguments[0])
        .ok_or_else(|| type_error("subseq", "sequence", &arguments[0]))?;
    let start = index_argument("subseq", &arguments[1])?;
    let end = arguments
        .get(2)
        .map(|value| index_argument("subseq", value))
        .transpose()?
        .unwrap_or(length);
    if start > end || end > length {
        return Err(RuntimeError::InvalidForm {
            message: "subseq bounds are invalid".to_string(),
            span: None,
        });
    }
    match &arguments[0] {
        Value::Nil => Ok(Value::Nil),
        Value::List(items) => Ok(Value::list(items[start..end].to_vec())),
        Value::Vector(_) => Ok(Value::vector(
            arguments[0].vector_sequence_items().unwrap()[start..end].to_vec(),
        )),
        Value::String(value) => {
            let result = value
                .chars()
                .skip(start)
                .take(end - start)
                .collect::<String>();
            Ok(Value::string(result))
        }
        _ => Err(type_error("subseq", "sequence", &arguments[0])),
    }
}

pub fn fill(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() < 2 {
        return Err(arity("fill", "at least two", arguments.len()));
    }
    if !(arguments.len() - 2).is_multiple_of(2) {
        return Err(arity(
            "fill",
            "an item, a sequence, and keyword/value pairs",
            arguments.len(),
        ));
    }
    let length = sequence_length(&arguments[1])
        .ok_or_else(|| type_error("fill", "sequence", &arguments[1]))?;
    let (start, end) = sequence_bounds("fill", length, &arguments[2..])?;
    if matches!(arguments[1], Value::String(_)) && !matches!(arguments[0], Value::Character(_)) {
        return Err(type_error(
            "fill",
            "a character for a string",
            &arguments[0],
        ));
    }
    let mut items = sequence_elements("fill", &arguments[1])?;
    for item in &mut items[start..end] {
        *item = arguments[0].clone();
    }
    rebuild_sequence("fill", &arguments[1], items)
}

pub fn replace(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() < 2 {
        return Err(arity("replace", "at least two", arguments.len()));
    }
    if !(arguments.len() - 2).is_multiple_of(2) {
        return Err(arity(
            "replace",
            "two sequences and keyword/value pairs",
            arguments.len(),
        ));
    }
    let first_length = sequence_length(&arguments[0])
        .ok_or_else(|| type_error("replace", "sequence", &arguments[0]))?;
    let second_length = sequence_length(&arguments[1])
        .ok_or_else(|| type_error("replace", "sequence", &arguments[1]))?;
    let (start1, end1, start2, end2) =
        replace_bounds(first_length, second_length, &arguments[2..])?;
    let mut result = sequence_elements("replace", &arguments[0])?;
    let source = sequence_elements("replace", &arguments[1])?;
    let count = (end1 - start1).min(end2 - start2);
    if matches!(arguments[0], Value::String(_))
        && source[start2..start2 + count]
            .iter()
            .any(|value| !matches!(value, Value::Character(_)))
    {
        return Err(type_error(
            "replace",
            "characters in the source sequence for a string destination",
            &arguments[1],
        ));
    }
    result[start1..start1 + count].clone_from_slice(&source[start2..start2 + count]);
    rebuild_sequence("replace", &arguments[0], result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subseq_reports_arity_and_type_errors() {
        assert!(matches!(subseq(&[]), Err(RuntimeError::Arity { .. })));
        assert!(matches!(
            subseq(&[Value::Integer(1), Value::Integer(0)]),
            Err(RuntimeError::Type { .. })
        ));
    }

    #[test]
    fn subseq_of_nil_is_nil() {
        assert!(matches!(
            subseq(&[Value::Nil, Value::Integer(0)]),
            Ok(Value::Nil)
        ));
    }

    #[test]
    fn fill_reports_arity_and_type_errors() {
        assert!(matches!(
            fill(&[Value::Integer(1)]),
            Err(RuntimeError::Arity { .. })
        ));
        assert!(matches!(
            fill(&[Value::Integer(1), Value::Nil, Value::keyword("a")]),
            Err(RuntimeError::Arity { .. })
        ));
        assert!(matches!(
            fill(&[Value::Integer(1), Value::string("ab")]),
            Err(RuntimeError::Type { .. })
        ));
    }

    #[test]
    fn replace_reports_arity_and_type_errors() {
        assert!(matches!(
            replace(&[Value::Nil]),
            Err(RuntimeError::Arity { .. })
        ));
        assert!(matches!(
            replace(&[Value::Nil, Value::Nil, Value::keyword("a")]),
            Err(RuntimeError::Arity { .. })
        ));
        let destination = Value::string("ab");
        let source = Value::list(vec![Value::Integer(1), Value::Integer(2)]);
        assert!(matches!(
            replace(&[destination, source]),
            Err(RuntimeError::Type { .. })
        ));
    }
}
