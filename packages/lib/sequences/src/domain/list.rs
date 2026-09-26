use ncl_object::{
    LispError, List, ObjectError, ObjectRef, Runtime, Sequence, ThreadContext, Word,
    car as object_car, cdr as object_cdr, make_cons, make_simple_vector, make_string,
    rplaca as object_rplaca, rplacd as object_rplacd, simple_vector_length, simple_vector_ref,
    string_length, string_ref,
};

fn make_cons_rooted(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    mut car: Word,
    mut cdr: Word,
) -> Result<Word, ObjectError> {
    let first_token = ncl_object::push_root(ctx, &mut car);
    let second_token = ncl_object::push_root(ctx, &mut cdr);
    let result = make_cons(ctx, runtime, car, cdr);
    let second_root_error =
        (!ncl_object::pop_root(ctx, second_token)).then_some(ObjectError::Layout);
    let first_root_error = (!ncl_object::pop_root(ctx, first_token)).then_some(ObjectError::Layout);
    if let Some(error) = second_root_error.or(first_root_error) {
        return Err(error);
    }
    result
}

fn list_word(value: List) -> Word {
    match value {
        List::Nil => Word::NIL,
        List::Cons(value) => value.into(),
    }
}

fn proper_list(ctx: &mut ThreadContext, mut cursor: Word) -> Result<(), LispError> {
    while cursor.is_cons() {
        cursor = object_cdr(ctx, cursor)?;
    }
    if cursor == Word::NIL {
        Ok(())
    } else {
        Err(LispError::TypeError {
            datum: cursor,
            expected: ncl_object::ObjectType::Cons,
        })
    }
}

#[allow(clippy::missing_const_for_fn, clippy::unnecessary_wraps)]
pub fn atom(_: &mut ThreadContext, _: &Runtime, value: Word) -> Result<Word, LispError> {
    Ok(if value.is_cons() {
        Word::NIL
    } else {
        Word::TRUE
    })
}
#[allow(clippy::missing_const_for_fn, clippy::unnecessary_wraps)]
pub fn cons_p(_: &mut ThreadContext, _: &Runtime, value: Word) -> Result<Word, LispError> {
    Ok(if value.is_cons() {
        Word::TRUE
    } else {
        Word::NIL
    })
}
#[allow(clippy::unnecessary_wraps)]
pub fn list_p(ctx: &mut ThreadContext, _: &Runtime, value: Word) -> Result<Word, LispError> {
    Ok(if proper_list(ctx, value).is_ok() {
        Word::TRUE
    } else {
        Word::NIL
    })
}
pub fn end_p(_: &mut ThreadContext, _: &Runtime, value: Word) -> Result<Word, LispError> {
    match value {
        v if v == Word::NIL => Ok(Word::TRUE),
        v if v.is_cons() => Ok(Word::NIL),
        v => Err(LispError::TypeError {
            datum: v,
            expected: ncl_object::ObjectType::Cons,
        }),
    }
}

pub fn car(ctx: &mut ThreadContext, _: &Runtime, value: List) -> Result<Word, LispError> {
    Ok(object_car(ctx, list_word(value))?)
}
pub fn cdr(ctx: &mut ThreadContext, _: &Runtime, value: List) -> Result<Word, LispError> {
    Ok(object_cdr(ctx, list_word(value))?)
}
pub fn cons(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    car: Word,
    cdr: Word,
) -> Result<Word, LispError> {
    Ok(make_cons_rooted(ctx, runtime, car, cdr)?)
}
pub fn list(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
) -> Result<Word, LispError> {
    super::list_from(ctx, runtime, values).map_err(LispError::from)
}

