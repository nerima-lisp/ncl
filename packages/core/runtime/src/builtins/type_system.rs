use super::*;

pub(crate) fn typep_value(value: &Value, type_designator: &Value) -> Result<bool, RuntimeError> {
    type_matches_designator("typep", value, type_designator)
}

pub(crate) fn subtypep_value(
    subtype: &Value,
    supertype: &Value,
    environment: &Environment,
) -> Result<Value, RuntimeError> {
    validate_subtype_designator("subtypep", subtype, environment)?;
    validate_subtype_designator("subtypep", supertype, environment)?;
    let relation = subtype_relation(subtype, supertype, environment)?;
    Ok(Value::values(vec![
        Value::boolean(relation.unwrap_or(false)),
        Value::boolean(relation.is_some()),
    ]))
}

fn validate_subtype_designator(
    function: &str,
    designator: &Value,
    environment: &Environment,
) -> Result<(), RuntimeError> {
    match designator {
        Value::List(items) => {
            let Some(operator_value) = items.first() else {
                return Err(invalid_type_spec(
                    function,
                    "compound type designator must name an operator",
                ));
            };
            let operator = type_designator_name(function, operator_value)?;
            let arguments = &items[1..];
            match operator.as_str() {
                "OR" | "AND" => {
                    for argument in arguments {
                        validate_subtype_designator(function, argument, environment)?;
                    }
                }
                "NOT" | "EQL" => {
                    require_type_spec_arity(function, &operator, arguments, 1, 1)?;
                    if operator == "NOT" {
                        validate_subtype_designator(function, &arguments[0], environment)?;
                    }
                }
                "MEMBER" => {}
                "INTEGER" => {
                    require_type_spec_arity(function, &operator, arguments, 0, 2)?;
                    for bound in arguments {
                        integer_type_bound(function, bound)?;
                    }
                }
                "MOD" => {
                    require_type_spec_arity(function, &operator, arguments, 1, 1)?;
                    let Value::Integer(modulus) = arguments[0] else {
                        return Err(type_error(function, "non-negative integer", &arguments[0]));
                    };
                    if modulus < 0 {
                        return Err(invalid_type_spec(
                            function,
                            "MOD type specifier requires a non-negative modulus",
                        ));
                    }
                }
                "SIGNED-BYTE" | "UNSIGNED-BYTE" => {
                    byte_type_size(function, &operator, arguments)?;
                }
                "CONS" => {
                    require_type_spec_arity(function, &operator, arguments, 0, 2)?;
                    for argument in arguments {
                        validate_subtype_designator(function, argument, environment)?;
                    }
                }
                "VECTOR" => {
                    require_type_spec_arity(function, &operator, arguments, 0, 2)?;
                    if let Some(element_type) = arguments.first() {
                        validate_element_subtype_designator(function, element_type, environment)?;
                    }
                    if let Some(size) = arguments.get(1) {
                        type_spec_size(function, size)?;
                    }
                }
                "SIMPLE-VECTOR" | "BIT-VECTOR" | "SIMPLE-BIT-VECTOR" => {
                    require_type_spec_arity(function, &operator, arguments, 0, 1)?;
                    if let Some(size) = arguments.first() {
                        type_spec_size(function, size)?;
                    }
                }
                "ARRAY" | "SIMPLE-ARRAY" => {
                    require_type_spec_arity(function, &operator, arguments, 0, 2)?;
                    if let Some(element_type) = arguments.first() {
                        validate_element_subtype_designator(function, element_type, environment)?;
                    }
                    if let Some(dimensions) = arguments.get(1) {
                        validate_array_dimensions_spec(function, dimensions)?;
                    }
                }
                _ => {
                    return Err(invalid_type_spec(
                        function,
                        format!("unknown compound type designator {operator}"),
                    ));
                }
            }
            Ok(())
        }
        Value::DottedList { .. } => Err(invalid_type_spec(
            function,
            "type designator must be a proper list",
        )),
        _ => {
            let type_name = type_designator_name(function, designator)?;
            if known_type_name(&type_name, environment) {
                Ok(())
            } else {
                Err(invalid_type_spec(
                    function,
                    format!("unknown type designator {type_name}"),
                ))
            }
        }
    }
}

