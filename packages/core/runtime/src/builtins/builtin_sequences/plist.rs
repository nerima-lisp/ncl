use super::{arity, exact, type_error};
use crate::{RuntimeError, Value};

pub fn getf(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(2..=3).contains(&arguments.len()) {
        return Err(arity("getf", "2 or 3", arguments.len()));
    }
    let Some(items) = arguments[0].list_items() else {
        return Err(type_error("getf", "property list", &arguments[0]));
    };
    if !items.len().is_multiple_of(2) {
        return Err(RuntimeError::InvalidForm {
            message: "getf requires an even-length property list".to_string(),
            span: None,
        });
    }
    for pair in items.as_chunks::<2>().0 {
        if arguments[1].eq_value(&pair[0]) {
            return Ok(pair[1].clone());
        }
    }
    Ok(arguments.get(2).cloned().unwrap_or(Value::Nil))
}

pub fn get_properties(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "get-properties", 2)?;
    let Some(plist) = arguments[0].list_items() else {
        return Err(type_error("get-properties", "property list", &arguments[0]));
    };
    let Some(indicators) = arguments[1].list_items() else {
        return Err(type_error("get-properties", "list", &arguments[1]));
    };
    if !plist.len().is_multiple_of(2) {
        return Err(RuntimeError::InvalidForm {
            message: "get-properties requires an even-length property list".to_string(),
            span: None,
        });
    }
    for (index, pair) in plist.as_chunks::<2>().0.iter().enumerate() {
        if indicators
            .iter()
            .any(|indicator| indicator.eq_value(&pair[0]))
        {
            return Ok(Value::values(vec![
                pair[0].clone(),
                pair[1].clone(),
                Value::list(plist[index * 2..].to_vec()),
            ]));
        }
    }
    Ok(Value::values(vec![Value::Nil, Value::Nil, Value::Nil]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn getf_reports_arity_type_and_odd_length_errors() {
        assert!(matches!(getf(&[]), Err(RuntimeError::Arity { .. })));
        assert!(matches!(
            getf(&[Value::Integer(1), Value::keyword("a")]),
            Err(RuntimeError::Type { .. })
        ));
        let odd_plist = Value::list(vec![Value::keyword("a")]);
        assert!(matches!(
            getf(&[odd_plist, Value::keyword("a")]),
            Err(RuntimeError::InvalidForm { .. })
        ));
    }

    #[test]
    fn getf_falls_back_to_the_supplied_default() {
        let plist = Value::list(vec![Value::keyword("a"), Value::Integer(1)]);
        match getf(&[plist, Value::keyword("b"), Value::Integer(42)]) {
            Ok(value) => assert_eq!(value.to_string(), "42"),
            Err(error) => panic!("expected Ok, got {error:?}"),
        }
    }

    #[test]
    fn get_properties_reports_type_and_odd_length_errors() {
        let list = Value::list(vec![Value::keyword("a")]);
        assert!(matches!(
            get_properties(&[Value::Integer(1), list.clone()]),
            Err(RuntimeError::Type { .. })
        ));
        assert!(matches!(
            get_properties(&[list.clone(), Value::Integer(1)]),
            Err(RuntimeError::Type { .. })
        ));
        let odd_plist = Value::list(vec![Value::keyword("a")]);
        assert!(matches!(
            get_properties(&[odd_plist, list]),
            Err(RuntimeError::InvalidForm { .. })
        ));
    }
}
