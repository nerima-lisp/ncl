use ncl_object::{
    Cons, FunctionObject, ObjectError, ObjectRef, SimpleVector, StringObject, classify_object,
    make_cons, make_simple_vector, make_string, simple_vector_length, simple_vector_ref,
    simple_vector_set, string_length, string_ref, string_set, symbol_function, symbol_name,
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

fn list_copy(ctx: &mut ThreadContext, runtime: &Runtime, value: Word) -> Result<Word, LispError> {
    let mut values = Vec::new();
    let mut cursor = value;
    while cursor.is_cons() {
        values.push(object_car(ctx, cursor)?);
        cursor = object_cdr(ctx, cursor)?;
    }
    let mut result = cursor;
    for item in values.into_iter().rev() {
        result = make_cons(ctx, runtime, item, result)?;
    }
    Ok(result)
}
fn tree_copy(ctx: &mut ThreadContext, runtime: &Runtime, value: Word) -> Result<Word, LispError> {
    if !value.is_cons() {
        return Ok(value);
    }
    let car_value = object_car(ctx, value)?;
    let car = tree_copy(ctx, runtime, car_value)?;
    let cdr_value = object_cdr(ctx, value)?;
    let cdr = tree_copy(ctx, runtime, cdr_value)?;
    Ok(make_cons(ctx, runtime, car, cdr)?)
}
pub fn list(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
) -> Result<Word, LispError> {
    Ok(list_from(ctx, runtime, values)?)
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
        result = make_cons(ctx, runtime, value, result)?;
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
        .map_or((Word::NIL, &[][..]), |(x, xs)| (*x, xs));
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
            result = make_cons(ctx, runtime, item, result)?;
        }
    }
    Ok(result)
}
pub fn cons(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    car: Word,
    cdr: Word,
) -> Result<Word, LispError> {
    Ok(make_cons(ctx, runtime, car, cdr)?)
}
pub fn copy_list(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    value: List,
) -> Result<Word, LispError> {
    list_copy(ctx, runtime, list_word(value))
}
pub fn copy_tree(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    value: Word,
) -> Result<Word, LispError> {
    tree_copy(ctx, runtime, value)
}
pub fn acons(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    key: Word,
    datum: Word,
    alist: List,
) -> Result<Word, LispError> {
    let entry = make_cons(ctx, runtime, key, datum)?;
    Ok(make_cons(ctx, runtime, entry, list_word(alist))?)
}
pub fn pairlis(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    keys: List,
    data: List,
    alist: List,
) -> Result<Word, LispError> {
    let mut keys = list_word(keys);
    let mut data = list_word(data);
    let mut result = list_word(alist);
    while keys != Word::NIL && data != Word::NIL {
        let key = object_car(ctx, keys)?;
        let datum = object_car(ctx, data)?;
        let entry = make_cons(ctx, runtime, key, datum)?;
        result = make_cons(ctx, runtime, entry, result)?;
        keys = object_cdr(ctx, keys)?;
        data = object_cdr(ctx, data)?;
    }
    if keys != Word::NIL || data != Word::NIL {
        return Err(LispError::TypeError {
            datum: if keys != Word::NIL { keys } else { data },
            expected: ncl_object::ObjectType::Cons,
        });
    }
    Ok(result)
}
pub fn getf(
    ctx: &mut ThreadContext,
    plist: List,
    indicator: Word,
    default: Word,
) -> Result<Word, LispError> {
    let mut cursor = list_word(plist);
    while cursor != Word::NIL {
        let key = object_car(ctx, cursor)?;
        cursor = object_cdr(ctx, cursor)?;
        if cursor == Word::NIL {
            return Err(LispError::TypeError {
                datum: cursor,
                expected: ncl_object::ObjectType::Cons,
            });
        }
        let value = object_car(ctx, cursor)?;
        cursor = object_cdr(ctx, cursor)?;
        if key == indicator {
            return Ok(value);
        }
    }
    Ok(default)
}
fn predicate(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    function: Word,
    value: Word,
) -> Result<Word, LispError> {
    let function = match classify_object(ctx, function) {
        ObjectRef::Function(word) => FunctionObject::try_from(word).map_err(LispError::from)?,
        ObjectRef::Symbol(symbol) => {
            FunctionObject::try_from(symbol_function(ctx, symbol)?).map_err(LispError::from)?
        }
        _ => {
            return Err(LispError::TypeError {
                datum: function,
                expected: ncl_object::ObjectType::Function,
            });
        }
    };
    Ok(runtime
        .call_builtin(ctx, function, &[value])
        .map_err(LispError::from)?)
}
fn alist_entry(ctx: &mut ThreadContext, entry: Word) -> Result<Option<(Word, Word)>, LispError> {
    if !entry.is_cons() {
        return Ok(None);
    }
    Ok(Some((object_car(ctx, entry)?, object_cdr(ctx, entry)?)))
}
pub fn assoc_if(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    function: Word,
    alist: List,
    invert: bool,
) -> Result<Word, LispError> {
    let mut cursor = list_word(alist);
    while cursor != Word::NIL {
        let entry = object_car(ctx, cursor)?;
        if let Some((key, _)) = alist_entry(ctx, entry)? {
            if (predicate(ctx, runtime, function, key)? != Word::NIL) != invert {
                return Ok(entry);
            }
        }
        cursor = object_cdr(ctx, cursor)?;
    }
    Ok(Word::NIL)
}
pub fn rassoc(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    item: Word,
    alist: List,
) -> Result<Word, LispError> {
    let mut cursor = list_word(alist);
    while cursor != Word::NIL {
        let entry = object_car(ctx, cursor)?;
        if let Some((_, value)) = alist_entry(ctx, entry)?
            && value == item
        {
            return Ok(entry);
        }
        cursor = object_cdr(ctx, cursor)?;
    }
    Ok(Word::NIL)
}
pub fn rassoc_if(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    function: Word,
    alist: List,
    invert: bool,
) -> Result<Word, LispError> {
    let mut cursor = list_word(alist);
    while cursor != Word::NIL {
        let entry = object_car(ctx, cursor)?;
        if let Some((_, value)) = alist_entry(ctx, entry)? {
            if (predicate(ctx, runtime, function, value)? != Word::NIL) != invert {
                return Ok(entry);
            }
        }
        cursor = object_cdr(ctx, cursor)?;
    }
    Ok(Word::NIL)
}
pub fn car(ctx: &mut ThreadContext, _runtime: &Runtime, value: List) -> Result<Word, LispError> {
    Ok(object_car(ctx, list_word(value))?)
}
pub fn cdr(ctx: &mut ThreadContext, _runtime: &Runtime, value: List) -> Result<Word, LispError> {
    Ok(object_cdr(ctx, list_word(value))?)
}