fn validate_element_subtype_designator(
    function: &str,
    designator: &Value,
    environment: &Environment,
) -> Result<(), RuntimeError> {
    if is_type_wildcard(designator) {
        Ok(())
    } else {
        validate_subtype_designator(function, designator, environment)
    }
}

fn validate_array_dimensions_spec(function: &str, dimensions: &Value) -> Result<(), RuntimeError> {
    if is_type_wildcard(dimensions) {
        return Ok(());
    }
    match dimensions {
        Value::Nil | Value::Boolean(false) => Ok(()),
        Value::Integer(rank) => usize::try_from(*rank)
            .map(|_| ())
            .map_err(|_| invalid_type_spec(function, "array rank must be non-negative")),
        Value::List(dimensions) => {
            for dimension in dimensions.iter() {
                if is_type_wildcard(dimension) {
                    continue;
                }
                let Value::Integer(dimension) = dimension else {
                    return Err(type_error(function, "array dimension or *", dimension));
                };
                if *dimension < 0 {
                    return Err(invalid_type_spec(
                        function,
                        "array dimensions must be non-negative",
                    ));
                }
            }
            Ok(())
        }
        value => Err(type_error(function, "array dimensions", value)),
    }
}

fn known_type_name(type_name: &str, environment: &Environment) -> bool {
    is_builtin_type_name(type_name)
        || environment.lookup_class(type_name).is_some()
        || environment.lookup_structure(type_name).is_some()
}

fn is_builtin_type_name(type_name: &str) -> bool {
    matches!(
        type_name,
        "T" | "OBJECT"
            | "NIL"
            | "NULL"
            | "BOOLEAN"
            | "NUMBER"
            | "REAL"
            | "RATIONAL"
            | "RATIO"
            | "INTEGER"
            | "FIXNUM"
            | "BIGNUM"
            | "BIT"
            | "FLOAT"
            | "CHARACTER"
            | "BASE-CHAR"
            | "STANDARD-CHAR"
            | "EXTENDED-CHAR"
            | "STRING"
            | "BASE-STRING"
            | "SIMPLE-STRING"
            | "SIMPLE-BASE-STRING"
            | "STREAM"
            | "SYMBOL"
            | "PACKAGE"
            | "ENVIRONMENT"
            | "KEYWORD"
            | "CONS"
            | "LIST"
            | "ATOM"
            | "VECTOR"
            | "SIMPLE-VECTOR"
            | "BIT-VECTOR"
            | "SIMPLE-BIT-VECTOR"
            | "ARRAY"
            | "SIMPLE-ARRAY"
            | "HASH-TABLE"
            | "CONDITION"
            | "RESTART"
            | "STRUCTURE"
            | "SEQUENCE"
            | "FUNCTION"
            | "COMPILED-FUNCTION"
            | "UNBOUND"
            | "VALUES"
            | "CLASS"
            | "STANDARD-OBJECT"
    )
}

