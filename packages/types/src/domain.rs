//! Object-independent parser for type specifier forms.

use crate::{
    ArrayDimension, ArrayDimensions, IntegerBound, NamedType, TypeError, TypeSpecifier, Value,
};

/// A runtime-independent type specifier form.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum TypeForm {
    /// An upper-case symbol name.
    Symbol(String),
    /// A proper list of forms.
    List(Vec<Self>),
    /// A non-symbol object value.
    Value(Value),
}

/// Parse a runtime-independent form into a type specifier.
pub(crate) fn parse_type_form(form: TypeForm) -> Result<TypeSpecifier, TypeError> {
    match form {
        TypeForm::Symbol(name) => Ok(named_or_deftype(name)),
        TypeForm::List(items) => parse_list(&items),
        TypeForm::Value(Value::Nil) => Ok(TypeSpecifier::Named(NamedType::Nil)),
        TypeForm::Value(Value::True) => Ok(TypeSpecifier::Named(NamedType::T)),
        TypeForm::Value(_) => Err(TypeError::InvalidForm),
    }
}

fn named_or_deftype(name: String) -> TypeSpecifier {
    NamedType::from_name(&name).map_or_else(
        || TypeSpecifier::Deftype {
            name,
            args: Vec::new(),
        },
        TypeSpecifier::Named,
    )
}

fn parse_list(items: &[TypeForm]) -> Result<TypeSpecifier, TypeError> {
    let Some(TypeForm::Symbol(operator)) = items.first() else {
        return Err(TypeError::InvalidForm);
    };
    let (_, tail) = items.split_first().ok_or(TypeError::InvalidForm)?;
    match operator.as_str() {
        "INTEGER" => parse_integer_range(tail),
        "OR" => parse_many(tail, TypeSpecifier::Or),
        "AND" => parse_many(tail, TypeSpecifier::And),
        "VALUES" => parse_many(tail, TypeSpecifier::Values),
        "NOT" => Ok(TypeSpecifier::Not(Box::new(parse_single_type(tail)?))),
        "MEMBER" => Ok(TypeSpecifier::Member(parse_values(tail)?)),
        "EQL" => Ok(TypeSpecifier::Eql(parse_single_value(tail)?)),
        "SATISFIES" => parse_satisfies(tail),
        "CONS" => parse_cons(tail),
        "ARRAY" => parse_array(tail, false),
        "SIMPLE-ARRAY" => parse_array(tail, true),
        "VECTOR" => parse_vector(tail),
        "FUNCTION" => parse_function(tail),
        _ => Ok(TypeSpecifier::Deftype {
            name: operator.clone(),
            args: parse_values(tail)?,
        }),
    }
}

fn parse_integer_range(items: &[TypeForm]) -> Result<TypeSpecifier, TypeError> {
    let (low, rest) = parse_bound(items)?;
    let (high, rest) = parse_bound(rest)?;
    expect_empty(rest)?;
    Ok(TypeSpecifier::IntegerRange { low, high })
}

fn parse_bound(items: &[TypeForm]) -> Result<(IntegerBound, &[TypeForm]), TypeError> {
    let Some((first, rest)) = items.split_first() else {
        return Ok((IntegerBound::Unbounded, items));
    };
    let bound = if is_star(first) {
        IntegerBound::Unbounded
    } else if let Some(value) = as_integer(first) {
        IntegerBound::Inclusive(value)
    } else if let TypeForm::List(values) = first {
        IntegerBound::Exclusive(as_single_integer(values)?)
    } else {
        return Err(TypeError::InvalidForm);
    };
    Ok((bound, rest))
}

fn parse_many(
    items: &[TypeForm],
    wrap: fn(Vec<TypeSpecifier>) -> TypeSpecifier,
) -> Result<TypeSpecifier, TypeError> {
    items
        .iter()
        .map(|item| parse_type_form(item.clone()))
        .collect::<Result<Vec<_>, _>>()
        .map(wrap)
}

fn parse_single_type(items: &[TypeForm]) -> Result<TypeSpecifier, TypeError> {
    parse_type_form(single(items)?.clone())
}
fn parse_values(items: &[TypeForm]) -> Result<Vec<Value>, TypeError> {
    items.iter().map(as_value).collect()
}
fn parse_single_value(items: &[TypeForm]) -> Result<Value, TypeError> {
    as_value(single(items)?)
}

fn parse_satisfies(items: &[TypeForm]) -> Result<TypeSpecifier, TypeError> {
    let TypeForm::Symbol(name) = single(items)? else {
        return Err(TypeError::InvalidForm);
    };
    Ok(TypeSpecifier::Satisfies(name.clone()))
}

fn parse_cons(items: &[TypeForm]) -> Result<TypeSpecifier, TypeError> {
    let (rest, car) = parse_optional(items)?;
    let (rest, cdr) = parse_optional(rest)?;
    expect_empty(rest)?;
    Ok(TypeSpecifier::Cons {
        car: Box::new(car),
        cdr: Box::new(cdr),
    })
}

