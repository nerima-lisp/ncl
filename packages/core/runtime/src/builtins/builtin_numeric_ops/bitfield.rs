use super::{RuntimeError, Value, exact, number_from_big, number_to_value, type_error};
use crate::builtins::numbers::big_integer_argument;

fn byte_spec(function: &str, value: &Value) -> Result<(usize, usize), RuntimeError> {
    let Some(items) = value.list_items() else {
        return Err(type_error(function, "a byte specifier", value));
    };
    if items.len() != 2 {
        return Err(type_error(function, "a byte specifier", value));
    }
    let size = non_negative_index(function, &items[0])?;
    let position = non_negative_index(function, &items[1])?;
    Ok((size, position))
}

fn non_negative_index(function: &str, value: &Value) -> Result<usize, RuntimeError> {
    let index = big_integer_argument(function, value)?;
    if index < ibig::IBig::from(0) {
        return Err(type_error(function, "a non-negative integer", value));
    }
    usize::try_from(index).map_err(|_| RuntimeError::NumericOverflow)
}

fn mask(size: usize) -> ibig::IBig {
    (ibig::IBig::from(1) << size) - 1
}

fn shifted_mask(size: usize, position: usize) -> ibig::IBig {
    mask(size) << position
}

pub fn byte(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "byte", 2)?;
    non_negative_index("byte", &arguments[0])?;
    non_negative_index("byte", &arguments[1])?;
    Ok(Value::list(arguments.to_vec()))
}

pub fn ldb(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "ldb", 2)?;
    let (size, position) = byte_spec("ldb", &arguments[0])?;
    let integer = big_integer_argument("ldb", &arguments[1])?;
    number_to_value(number_from_big((integer >> position) & mask(size)))
}

pub fn mask_field(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "mask-field", 2)?;
    let (size, position) = byte_spec("mask-field", &arguments[0])?;
    let integer = big_integer_argument("mask-field", &arguments[1])?;
    number_to_value(number_from_big(integer & shifted_mask(size, position)))
}

pub fn dpb(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "dpb", 3)?;
    let (size, position) = byte_spec("dpb", &arguments[1])?;
    let new_bits = big_integer_argument("dpb", &arguments[0])?;
    let integer = big_integer_argument("dpb", &arguments[2])?;
    let field_mask = shifted_mask(size, position);
    let result = (integer & !field_mask.clone()) | ((new_bits & mask(size)) << position);
    number_to_value(number_from_big(result))
}

pub fn deposit_field(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "deposit-field", 3)?;
    let (size, position) = byte_spec("deposit-field", &arguments[1])?;
    let new_bits = big_integer_argument("deposit-field", &arguments[0])?;
    let integer = big_integer_argument("deposit-field", &arguments[2])?;
    let field_mask = shifted_mask(size, position);
    let result = (integer & !field_mask.clone()) | (new_bits & field_mask);
    number_to_value(number_from_big(result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitfield_operations_follow_common_lisp_layout() {
        let byte = byte(&[Value::Integer(4), Value::Integer(4)]).unwrap();
        assert!(ldb(&[byte.clone(), Value::Integer(0xabc)]).unwrap().equal_value(&Value::Integer(0xb)));
        assert!(mask_field(&[byte.clone(), Value::Integer(0xabc)]).unwrap().equal_value(&Value::Integer(0xb0)));
        assert!(dpb(&[Value::Integer(2), byte.clone(), Value::Integer(0xabc)]).unwrap().equal_value(&Value::Integer(0xa2c)));
        assert!(deposit_field(&[Value::Integer(0x050), byte, Value::Integer(0xabc)]).unwrap().equal_value(&Value::Integer(0xa5c)));
    }
}