pub fn concatenate(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    destination_type: Word,
    sequences: &[Word],
) -> Result<Word, LispError> {
    let ObjectRef::Symbol(_) = ncl_object::classify_object(ctx, destination_type) else {
        return Err(LispError::TypeError {
            datum: destination_type,
            expected: ncl_object::ObjectType::Symbol,
        });
    };
    let name = ncl_object::symbol_name(ctx, destination_type)?;
    let name = (0..string_length(ctx, name)?)
        .map(|index| string_ref(ctx, name, index))
        .collect::<Result<String, _>>()?;
    let mut values = Vec::new();
    for &sequence in sequences {
        values.extend(sequence_values(ctx, super::sequence_value(ctx, sequence)?)?);
    }
    match name.as_str() {
        "NIL" => Ok(Word::NIL),
        "LIST" => super::list_from(ctx, runtime, &values).map_err(LispError::from),
        "VECTOR" => sequence_result(
            ctx,
            runtime,
            Sequence::Vector(ncl_object::SimpleVector::from_word(Word::NIL)),
            &values,
        )
        .map_err(LispError::from),
        "STRING" => {
            let chars = values
                .iter()
                .map(|value| character(*value))
                .collect::<Result<Vec<_>, _>>()?;
            make_string(ctx, runtime, &chars).map_err(LispError::from)
        }
        _ => Err(LispError::TypeError {
            datum: destination_type,
            expected: ncl_object::ObjectType::Symbol,
        }),
    }
}
pub fn list_star(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
) -> Result<Word, LispError> {
    let (&tail, leading) = values.split_last().ok_or(LispError::TypeError {
        datum: Word::NIL,
        expected: ncl_object::ObjectType::Cons,
    })?;
    let mut result = tail;
    for &value in leading.iter().rev() {
        result = make_cons_rooted(ctx, runtime, value, result)?;
    }
    Ok(result)
}
pub fn append(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
) -> Result<Word, LispError> {
    let (last, lists) = values
        .split_last()
        .map_or((Word::NIL, &[][..]), |(last, lists)| (*last, lists));
    let mut result = last;
    for &list in lists.iter().rev() {
        let mut items = Vec::new();
        let mut cursor = list;
        while cursor.is_cons() {
            items.push(object_car(ctx, cursor)?);
            cursor = object_cdr(ctx, cursor)?;
        }
        if cursor != Word::NIL {
            return Err(LispError::TypeError {
                datum: cursor,
                expected: ncl_object::ObjectType::Cons,
            });
        }
        for item in items.into_iter().rev() {
            result = make_cons_rooted(ctx, runtime, item, result)?;
        }
    }
    Ok(result)
}
pub fn nconc(ctx: &mut ThreadContext, _: &Runtime, values: &[Word]) -> Result<Word, LispError> {
    let Some((&last, lists)) = values.split_last() else {
        return Ok(Word::NIL);
    };
    let mut result = last;
    for &list in lists.iter().rev() {
        proper_list(ctx, list)?;
        if list == Word::NIL {
            continue;
        }
        let mut tail = list;
        while object_cdr(ctx, tail)? != Word::NIL {
            tail = object_cdr(ctx, tail)?;
        }
        object_rplacd(ctx, tail, result)?;
        result = list;
    }
    Ok(result)
}
pub fn rplaca(
    ctx: &mut ThreadContext,
    _: &Runtime,
    cons: Word,
    value: Word,
) -> Result<Word, LispError> {
    Ok(object_rplaca(ctx, cons, value)?)
}
pub fn rplacd(
    ctx: &mut ThreadContext,
    _: &Runtime,
    cons: Word,
    value: Word,
) -> Result<Word, LispError> {
    Ok(object_rplacd(ctx, cons, value)?)
}
pub fn copy_list(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    value: List,
) -> Result<Word, LispError> {
    let mut items = Vec::new();
    let mut cursor = list_word(value);
    while cursor.is_cons() {
        items.push(object_car(ctx, cursor)?);
        cursor = object_cdr(ctx, cursor)?;
    }
    if cursor != Word::NIL {
        return Err(LispError::TypeError {
            datum: cursor,
            expected: ncl_object::ObjectType::Cons,
        });
    }
    super::list_from(ctx, runtime, &items).map_err(LispError::from)
}

pub fn length(ctx: &mut ThreadContext, _: &Runtime, value: Sequence) -> Result<Word, LispError> {
    let length =
        i64::try_from(sequence_length(ctx, value).map_err(LispError::from)?).map_err(|_| {
            LispError::TypeError {
                datum: Word::NIL,
                expected: ncl_object::ObjectType::Fixnum,
            }
        })?;
    Ok(Word::fixnum(length))
}
pub fn elt(
    ctx: &mut ThreadContext,
    _: &Runtime,
    value: Sequence,
    index: ncl_object::Fixnum,
) -> Result<Word, LispError> {
    sequence_elt(ctx, value, index.as_word()).map_err(LispError::from)
}
pub fn copy_seq(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    value: Sequence,
) -> Result<Word, LispError> {
    sequence_copy(ctx, runtime, value).map_err(LispError::from)
}
pub fn reverse(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    value: Sequence,
) -> Result<Word, LispError> {
    sequence_reverse(ctx, runtime, value).map_err(LispError::from)
}
pub fn nreverse(ctx: &mut ThreadContext, _: &Runtime, value: Sequence) -> Result<Word, LispError> {
    sequence_nreverse(ctx, value).map_err(LispError::from)
}
pub fn subseq(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    value: Sequence,
    start: ncl_object::Fixnum,
    end: Option<ncl_object::Fixnum>,
) -> Result<Word, LispError> {
    sequence_subseq(
        ctx,
        runtime,
        value,
        start.as_word(),
        end.map(ncl_object::Fixnum::as_word),
    )
    .map_err(LispError::from)
}