fn parse_array(items: &[TypeForm], simple: bool) -> Result<TypeSpecifier, TypeError> {
    let (rest, element_type) = parse_optional_element(items)?;
    let dimensions = parse_dimensions(rest)?;
    Ok(TypeSpecifier::Array {
        element_type,
        dimensions,
        simple,
    })
}

fn parse_vector(items: &[TypeForm]) -> Result<TypeSpecifier, TypeError> {
    let (rest, element_type) = parse_optional_element(items)?;
    let size = if rest.is_empty() {
        None
    } else {
        Some(parse_array_dimension(single(rest)?)?)
    };
    Ok(TypeSpecifier::Vector { element_type, size })
}

fn parse_function(items: &[TypeForm]) -> Result<TypeSpecifier, TypeError> {
    let (lambda_list, rest) = match items.split_first() {
        None => (Vec::new(), items),
        Some((first, rest)) if is_star(first) => (Vec::new(), rest),
        Some((TypeForm::List(list), rest)) => (
            list.iter()
                .map(|item| parse_type_form(item.clone()))
                .collect::<Result<_, _>>()?,
            rest,
        ),
        Some(_) => {
            return Err(TypeError::InvalidForm);
        }
    };
    let (rest, return_type) = parse_optional(rest)?;
    expect_empty(rest)?;
    Ok(TypeSpecifier::Function {
        lambda_list,
        return_type: Box::new(return_type),
    })
}

fn parse_optional(items: &[TypeForm]) -> Result<(&[TypeForm], TypeSpecifier), TypeError> {
    items.split_first().map_or_else(
        || Ok((items, TypeSpecifier::Named(NamedType::T))),
        |(first, rest)| Ok((rest, parse_type_form(first.clone())?)),
    )
}

fn parse_optional_element(
    items: &[TypeForm],
) -> Result<(&[TypeForm], Option<Box<TypeSpecifier>>), TypeError> {
    items.split_first().map_or_else(
        || Ok((items, None)),
        |(first, rest)| Ok((rest, Some(Box::new(parse_type_form(first.clone())?)))),
    )
}

fn parse_dimensions(items: &[TypeForm]) -> Result<Option<ArrayDimensions>, TypeError> {
    if items.is_empty() {
        return Ok(None);
    }
    let first = single(items)?;
    if is_star(first) {
        return Ok(Some(ArrayDimensions::Wild));
    }
    if let Some(rank) = as_integer(first) {
        return Ok(Some(ArrayDimensions::Rank(
            usize::try_from(rank).map_err(|_| TypeError::InvalidForm)?,
        )));
    }
    let TypeForm::List(values) = first else {
        return Err(TypeError::InvalidForm);
    };
    Ok(Some(ArrayDimensions::Ranks(
        values
            .iter()
            .map(parse_array_dimension)
            .collect::<Result<_, _>>()?,
    )))
}

fn parse_array_dimension(form: &TypeForm) -> Result<ArrayDimension, TypeError> {
    if is_star(form) {
        return Ok(ArrayDimension::Any);
    }
    let value = as_integer(form).ok_or(TypeError::InvalidForm)?;
    Ok(ArrayDimension::Exact(
        usize::try_from(value).map_err(|_| TypeError::InvalidForm)?,
    ))
}
fn as_single_integer(items: &[TypeForm]) -> Result<i64, TypeError> {
    as_integer(single(items)?).ok_or(TypeError::InvalidForm)
}
fn as_integer(form: &TypeForm) -> Option<i64> {
    match form {
        TypeForm::Value(Value::Integer(value)) => Some(*value),
        TypeForm::Symbol(_) | TypeForm::List(_) | TypeForm::Value(_) => None,
    }
}
fn as_value(form: &TypeForm) -> Result<Value, TypeError> {
    match form {
        TypeForm::Value(value) => Ok(value.clone()),
        TypeForm::Symbol(_) | TypeForm::List(_) => Err(TypeError::InvalidForm),
    }
}
fn is_star(form: &TypeForm) -> bool {
    matches!(form, TypeForm::Symbol(name) if name == "*")
}
fn single(items: &[TypeForm]) -> Result<&TypeForm, TypeError> {
    items
        .first()
        .filter(|_| items.len() == 1)
        .ok_or(TypeError::InvalidForm)
}
fn expect_empty(items: &[TypeForm]) -> Result<(), TypeError> {
    if items.is_empty() {
        Ok(())
    } else {
        Err(TypeError::InvalidForm)
    }
}

#[cfg(test)]
mod tests {
    use super::{TypeForm, parse_type_form};
    use crate::{IntegerBound, TypeError, TypeSpecifier, Value};

    #[test]
    fn parses_integer_range() {
        let form = TypeForm::List(vec![
            TypeForm::Symbol("INTEGER".into()),
            TypeForm::Value(Value::Integer(1)),
            TypeForm::Symbol("*".into()),
        ]);
        assert_eq!(
            parse_type_form(form),
            Ok(TypeSpecifier::IntegerRange {
                low: IntegerBound::Inclusive(1),
                high: IntegerBound::Unbounded
            })
        );
    }

    #[test]
    fn rejects_non_type_value() {
        assert_eq!(
            parse_type_form(TypeForm::Value(Value::Integer(1))),
            Err(TypeError::InvalidForm)
        );
    }
}