fn subtype_relation(
    subtype: &Value,
    supertype: &Value,
    environment: &Environment,
) -> Result<Option<bool>, RuntimeError> {
    if same_type_designator(subtype, supertype) {
        return Ok(Some(true));
    }

    if let Some((operator, arguments)) = compound_type_parts(subtype) {
        match operator.as_str() {
            "OR" => {
                let mut unknown = false;
                for argument in arguments {
                    match subtype_relation(argument, supertype, environment)? {
                        Some(true) => {}
                        Some(false) => return Ok(Some(false)),
                        None => unknown = true,
                    }
                }
                return Ok(if unknown { None } else { Some(true) });
            }
            "AND" => {
                for argument in arguments {
                    if subtype_relation(argument, supertype, environment)? == Some(true) {
                        return Ok(Some(true));
                    }
                }
                return Ok(None);
            }
            "MEMBER" | "EQL" => {
                let candidates = if operator == "MEMBER" {
                    arguments
                } else {
                    &arguments[..1]
                };
                let mut unknown = false;
                for candidate in candidates {
                    match type_matches_designator("subtypep", candidate, supertype) {
                        Ok(true) => {}
                        Ok(false) => return Ok(Some(false)),
                        Err(_) => unknown = true,
                    }
                }
                return Ok(if unknown { None } else { Some(true) });
            }
            "INTEGER" => {
                if let Some((super_operator, super_arguments)) = compound_type_parts(supertype)
                    && super_operator == "INTEGER"
                {
                    return Ok(Some(integer_spec_is_subtype(arguments, super_arguments)?));
                }
                if let Some(super_name) = atomic_type_name(supertype) {
                    return Ok(Some(compound_subtype_named(&operator, &super_name)));
                }
            }
            "MOD" | "SIGNED-BYTE" | "UNSIGNED-BYTE" => {
                if let Some(super_name) = atomic_type_name(supertype) {
                    return Ok(Some(compound_subtype_named(&operator, &super_name)));
                }
            }
            "CONS" | "VECTOR" | "SIMPLE-VECTOR" | "BIT-VECTOR" | "SIMPLE-BIT-VECTOR" | "ARRAY"
            | "SIMPLE-ARRAY" => {
                if let Some(super_name) = atomic_type_name(supertype) {
                    return Ok(Some(compound_subtype_named(&operator, &super_name)));
                }
            }
            _ => {}
        }
    }

    if let Some((operator, arguments)) = compound_type_parts(supertype) {
        match operator.as_str() {
            "OR" => {
                let mut unknown = false;
                for argument in arguments {
                    match subtype_relation(subtype, argument, environment)? {
                        Some(true) => return Ok(Some(true)),
                        Some(false) => {}
                        None => unknown = true,
                    }
                }
                return Ok(if unknown { None } else { Some(false) });
            }
            "AND" => {
                let mut unknown = false;
                for argument in arguments {
                    match subtype_relation(subtype, argument, environment)? {
                        Some(false) => return Ok(Some(false)),
                        Some(true) => {}
                        None => unknown = true,
                    }
                }
                return Ok(if unknown { None } else { Some(true) });
            }
            "INTEGER" => {
                if let Some(subtype_name) = atomic_type_name(subtype) {
                    return Ok(Some(named_integer_is_subtype(&subtype_name, arguments)?));
                }
            }
            _ => {}
        }
    }

    let Some(subtype_name) = atomic_type_name(subtype) else {
        return Ok(None);
    };
    let Some(supertype_name) = atomic_type_name(supertype) else {
        return Ok(None);
    };
    Ok(named_subtype_relation(
        &subtype_name,
        &supertype_name,
        environment,
    ))
}

fn compound_type_parts(value: &Value) -> Option<(String, &[Value])> {
    let Value::List(items) = value else {
        return None;
    };
    let operator = type_designator_name("subtypep", items.first()?).ok()?;
    Some((operator, &items[1..]))
}

fn atomic_type_name(value: &Value) -> Option<String> {
    if matches!(value, Value::List(_) | Value::DottedList { .. }) {
        None
    } else {
        type_designator_name("subtypep", value).ok()
    }
}

fn same_type_designator(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::List(left), Value::List(right)) => {
            if left.len() != right.len() {
                return false;
            }
            let Some(left_operator) = left
                .first()
                .and_then(|value| type_designator_name("subtypep", value).ok())
            else {
                return false;
            };
            let Some(right_operator) = right
                .first()
                .and_then(|value| type_designator_name("subtypep", value).ok())
            else {
                return false;
            };
            if left_operator != right_operator {
                return false;
            }
            left.iter()
                .zip(right.iter())
                .enumerate()
                .all(|(index, (left, right))| {
                    if index == 0 {
                        true
                    } else if matches!(left_operator.as_str(), "MEMBER" | "EQL") {
                        eql_value(left, right)
                    } else {
                        same_type_designator(left, right)
                    }
                })
        }
        (Value::DottedList { .. }, Value::DottedList { .. }) => false,
        (Value::List(_), _) | (_, Value::List(_)) => false,
        (Value::DottedList { .. }, _) | (_, Value::DottedList { .. }) => false,
        _ => match (
            type_designator_name("subtypep", left).ok(),
            type_designator_name("subtypep", right).ok(),
        ) {
            (Some(left), Some(right)) => left == right,
            (None, None) => eql_value(left, right),
            _ => false,
        },
    }
}

