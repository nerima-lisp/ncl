//! Common Lisp sequence filtering and replacement operations.
//!
//! This module deliberately keeps keyword decoding at the builtin boundary.  The
//! actual operations work on a snapshot of the sequence, which gives the
//! non-destructive functions their required copy-on-write behaviour and makes
//! `:from-end` and `:count` deterministic for lists, vectors, and strings.

use ncl_object::typed::FunctionDesignator;
use ncl_object::{
    Builtin, BuiltinArgs, BuiltinConvention, BuiltinFunctionCaller, BuiltinImplementation,
    BuiltinName, FunctionArguments, FunctionCaller, LambdaList, MultipleValues, ObjectError,
    ObjectRef, Parameter, ParameterType, Runtime, Sequence, ThreadContext, Word, pop_root,
    push_root,
};

const fn parameter(name: &'static str, ty: ParameterType) -> Parameter {
    Parameter {
        name: BuiltinName::new(name),
        ty,
    }
}

const SEQ: Parameter = parameter("sequence", ParameterType::Sequence);
const ITEM: Parameter = parameter("item", ParameterType::Any);
const PRED: Parameter = parameter("predicate", ParameterType::FunctionDesignator);
const KEY: Parameter = parameter("key", ParameterType::FunctionDesignator);
const TEST: Parameter = parameter("test", ParameterType::FunctionDesignator);
const TEST_NOT: Parameter = parameter("test-not", ParameterType::FunctionDesignator);
const START: Parameter = parameter("start", ParameterType::Fixnum);
const END: Parameter = parameter("end", ParameterType::Fixnum);
const FROM_END: Parameter = parameter("from-end", ParameterType::Any);
const COUNT: Parameter = parameter("count", ParameterType::Fixnum);
const RANGE_KEYS: &[Parameter] = &[START, END, FROM_END, COUNT, KEY, TEST, TEST_NOT];
const SEARCH_KEYS: &[Parameter] = &[START, END, FROM_END, KEY, TEST, TEST_NOT];

#[derive(Clone, Copy, Default)]
struct Options {
    start: usize,
    end: Option<usize>,
    from_end: bool,
    count: Option<usize>,
    key: Option<Word>,
    test: Option<Word>,
    test_not: Option<Word>,
}

fn list_word(list: ncl_object::List) -> Word {
    match list {
        ncl_object::List::Nil => Word::NIL,
        ncl_object::List::Cons(value) => value.into(),
    }
}

fn values(ctx: &mut ThreadContext, sequence: Sequence) -> Result<Vec<Word>, ObjectError> {
    let mut result = Vec::new();
    match sequence {
        Sequence::List(list) => {
            let mut cursor = list_word(list);
            while cursor != Word::NIL {
                if !cursor.is_cons() {
                    return Err(ObjectError::TypeError);
                }
                result.push(ncl_object::car(ctx, cursor)?);
                cursor = ncl_object::cdr(ctx, cursor)?;
            }
        }
        Sequence::Vector(vector) => {
            let word: Word = vector.into();
            for index in 0..ncl_object::simple_vector_length(ctx, word)? {
                result.push(ncl_object::simple_vector_ref(ctx, word, index)?);
            }
        }
        Sequence::String(string) => {
            let word: Word = string.into();
            for index in 0..ncl_object::string_length(ctx, word)? {
                result.push(Word::character(
                    ncl_object::string_ref(ctx, word, index)? as u32
                ));
            }
        }
    }
    Ok(result)
}

fn sequence(ctx: &ThreadContext, word: Word) -> Result<Sequence, ObjectError> {
    match ncl_object::classify_object(ctx, word) {
        ObjectRef::Cons(cons) => Ok(Sequence::List(ncl_object::List::Cons(
            ncl_object::Cons::from_word(cons),
        ))),
        ObjectRef::SimpleVector(vector) => Ok(Sequence::Vector(
            ncl_object::SimpleVector::from_word(vector),
        )),
        ObjectRef::String(string) => Ok(Sequence::String(ncl_object::StringObject::from_word(
            string,
        ))),
        _ if word == Word::NIL => Ok(Sequence::List(ncl_object::List::Nil)),
        _ => Err(ObjectError::TypeError),
    }
}

fn list_from(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    items: &[Word],
) -> Result<Word, ObjectError> {
    let mut result = Word::NIL;
    for &item in items.iter().rev() {
        result = ncl_object::make_cons(ctx, runtime, item, result)?;
    }
    Ok(result)
}

