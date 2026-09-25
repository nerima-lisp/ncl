//! Parsing of type specifier forms.

use ncl_object::{ObjectRef, ThreadContext, Word, car, cdr, classify_object, symbol_name};

use crate::adapter::from_word;
use crate::text::string_to_upper;
use crate::{ArrayDimension, ArrayDimensions, IntegerBound, NamedType, TypeError, TypeSpecifier};

/// Parse a type specifier form into a [`TypeSpecifier`].
///
/// A symbol resolves to a built-in [`NamedType`] or, when unknown, to a
/// [`TypeSpecifier::Deftype`] awaiting expansion. A proper list is parsed by
/// its leading operator symbol. Any other form is rejected.
///
/// # Errors
///
/// Returns [`TypeError::InvalidSpecifier`] when the form is neither a symbol
/// nor a proper list, and propagates object-layer errors as
/// [`TypeError::Object`].
pub fn parse_type_specifier(
    ctx: &mut ThreadContext,
    spec: Word,
) -> Result<TypeSpecifier, TypeError> {
    if spec == Word::NIL {
        return Ok(TypeSpecifier::Named(NamedType::Nil));
    }
    if spec == Word::TRUE {
        return Ok(TypeSpecifier::Named(NamedType::T));
    }
    if spec.is_list() {
        return parse_list(ctx, spec);
    }
    if matches!(classify_object(ctx, spec), ObjectRef::Symbol(_)) {
        return parse_atom(ctx, spec);
    }
    Err(TypeError::InvalidSpecifier(spec))
}

fn parse_atom(ctx: &ThreadContext, spec: Word) -> Result<TypeSpecifier, TypeError> {
    let name = symbol_name(ctx, spec)?;
    let text = string_to_upper(ctx, name)?;
    NamedType::from_name(&text).map_or_else(
        || {
            Ok(TypeSpecifier::Deftype {
                name: text,
                args: Vec::new(),
            })
        },
        |named| Ok(TypeSpecifier::Named(named)),
    )
}

fn parse_list(ctx: &mut ThreadContext, spec: Word) -> Result<TypeSpecifier, TypeError> {
    let head = car(ctx, spec)?;
    let tail = cdr(ctx, spec)?;
    match operator_name(ctx, head)?.as_str() {
        "INTEGER" => parse_integer_range(ctx, tail),
        "OR" => parse_many(ctx, tail, TypeSpecifier::Or),
        "AND" => parse_many(ctx, tail, TypeSpecifier::And),
        "VALUES" => parse_many(ctx, tail, TypeSpecifier::Values),
        "NOT" => parse_not(ctx, tail),
        "MEMBER" => parse_member(ctx, tail),
        "EQL" => parse_eql(ctx, tail),
        "SATISFIES" => parse_satisfies(ctx, tail),
        "CONS" => parse_cons(ctx, tail),
        "ARRAY" => parse_array(ctx, tail, false),
        "SIMPLE-ARRAY" => parse_array(ctx, tail, true),
        "VECTOR" => parse_vector(ctx, tail),
        "FUNCTION" => parse_function(ctx, tail),
        _ => Ok(TypeSpecifier::Deftype {
            name: operator_name(ctx, head)?,
            args: list_to_words(ctx, tail)?
                .into_iter()
                .map(|word| from_word(ctx, word))
                .collect::<Result<_, _>>()?,
        }),
    }
}

fn operator_name(ctx: &ThreadContext, word: Word) -> Result<String, TypeError> {
    if word == Word::NIL || !matches!(classify_object(ctx, word), ObjectRef::Symbol(_)) {
        return Err(TypeError::InvalidSpecifier(word));
    }
    string_to_upper(ctx, symbol_name(ctx, word)?)
}

fn parse_integer_range(ctx: &mut ThreadContext, tail: Word) -> Result<TypeSpecifier, TypeError> {
    let (low, rest) = parse_bound(ctx, tail)?;
    let (high, rest) = parse_bound(ctx, rest)?;
    expect_end(rest)?;
    Ok(TypeSpecifier::IntegerRange { low, high })
}

/// Reject a tail that still holds elements when the form takes no more.
fn expect_end(list: Word) -> Result<(), TypeError> {
    if list == Word::NIL {
        Ok(())
    } else {
        Err(TypeError::InvalidSpecifier(list))
    }
}

