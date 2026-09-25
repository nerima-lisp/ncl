use ncl_object::{
    Cons, ObjectError, ObjectRef, SimpleVector, StringObject, classify_object, make_cons,
    make_simple_vector, make_string, simple_vector_length, simple_vector_ref, simple_vector_set,
    string_length, string_ref, string_set, symbol_name,
};
use ncl_object::{
    LispError, List, Runtime, Sequence, ThreadContext, Word, car as object_car, cdr as object_cdr,
    rplaca as object_rplaca, rplacd as object_rplacd,
};

fn proper_list(ctx: &mut ThreadContext, value: Word) -> Result<(), LispError> {
    let mut cursor = value;
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

pub fn atom(_ctx: &mut ThreadContext, _runtime: &Runtime, value: Word) -> Result<Word, LispError> {
    Ok(if value.is_cons() {
        Word::NIL
    } else {
        Word::TRUE
    })
}
pub fn cons_p(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    value: Word,
) -> Result<Word, LispError> {
    Ok(if value.is_cons() {
        Word::TRUE
    } else {
        Word::NIL
    })
}
pub fn list_p(ctx: &mut ThreadContext, _runtime: &Runtime, value: Word) -> Result<Word, LispError> {
    Ok(if proper_list(ctx, value).is_ok() {
        Word::TRUE
    } else {
        Word::NIL
    })
}

pub fn end_p(_ctx: &mut ThreadContext, _runtime: &Runtime, value: Word) -> Result<Word, LispError> {
    if value == Word::NIL {
        Ok(Word::TRUE)
    } else if value.is_cons() {
        Ok(Word::NIL)
    } else {
        Err(LispError::TypeError {
            datum: value,
            expected: ncl_object::ObjectType::Cons,
        })
    }
}

fn list_word(value: List) -> Word {
    match value {
        List::Nil => Word::NIL,
        List::Cons(value) => value.into(),
    }
}
pub fn car(ctx: &mut ThreadContext, _runtime: &Runtime, value: List) -> Result<Word, LispError> {
    Ok(object_car(ctx, list_word(value))?)
}
pub fn cdr(ctx: &mut ThreadContext, _runtime: &Runtime, value: List) -> Result<Word, LispError> {
    Ok(object_cdr(ctx, list_word(value))?)
}

pub fn list_length(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    value: List,
) -> Result<Word, LispError> {
    let mut cursor = list_word(value);
    proper_list(ctx, cursor)?;
    let mut length = 0_i64;
    while cursor.is_cons() {
        cursor = object_cdr(ctx, cursor)?;
        length += 1;
    }
    Ok(Word::fixnum(length))
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
    _runtime: &Runtime,
    index: ncl_object::Fixnum,
    value: List,
) -> Result<Word, LispError> {
    let value = list_word(value);
    proper_list(ctx, value)?;
    nth_word(ctx, index.value(), value)
}
pub fn nthcdr(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
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
pub fn member(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    item: Word,
    value: List,
) -> Result<Word, LispError> {
    let mut cursor = list_word(value);
    proper_list(ctx, cursor)?;
    while cursor != Word::NIL {
        if object_car(ctx, cursor)? == item {
            return Ok(cursor);
        }
        cursor = object_cdr(ctx, cursor)?;
    }
    Ok(Word::NIL)
}
pub fn assoc(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    item: Word,
    value: List,
) -> Result<Word, LispError> {
    let mut cursor = list_word(value);
    proper_list(ctx, cursor)?;
    while cursor != Word::NIL {
        let entry = object_car(ctx, cursor)?;
        if entry.is_cons() && object_car(ctx, entry)? == item {
            return Ok(entry);
        }
        cursor = object_cdr(ctx, cursor)?;
    }
    Ok(Word::NIL)
}
pub fn rplaca(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    cons: Word,
    value: Word,
) -> Result<Word, LispError> {
    Ok(object_rplaca(ctx, cons, value)?)
}
pub fn rplacd(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    cons: Word,
    value: Word,
) -> Result<Word, LispError> {
    Ok(object_rplacd(ctx, cons, value)?)
}

fn seq_values(ctx: &mut ThreadContext, value: Sequence) -> Result<Vec<Word>, ObjectError> {
    let mut out = Vec::new();
    match value {
        Sequence::List(list) => {
            let mut cursor = list_word(list);
            while cursor != Word::NIL {
                if !cursor.is_cons() {
                    return Err(ObjectError::TypeError);
                }
                out.push(object_car(ctx, cursor)?);
                cursor = object_cdr(ctx, cursor)?;
            }
        }
        Sequence::String(s) => {
            let word: Word = s.into();
            for i in 0..string_length(ctx, word)? {
                out.push(Word::character(string_ref(ctx, word, i)? as u32));
            }
        }
        Sequence::Vector(v) => {
            let word: Word = v.into();
            for i in 0..simple_vector_length(ctx, word)? {
                out.push(simple_vector_ref(ctx, word, i)?);
            }
        }
    }
    Ok(out)
}
fn list_from(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
) -> Result<Word, ObjectError> {
    let mut result = Word::NIL;
    for &value in values.iter().rev() {
        let token = ncl_object::push_root(ctx, &mut result);
        let next = make_cons(ctx, runtime, value, result)?;
        if !ncl_object::pop_root(ctx, token) {
            return Err(ObjectError::Layout);
        }
        result = next;
    }
    Ok(result)
}
fn seq_result(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    value: Sequence,
    values: &[Word],
) -> Result<Word, ObjectError> {
    match value {
        Sequence::List(_) => list_from(ctx, runtime, values),
        Sequence::Vector(_) => make_simple_vector(ctx, runtime, values),
        Sequence::String(_) => {
            let chars = values
                .iter()
                .map(|w| char::from_u32((w.bits() >> 4) as u32).ok_or(ObjectError::TypeError))
                .collect::<Result<Vec<_>, _>>()?;
            make_string(ctx, runtime, &chars)
        }
    }
}
pub(crate) fn sequence_value(ctx: &ThreadContext, word: Word) -> Result<Sequence, ObjectError> {
    if word == Word::NIL {
        return Ok(Sequence::List(List::Nil));
    }
    Ok(match classify_object(ctx, word) {
        ObjectRef::Cons(_) => Sequence::List(List::Cons(Cons::from(word))),
        ObjectRef::String(_) => Sequence::String(StringObject::from(word)),
        ObjectRef::SimpleVector(_) => Sequence::Vector(SimpleVector::from(word)),
        _ => return Err(ObjectError::TypeError),
    })
}
pub(crate) fn sequence_length(
    ctx: &mut ThreadContext,
    value: Sequence,
) -> Result<usize, ObjectError> {
    Ok(seq_values(ctx, value)?.len())
}
pub(crate) fn sequence_elt(
    ctx: &mut ThreadContext,
    value: Sequence,
    index: Word,
) -> Result<Word, ObjectError> {
    let i = usize::try_from(index.as_fixnum().ok_or(ObjectError::TypeError)?)
        .map_err(|_| ObjectError::TypeError)?;
    seq_values(ctx, value)?
        .get(i)
        .copied()
        .ok_or(ObjectError::TypeError)
}
pub(crate) fn sequence_subseq(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    value: Sequence,
    start: Word,
    end: Option<Word>,
) -> Result<Word, ObjectError> {
    let all = seq_values(ctx, value)?;
    let s = usize::try_from(start.as_fixnum().ok_or(ObjectError::TypeError)?)
        .map_err(|_| ObjectError::TypeError)?;
    let e = end
        .map(|x| {
            usize::try_from(x.as_fixnum().ok_or(ObjectError::TypeError)?)
                .map_err(|_| ObjectError::TypeError)
        })
        .transpose()?
        .unwrap_or(all.len());
    if s > e || e > all.len() {
        return Err(ObjectError::TypeError);
    }
    seq_result(ctx, runtime, value, &all[s..e])
}
pub(crate) fn sequence_copy(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    value: Sequence,
) -> Result<Word, ObjectError> {
    let all = seq_values(ctx, value)?;
    seq_result(ctx, runtime, value, &all)
}
pub(crate) fn sequence_reverse(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    value: Sequence,
) -> Result<Word, ObjectError> {
    let mut all = seq_values(ctx, value)?;
    all.reverse();
    seq_result(ctx, runtime, value, &all)
}
pub(crate) fn sequence_nreverse(
    ctx: &mut ThreadContext,
    value: Sequence,
) -> Result<Word, ObjectError> {
    if let Sequence::List(list) = value {
        let mut old = list_word(list);
        let mut new = Word::NIL;
        while old != Word::NIL {
            let next = object_cdr(ctx, old)?;
            object_rplacd(ctx, old, new)?;
            new = old;
            old = next;
        }
        return Ok(new);
    }
    Err(ObjectError::Unsupported)
}
pub(crate) fn sequence_fill(
    ctx: &mut ThreadContext,
    value: Sequence,
    item: Word,
) -> Result<Word, ObjectError> {
    let len = sequence_length(ctx, value)?;
    for i in 0..len {
        match value {
            Sequence::List(list) => {
                let mut cursor = list_word(list);
                for _ in 0..i {
                    cursor = object_cdr(ctx, cursor)?;
                }
                object_rplaca(ctx, cursor, item)?;
            }
            Sequence::String(s) => string_set(
                ctx,
                s.into(),
                i,
                char::from_u32((item.bits() >> 4) as u32).ok_or(ObjectError::TypeError)?,
            )?,
            Sequence::Vector(v) => simple_vector_set(ctx, v.into(), i, item)?,
        }
    }
    Ok(match value {
        Sequence::List(x) => list_word(x),
        Sequence::String(x) => x.into(),
        Sequence::Vector(x) => x.into(),
    })
}
pub(crate) fn sequence_replace(
    ctx: &mut ThreadContext,
    destination: Sequence,
    source: Sequence,
) -> Result<Word, ObjectError> {
    let values = seq_values(ctx, source)?;
    let len = sequence_length(ctx, destination)?.min(values.len());
    for (i, item) in values.into_iter().take(len).enumerate() {
        match destination {
            Sequence::List(x) => {
                let mut cursor = list_word(x);
                for _ in 0..i {
                    cursor = object_cdr(ctx, cursor)?;
                }
                object_rplaca(ctx, cursor, item)?;
            }
            Sequence::String(s) => string_set(
                ctx,
                s.into(),
                i,
                char::from_u32((item.bits() >> 4) as u32).ok_or(ObjectError::TypeError)?,
            )?,
            Sequence::Vector(v) => simple_vector_set(ctx, v.into(), i, item)?,
        }
    }
    Ok(match destination {
        Sequence::List(x) => list_word(x),
        Sequence::String(x) => x.into(),
        Sequence::Vector(x) => x.into(),
    })
}
pub(crate) fn make_sequence(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    type_word: Word,
    length: Word,
    initial: Option<Word>,
) -> Result<Word, ObjectError> {
    let n = usize::try_from(length.as_fixnum().ok_or(ObjectError::TypeError)?)
        .map_err(|_| ObjectError::TypeError)?;
    let name = symbol_name(ctx, type_word)?;
    let mut text = String::new();
    for i in 0..string_length(ctx, name)? {
        text.push(string_ref(ctx, name, i)?);
    }
    let values = vec![initial.unwrap_or(Word::NIL); n];
    match text.as_str() {
        "LIST" => list_from(ctx, runtime, &values),
        "VECTOR" => make_simple_vector(ctx, runtime, &values),
        "STRING" => {
            let chars = values
                .iter()
                .map(|x| char::from_u32((x.bits() >> 4) as u32).ok_or(ObjectError::TypeError))
                .collect::<Result<Vec<_>, _>>()?;
            make_string(ctx, runtime, &chars)
        }
        _ => Err(ObjectError::TypeError),
    }
}