fn call(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    designator: Word,
    args: &[Word],
) -> Result<Word, ObjectError> {
    let mut designator_word = designator;
    let mut rooted_args = args.to_vec();
    let designator_token = push_root(ctx, &mut designator_word);
    let argument_tokens = rooted_args
        .iter_mut()
        .map(|argument| push_root(ctx, argument))
        .collect::<Vec<_>>();
    let result = FunctionDesignator::try_from_word(ctx, designator_word).and_then(|designator| {
        let mut caller = BuiltinFunctionCaller;
        let mut values = MultipleValues::new();
        caller.call_function(
            ctx,
            runtime,
            designator,
            FunctionArguments::new(&rooted_args),
            &mut values,
        )
    });
    let argument_root_error = argument_tokens
        .into_iter()
        .rev()
        .find_map(|token| (!pop_root(ctx, token)).then_some(ObjectError::Layout));
    let designator_root_error = (!pop_root(ctx, designator_token)).then_some(ObjectError::Layout);
    if let Some(error) = argument_root_error.or(designator_root_error) {
        return Err(error);
    }
    result
}

fn truth(value: Word) -> bool {
    value != Word::NIL
}

fn keyword_name(ctx: &ThreadContext, word: Word) -> Result<String, ObjectError> {
    let ObjectRef::Symbol(symbol) = ncl_object::classify_object(ctx, word) else {
        return Err(ObjectError::TypeError);
    };
    let name = ncl_object::symbol_name(ctx, symbol)?;
    (0..ncl_object::string_length(ctx, name)?)
        .map(|i| ncl_object::string_ref(ctx, name, i))
        .collect()
}

fn parse_options(
    ctx: &ThreadContext,
    args: &[Word],
    required: usize,
) -> Result<(Vec<Word>, Options), ObjectError> {
    let mut positional = Vec::new();
    let mut options = Options::default();
    let mut index = 0;
    while index < args.len() {
        if args[index] != Word::NIL
            && keyword_name(ctx, args[index]).is_ok_and(|n| {
                matches!(
                    n.as_str(),
                    "START" | "END" | "FROM-END" | "COUNT" | "KEY" | "TEST" | "TEST-NOT"
                )
            })
        {
            let name = keyword_name(ctx, args[index])?;
            let value = *args.get(index + 1).ok_or(ObjectError::TypeError)?;
            match name.as_str() {
                "START" => {
                    options.start =
                        usize::try_from(value.as_fixnum().ok_or(ObjectError::TypeError)?)
                            .map_err(|_| ObjectError::TypeError)?;
                }
                "END" => {
                    options.end = Some(
                        usize::try_from(value.as_fixnum().ok_or(ObjectError::TypeError)?)
                            .map_err(|_| ObjectError::TypeError)?,
                    );
                }
                "FROM-END" => options.from_end = truth(value),
                "COUNT" => {
                    options.count = Some(
                        usize::try_from(value.as_fixnum().ok_or(ObjectError::TypeError)?)
                            .map_err(|_| ObjectError::TypeError)?,
                    );
                }
                "KEY" => options.key = (value != Word::NIL).then_some(value),
                "TEST" => options.test = Some(value),
                "TEST-NOT" => options.test_not = Some(value),
                _ => return Err(ObjectError::TypeError),
            }
            index += 2;
        } else {
            positional.push(args[index]);
            index += 1;
        }
    }
    if positional.len() < required || options.test.is_some() && options.test_not.is_some() {
        return Err(ObjectError::TypeError);
    }
    Ok((positional, options))
}

fn bounds(options: Options, length: usize) -> Result<(usize, usize), ObjectError> {
    let end = options.end.unwrap_or(length);
    if options.start > end || end > length {
        return Err(ObjectError::TypeError);
    }
    Ok((options.start, end))
}

fn matches(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    value: Word,
    item: Word,
    options: Options,
) -> Result<bool, ObjectError> {
    let value = if let Some(key) = options.key {
        call(ctx, runtime, key, &[value])?
    } else {
        value
    };
    if let Some(test) = options.test {
        return Ok(truth(call(ctx, runtime, test, &[item, value])?));
    }
    if let Some(test_not) = options.test_not {
        return Ok(!truth(call(ctx, runtime, test_not, &[item, value])?));
    }
    Ok(value == item)
}