fn integer_spec_is_subtype(
    subtype_arguments: &[Value],
    supertype_arguments: &[Value],
) -> Result<bool, RuntimeError> {
    let subtype_lower = subtype_arguments
        .first()
        .map(|bound| integer_type_bound("subtypep", bound))
        .transpose()?
        .flatten();
    let subtype_upper = subtype_arguments
        .get(1)
        .map(|bound| integer_type_bound("subtypep", bound))
        .transpose()?
        .flatten();
    let supertype_lower = supertype_arguments
        .first()
        .map(|bound| integer_type_bound("subtypep", bound))
        .transpose()?
        .flatten();
    let supertype_upper = supertype_arguments
        .get(1)
        .map(|bound| integer_type_bound("subtypep", bound))
        .transpose()?
        .flatten();

    let subtype_empty = subtype_lower
        .zip(subtype_upper)
        .is_some_and(|(lower, upper)| lower > upper);
    let supertype_empty = supertype_lower
        .zip(supertype_upper)
        .is_some_and(|(lower, upper)| lower > upper);
    if subtype_empty {
        return Ok(true);
    }
    if supertype_empty {
        return Ok(false);
    }

    let lower_ok = match (subtype_lower, supertype_lower) {
        (_, None) => true,
        (Some(subtype), Some(supertype)) => subtype >= supertype,
        (None, Some(_)) => false,
    };
    let upper_ok = match (subtype_upper, supertype_upper) {
        (_, None) => true,
        (Some(subtype), Some(supertype)) => subtype <= supertype,
        (None, Some(_)) => false,
    };
    Ok(lower_ok && upper_ok)
}

fn named_integer_is_subtype(
    subtype_name: &str,
    supertype_arguments: &[Value],
) -> Result<bool, RuntimeError> {
    if subtype_name == "BIT" {
        return integer_spec_is_subtype(
            &[Value::Integer(0), Value::Integer(1)],
            supertype_arguments,
        );
    }
    if matches!(subtype_name, "INTEGER" | "FIXNUM" | "BIGNUM") {
        let lower = supertype_arguments
            .first()
            .map(|bound| integer_type_bound("subtypep", bound))
            .transpose()?
            .flatten();
        let upper = supertype_arguments
            .get(1)
            .map(|bound| integer_type_bound("subtypep", bound))
            .transpose()?
            .flatten();
        return Ok(lower.is_none() && upper.is_none());
    }
    Ok(false)
}

fn compound_subtype_named(operator: &str, supertype_name: &str) -> bool {
    match operator {
        "INTEGER" => matches!(
            supertype_name,
            "INTEGER" | "RATIONAL" | "NUMBER" | "REAL" | "ATOM"
        ),
        "MOD" | "SIGNED-BYTE" | "UNSIGNED-BYTE" => matches!(
            supertype_name,
            "INTEGER" | "RATIONAL" | "NUMBER" | "REAL" | "ATOM"
        ),
        "CONS" => matches!(supertype_name, "CONS" | "LIST" | "SEQUENCE"),
        "VECTOR" | "SIMPLE-VECTOR" => matches!(
            supertype_name,
            "VECTOR" | "SIMPLE-VECTOR" | "ARRAY" | "SIMPLE-ARRAY" | "SEQUENCE" | "ATOM"
        ),
        "BIT-VECTOR" | "SIMPLE-BIT-VECTOR" => matches!(
            supertype_name,
            "BIT-VECTOR"
                | "SIMPLE-BIT-VECTOR"
                | "VECTOR"
                | "SIMPLE-VECTOR"
                | "ARRAY"
                | "SIMPLE-ARRAY"
                | "SEQUENCE"
                | "ATOM"
        ),
        "ARRAY" | "SIMPLE-ARRAY" => {
            matches!(supertype_name, "ARRAY" | "SIMPLE-ARRAY" | "ATOM")
        }
        _ => false,
    }
}

fn named_subtype_relation(
    subtype_name: &str,
    supertype_name: &str,
    environment: &Environment,
) -> Option<bool> {
    if subtype_name == supertype_name
        || matches!(supertype_name, "T" | "OBJECT")
        || builtin_subtype(subtype_name, supertype_name)
    {
        return Some(true);
    }

    if let Some(class) = environment.lookup_class(subtype_name)
        && class
            .precedence
            .iter()
            .any(|name| name.eq_ignore_ascii_case(supertype_name))
    {
        return Some(true);
    }
    if let Some(structure) = environment.lookup_structure(subtype_name)
        && (supertype_name == "STRUCTURE"
            || structure
                .type_names
                .iter()
                .any(|name| name.eq_ignore_ascii_case(supertype_name)))
    {
        return Some(true);
    }

    if known_type_name(subtype_name, environment) && known_type_name(supertype_name, environment) {
        Some(false)
    } else {
        None
    }
}

