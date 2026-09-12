use super::{exact, index_argument, type_error};
use crate::{RuntimeError, Value};

pub fn cons(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "cons", 2)?;
    Ok(Value::cons(arguments[0].clone(), arguments[1].clone()))
}

pub fn car(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "car", 1)?;
    match &arguments[0] {
        Value::Nil | Value::Boolean(false) => Ok(Value::Nil),
        Value::Cons(cell) => Ok(cell.car()),
        value => Err(type_error("car", "list", value)),
    }
}

pub fn cdr(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "cdr", 1)?;
    match &arguments[0] {
        Value::Nil | Value::Boolean(false) => Ok(Value::Nil),
        Value::Cons(cell) => Ok(cell.cdr()),
        value => Err(type_error("cdr", "list", value)),
    }
}

pub fn cddr(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "cddr", 1)?;
    cdr(&[cdr(arguments)?])
}

pub fn first(arguments: &[Value]) -> Result<Value, RuntimeError> {
    car(arguments)
}

pub fn rest(arguments: &[Value]) -> Result<Value, RuntimeError> {
    cdr(arguments)
}

pub fn nthcdr(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "nthcdr", 2)?;
    let index = index_argument("nthcdr", &arguments[0])?;
    arguments[1]
        .nth_tail(index)
        .ok_or_else(|| type_error("nthcdr", "list", &arguments[1]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok_string(result: Result<Value, RuntimeError>) -> String {
        match result {
            Ok(value) => value.to_string(),
            Err(error) => panic!("expected Ok, got {error:?}"),
        }
    }

    #[test]
    fn cons_onto_a_non_list_builds_a_dotted_pair() {
        assert_eq!(
            ok_string(cons(&[Value::Integer(1), Value::Integer(2)])),
            "(1 . 2)"
        );
    }

    #[test]
    fn car_reads_the_head_of_a_dotted_list() {
        let dotted = Value::dotted_list(
            vec![Value::Integer(1), Value::Integer(2)],
            Value::Integer(3),
        );
        assert_eq!(ok_string(car(&[dotted])), "1");
    }

    #[test]
    fn cdr_returns_the_tail_when_a_single_item_dotted_list_shrinks_to_it() {
        let dotted = Value::dotted_list(vec![Value::Integer(1)], Value::Integer(2));
        assert_eq!(ok_string(cdr(&[dotted])), "2");
    }

    #[test]
    fn cdr_reports_a_type_error_for_a_non_list_argument() {
        assert!(matches!(
            cdr(&[Value::Integer(1)]),
            Err(RuntimeError::Type { .. })
        ));
    }

    #[test]
    fn cddr_preserves_the_actual_tail() -> Result<(), RuntimeError> {
        let tail = Value::list(vec![Value::Integer(3)]);
        let list = Value::cons(
            Value::Integer(1),
            Value::cons(Value::Integer(2), tail.clone()),
        );
        assert!(cddr(&[list])?.eq_value(&tail));
        Ok(())
    }

    #[test]
    fn cddr_handles_nil_and_dotted_tails() {
        assert_eq!(ok_string(cddr(&[Value::Nil])), "NIL");
        assert_eq!(
            ok_string(cddr(&[Value::dotted_list(
                vec![Value::Integer(1), Value::Integer(2)],
                Value::Integer(3),
            )])),
            "3"
        );
        assert!(matches!(
            cddr(&[Value::cons(Value::Integer(1), Value::Integer(2))]),
            Err(RuntimeError::Type { .. })
        ));
        assert!(cddr(&[]).is_err());
        assert!(cddr(&[Value::Nil, Value::Nil]).is_err());
    }
}