fn selected_indices(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    items: &[Word],
    needle: Word,
    options: Options,
) -> Result<Vec<usize>, ObjectError> {
    let (start, end) = bounds(options, items.len())?;
    let mut indices: Vec<usize> = (start..end).collect();
    if options.from_end {
        indices.reverse();
    }
    let mut selected = Vec::new();
    for index in indices {
        if matches(ctx, runtime, items[index], needle, options)? {
            selected.push(index);
            if options.count.is_some_and(|count| selected.len() >= count) {
                break;
            }
        }
    }
    Ok(selected)
}

fn character(value: Word) -> Result<char, ObjectError> {
    char::from_u32(u32::try_from(value.bits() >> 4).map_err(|_| ObjectError::TypeError)?)
        .ok_or(ObjectError::TypeError)
}

fn fixnum(value: usize) -> Result<Word, ObjectError> {
    Ok(Word::fixnum(
        i64::try_from(value).map_err(|_| ObjectError::TypeError)?,
    ))
}

fn set_value(
    ctx: &mut ThreadContext,
    sequence: Sequence,
    index: usize,
    value: Word,
) -> Result<(), ObjectError> {
    match sequence {
        Sequence::List(list) => {
            let mut cursor = list_word(list);
            for _ in 0..index {
                cursor = ncl_object::cdr(ctx, cursor)?;
            }
            ncl_object::rplaca(ctx, cursor, value)?;
        }
        Sequence::Vector(vector) => {
            ncl_object::simple_vector_set(ctx, vector.into(), index, value)?;
        }
        Sequence::String(string) => {
            ncl_object::string_set(ctx, string.into(), index, character(value)?)?
        }
    }
    Ok(())
}

fn result(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    original: Sequence,
    items: &[Word],
) -> Result<Word, ObjectError> {
    match original {
        Sequence::List(_) => list_from(ctx, runtime, items),
        Sequence::Vector(_) => ncl_object::make_simple_vector(ctx, runtime, items),
        Sequence::String(_) => {
            let chars = items
                .iter()
                .map(|v| character(*v))
                .collect::<Result<Vec<_>, _>>()?;
            ncl_object::make_string(ctx, runtime, &chars)
        }
    }
}

fn pass(args: &BuiltinArgs<'_>) -> Result<Vec<Word>, ObjectError> {
    (0..args.len())
        .map(|index| args.get(index).ok_or(ObjectError::TypeError))
        .collect()
}

fn filter_callback(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    mode: u8,
    destructive: bool,
) -> Result<Word, ObjectError> {
    if mode == 3 {
        let (positional, options) = parse_options(ctx, args.as_slice(), 1)?;
        let sequence = sequence(ctx, positional[0])?;
        let items = values(ctx, sequence)?;
        let mut output = Vec::with_capacity(items.len());
        let mut retained = Vec::new();
        let mut order: Vec<usize> = (0..items.len()).collect();
        if options.from_end {
            order.reverse();
        }
        for index in order {
            let mut duplicate = false;
            for previous in retained.iter().copied() {
                if matches(ctx, runtime, items[index], items[previous], options)? {
                    duplicate = true;
                    break;
                }
            }
            if !duplicate {
                retained.push(index);
            }
        }
        retained.sort_unstable();
        for (index, value) in items.iter().copied().enumerate() {
            if retained.binary_search(&index).is_ok() {
                output.push(value);
            }
        }
        if destructive {
            for (index, value) in output.iter().copied().enumerate() {
                set_value(ctx, sequence, index, value)?;
            }
            return Ok(match sequence {
                Sequence::List(list) => list_word(list),
                Sequence::Vector(v) => v.into(),
                Sequence::String(s) => s.into(),
            });
        }
        return result(ctx, runtime, sequence, &output);
    }
    let (positional, options) = parse_options(ctx, args.as_slice(), 2)?;
    let (item, sequence_word) = (positional[0], positional[1]);
    let sequence = sequence(ctx, sequence_word)?;
    let items = values(ctx, sequence)?;
    let predicate = mode == 1 || mode == 2;
    let mut keep = vec![true; items.len()];
    let (start, end) = bounds(options, items.len())?;
    let mut candidates: Vec<usize> = (start..end).collect();
    if options.from_end {
        candidates.reverse();
    }
    let mut hit = 0;
    for index in candidates {
        let argument = if let Some(key) = options.key {
            call(ctx, runtime, key, &[items[index]])?
        } else {
            items[index]
        };
        let matched = if predicate {
            truth(call(ctx, runtime, item, &[argument])?)
        } else {
            matches(ctx, runtime, items[index], item, options)?
        };
        let matched = if mode == 2 { !matched } else { matched };
        if matched && options.count.is_none_or(|count| hit < count) {
            keep[index] = mode == 3;
            hit += 1;
        }
    }
    let output: Vec<Word> = items
        .iter()
        .enumerate()
        .filter_map(|(i, v)| keep[i].then_some(*v))
        .collect();
    if destructive {
        for (index, value) in output.iter().copied().enumerate().take(items.len()) {
            set_value(ctx, sequence, index, value)?;
        }
        Ok(match sequence {
            Sequence::List(list) => list_word(list),
            Sequence::Vector(v) => v.into(),
            Sequence::String(s) => s.into(),
        })
    } else {
        result(ctx, runtime, sequence, &output)
    }
}