fn builtin_subtype(subtype_name: &str, supertype_name: &str) -> bool {
    match subtype_name {
        "NIL" | "NULL" => matches!(
            supertype_name,
            "SYMBOL" | "LIST" | "SEQUENCE" | "ATOM" | "BOOLEAN" | "NIL" | "NULL"
        ),
        "BOOLEAN" => matches!(supertype_name, "SYMBOL" | "ATOM"),
        "NUMBER" => matches!(supertype_name, "REAL" | "ATOM"),
        "REAL" => matches!(supertype_name, "NUMBER" | "ATOM"),
        "FIXNUM" | "BIGNUM" | "BIT" => matches!(
            supertype_name,
            "INTEGER" | "RATIONAL" | "NUMBER" | "REAL" | "ATOM"
        ),
        "INTEGER" => matches!(supertype_name, "RATIONAL" | "NUMBER" | "REAL" | "ATOM"),
        "RATIO" => matches!(supertype_name, "RATIONAL" | "NUMBER" | "REAL" | "ATOM"),
        "RATIONAL" => matches!(supertype_name, "NUMBER" | "REAL" | "ATOM"),
        "FLOAT" => matches!(supertype_name, "NUMBER" | "REAL" | "ATOM"),
        "BASE-CHAR" => matches!(supertype_name, "CHARACTER" | "ATOM"),
        "STANDARD-CHAR" => matches!(supertype_name, "BASE-CHAR" | "CHARACTER" | "ATOM"),
        "EXTENDED-CHAR" => matches!(supertype_name, "CHARACTER" | "ATOM"),
        "CHARACTER" => supertype_name == "ATOM",
        "STRING" | "BASE-STRING" => {
            matches!(
                supertype_name,
                "STRING" | "BASE-STRING" | "SEQUENCE" | "ATOM"
            )
        }
        "SIMPLE-STRING" | "SIMPLE-BASE-STRING" => matches!(
            supertype_name,
            "STRING" | "BASE-STRING" | "SIMPLE-STRING" | "SIMPLE-BASE-STRING" | "SEQUENCE" | "ATOM"
        ),
        "SYMBOL" => supertype_name == "ATOM",
        "KEYWORD" => matches!(supertype_name, "SYMBOL" | "ATOM"),
        "CONS" => matches!(supertype_name, "LIST" | "SEQUENCE"),
        "LIST" => supertype_name == "SEQUENCE",
        "VECTOR" | "SIMPLE-VECTOR" => matches!(
            supertype_name,
            "VECTOR" | "SIMPLE-VECTOR" | "ARRAY" | "SIMPLE-ARRAY" | "SEQUENCE" | "ATOM"
        ),
        "BIT-VECTOR" | "SIMPLE-BIT-VECTOR" => matches!(
            supertype_name,
            "BIT-VECTOR"
                | "SIMPLE-BIT-VECTOR"
                | "VECTOR"
                | "SIMPLE-VECTOR"
                | "ARRAY"
                | "SIMPLE-ARRAY"
                | "SEQUENCE"
                | "ATOM"
        ),
        "ARRAY" | "SIMPLE-ARRAY" => {
            matches!(supertype_name, "ARRAY" | "SIMPLE-ARRAY" | "ATOM")
        }
        "COMPILED-FUNCTION" => matches!(supertype_name, "FUNCTION" | "ATOM"),
        "FUNCTION" | "STREAM" | "PACKAGE" | "ENVIRONMENT" | "HASH-TABLE" | "CONDITION"
        | "RESTART" | "STRUCTURE" | "UNBOUND" | "VALUES" | "CLASS" | "STANDARD-OBJECT" => {
            supertype_name == "ATOM"
        }
        _ => false,
    }
}

