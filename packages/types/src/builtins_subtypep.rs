#[allow(clippy::wildcard_enum_match_arm)]
fn subtypep_with_class_graph(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    sub: &TypeSpecifier,
    sup: &TypeSpecifier,
) -> Result<(bool, bool), crate::TypeError> {
    if sub == sup {
        return Ok((true, true));
    }
    if let (Some(sub_name), Some(sup_name)) = (class_name_for_type(sub), class_name_for_type(sup))
        && let (Some(sub_class), Some(sup_class)) = (
            registered_class_descriptor(ctx, runtime, sub_name)?,
            registered_class_descriptor(ctx, runtime, sup_name)?,
        )
    {
        if class_is_subclass(ctx, sub_class, sup_class)? {
            return Ok((true, true));
        }
        if matches!(sub, TypeSpecifier::Class(_)) && matches!(sup, TypeSpecifier::Class(_)) {
            return Ok((false, true));
        }
    }
    match (sub, sup) {
        (TypeSpecifier::Or(subs), sup) => {
            let mut all_reported_subtypes = true;
            let mut definite = true;
            for sub in subs {
                let (is_sub, is_definite) = subtypep_with_class_graph(ctx, runtime, sub, sup)?;
                if !is_sub {
                    all_reported_subtypes = false;
                    if is_definite {
                        return Ok((false, true));
                    }
                }
                definite &= is_definite;
            }
            Ok((all_reported_subtypes, definite))
        }
        (sub, TypeSpecifier::Or(sups)) => {
            let mut uncertain = false;
            for sup in sups {
                let (is_sub, is_definite) = subtypep_with_class_graph(ctx, runtime, sub, sup)?;
                if is_sub {
                    if is_definite {
                        return Ok((true, true));
                    }
                    uncertain = true;
                } else if !is_definite {
                    uncertain = true;
                }
            }
            Ok((false, !uncertain))
        }
        (TypeSpecifier::And(subs), sup) => {
            let mut reported_subtype = false;
            for sub in subs {
                let (is_sub, is_definite) = subtypep_with_class_graph(ctx, runtime, sub, sup)?;
                if is_sub {
                    if is_definite {
                        return Ok((true, true));
                    }
                    reported_subtype = true;
                }
            }
            Ok((reported_subtype, false))
        }
        (sub, TypeSpecifier::And(sups)) => {
            let mut all_reported_subtypes = true;
            let mut definite = true;
            for sup in sups {
                let (is_sub, is_definite) = subtypep_with_class_graph(ctx, runtime, sub, sup)?;
                if !is_sub {
                    all_reported_subtypes = false;
                    if is_definite {
                        return Ok((false, true));
                    }
                }
                definite &= is_definite;
            }
            Ok((all_reported_subtypes, definite))
        }
        _ => subtypep(sub, sup), // check-added-lines: allow(wildcard) other forms use the general subtype relation
    }
}