fn sequence_values(ctx: &mut ThreadContext, value: Sequence) -> Result<Vec<Word>, ObjectError> {
    match value {
        Sequence::List(list) => {
            let mut out = Vec::new();
            let mut cursor = list_word(list);
            while cursor != Word::NIL {
                if !cursor.is_cons() {
                    return Err(ObjectError::TypeError);
                }
                out.push(object_car(ctx, cursor)?);
                cursor = object_cdr(ctx, cursor)?;
            }
            Ok(out)
        }
        Sequence::String(string) => {
            let word: Word = string.into();
            (0..string_length(ctx, word)?)
                .map(|i| Ok(Word::character(string_ref(ctx, word, i)? as u32)))
                .collect()
        }
        Sequence::Vector(vector) => {
            let word: Word = vector.into();
            (0..simple_vector_length(ctx, word)?)
                .map(|i| simple_vector_ref(ctx, word, i))
                .collect()
        }
    }
}
fn sequence_result(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    value: Sequence,
    values: &[Word],
) -> Result<Word, ObjectError> {
    match value {
        Sequence::List(_) => super::list_from(ctx, runtime, values),
        Sequence::Vector(_) => {
            let mut values = values.to_vec();
            let tokens = values
                .iter_mut()
                .map(|value| ncl_object::push_root(ctx, value))
                .collect::<Vec<_>>();
            let result = make_simple_vector(ctx, runtime, &values);
            let root_error = tokens.into_iter().rev().find_map(|token| {
                (!ncl_object::pop_root(ctx, token)).then_some(ObjectError::Layout)
            });
            root_error.map_or(result, Err)
        }
        Sequence::String(_) => values
            .iter()
            .map(|value| character(*value))
            .collect::<Result<Vec<_>, _>>()
            .and_then(|chars| make_string(ctx, runtime, &chars)),
    }
}

fn character(value: Word) -> Result<char, ObjectError> {
    value
        .as_character()
        .and_then(char::from_u32)
        .ok_or(ObjectError::TypeError)
}

fn sequence_word(value: Sequence) -> Word {
    match value {
        Sequence::List(list) => list_word(list),
        Sequence::String(string) => string.into(),
        Sequence::Vector(vector) => vector.into(),
    }
}