pub(crate) fn the_check(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "the", 2)?;
    let type_description = arguments[1].to_string();
    if type_matches_designator("the", &arguments[0], &arguments[1])? {
        Ok(arguments[0].clone())
    } else {
        Err(RuntimeError::Type {
            expected: format!("the requires value of type {type_description}"),
            actual: arguments[0].type_name().to_string(),
            span: None,
        })
    }
}

pub(crate) fn ecase_error(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "__NCL_ECASE_ERROR", 0)?;
    Err(RuntimeError::InvalidForm {
        message: "ecase fell through".to_string(),
        span: None,
    })
}

pub(crate) fn etypecase_error(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "__NCL_ETYPECASE_ERROR", 0)?;
    Err(RuntimeError::InvalidForm {
        message: "etypecase fell through".to_string(),
        span: None,
    })
}

pub(crate) fn type_designator_name(function: &str, value: &Value) -> Result<String, RuntimeError> {
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
) -> Result<bool, RuntimeError> {
    match type_designator {
        Value::List(items) => type_matches_compound(function, value, items.as_ref()),
        Value::DottedList { .. } => Err(invalid_type_spec(
            function,
            "type designator must be a proper list",
        )),
        _ => {
            let type_name = type_designator_name(function, type_designator)?;
            type_matches(value, &type_name)
        }
    }
}

fn type_matches_compound(
    function: &str,
    value: &Value,
    items: &[Value],
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
                if type_matches_designator(function, value, type_designator)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        "AND" => {
            for type_designator in arguments {
                if !type_matches_designator(function, value, type_designator)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        "NOT" => {
            require_type_spec_arity(function, &operator, arguments, 1, 1)?;
            Ok(!type_matches_designator(function, value, &arguments[0])?)
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
        "CONS" => cons_type_matches(function, value, arguments),
        "VECTOR" => vector_type_matches(function, value, arguments),
        "SIMPLE-VECTOR" => simple_vector_type_matches(function, value, arguments),
        "BIT-VECTOR" | "SIMPLE-BIT-VECTOR" => bit_vector_type_matches(function, value, arguments),
        "ARRAY" | "SIMPLE-ARRAY" => array_type_matches(function, &operator, value, arguments),
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
) -> Result<bool, RuntimeError> {
    require_type_spec_arity(function, "CONS", arguments, 0, 2)?;
    let Some((car, cdr)) = cons_parts(value) else {
        return Ok(false);
    };
    if let Some(car_type) = arguments.first()
        && !type_matches_designator(function, &car, car_type)?
    {
        return Ok(false);
    }
    if let Some(cdr_type) = arguments.get(1)
        && !type_matches_designator(function, &cdr, cdr_type)?
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
            if !type_matches_element_spec(function, item, element_type)? {
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
    matches!(value, Value::Vector(items) if items.iter().all(is_bit_value))
}

fn is_bit_value(value: &Value) -> bool {
    matches!(value, Value::Integer(bit) if *bit == 0 || *bit == 1)
}

fn array_type_matches(
    function: &str,
    operator: &str,
    value: &Value,
    arguments: &[Value],
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
            if !type_matches_element_spec(function, element, element_type)? {
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
) -> Result<bool, RuntimeError> {
    if is_type_wildcard(type_designator) {
        Ok(true)
    } else {
        type_matches_designator(function, value, type_designator)
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

fn type_matches(value: &Value, type_name: &str) -> Result<bool, RuntimeError> {
    let result = match type_name {
        "T" | "OBJECT" => true,
        "NIL" | "NULL" => matches!(value, Value::Nil | Value::Boolean(false)),
        "BOOLEAN" => matches!(value, Value::Nil | Value::Boolean(_)),
        "NUMBER" | "REAL" => matches!(
            value,
            Value::Integer(_) | Value::Rational(_) | Value::Float(_)
        ),
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
        "VECTOR" | "SIMPLE-VECTOR" => matches!(value, Value::Vector(_)),
        "BIT-VECTOR" | "SIMPLE-BIT-VECTOR" => is_bit_vector_value(value),
        "ARRAY" | "SIMPLE-ARRAY" => dimensions_for_array(value).is_some(),
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
        "UNBOUND" => matches!(value, Value::Unbound),
        "VALUES" => matches!(value, Value::Values(_)),
        "CLASS" => matches!(value, Value::Class(_)),
        "STANDARD-OBJECT" => matches!(value, Value::Instance(_)),
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