#[allow(clippy::wildcard_enum_match_arm)]
fn class_name_for_type(spec: &TypeSpecifier) -> Option<&str> {
    match spec {
        TypeSpecifier::Class(name) => Some(name),
        TypeSpecifier::Named(named) => Some(match named {
            NamedType::T => "T",
            NamedType::Symbol => "SYMBOL",
            NamedType::Package => "PACKAGE",
            NamedType::Function => "FUNCTION",
            NamedType::CompiledFunction => "COMPILED-FUNCTION",
            NamedType::Cons => "CONS",
            NamedType::List => "LIST",
            NamedType::Null => "NULL",
            NamedType::Number => "NUMBER",
            NamedType::Real => "REAL",
            NamedType::Rational => "RATIONAL",
            NamedType::Integer => "INTEGER",
            NamedType::Fixnum => "FIXNUM",
            NamedType::Bignum => "BIGNUM",
            NamedType::Ratio => "RATIO",
            NamedType::Float => "FLOAT",
            NamedType::ShortFloat => "SHORT-FLOAT",
            NamedType::SingleFloat => "SINGLE-FLOAT",
            NamedType::DoubleFloat => "DOUBLE-FLOAT",
            NamedType::LongFloat => "LONG-FLOAT",
            NamedType::Complex => "COMPLEX",
            NamedType::Character => "CHARACTER",
            NamedType::BaseChar => "BASE-CHAR",
            NamedType::StandardChar => "STANDARD-CHAR",
            NamedType::ExtendedChar => "EXTENDED-CHAR",
            NamedType::String => "STRING",
            NamedType::BaseString => "BASE-STRING",
            NamedType::SimpleString => "SIMPLE-STRING",
            NamedType::SimpleBaseString => "SIMPLE-BASE-STRING",
            NamedType::BitVector => "BIT-VECTOR",
            NamedType::SimpleBitVector => "SIMPLE-BIT-VECTOR",
            NamedType::Vector => "VECTOR",
            NamedType::SimpleVector => "SIMPLE-VECTOR",
            NamedType::Array => "ARRAY",
            NamedType::SimpleArray => "SIMPLE-ARRAY",
            NamedType::Sequence => "SEQUENCE",
            NamedType::HashTable => "HASH-TABLE",
            NamedType::Stream => "STREAM",
            NamedType::RandomState => "RANDOM-STATE",
            NamedType::Restart => "RESTART",
            NamedType::Structure => "STRUCTURE-OBJECT",
            NamedType::Nil
            | NamedType::Boolean
            | NamedType::Keyword
            | NamedType::Atom
            | NamedType::ValuesType => return None,
        }),
        TypeSpecifier::Deftype { name, args } if args.is_empty() => Some(name),
        _ => None, // check-added-lines: allow(wildcard) compound type forms have no single class name
    }
}

#[allow(clippy::wildcard_enum_match_arm)]
fn registered_class_descriptor(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
) -> Result<Option<Word>, ObjectError> {
    let Some(class) = runtime.class(ctx, name) else {
        return Ok(None);
    };
    // check-added-lines: allow(unbound) ownership registers placeholders before CLOS
    if class == Word::UNBOUND || class == Word::fixnum(1) {
        return Ok(None);
    }
    if !matches!(classify_object(ctx, class), ObjectRef::SimpleVector(_))
        || simple_vector_length(ctx, class)? < 3
    {
        return Ok(None);
    }
    let name_word = simple_vector_ref(ctx, class, 0)?;
    let name_word = match classify_object(ctx, name_word) {
        ObjectRef::Symbol(_) => symbol_name(ctx, name_word)?,
        ObjectRef::String(_) => name_word,
        _ => return Ok(None), // check-added-lines: allow(wildcard) class names must be symbols or strings
    };
    if string_matches(ctx, name_word, name)? {
        Ok(Some(class))
    } else {
        Ok(None)
    }
}

fn string_matches(ctx: &ThreadContext, word: Word, expected: &str) -> Result<bool, ObjectError> {
    let chars = expected.chars().collect::<Vec<_>>();
    let length = string_length(ctx, word)?;
    if length != chars.len() {
        return Ok(false);
    }
    for (index, expected) in chars.iter().enumerate() {
        if string_ref(ctx, word, index)? != *expected {
            return Ok(false);
        }
    }
    Ok(true)
}

fn class_is_subclass(
    ctx: &ThreadContext,
    class: Word,
    expected: Word,
) -> Result<bool, ObjectError> {
    let mut pending = vec![class];
    let mut visited = Vec::new();
    while let Some(current) = pending.pop() {
        if current == expected {
            return Ok(true);
        }
        if visited.contains(&current) {
            continue;
        }
        visited.push(current);
        let direct = simple_vector_ref(ctx, current, 1)?;
        if direct == Word::NIL {
            continue;
        }
        if direct.is_cons() {
            let mut rest = direct;
            while rest != Word::NIL {
                if !rest.is_cons() {
                    return Err(ObjectError::Layout);
                }
                pending.push(car(ctx, rest)?);
                rest = cdr(ctx, rest)?;
            }
        } else {
            pending.push(direct);
        }
    }
    Ok(false)
}
