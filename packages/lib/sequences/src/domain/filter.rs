//! Basic destructive sequence updates.

use ncl_object::{
    Builtin, BuiltinArgs, BuiltinConvention, BuiltinImplementation, BuiltinName, LambdaList,
    MultipleValues, ObjectError, ObjectRef, Parameter, ParameterType, Runtime, Sequence,
    ThreadContext, Word,
};

const fn parameter(name: &'static str, ty: ParameterType) -> Parameter {
    Parameter {
        name: BuiltinName::new(name),
        ty,
    }
}

const SEQ: Parameter = parameter("sequence", ParameterType::Sequence);
const ITEM: Parameter = parameter("item", ParameterType::Any);
const START: Parameter = parameter("start", ParameterType::Fixnum);
const END: Parameter = parameter("end", ParameterType::Fixnum);
const RANGE_KEYS: &[Parameter] = &[START, END];

#[derive(Clone, Copy, Default)]
struct Options {
    start: usize,
    end: Option<usize>,
}

fn list_word(list: ncl_object::List) -> Word {
    match list {
        ncl_object::List::Nil => Word::NIL,
        ncl_object::List::Cons(value) => value.into(),
    }
}

fn sequence(ctx: &ThreadContext, word: Word) -> Result<Sequence, ObjectError> {
    if word == Word::NIL {
        return Ok(Sequence::List(ncl_object::List::Nil));
    }
    if word.is_cons() {
        return Ok(Sequence::List(ncl_object::List::Cons(
            ncl_object::Cons::from_word(word),
        )));
    }
    match ncl_object::classify_object(ctx, word) {
        ObjectRef::SimpleVector(vector) => Ok(Sequence::Vector(
            ncl_object::SimpleVector::from_word(vector),
        )),
        ObjectRef::String(string) => Ok(Sequence::String(ncl_object::StringObject::from_word(
            string,
        ))),
        _ => Err(ObjectError::TypeError),
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
                let code = ncl_object::string_ref(ctx, word, index)?;
                result.push(Word::character(u32::from(code)));
            }
        }
    }
    Ok(result)
}

fn keyword_name(ctx: &ThreadContext, word: Word) -> Result<String, ObjectError> {
    let ObjectRef::Symbol(symbol) = ncl_object::classify_object(ctx, word) else {
        return Err(ObjectError::TypeError);
    };
    let name = ncl_object::symbol_name(ctx, symbol)?;
    (0..ncl_object::string_length(ctx, name)?)
        .map(|index| ncl_object::string_ref(ctx, name, index))
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
        if args[index] != Word::NIL {
            let name = keyword_name(ctx, args[index]).ok();
            if name
                .as_deref()
                .is_some_and(|name| name == "START" || name == "END")
            {
                let value = *args.get(index + 1).ok_or(ObjectError::TypeError)?;
                let number = usize::try_from(value.as_fixnum().ok_or(ObjectError::TypeError)?)
                    .map_err(|_| ObjectError::TypeError)?;
                if name.as_deref() == Some("START") {
                    options.start = number;
                } else {
                    options.end = Some(number);
                }
                index += 2;
                continue;
            }
        }
        positional.push(args[index]);
        index += 1;
    }
    if positional.len() < required {
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

fn character(value: Word) -> Result<char, ObjectError> {
    value
        .as_character()
        .and_then(char::from_u32)
        .ok_or(ObjectError::TypeError)
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
            ncl_object::string_set(ctx, string.into(), index, character(value)?)?;
        }
    }
    Ok(())
}

fn fill_callback(ctx: &mut ThreadContext, args: &BuiltinArgs<'_>) -> Result<Word, ObjectError> {
    let (positional, options) = parse_options(ctx, args.as_slice(), 2)?;
    let sequence = sequence(ctx, positional[0])?;
    let length = values(ctx, sequence)?.len();
    let (start, end) = bounds(options, length)?;
    for index in start..end {
        set_value(ctx, sequence, index, positional[1])?;
    }
    Ok(positional[0])
}

fn replace_callback(ctx: &mut ThreadContext, args: &BuiltinArgs<'_>) -> Result<Word, ObjectError> {
    let (positional, options) = parse_options(ctx, args.as_slice(), 2)?;
    let destination = sequence(ctx, positional[0])?;
    let source = values(ctx, sequence(ctx, positional[1])?)?;
    let length = values(ctx, destination)?.len();
    let (start, end) = bounds(options, length)?;
    for (offset, value) in source.into_iter().take(end - start).enumerate() {
        set_value(ctx, destination, start + offset, value)?;
    }
    Ok(positional[0])
}

fn entry_callback(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
    replace: bool,
) -> Result<Word, ObjectError> {
    let result = if replace {
        replace_callback(ctx, args)
    } else {
        fill_callback(ctx, args)
    }?;
    values.clear();
    Ok(result)
}

fn fill_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    entry_callback(ctx, runtime, args, values, false)
}

fn replace_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    entry_callback(ctx, runtime, args, values, true)
}

#[allow(clippy::unnecessary_wraps)]
fn pass(args: &BuiltinArgs<'_>) -> Result<Vec<Word>, ObjectError> {
    Ok(args.as_slice().to_vec())
}

/// Return the implementation for the basic destructive sequence operations.
pub fn filter_entry(name: &str) -> Option<BuiltinImplementation> {
    let descriptor = |required: &'static [Parameter]| Builtin {
        lambda_list: LambdaList::new(required, &[], None, RANGE_KEYS, true),
        convention: BuiltinConvention::Adapted,
    };
    let implementation = match name {
        "FILL" => BuiltinImplementation::adapted(descriptor(&[SEQ, ITEM]), fill_entry, pass),
        "REPLACE" => BuiltinImplementation::adapted(descriptor(&[SEQ, SEQ]), replace_entry, pass),
        _ => return None,
    };
    Some(implementation)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn character_accepts_only_valid_character_words() {
        assert_eq!(character(Word::character(u32::from('λ'))), Ok('λ'));
        assert_eq!(character(Word::fixnum(65)), Err(ObjectError::TypeError));
    }
}