fn find_position_count(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    kind: u8,
) -> Result<Word, ObjectError> {
    let (positional, options) = parse_options(ctx, args.as_slice(), 2)?;
    let sequence = sequence(ctx, positional[1])?;
    let items = values(ctx, sequence)?;
    let found = selected_indices(ctx, runtime, &items, positional[0], options)?;
    match kind {
        0 => Ok(found.first().map_or(Word::NIL, |i| items[*i])),
        1 => found.first().map_or(Ok(Word::NIL), |i| fixnum(*i)),
        _ => fixnum(found.len()),
    }
}

fn replace_callback(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    nsubstitute: bool,
    predicate: bool,
) -> Result<Word, ObjectError> {
    let (positional, options) = parse_options(ctx, args.as_slice(), 3)?;
    let (new, old_or_pred, sequence_word) = (positional[0], positional[1], positional[2]);
    let sequence = sequence(ctx, sequence_word)?;
    let items = values(ctx, sequence)?;
    let mut output = items.clone();
    let (start, end) = bounds(options, items.len())?;
    let mut candidates: Vec<usize> = (start..end).collect();
    if options.from_end {
        candidates.reverse();
    }
    let mut hit = 0;
    for index in candidates {
        let matched = if predicate {
            truth(call(ctx, runtime, old_or_pred, &[items[index]])?)
        } else {
            matches(ctx, runtime, items[index], old_or_pred, options)?
        };
        if matched && options.count.is_none_or(|count| hit < count) {
            output[index] = new;
            hit += 1;
            if nsubstitute {
                set_value(ctx, sequence, index, new)?;
            }
        }
    }
    if nsubstitute {
        Ok(sequence_word)
    } else {
        result(ctx, runtime, sequence, &output)
    }
}

fn fill_callback(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let (positional, options) = parse_options(ctx, args.as_slice(), 2)?;
    let sequence = sequence(ctx, positional[0])?;
    let len = values(ctx, sequence)?.len();
    let (start, end) = bounds(options, len)?;
    for index in start..end {
        set_value(ctx, sequence, index, positional[1])?;
    }
    Ok(positional[0])
}