fn parse_bound(ctx: &mut ThreadContext, list: Word) -> Result<(IntegerBound, Word), TypeError> {
    if list == Word::NIL {
        return Ok((IntegerBound::Unbounded, Word::NIL));
    }
    let first = car(ctx, list)?;
    let rest = cdr(ctx, list)?;
    let bound = if is_star(ctx, first)? {
        IntegerBound::Unbounded
    } else if let Some(value) = first.as_fixnum() {
        IntegerBound::Inclusive(value)
    } else if first.is_list() {
        IntegerBound::Exclusive(
            single(ctx, first)?
                .as_fixnum()
                .ok_or(TypeError::InvalidSpecifier(first))?,
        )
    } else {
        return Err(TypeError::InvalidSpecifier(first));
    };
    Ok((bound, rest))
}

fn parse_many(
    ctx: &mut ThreadContext,
    tail: Word,
    wrap: fn(Vec<TypeSpecifier>) -> TypeSpecifier,
) -> Result<TypeSpecifier, TypeError> {
    let mut specs = Vec::new();
    let mut rest = tail;
    while rest != Word::NIL {
        let item = car(ctx, rest)?;
        specs.push(parse_type_specifier(ctx, item)?);
        rest = cdr(ctx, rest)?;
    }
    Ok(wrap(specs))
}

fn parse_not(ctx: &mut ThreadContext, tail: Word) -> Result<TypeSpecifier, TypeError> {
    let item = single(ctx, tail)?;
    let inner = parse_type_specifier(ctx, item)?;
    Ok(TypeSpecifier::Not(Box::new(inner)))
}

fn parse_member(ctx: &mut ThreadContext, tail: Word) -> Result<TypeSpecifier, TypeError> {
    Ok(TypeSpecifier::Member(
        list_to_words(ctx, tail)?
            .into_iter()
            .map(|word| from_word(ctx, word))
            .collect::<Result<_, _>>()?,
    ))
}

fn parse_eql(ctx: &mut ThreadContext, tail: Word) -> Result<TypeSpecifier, TypeError> {
    let item = single(ctx, tail)?;
    Ok(TypeSpecifier::Eql(from_word(ctx, item)?))
}

fn parse_satisfies(ctx: &mut ThreadContext, tail: Word) -> Result<TypeSpecifier, TypeError> {
    let predicate = single(ctx, tail)?;
    Ok(TypeSpecifier::Satisfies(string_to_upper(
        ctx,
        symbol_name(ctx, predicate)?,
    )?))
}

#[allow(clippy::similar_names)]
fn parse_cons(ctx: &mut ThreadContext, tail: Word) -> Result<TypeSpecifier, TypeError> {
    let (rest, car_type) = parse_optional(ctx, tail)?;
    let (rest, cdr_type) = parse_optional(ctx, rest)?;
    expect_end(rest)?;
    Ok(TypeSpecifier::Cons {
        car: Box::new(car_type),
        cdr: Box::new(cdr_type),
    })
}

fn parse_array(
    ctx: &mut ThreadContext,
    tail: Word,
    simple: bool,
) -> Result<TypeSpecifier, TypeError> {
    let (rest, element_type) = parse_optional_element(ctx, tail)?;
    let dimensions = parse_dimensions(ctx, rest)?;
    Ok(TypeSpecifier::Array {
        element_type,
        dimensions,
        simple,
    })
}

fn parse_vector(ctx: &mut ThreadContext, tail: Word) -> Result<TypeSpecifier, TypeError> {
    let (rest, element_type) = parse_optional_element(ctx, tail)?;
    let size = if rest == Word::NIL {
        None
    } else {
        Some({
            let item = single(ctx, rest)?;
            parse_array_dimension(ctx, item)?
        })
    };
    Ok(TypeSpecifier::Vector { element_type, size })
}

fn parse_function(ctx: &mut ThreadContext, tail: Word) -> Result<TypeSpecifier, TypeError> {
    let (lambda_list, rest) = parse_lambda_list(ctx, tail)?;
    let (rest, return_type) = parse_optional(ctx, rest)?;
    expect_end(rest)?;
    Ok(TypeSpecifier::Function {
        lambda_list,
        return_type: Box::new(return_type),
    })
}

