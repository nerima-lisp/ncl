#![allow(dead_code)]
use ncl_object::typed::FunctionDesignator;
use ncl_object::{
    BuiltinFunctionCaller, FunctionArguments, FunctionCaller, MultipleValues, ObjectError,
    ObjectRef, Runtime, ThreadContext, Word, car, cdr, classify_object, pop_root, push_root,
    rplaca, simple_vector_length, simple_vector_ref, simple_vector_set,
};
fn list_from(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
) -> Result<Word, ObjectError> {
    let mut rooted = values.to_vec();
    let roots = rooted
        .iter_mut()
        .map(|value| push_root(ctx, value))
        .collect::<Vec<_>>();
    let mut result = Word::NIL;
    let result_root = push_root(ctx, &mut result);
    for &value in rooted.iter().rev() {
        result = ncl_object::make_cons(ctx, runtime, value, result)?;
    }
    let valid =
        pop_root(ctx, result_root) && roots.into_iter().rev().all(|root| pop_root(ctx, root));
    if valid {
        Ok(result)
    } else {
        Err(ObjectError::Layout)
    }
}
#[derive(Clone, Copy)]
pub struct Options {
    pub key: Word,
    pub test: Word,
    pub test_not: Word,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            key: Word::NIL,
            test: Word::NIL,
            test_not: Word::NIL,
        }
    }
}
fn symbol_name(ctx: &ThreadContext, word: Word) -> Result<String, ObjectError> {
    let ObjectRef::Symbol(symbol) = classify_object(ctx, word) else {
        return Err(ObjectError::TypeError);
    };
    let name = ncl_object::symbol_name(ctx, symbol)?;
    (0..ncl_object::string_length(ctx, name)?)
        .map(|i| ncl_object::string_ref(ctx, name, i))
        .collect()
}
fn options(
    ctx: &ThreadContext,
    args: &[Word],
    required: usize,
) -> Result<(Vec<Word>, Options), ObjectError> {
    let mut positional = Vec::new();
    let mut out = Options {
        key: Word::NIL,
        test: Word::NIL,
        test_not: Word::NIL,
    };
    let mut i = 0;
    while i < args.len() {
        let name = symbol_name(ctx, args[i]).ok();
        let keyword = name.as_deref().map(str::to_ascii_uppercase);
        if matches!(keyword.as_deref(), Some("KEY" | "TEST" | "TEST-NOT")) {
            let value = *args.get(i + 1).ok_or(ObjectError::TypeError)?;
            if keyword.as_deref() == Some("KEY") {
                out.key = value;
            } else if keyword.as_deref() == Some("TEST") {
                out.test = value;
            } else if keyword.as_deref() == Some("TEST-NOT") {
                out.test_not = value;
            }
            i += 2;
        } else {
            positional.push(args[i]);
            i += 1;
        }
    }
    if positional.len() < required || (out.test != Word::NIL && out.test_not != Word::NIL) {
        return Err(ObjectError::TypeError);
    }
    Ok((positional, out))
}
pub fn parse_options(
    ctx: &ThreadContext,
    args: &[Word],
    required: usize,
) -> Result<(Vec<Word>, Options), ObjectError> {
    options(ctx, args, required)
}
fn sequence_values(ctx: &mut ThreadContext, sequence: Word) -> Result<Vec<Word>, ObjectError> {
    let mut result = Vec::new();
    if sequence == Word::NIL {
        return Ok(result);
    }
    if sequence.is_cons() {
        let mut cursor = sequence;
        while cursor != Word::NIL {
            if !cursor.is_cons() {
                return Err(ObjectError::TypeError);
            }
            result.push(car(ctx, cursor)?);
            cursor = cdr(ctx, cursor)?;
        }
        return Ok(result);
    }
    if let ObjectRef::SimpleVector(vector) = classify_object(ctx, sequence) {
        let vector: Word = ncl_object::SimpleVector::from_word(vector).into();
        for i in 0..simple_vector_length(ctx, vector)? {
            result.push(simple_vector_ref(ctx, vector, i)?);
        }
        Ok(result)
    } else {
        Err(ObjectError::TypeError)
    }
}
fn set_sequence_value(
    ctx: &mut ThreadContext,
    sequence: Word,
    index: usize,
    value: Word,
) -> Result<(), ObjectError> {
    if sequence.is_cons() {
        let mut cursor = sequence;
        for _ in 0..index {
            cursor = cdr(ctx, cursor)?;
        }
        rplaca(ctx, cursor, value)?;
        Ok(())
    } else {
        simple_vector_set(ctx, sequence, index, value)
    }
}
fn call_predicate(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    function: Word,
    args: &[Word],
) -> Result<Word, ObjectError> {
    let mut designator = function;
    let designator_root = push_root(ctx, &mut designator);
    let mut rooted = args.to_vec();
    let roots = rooted
        .iter_mut()
        .map(|word| push_root(ctx, word))
        .collect::<Vec<_>>();
    let result = (|| {
        let designator = FunctionDesignator::try_from_word(ctx, designator)?;
        let mut caller = BuiltinFunctionCaller;
        let mut values = MultipleValues::new();
        caller.call_function(
            ctx,
            runtime,
            designator,
            FunctionArguments::new(&rooted),
            &mut values,
        )
    })();
    let valid =
        roots.into_iter().rev().all(|token| pop_root(ctx, token)) && pop_root(ctx, designator_root);
    if valid {
        result
    } else {
        Err(ObjectError::Layout)
    }
}
fn key(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    key: Word,
    value: Word,
) -> Result<Word, ObjectError> {
    if key == Word::NIL {
        Ok(value)
    } else {
        call_predicate(ctx, runtime, key, &[value])
    }
}
fn matches(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    a: Word,
    b: Word,
    opts: Options,
) -> Result<bool, ObjectError> {
    let a = key(ctx, runtime, opts.key, a)?;
    let b = key(ctx, runtime, opts.key, b)?;
    let result = if opts.test != Word::NIL {
        call_predicate(ctx, runtime, opts.test, &[a, b])? != Word::NIL
    } else if opts.test_not != Word::NIL {
        call_predicate(ctx, runtime, opts.test_not, &[a, b])? == Word::NIL
    } else {
        a == b
    };
    Ok(result)
}
fn compare(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    function: Word,
    a: Word,
    b: Word,
) -> Result<bool, ObjectError> {
    Ok(call_predicate(ctx, runtime, function, &[a, b])? != Word::NIL)
}
pub fn sort(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    sequence: Word,
    predicate: Word,
    key_fn: Word,
    stable: bool,
) -> Result<Word, ObjectError> {
    let mut values = sequence_values(ctx, sequence)?;
    let roots = values
        .iter_mut()
        .map(|word| push_root(ctx, word))
        .collect::<Vec<_>>();
    let result = (|| {
        let key_values = if key_fn == Word::NIL {
            values.clone()
        } else {
            values
                .iter()
                .map(|&value| key(ctx, runtime, key_fn, value))
                .collect::<Result<Vec<_>, _>>()?
        };
        let mut order: Vec<usize> = Vec::with_capacity(values.len());
        for index in 0..values.len() {
            let mut position = order.len();
            for (offset, &other) in order.iter().enumerate() {
                if compare(
                    ctx,
                    runtime,
                    predicate,
                    key_values[index],
                    key_values[other],
                )? {
                    position = offset;
                    break;
                }
            }
            if !stable && position < order.len() && key_values[index] == key_values[order[position]]
            {
                position += 1;
            }
            order.insert(position, index);
        }
        let sorted = order.into_iter().map(|i| values[i]).collect::<Vec<_>>();
        for (i, value) in sorted.into_iter().enumerate() {
            set_sequence_value(ctx, sequence, i, value)?;
        }
        Ok(sequence)
    })();
    let valid = roots.into_iter().rev().all(|token| pop_root(ctx, token));
    if valid {
        result
    } else {
        Err(ObjectError::Layout)
    }
}
pub fn stable_sort(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    sequence: Word,
    predicate: Word,
    key_fn: Word,
) -> Result<Word, ObjectError> {
    sort(ctx, runtime, sequence, predicate, key_fn, true)
}
fn set_operation(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    first: Word,
    second: Word,
    opts: Options,
    exclusive: bool,
    difference: bool,
) -> Result<Word, ObjectError> {
    let left = sequence_values(ctx, first)?;
    let right = sequence_values(ctx, second)?;
    let mut result = Vec::new();
    let candidates = if difference {
        left.clone()
    } else {
        left.iter().chain(right.iter()).copied().collect()
    };
    for value in candidates {
        let mut in_left = false;
        for candidate in &left {
            if matches(ctx, runtime, value, *candidate, opts)? {
                in_left = true;
                break;
            }
        }
        let mut in_right = false;
        for candidate in &right {
            if matches(ctx, runtime, value, *candidate, opts)? {
                in_right = true;
                break;
            }
        }
        let include = if difference {
            in_left && !in_right
        } else if exclusive {
            in_left != in_right
        } else {
            true
        };
        let mut duplicate = false;
        for candidate in &result {
            if matches(ctx, runtime, value, *candidate, opts)? {
                duplicate = true;
                break;
            }
        }
        if include && !duplicate {
            result.push(value);
        }
    }
    list_from(ctx, runtime, &result)
}
pub fn union(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    first: Word,
    second: Word,
    opts: Options,
) -> Result<Word, ObjectError> {
    set_operation(ctx, runtime, first, second, opts, false, false)
}
pub fn intersection(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    first: Word,
    second: Word,
    opts: Options,
) -> Result<Word, ObjectError> {
    let left = sequence_values(ctx, first)?;
    let right = sequence_values(ctx, second)?;
    let mut values = Vec::new();
    for value in left {
        let mut found = false;
        for candidate in &right {
            if matches(ctx, runtime, value, *candidate, opts)? {
                found = true;
                break;
            }
        }
        if found {
            let mut duplicate = false;
            for candidate in &values {
                if matches(ctx, runtime, value, *candidate, opts)? {
                    duplicate = true;
                    break;
                }
            }
            if !duplicate {
                values.push(value);
            }
        }
    }
    list_from(ctx, runtime, &values)
}
pub fn set_difference(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    first: Word,
    second: Word,
    opts: Options,
) -> Result<Word, ObjectError> {
    set_operation(ctx, runtime, first, second, opts, false, true)
}
pub fn set_exclusive_or(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    first: Word,
    second: Word,
    opts: Options,
) -> Result<Word, ObjectError> {
    set_operation(ctx, runtime, first, second, opts, true, false)
}
pub fn subsetp(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    first: Word,
    second: Word,
    opts: Options,
) -> Result<Word, ObjectError> {
    let a = sequence_values(ctx, first)?;
    let b = sequence_values(ctx, second)?;
    for value in a {
        let mut found = false;
        for candidate in &b {
            if matches(ctx, runtime, value, *candidate, opts)? {
                found = true;
                break;
            }
        }
        if !found {
            return Ok(Word::NIL);
        }
    }
    Ok(Word::TRUE)
}
pub fn adjoin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    item: Word,
    list: Word,
    opts: Options,
) -> Result<Word, ObjectError> {
    let tail = sequence_values(ctx, list)?;
    for value in &tail {
        if matches(ctx, runtime, item, *value, opts)? {
            return Ok(list);
        }
    }
    let mut values = vec![item];
    values.extend(tail);
    list_from(ctx, runtime, &values)
}
fn assoc_like(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    item: Word,
    alist: Word,
    opts: Options,
    reverse: bool,
) -> Result<Word, ObjectError> {
    let mut cursor = alist;
    while cursor != Word::NIL {
        if !cursor.is_cons() {
            return Err(ObjectError::TypeError);
        }
        let pair = car(ctx, cursor)?;
        if !pair.is_cons() {
            return Err(ObjectError::TypeError);
        }
        let value = if reverse {
            cdr(ctx, pair)?
        } else {
            car(ctx, pair)?
        };
        if matches(ctx, runtime, item, value, opts)? {
            return Ok(pair);
        }
        cursor = cdr(ctx, cursor)?;
    }
    Ok(Word::NIL)
}
pub fn assoc(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    item: Word,
    alist: Word,
    opts: Options,
) -> Result<Word, ObjectError> {
    assoc_like(ctx, runtime, item, alist, opts, false)
}
pub fn rassoc(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    item: Word,
    alist: Word,
    opts: Options,
) -> Result<Word, ObjectError> {
    assoc_like(ctx, runtime, item, alist, opts, true)
}
pub fn member(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    item: Word,
    list: Word,
    opts: Options,
) -> Result<Word, ObjectError> {
    let mut cursor = list;
    while cursor != Word::NIL {
        if !cursor.is_cons() {
            return Err(ObjectError::TypeError);
        }
        let value = car(ctx, cursor)?;
        if matches(ctx, runtime, item, value, opts)? {
            return Ok(cursor);
        }
        cursor = cdr(ctx, cursor)?;
    }
    Ok(Word::NIL)
}