macro_rules! find_callbacks {
    ($($name:ident, $kind:expr);+ $(;)?) => { $(fn $name(ctx: &mut ThreadContext, runtime: &Runtime, args: &BuiltinArgs<'_>, _: &mut MultipleValues) -> Result<Word, ObjectError> { find_position_count(ctx, runtime, args, $kind) })+ };
}
find_callbacks!(find_cb, 0; position_cb, 1; count_cb, 2);

macro_rules! filter_callbacks {
    ($($name:ident, $mode:expr, $destructive:expr);+ $(;)?) => { $(fn $name(ctx: &mut ThreadContext, runtime: &Runtime, args: &BuiltinArgs<'_>, _: &mut MultipleValues) -> Result<Word, ObjectError> { filter_callback(ctx, runtime, args, $mode, $destructive) })+ };
}
filter_callbacks!(remove_cb, 0, false; delete_cb, 0, true; remove_if_cb, 1, false; delete_if_cb, 1, true; remove_if_not_cb, 2, false; delete_if_not_cb, 2, true; remove_duplicates_cb, 3, false; delete_duplicates_cb, 3, true);

macro_rules! replace_callbacks {
    ($($name:ident, $nsubstitute:expr, $predicate:expr);+ $(;)?) => { $(fn $name(ctx: &mut ThreadContext, runtime: &Runtime, args: &BuiltinArgs<'_>, _: &mut MultipleValues) -> Result<Word, ObjectError> { replace_callback(ctx, runtime, args, $nsubstitute, $predicate) })+ };
}
replace_callbacks!(substitute_cb, false, false; nsubstitute_cb, true, false; substitute_if_cb, false, true; nsubstitute_if_cb, true, true);

fn find_if_callback(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    kind: u8,
    negate: bool,
) -> Result<Word, ObjectError> {
    let (positional, options) = parse_options(ctx, args.as_slice(), 2)?;
    let sequence = sequence(ctx, positional[1])?;
    let items = values(ctx, sequence)?;
    let (start, end) = bounds(options, items.len())?;
    let mut indexes: Vec<usize> = (start..end).collect();
    if options.from_end {
        indexes.reverse();
    }
    let mut found = Vec::new();
    for index in indexes {
        let value = if let Some(key) = options.key {
            call(ctx, runtime, key, &[items[index]])?
        } else {
            items[index]
        };
        let matched = truth(call(ctx, runtime, positional[0], &[value])?);
        if matched != negate {
            found.push(index);
            if options.count == Some(1) {
                break;
            }
        }
    }
    match kind {
        0 => Ok(found.first().map_or(Word::NIL, |i| items[*i])),
        1 => found.first().map_or(Ok(Word::NIL), |i| fixnum(*i)),
        _ => fixnum(found.len()),
    }
}
macro_rules! find_if_callbacks {
    ($($name:ident, $kind:expr, $negate:expr);+ $(;)?) => { $(fn $name(ctx: &mut ThreadContext, runtime: &Runtime, args: &BuiltinArgs<'_>, _: &mut MultipleValues) -> Result<Word, ObjectError> { find_if_callback(ctx, runtime, args, $kind, $negate) })+ };
}
find_if_callbacks!(find_if_cb, 0, false; find_if_not_cb, 0, true; position_if_cb, 1, false; position_if_not_cb, 1, true; count_if_cb, 2, false; count_if_not_cb, 2, true);

fn fill_entry_callback(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    fill_callback(ctx, runtime, args, &mut MultipleValues::new())
}

fn replace_entry_callback(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let (positional, options) = parse_options(ctx, args.as_slice(), 2)?;
    let destination = sequence(ctx, positional[0])?;
    let source_sequence = sequence(ctx, positional[1])?;
    let source = values(ctx, source_sequence)?;
    let destination_len = values(ctx, destination)?.len();
    let (start, end) = bounds(options, destination_len)?;
    let amount = (end - start).min(source.len());
    for (index, value) in source.iter().copied().enumerate().take(amount) {
        set_value(ctx, destination, start + index, value)?;
    }
    Ok(positional[0])
}

fn mismatch_entry_callback(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let (positional, options) = parse_options(ctx, args.as_slice(), 2)?;
    let left_sequence = sequence(ctx, positional[0])?;
    let left = values(ctx, left_sequence)?;
    let right_sequence = sequence(ctx, positional[1])?;
    let right = values(ctx, right_sequence)?;
    let (start, end) = bounds(options, left.len())?;
    let limit = (end - start).min(right.len());
    for offset in 0..limit {
        if left[start + offset] != right[offset] {
            return fixnum(start + offset);
        }
    }
    if end - start == right.len() {
        Ok(Word::NIL)
    } else {
        fixnum(start + limit)
    }
}

/// Return the typed implementation for a sequence filtering builtin.
pub fn filter_entry(name: &str) -> Option<BuiltinImplementation> {
    let required = |parameters: &'static [Parameter]| Builtin {
        lambda_list: LambdaList::new(parameters, &[], None, RANGE_KEYS, true),
        convention: BuiltinConvention::Adapted,
    };
    let implementation = match name {
        "FIND" => BuiltinImplementation::adapted(required(&[ITEM, SEQ]), find_cb, pass),
        "POSITION" => BuiltinImplementation::adapted(required(&[ITEM, SEQ]), position_cb, pass),
        "COUNT" => BuiltinImplementation::adapted(required(&[ITEM, SEQ]), count_cb, pass),
        "REMOVE" => BuiltinImplementation::adapted(required(&[ITEM, SEQ]), remove_cb, pass),
        "DELETE" => BuiltinImplementation::adapted(required(&[ITEM, SEQ]), delete_cb, pass),
        "REMOVE-DUPLICATES" => {
            BuiltinImplementation::adapted(required(&[SEQ]), remove_duplicates_cb, pass)
        }
        "DELETE-DUPLICATES" => {
            BuiltinImplementation::adapted(required(&[SEQ]), delete_duplicates_cb, pass)
        }
        "FILL" => BuiltinImplementation::adapted(required(&[SEQ, ITEM]), fill_entry_callback, pass),
        "FIND-IF" => BuiltinImplementation::adapted(required(&[PRED, SEQ]), find_if_cb, pass),
        "FIND-IF-NOT" => {
            BuiltinImplementation::adapted(required(&[PRED, SEQ]), find_if_not_cb, pass)
        }
        "POSITION-IF" => {
            BuiltinImplementation::adapted(required(&[PRED, SEQ]), position_if_cb, pass)
        }
        "POSITION-IF-NOT" => {
            BuiltinImplementation::adapted(required(&[PRED, SEQ]), position_if_not_cb, pass)
        }
        "COUNT-IF" => BuiltinImplementation::adapted(required(&[PRED, SEQ]), count_if_cb, pass),
        "COUNT-IF-NOT" => {
            BuiltinImplementation::adapted(required(&[PRED, SEQ]), count_if_not_cb, pass)
        }
        "REMOVE-IF" => BuiltinImplementation::adapted(required(&[PRED, SEQ]), remove_if_cb, pass),
        "REMOVE-IF-NOT" => {
            BuiltinImplementation::adapted(required(&[PRED, SEQ]), remove_if_not_cb, pass)
        }
        "DELETE-IF" => BuiltinImplementation::adapted(required(&[PRED, SEQ]), delete_if_cb, pass),
        "DELETE-IF-NOT" => {
            BuiltinImplementation::adapted(required(&[PRED, SEQ]), delete_if_not_cb, pass)
        }
        "SUBSTITUTE" => {
            BuiltinImplementation::adapted(required(&[ITEM, ITEM, SEQ]), substitute_cb, pass)
        }
        "NSUBSTITUTE" => {
            BuiltinImplementation::adapted(required(&[ITEM, ITEM, SEQ]), nsubstitute_cb, pass)
        }
        "SUBSTITUTE-IF" => {
            BuiltinImplementation::adapted(required(&[ITEM, PRED, SEQ]), substitute_if_cb, pass)
        }
        "NSUBSTITUTE-IF" => {
            BuiltinImplementation::adapted(required(&[ITEM, PRED, SEQ]), nsubstitute_if_cb, pass)
        }
        "SEARCH" => BuiltinImplementation::adapted(
            Builtin {
                lambda_list: LambdaList::new(&[SEQ, SEQ], &[], None, SEARCH_KEYS, true),
                convention: BuiltinConvention::Adapted,
            },
            search_entry_callback,
            pass,
        ),
        "MISMATCH" => BuiltinImplementation::adapted(
            Builtin {
                lambda_list: LambdaList::new(&[SEQ, SEQ], &[], None, SEARCH_KEYS, true),
                convention: BuiltinConvention::Adapted,
            },
            mismatch_entry_callback,
            pass,
        ),
        "REPLACE" => BuiltinImplementation::adapted(
            Builtin {
                lambda_list: LambdaList::new(&[SEQ, SEQ], &[], None, RANGE_KEYS, true),
                convention: BuiltinConvention::Adapted,
            },
            replace_entry_callback,
            pass,
        ),
        _ => return None,
    };
    Some(implementation)
}

fn search_entry_callback(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let (positional, options) = parse_options(ctx, args.as_slice(), 2)?;
    let needle_sequence = sequence(ctx, positional[0])?;
    let needle = values(ctx, needle_sequence)?;
    let haystack_sequence = sequence(ctx, positional[1])?;
    let haystack = values(ctx, haystack_sequence)?;
    let (start, end) = bounds(options, haystack.len())?;
    if needle.is_empty() {
        return fixnum(if options.from_end { end } else { start });
    }
    let range = if options.from_end {
        (start..=end.saturating_sub(needle.len()))
            .rev()
            .collect::<Vec<_>>()
    } else {
        (start..=end.saturating_sub(needle.len())).collect::<Vec<_>>()
    };
    for index in range {
        if haystack[index..index + needle.len()] == needle[..] {
            return fixnum(index);
        }
    }
    Ok(Word::NIL)
}