fn composite(ctx: &mut ThreadContext, mut value: Word, path: &[u8]) -> Result<Word, LispError> {
    for step in path.iter().rev() {
        value = if *step == b'a' {
            object_car(ctx, value)?
        } else {
            object_cdr(ctx, value)?
        };
    }
    Ok(value)
}
macro_rules! composite_builtin {
    ($name:ident, $path:literal) => {
        pub fn $name(
            ctx: &mut ThreadContext,
            _runtime: &Runtime,
            value: Word,
        ) -> Result<Word, LispError> {
            composite(ctx, value, $path.as_bytes())
        }
    };
}
composite_builtin!(caar, "aa");
composite_builtin!(cadr, "ad");
composite_builtin!(cdar, "da");
composite_builtin!(cddr, "dd");
composite_builtin!(caaar, "aaa");
composite_builtin!(caadr, "aad");
composite_builtin!(cadar, "ada");
composite_builtin!(caddr, "add");
composite_builtin!(cdaar, "daa");
composite_builtin!(cdadr, "dad");
composite_builtin!(cddar, "dda");
composite_builtin!(cdddr, "ddd");
composite_builtin!(caaaar, "aaaa");
composite_builtin!(caaadr, "aaad");
composite_builtin!(caadar, "aada");
composite_builtin!(caaddr, "aadd");
composite_builtin!(cadaar, "adaa");
composite_builtin!(cadadr, "adad");
composite_builtin!(caddar, "adda");
composite_builtin!(cadddr, "addd");
composite_builtin!(cdaaar, "daaa");
composite_builtin!(cdaadr, "daad");
composite_builtin!(cdadar, "dada");
composite_builtin!(cdaddr, "dadd");
composite_builtin!(cddaar, "ddaa");
composite_builtin!(cddadr, "ddad");
composite_builtin!(cdddar, "ddda");
composite_builtin!(cddddr, "dddd");

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
        ObjectRef::Cons(_) => Sequence::List(List::Cons(Cons::from_word(word))),
        ObjectRef::String(_) => Sequence::String(StringObject::from_word(word)),
        ObjectRef::SimpleVector(_) => Sequence::Vector(SimpleVector::from_word(word)),
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
