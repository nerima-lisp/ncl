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

#[derive(Clone, Copy)]
struct SearchOptions {
    start: usize,
    end: Option<usize>,
    from_end: bool,
    test: Option<Word>,
    test_not: Option<Word>,
    key: Option<Word>,
}

fn keyword_name(ctx: &ThreadContext, word: Word) -> Result<String, LispError> {
    let name = symbol_name(ctx, word)?;
    (0..string_length(ctx, name)?)
        .map(|index| string_ref(ctx, name, index))
        .collect::<Result<String, _>>()
        .map_err(LispError::from)
}

fn search_options(
    ctx: &ThreadContext,
    args: &[Word],
    length: usize,
) -> Result<SearchOptions, LispError> {
    let mut options = SearchOptions {
        start: 0,
        end: None,
        from_end: false,
        test: None,
        test_not: None,
        key: None,
    };
    let mut index = 2;
    while index < args.len() {
        let keyword = keyword_name(ctx, args[index])?;
        let value = args.get(index + 1).copied().ok_or(LispError::ProgramError(
            ncl_object::ProgramError::OddKeywordArguments,
        ))?;
        match keyword.as_str() {
            "START" => {
                options.start = usize::try_from(
                    value
                        .as_fixnum()
                        .ok_or(LispError::TypeError {
                            datum: value,
                            expected: ncl_object::ObjectType::Fixnum,
                        })?,
                )
                .map_err(|_| LispError::TypeError {
                    datum: value,
                    expected: ncl_object::ObjectType::Fixnum,
                })?;
            }
            "END" => {
                options.end = Some(usize::try_from(
                    value
                        .as_fixnum()
                        .ok_or(LispError::TypeError {
                            datum: value,
                            expected: ncl_object::ObjectType::Fixnum,
                        })?,
                )
                .map_err(|_| LispError::TypeError {
                    datum: value,
                    expected: ncl_object::ObjectType::Fixnum,
                })?);
            }
            "FROM-END" => options.from_end = value != Word::NIL,
            "TEST" => options.test = Some(value),
            "TEST-NOT" => options.test_not = Some(value),
            "KEY" => options.key = (value != Word::NIL).then_some(value),
            _ => {
                return Err(LispError::ProgramError(
                    ncl_object::ProgramError::UnknownKeyword,
                ));
            }
        }
        index += 2;
    }
    let end = options.end.unwrap_or(length);
    if options.test.is_some() && options.test_not.is_some()
        || options.start > end
        || end > length
    {
        return Err(LispError::TypeError {
            datum: Word::fixnum(end as i64),
            expected: ncl_object::ObjectType::Fixnum,
        });
    }
    options.end = Some(end);
    Ok(options)
}

fn call_designator(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    function: Word,
    args: &[Word],
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
    runtime
        .call_builtin(ctx, function, args)
        .map_err(LispError::from)
}

fn keyed(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    key: Option<Word>,
    value: Word,
) -> Result<Word, LispError> {
    key.map_or(Ok(value), |function| call_designator(ctx, runtime, function, &[value]))
}

fn matches(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    item: Word,
    value: Word,
    options: SearchOptions,
    predicate: Option<Word>,
    invert: bool,
) -> Result<bool, LispError> {
    let value = keyed(ctx, runtime, options.key, value)?;
    let result = if let Some(predicate) = predicate {
        call_designator(ctx, runtime, predicate, &[value])? != Word::NIL
    } else if let Some(test) = options.test {
        call_designator(ctx, runtime, test, &[item, value])? != Word::NIL
    } else if let Some(test_not) = options.test_not {
        call_designator(ctx, runtime, test_not, &[item, value])? == Word::NIL
    } else {
        item == value
    };
    Ok(result != invert)
}

fn search_index(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
    item: Word,
    options: SearchOptions,
    predicate: Option<Word>,
    invert: bool,
) -> Result<Option<usize>, LispError> {
    let range = options.start..options.end.unwrap_or(values.len());
    if options.from_end {
        for index in range.rev() {
            if matches(ctx, runtime, item, values[index], options, predicate, invert)? {
                return Ok(Some(index));
            }
        }
    } else {
        for index in range {
            if matches(ctx, runtime, item, values[index], options, predicate, invert)? {
                return Ok(Some(index));
            }
        }
    }
    Ok(None)
}

pub(crate) fn sequence_search(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
    predicate: Option<Word>,
    invert: bool,
    result: SearchResult,
) -> Result<Word, LispError> {
    let sequence = sequence_value(ctx, args[1]).map_err(LispError::from)?;
    let values = seq_values(ctx, sequence).map_err(LispError::from)?;
    let options = search_options(ctx, args, values.len())?;
    let index = search_index(ctx, runtime, &values, args[0], options, predicate, invert)?;
    match result {
        SearchResult::Find => Ok(index.map_or(Word::NIL, |index| values[index])),
        SearchResult::Position => Ok(index.map_or(Word::NIL, |index| Word::fixnum(index as i64))),
        SearchResult::Count => {
            let mut count = 0_i64;
            let end = options.end.unwrap_or(values.len());
            for index in options.start..end {
                if matches(ctx, runtime, args[0], values[index], options, predicate, invert)? {
                    count += 1;
                }
            }
            Ok(Word::fixnum(count))
        }
        SearchResult::Member => {
            let Some(index) = index else { return Ok(Word::NIL) };
            if let Sequence::List(list) = sequence {
                let mut cursor = list_word(list);
                for _ in 0..index {
                    cursor = object_cdr(ctx, cursor)?;
                }
                Ok(cursor)
            } else {
                Err(LispError::TypeError {
                    datum: args[1],
                    expected: ncl_object::ObjectType::Cons,
                })
            }
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum SearchResult {
    Find,
    Position,
    Count,
    Member,
}
