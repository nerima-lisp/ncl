#![allow(clippy::wildcard_imports)]
use super::*;

#[path = "builtin_type_matching.rs"]
mod type_matching;
#[allow(clippy::wildcard_imports)]
use type_matching::*;

pub(super) fn ecase_error(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "__NCL_ECASE_ERROR", 0)?;
    Err(RuntimeError::InvalidForm {
        message: "ecase fell through".to_string(),
        span: None,
    })
}

pub(super) fn etypecase_error(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "__NCL_ETYPECASE_ERROR", 0)?;
    Err(RuntimeError::InvalidForm {
        message: "etypecase fell through".to_string(),
        span: None,
    })
}

pub(super) fn type_designator_name(function: &str, value: &Value) -> Result<String, RuntimeError> {
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

pub(super) fn endp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "endp", 1)?;
    match &arguments[0] {
        Value::Nil => Ok(Value::boolean(true)),
        Value::List(_) => Ok(Value::boolean(false)),
        value => Err(type_error("endp", "list", value)),
    }
}

pub(super) fn characterp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "characterp", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::Character(_))))
}

pub(super) fn keywordp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "keywordp", 1)?;
    Ok(Value::boolean(matches!(
        &arguments[0],
        Value::Keyword(_) | Value::KeywordExact(_)
    )))
}

pub(super) fn symbol_name_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "symbol-name", 1)?;
    let name = match &arguments[0] {
        Value::UninternedSymbol(name) => name.to_string(),
        value => {
            let name = value
                .symbol_name()
                .ok_or_else(|| type_error("symbol-name", "a symbol", &arguments[0]))?;
            let name = match package::split_symbol(name) {
                Some((_, symbol_name, _)) => symbol_name,
                None => name,
            };
            name.to_string()
        }
    };
    Ok(Value::string(name))
}

pub(super) fn symbol_package_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "symbol-package", 1)?;
    let package_name = match &arguments[0] {
        Value::UninternedSymbol(_) => return Ok(Value::Nil),
        Value::Keyword(_) | Value::KeywordExact(_) => KEYWORD_PACKAGE.to_string(),
        Value::Nil | Value::Boolean(_) => COMMON_LISP_PACKAGE.to_string(),
        Value::Symbol(name) | Value::SymbolExact(name) => {
            match package::split_symbol(name.as_ref()) {
                Some((package_name, _, _)) => package::normalize_package_name(package_name),
                None => package::DEFAULT_PACKAGE.to_string(),
            }
        }
        value => return Err(type_error("symbol-package", "a symbol", value)),
    };
    Ok(Value::symbol(package_name))
}

pub(super) fn vectorp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "vectorp", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::Vector(_))))
}

pub(super) fn simple_vector_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "simple-vector-p", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::Vector(_))))
}

pub(super) fn typep(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "typep", 2)?;
    Ok(Value::boolean(typep_value(&arguments[0], &arguments[1])?))
}

pub(super) fn simple_condition_format_control(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "simple-condition-format-control", 1)?;
    arguments[0]
        .simple_condition_format_control()
        .map(|control| Value::string(control.to_owned()))
        .ok_or_else(|| {
            type_error(
                "simple-condition-format-control",
                "SIMPLE-CONDITION",
                &arguments[0],
            )
        })
}

pub(super) fn simple_condition_format_arguments(
    arguments: &[Value],
) -> Result<Value, RuntimeError> {
    exact(arguments, "simple-condition-format-arguments", 1)?;
    arguments[0]
        .simple_condition_format_arguments()
        .map(Value::list)
        .ok_or_else(|| {
            type_error(
                "simple-condition-format-arguments",
                "SIMPLE-CONDITION",
                &arguments[0],
            )
        })
}

pub fn typep_value(value: &Value, type_designator: &Value) -> Result<bool, RuntimeError> {
    type_matches_designator("typep", value, type_designator)
}

pub fn subtypep_value(
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

pub(super) fn validate_subtype_designator(
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

pub(super) fn validate_element_subtype_designator(
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

pub(super) fn validate_array_dimensions_spec(
    function: &str,
    dimensions: &Value,
) -> Result<(), RuntimeError> {
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

pub(super) fn known_type_name(type_name: &str, environment: &Environment) -> bool {
    is_builtin_type_name(type_name)
        || environment.lookup_class(type_name).is_some()
        || environment.lookup_structure(type_name).is_some()
}

pub(super) fn is_builtin_type_name(type_name: &str) -> bool {
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

pub(super) fn subtype_relation(
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
            "MOD" | "SIGNED-BYTE" | "UNSIGNED-BYTE" | "CONS" | "VECTOR" | "SIMPLE-VECTOR"
            | "BIT-VECTOR" | "SIMPLE-BIT-VECTOR" | "ARRAY" | "SIMPLE-ARRAY" => {
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

    Ok(atomic_subtype_relation(subtype, supertype, environment))
}

pub(super) fn atomic_subtype_relation(
    subtype: &Value,
    supertype: &Value,
    environment: &Environment,
) -> Option<bool> {
    let subtype_name = atomic_type_name(subtype)?;
    let supertype_name = atomic_type_name(supertype)?;
    named_subtype_relation(&subtype_name, &supertype_name, environment)
}

pub(super) fn compound_type_parts(value: &Value) -> Option<(String, &[Value])> {
    let Value::List(items) = value else {
        return None;
    };
    let operator = type_designator_name("subtypep", items.first()?).ok()?;
    Some((operator, &items[1..]))
}

pub(super) fn atomic_type_name(value: &Value) -> Option<String> {
    if matches!(value, Value::List(_) | Value::DottedList { .. }) {
        None
    } else {
        type_designator_name("subtypep", value).ok()
    }
}

pub(super) fn same_type_designator(left: &Value, right: &Value) -> bool {
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
        (Value::List(_) | Value::DottedList { .. }, _)
        | (_, Value::List(_) | Value::DottedList { .. }) => false,
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

pub(super) fn integer_spec_is_subtype(
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

pub(super) fn named_integer_is_subtype(
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

pub(super) fn compound_subtype_named(operator: &str, supertype_name: &str) -> bool {
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

pub(super) fn named_subtype_relation(
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

pub(super) fn builtin_subtype(subtype_name: &str, supertype_name: &str) -> bool {
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
        "CHARACTER" | "SYMBOL" => supertype_name == "ATOM",
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

pub fn the_check(arguments: &[Value]) -> Result<Value, RuntimeError> {
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