fn parse_lambda_list(
    ctx: &mut ThreadContext,
    tail: Word,
) -> Result<(Vec<TypeSpecifier>, Word), TypeError> {
    if tail == Word::NIL {
        return Ok((Vec::new(), Word::NIL));
    }
    let first = car(ctx, tail)?;
    let rest = cdr(ctx, tail)?;
    if is_star(ctx, first)? {
        return Ok((Vec::new(), rest));
    }
    if !first.is_list() {
        return Err(TypeError::InvalidSpecifier(first));
    }
    let mut specs = Vec::new();
    let mut cursor = first;
    while cursor != Word::NIL {
        let item = car(ctx, cursor)?;
        specs.push(parse_type_specifier(ctx, item)?);
        cursor = cdr(ctx, cursor)?;
    }
    Ok((specs, rest))
}

fn parse_optional(ctx: &mut ThreadContext, list: Word) -> Result<(Word, TypeSpecifier), TypeError> {
    if list == Word::NIL {
        return Ok((Word::NIL, TypeSpecifier::Named(NamedType::T)));
    }
    let first = car(ctx, list)?;
    let rest = cdr(ctx, list)?;
    Ok((rest, parse_type_specifier(ctx, first)?))
}

fn parse_optional_element(
    ctx: &mut ThreadContext,
    list: Word,
) -> Result<(Word, Option<Box<TypeSpecifier>>), TypeError> {
    if list == Word::NIL {
        return Ok((Word::NIL, None));
    }
    let first = car(ctx, list)?;
    let rest = cdr(ctx, list)?;
    Ok((rest, Some(Box::new(parse_type_specifier(ctx, first)?))))
}

fn parse_dimensions(
    ctx: &mut ThreadContext,
    list: Word,
) -> Result<Option<ArrayDimensions>, TypeError> {
    if list == Word::NIL {
        return Ok(None);
    }
    let first = car(ctx, list)?;
    expect_end(cdr(ctx, list)?)?;
    if is_star(ctx, first)? {
        return Ok(Some(ArrayDimensions::Wild));
    }
    if let Some(rank) = first.as_fixnum() {
        let rank = usize::try_from(rank).map_err(|_| TypeError::InvalidSpecifier(first))?;
        return Ok(Some(ArrayDimensions::Rank(rank)));
    }
    if first.is_list() {
        let mut ranks = Vec::new();
        let mut cursor = first;
        while cursor != Word::NIL {
            let item = car(ctx, cursor)?;
            ranks.push(parse_array_dimension(ctx, item)?);
            cursor = cdr(ctx, cursor)?;
        }
        return Ok(Some(ArrayDimensions::Ranks(ranks)));
    }
    Err(TypeError::InvalidSpecifier(first))
}

fn parse_array_dimension(ctx: &ThreadContext, word: Word) -> Result<ArrayDimension, TypeError> {
    if is_star(ctx, word)? {
        return Ok(ArrayDimension::Any);
    }
    let value = word.as_fixnum().ok_or(TypeError::InvalidSpecifier(word))?;
    let value = usize::try_from(value).map_err(|_| TypeError::InvalidSpecifier(word))?;
    Ok(ArrayDimension::Exact(value))
}

fn single(ctx: &mut ThreadContext, tail: Word) -> Result<Word, TypeError> {
    if tail == Word::NIL {
        return Err(TypeError::InvalidSpecifier(Word::NIL));
    }
    let item = car(ctx, tail)?;
    expect_end(cdr(ctx, tail)?)?;
    Ok(item)
}

fn list_to_words(ctx: &mut ThreadContext, list: Word) -> Result<Vec<Word>, TypeError> {
    let mut words = Vec::new();
    let mut rest = list;
    while rest != Word::NIL {
        words.push(car(ctx, rest)?);
        rest = cdr(ctx, rest)?;
    }
    Ok(words)
}

fn is_star(ctx: &ThreadContext, word: Word) -> Result<bool, TypeError> {
    if word == Word::NIL || !matches!(classify_object(ctx, word), ObjectRef::Symbol(_)) {
        return Ok(false);
    }
    Ok(string_to_upper(ctx, symbol_name(ctx, word)?)? == "*")
}
