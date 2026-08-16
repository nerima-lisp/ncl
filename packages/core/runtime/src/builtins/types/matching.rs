fn type_designator_name(function: &str, value: &Value) -> Result<String, RuntimeError> {
    let type_name = match value {
        Value::Nil | Value::Boolean(false) => "NIL",
        Value::Boolean(true) => "T",
        Value::Symbol(name)
        | Value::UninternedSymbol(name)
        | Value::Keyword(name)
        | Value::SymbolExact(name)
        | Value::KeywordExact(name) => name.as_ref(),
        value => return Err(type_error(function, "type designator", value)),
    };
    let type_name = type_name.rsplit("::").next().unwrap_or(type_name);
    Ok(package::normalize_symbol_name(type_name))
}

fn type_matches_designator(
    function: &str,
    value: &Value,
    type_designator: &Value,
    environment: Option<&Environment>,
) -> Result<bool, RuntimeError> {
    match type_designator {
        Value::List(items) => type_matches_compound(function, value, items.as_ref(), environment),
        Value::DottedList { .. } => Err(invalid_type_spec(
            function,
            "type designator must be a proper list",
        )),
        _ => {
            let type_name = type_designator_name(function, type_designator)?;
            type_matches(value, &type_name, environment)
        }
    }
}

fn type_matches_compound(
    function: &str,
    value: &Value,
    items: &[Value],
    environment: Option<&Environment>,
) -> Result<bool, RuntimeError> {
    let Some(operator_value) = items.first() else {
        return Err(invalid_type_spec(
            function,
            "compound type designator must name an operator",
        ));
    };
    let operator = type_designator_name(function, operator_value)?;
    let arguments = &items[1..];
    match operator.as_str() {
        "OR" => {
            for type_designator in arguments {
                if type_matches_designator(function, value, type_designator, environment)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        "AND" => {
            for type_designator in arguments {
                if !type_matches_designator(function, value, type_designator, environment)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        "NOT" => {
            require_type_spec_arity(function, &operator, arguments, 1, 1)?;
            Ok(!type_matches_designator(
                function,
                value,
                &arguments[0],
                environment,
            )?)
        }
        "MEMBER" => Ok(arguments
            .iter()
            .any(|candidate| eql_value(value, candidate))),
        "EQL" => {
            require_type_spec_arity(function, &operator, arguments, 1, 1)?;
            Ok(eql_value(value, &arguments[0]))
        }
        "INTEGER" => integer_type_matches(function, value, arguments),
        "MOD" => mod_type_matches(function, value, arguments),
        "SIGNED-BYTE" => signed_byte_type_matches(function, value, arguments),
        "UNSIGNED-BYTE" => unsigned_byte_type_matches(function, value, arguments),
        "CONS" => cons_type_matches(function, value, arguments, environment),
        "VECTOR" => vector_type_matches(function, value, arguments, environment),
        "SIMPLE-VECTOR" => simple_vector_type_matches(function, value, arguments),
        "BIT-VECTOR" | "SIMPLE-BIT-VECTOR" => bit_vector_type_matches(function, value, arguments),
        "ARRAY" | "SIMPLE-ARRAY" => {
            array_type_matches(function, &operator, value, arguments, environment)
        }
        _ => Err(invalid_type_spec(
            function,
            format!("unknown compound type designator {operator}"),
        )),
    }
}

fn require_type_spec_arity(
    function: &str,
    operator: &str,
    arguments: &[Value],
    minimum: usize,
    maximum: usize,
) -> Result<(), RuntimeError> {
    if (minimum..=maximum).contains(&arguments.len()) {
        Ok(())
    } else {
        Err(invalid_type_spec(
            function,
            format!("{operator} type specifier expects between {minimum} and {maximum} arguments"),
        ))
    }
}

fn invalid_type_spec(function: &str, message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidForm {
        message: format!("{function}: {}", message.into()),
        span: None,
    }
}

fn integer_type_matches(
    function: &str,
    value: &Value,
    arguments: &[Value],
) -> Result<bool, RuntimeError> {
    require_type_spec_arity(function, "INTEGER", arguments, 0, 2)?;
    let lower = arguments
        .first()
        .map(|bound| integer_type_bound(function, bound))
        .transpose()?
        .flatten();
    let upper = arguments
        .get(1)
        .map(|bound| integer_type_bound(function, bound))
        .transpose()?
        .flatten();
    let Value::Integer(number) = value else {
        return Ok(false);
    };
    Ok(lower.is_none_or(|bound| *number >= bound) && upper.is_none_or(|bound| *number <= bound))
}

fn integer_type_bound(function: &str, value: &Value) -> Result<Option<i64>, RuntimeError> {
    if is_type_wildcard(value) {
        return Ok(None);
    }
    match value {
        Value::Integer(bound) => Ok(Some(*bound)),
        value => Err(type_error(function, "integer or *", value)),
    }
}

fn mod_type_matches(
    function: &str,
    value: &Value,
    arguments: &[Value],
) -> Result<bool, RuntimeError> {
    require_type_spec_arity(function, "MOD", arguments, 1, 1)?;
    let Value::Integer(modulus) = arguments[0] else {
        return Err(type_error(function, "non-negative integer", &arguments[0]));
    };
    if modulus < 0 {
        return Err(invalid_type_spec(
            function,
            "MOD type specifier requires a non-negative modulus",
        ));
    }
    let Value::Integer(number) = value else {
        return Ok(false);
    };
    Ok(*number >= 0 && *number < modulus)
}

fn unsigned_byte_type_matches(
    function: &str,
    value: &Value,
    arguments: &[Value],
) -> Result<bool, RuntimeError> {
    let size = byte_type_size(function, "UNSIGNED-BYTE", arguments)?;
    let Value::Integer(number) = value else {
        return Ok(false);
    };
    if *number < 0 {
        return Ok(false);
    }
    let Some(size) = size else {
        return Ok(true);
    };
    if size >= 63 {
        return Ok(true);
    }
    let upper = (1_i128 << size) - 1;
    Ok((*number as i128) <= upper)
}

fn signed_byte_type_matches(
    function: &str,
    value: &Value,
    arguments: &[Value],
) -> Result<bool, RuntimeError> {
    let size = byte_type_size(function, "SIGNED-BYTE", arguments)?;
    let Value::Integer(number) = value else {
        return Ok(false);
    };
    let Some(size) = size else {
        return Ok(true);
    };
    if size == 0 {
        return Ok(false);
    }
    if size >= 64 {
        return Ok(true);
    }
    let half = 1_i128 << (size - 1);
    let number = *number as i128;
    Ok(number >= -half && number < half)
}

fn byte_type_size(
    function: &str,
    operator: &str,
    arguments: &[Value],
) -> Result<Option<usize>, RuntimeError> {
    require_type_spec_arity(function, operator, arguments, 0, 1)?;
    let Some(size) = arguments.first() else {
        return Ok(None);
    };
    if is_type_wildcard(size) {
        return Ok(None);
    }
    let Value::Integer(size) = size else {
        return Err(type_error(function, "non-negative integer or *", size));
    };
    usize::try_from(*size).map(Some).map_err(|_| {
        invalid_type_spec(
            function,
            format!("{operator} type specifier requires a non-negative size"),
        )
    })
}

fn cons_type_matches(
    function: &str,
    value: &Value,
    arguments: &[Value],
    environment: Option<&Environment>,
) -> Result<bool, RuntimeError> {
    require_type_spec_arity(function, "CONS", arguments, 0, 2)?;
    let Some((car, cdr)) = cons_parts(value) else {
        return Ok(false);
    };
    if let Some(car_type) = arguments.first()
        && !type_matches_designator(function, &car, car_type, environment)?
    {
        return Ok(false);
    }
    if let Some(cdr_type) = arguments.get(1)
        && !type_matches_designator(function, &cdr, cdr_type, environment)?
    {
        return Ok(false);
    }
    Ok(true)
}

fn cons_parts(value: &Value) -> Option<(Value, Value)> {
    match value {
        Value::List(items) if !items.is_empty() => {
            let items = items.as_ref();
            let tail = if items.len() == 1 {
                Value::Nil
            } else {
                Value::list(items[1..].to_vec())
            };
            Some((items[0].clone(), tail))
        }
        Value::DottedList { items, tail } if !items.is_empty() => {
            Some((items[0].clone(), (*tail).as_ref().clone()))
        }
        _ => None,
    }
}

fn vector_type_matches(
    function: &str,
    value: &Value,
    arguments: &[Value],
    environment: Option<&Environment>,
) -> Result<bool, RuntimeError> {
    require_type_spec_arity(function, "VECTOR", arguments, 0, 2)?;
    let expected_size = arguments
        .get(1)
        .map(|size| type_spec_size(function, size))
        .transpose()?
        .flatten();
    let Some(items) = value.vector_items() else {
        return Ok(false);
    };
    if expected_size.is_some_and(|size| size != items.len()) {
        return Ok(false);
    }
    if let Some(element_type) = arguments.first() {
        for item in &items {
            if !type_matches_element_spec(function, item, element_type, environment)? {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn simple_vector_type_matches(
    function: &str,
    value: &Value,
    arguments: &[Value],
) -> Result<bool, RuntimeError> {
    require_type_spec_arity(function, "SIMPLE-VECTOR", arguments, 0, 1)?;
    let expected_size = arguments
        .first()
        .map(|size| type_spec_size(function, size))
        .transpose()?
        .flatten();
    let Some(items) = value.vector_items() else {
        return Ok(false);
    };
    Ok(expected_size.is_none_or(|size| size == items.len()))
}

fn bit_vector_type_matches(
    function: &str,
    value: &Value,
    arguments: &[Value],
) -> Result<bool, RuntimeError> {
    require_type_spec_arity(function, "BIT-VECTOR", arguments, 0, 1)?;
    let expected_size = arguments
        .first()
        .map(|size| type_spec_size(function, size))
        .transpose()?
        .flatten();
    let Some(items) = value.vector_items() else {
        return Ok(false);
    };
    if expected_size.is_some_and(|size| size != items.len()) {
        return Ok(false);
    }
    Ok(items.iter().all(is_bit_value))
}

fn is_bit_vector_value(value: &Value) -> bool {
    value
        .vector_items()
        .is_some_and(|items| items.iter().all(is_bit_value))
}

fn is_bit_value(value: &Value) -> bool {
    matches!(value, Value::Integer(bit) if *bit == 0 || *bit == 1)
}

fn array_type_matches(
    function: &str,
    operator: &str,
    value: &Value,
    arguments: &[Value],
    environment: Option<&Environment>,
) -> Result<bool, RuntimeError> {
    require_type_spec_arity(function, operator, arguments, 0, 2)?;
    let Some(actual_dimensions) = dimensions_for_array(value) else {
        return Ok(false);
    };
    if let Some(expected_dimensions) = arguments.get(1)
        && !array_dimensions_match(function, expected_dimensions, &actual_dimensions)?
    {
        return Ok(false);
    }
    let Some(elements) = array_elements(value) else {
        return Ok(false);
    };
    if let Some(element_type) = arguments.first() {
        for element in &elements {
            if !type_matches_element_spec(function, element, element_type, environment)? {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn type_matches_element_spec(
    function: &str,
    value: &Value,
    type_designator: &Value,
    environment: Option<&Environment>,
) -> Result<bool, RuntimeError> {
    if is_type_wildcard(type_designator) {
        Ok(true)
    } else {
        type_matches_designator(function, value, type_designator, environment)
    }
}

fn type_spec_size(function: &str, value: &Value) -> Result<Option<usize>, RuntimeError> {
    if is_type_wildcard(value) {
        return Ok(None);
    }
    let Value::Integer(size) = value else {
        return Err(type_error(function, "non-negative integer or *", value));
    };
    usize::try_from(*size)
        .map(Some)
        .map_err(|_| invalid_type_spec(function, "sequence or array size must be non-negative"))
}

fn array_dimensions_match(
    function: &str,
    expected: &Value,
    actual: &[usize],
) -> Result<bool, RuntimeError> {
    if is_type_wildcard(expected) {
        return Ok(true);
    }
    match expected {
        Value::Nil | Value::Boolean(false) => Ok(actual.is_empty()),
        Value::Integer(rank) => {
            let rank = usize::try_from(*rank)
                .map_err(|_| invalid_type_spec(function, "array rank must be non-negative"))?;
            Ok(actual.len() == rank)
        }
        Value::List(dimensions) => {
            let dimensions = dimensions.as_ref();
            if dimensions.len() != actual.len() {
                return Ok(false);
            }
            for (dimension, actual) in dimensions.iter().zip(actual) {
                if is_type_wildcard(dimension) {
                    continue;
                }
                let Value::Integer(expected) = dimension else {
                    return Err(type_error(function, "array dimension or *", dimension));
                };
                let expected = usize::try_from(*expected).map_err(|_| {
                    invalid_type_spec(function, "array dimensions must be non-negative")
                })?;
                if expected != *actual {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        value => Err(type_error(function, "array dimensions", value)),
    }
}

fn is_type_wildcard(value: &Value) -> bool {
    value
        .symbol_name()
        .is_some_and(|name| name.eq_ignore_ascii_case("*"))
}

fn type_matches(
    value: &Value,
    type_name: &str,
    environment: Option<&Environment>,
) -> Result<bool, RuntimeError> {
    let result = match type_name {
        "T" | "OBJECT" => true,
        "NIL" | "NULL" => matches!(value, Value::Nil | Value::Boolean(false)),
        "BOOLEAN" => matches!(value, Value::Nil | Value::Boolean(_)),
        "NUMBER" => matches!(
            value,
            Value::Integer(_) | Value::Rational(_) | Value::Float(_) | Value::Complex { .. }
        ),
        "REAL" => matches!(
            value,
            Value::Integer(_) | Value::Rational(_) | Value::Float(_)
        ),
        "COMPLEX" => matches!(value, Value::Complex { .. }),
        "RATIONAL" => matches!(value, Value::Integer(_) | Value::Rational(_)),
        "RATIO" => matches!(value, Value::Rational(_)),
        "INTEGER" | "FIXNUM" | "BIGNUM" => matches!(value, Value::Integer(_)),
        "BIT" => is_bit_value(value),
        "FLOAT" => matches!(value, Value::Float(_)),
        "CHARACTER" | "BASE-CHAR" | "STANDARD-CHAR" | "EXTENDED-CHAR" => {
            matches!(value, Value::Character(_))
        }
        "STRING" | "BASE-STRING" | "SIMPLE-STRING" | "SIMPLE-BASE-STRING" => {
            matches!(value, Value::String(_))
        }
        "STREAM" => matches!(value, Value::Stream(_)),
        "SYMBOL" => matches!(
            value,
            Value::Nil
                | Value::Boolean(_)
                | Value::Symbol(_)
                | Value::UninternedSymbol(_)
                | Value::Keyword(_)
                | Value::SymbolExact(_)
                | Value::KeywordExact(_)
        ),
        "PACKAGE" => matches!(value, Value::Package(_)),
        "ENVIRONMENT" => matches!(value, Value::Environment(_)),
        "KEYWORD" => matches!(value, Value::Keyword(_) | Value::KeywordExact(_)),
        "CONS" => matches!(value, Value::List(_) | Value::DottedList { .. }),
        "LIST" => matches!(value, Value::Nil | Value::Boolean(false) | Value::List(_)),
        "ATOM" => !matches!(value, Value::List(_) | Value::DottedList { .. }),
        "VECTOR" => value.vector_items().is_some(),
        "SIMPLE-VECTOR" => value.is_simple_vector(),
        "BIT-VECTOR" | "SIMPLE-BIT-VECTOR" => is_bit_vector_value(value),
        "ARRAY" => dimensions_for_array(value).is_some(),
        "SIMPLE-ARRAY" => simple_array_value(value),
        "HASH-TABLE" => matches!(value, Value::HashTable { .. }),
        "CONDITION" => matches!(value, Value::Condition(_)),
        "RESTART" => matches!(value, Value::Restart(_)),
        "ERROR" | "SERIOUS-CONDITION" | "WARNING" | "SIMPLE-CONDITION" | "SIMPLE-ERROR"
        | "SIMPLE-WARNING" | "ARITHMETIC-ERROR" | "DIVISION-BY-ZERO" | "TYPE-ERROR"
        | "PROGRAM-ERROR" | "PACKAGE-ERROR" | "READER-ERROR" | "COMPILER-ERROR" | "FILE-ERROR"
        | "UNBOUND-VARIABLE" | "CONTROL-ERROR" => value.condition_is_type(type_name),
        "STRUCTURE" => value.structure_name().is_some(),
        "SEQUENCE" => matches!(value, Value::Boolean(false)) || sequence_length(value).is_some(),
        "FUNCTION" | "COMPILED-FUNCTION" => matches!(value, Value::Function(_)),
        "GENERIC-FUNCTION" | "STANDARD-GENERIC-FUNCTION" => matches!(
            value,
            Value::Function(function) if matches!(function.as_ref(), crate::Function::Generic { .. })
        ),
        "UNBOUND" => matches!(value, Value::Unbound),
        "VALUES" => matches!(value, Value::Values(_)),
        "CLASS" => matches!(value, Value::Class(_)),
        "METHOD" | "STANDARD-METHOD" => matches!(value, Value::Method(_)),
        "STANDARD-OBJECT" => matches!(value, Value::Instance(_)),
        _ if environment
            .is_some_and(|environment| environment.lookup_condition(type_name).is_some()) =>
        {
            value.condition_is_type(type_name)
        }
        _ if value.instance_is_type(type_name) => true,
        _ if value.structure_is_type(type_name) => true,
        _ => {
            return Err(RuntimeError::InvalidForm {
                message: format!("unknown type designator {type_name}"),
                span: None,
            });
        }
    };
    Ok(result)
}