fn sequence_length(ctx: &mut ThreadContext, value: Sequence) -> Result<usize, ObjectError> {
    Ok(sequence_values(ctx, value)?.len())
}
fn sequence_elt(
    ctx: &mut ThreadContext,
    value: Sequence,
    index: Word,
) -> Result<Word, ObjectError> {
    let index = usize::try_from(index.as_fixnum().ok_or(ObjectError::TypeError)?)
        .map_err(|_| ObjectError::TypeError)?;
    sequence_values(ctx, value)?
        .get(index)
        .copied()
        .ok_or(ObjectError::TypeError)
}
fn sequence_copy(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    value: Sequence,
) -> Result<Word, ObjectError> {
    let values = sequence_values(ctx, value)?;
    let mut source = sequence_word(value);
    let token = ncl_object::push_root(ctx, &mut source);
    let result = sequence_result(ctx, runtime, value, &values);
    let root_error = (!ncl_object::pop_root(ctx, token)).then_some(ObjectError::Layout);
    root_error.map_or(result, Err)
}
fn sequence_reverse(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    value: Sequence,
) -> Result<Word, ObjectError> {
    let mut values = sequence_values(ctx, value)?;
    values.reverse();
    let mut source = sequence_word(value);
    let token = ncl_object::push_root(ctx, &mut source);
    let result = sequence_result(ctx, runtime, value, &values);
    let root_error = (!ncl_object::pop_root(ctx, token)).then_some(ObjectError::Layout);
    root_error.map_or(result, Err)
}
fn sequence_subseq(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    value: Sequence,
    start: Word,
    end: Option<Word>,
) -> Result<Word, ObjectError> {
    let values = sequence_values(ctx, value)?;
    let start = usize::try_from(start.as_fixnum().ok_or(ObjectError::TypeError)?)
        .map_err(|_| ObjectError::TypeError)?;
    let end = match end {
        Some(end) => usize::try_from(end.as_fixnum().ok_or(ObjectError::TypeError)?)
            .map_err(|_| ObjectError::TypeError)?,
        None => values.len(),
    };
    if start > end || end > values.len() {
        return Err(ObjectError::TypeError);
    }
    let mut source = sequence_word(value);
    let token = ncl_object::push_root(ctx, &mut source);
    let result = sequence_result(ctx, runtime, value, &values[start..end]);
    let root_error = (!ncl_object::pop_root(ctx, token)).then_some(ObjectError::Layout);
    root_error.map_or(result, Err)
}
fn sequence_nreverse(ctx: &mut ThreadContext, value: Sequence) -> Result<Word, ObjectError> {
    match value {
        Sequence::List(list) => {
            proper_list(ctx, list_word(list)).map_err(|_| ObjectError::TypeError)?;
            let mut previous = Word::NIL;
            let mut cursor = list_word(list);
            while cursor.is_cons() {
                let next = object_cdr(ctx, cursor)?;
                object_rplacd(ctx, cursor, previous)?;
                previous = cursor;
                cursor = next;
            }
            Ok(previous)
        }
        _ => Err(ObjectError::TypeError),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_list_sequence_has_no_elements() {
        let runtime = Runtime::new().unwrap();
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime).unwrap();
        let sequence = Sequence::List(List::Nil);

        assert_eq!(sequence_length(&mut ctx, sequence).unwrap(), 0);
        assert!(sequence_elt(&mut ctx, sequence, Word::fixnum(0)).is_err());
    }

    #[test]
    fn string_sequence_conversion_validates_word_character() {
        let runtime = Runtime::new().unwrap();
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime).unwrap();
        let marker = Sequence::String(ncl_object::StringObject::from_word(Word::NIL));
        let string = sequence_result(
            &mut ctx,
            &runtime,
            marker,
            &[Word::character(u32::from('λ'))],
        )
        .unwrap();
        assert_eq!(ncl_object::string_ref(&ctx, string, 0).unwrap(), 'λ');
        assert!(sequence_result(&mut ctx, &runtime, marker, &[Word::fixnum(65)]).is_err());
    }
}

fn nth_word(ctx: &mut ThreadContext, index: i64, mut cursor: Word) -> Result<Word, LispError> {
    if index < 0 {
        return Err(LispError::TypeError {
            datum: Word::fixnum(index),
            expected: ncl_object::ObjectType::Fixnum,
        });
    }
    for _ in 0..index {
        if cursor == Word::NIL {
            return Ok(Word::NIL);
        }
        cursor = object_cdr(ctx, cursor)?;
    }
    if cursor == Word::NIL {
        Ok(Word::NIL)
    } else {
        Ok(object_car(ctx, cursor)?)
    }
}
pub fn nth(
    ctx: &mut ThreadContext,
    _: &Runtime,
    index: ncl_object::Fixnum,
    value: List,
) -> Result<Word, LispError> {
    let cursor = list_word(value);
    proper_list(ctx, cursor)?;
    nth_word(ctx, index.value(), cursor)
}
pub fn nthcdr(
    ctx: &mut ThreadContext,
    _: &Runtime,
    index: ncl_object::Fixnum,
    value: List,
) -> Result<Word, LispError> {
    if index.value() < 0 {
        return Err(LispError::TypeError {
            datum: index.as_word(),
            expected: ncl_object::ObjectType::Fixnum,
        });
    }
    let mut cursor = list_word(value);
    for _ in 0..index.value() {
        if cursor == Word::NIL {
            return Ok(Word::NIL);
        }
        cursor = object_cdr(ctx, cursor)?;
    }
    proper_list(ctx, cursor)?;
    Ok(cursor)
}
pub fn list_length(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    value: List,
) -> Result<Word, LispError> {
    length(ctx, runtime, Sequence::List(value))
}

macro_rules! list_nth { ($($name:ident => $index:literal),+ $(,)?) => { $(pub fn $name(ctx: &mut ThreadContext, _: &Runtime, value: List) -> Result<Word, LispError> { let cursor = list_word(value); proper_list(ctx, cursor)?; nth_word(ctx, $index, cursor) })+ }; }
list_nth!(first => 0, second => 1, third => 2, fourth => 3, fifth => 4, sixth => 5, seventh => 6, eighth => 7, ninth => 8, tenth => 9);
