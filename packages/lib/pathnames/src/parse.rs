use super::{
    BuiltinArgs, MultipleValues, ObjectError, Runtime, SLOTS, ThreadContext, Word, keyword_symbol,
    list, make_pathname, make_string, relative_directory, structure_ref, text,
};
use crate::is_pathname;
fn parse_source(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    source: &str,
    host: Word,
) -> Result<Word, ObjectError> {
    let absolute = source.starts_with('/');
    let trimmed = source.trim_start_matches('/');
    let mut parts = trimmed
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    let filename = if source.ends_with('/') {
        None
    } else {
        parts.pop()
    };
    let (name, type_) = filename.map_or((None, None), |file| match file.rsplit_once('.') {
        Some((base, extension)) if !base.is_empty() => (Some(base), Some(extension)),
        _ => (Some(file), None), // check-added-lines: allow(wildcard) filenames without an extension
    });
    let marker = keyword_symbol(ctx, runtime, if absolute { "ABSOLUTE" } else { "RELATIVE" })?;
    let components = std::iter::once(marker)
        .chain(
            parts
                .into_iter()
                .map(|part| make_string(ctx, runtime, &part.chars().collect::<Vec<_>>()))
                .collect::<Result<Vec<_>, _>>()?,
        )
        .collect::<Vec<_>>();
    let directory = ncl_object::with_roots(ctx, &components, |ctx, rooted| {
        list(
            ctx,
            runtime,
            &rooted.iter().map(|slot| slot.get()).collect::<Vec<_>>(),
        )
    })?;
    let mut slots = [Word::NIL; SLOTS];
    slots[0] = host; // check-added-lines: allow(index) fixed pathname slot layout
    slots[2] = directory; // check-added-lines: allow(index) fixed pathname slot layout
    slots[3] = name.map_or(Ok(Word::NIL), |value| {
        // check-added-lines: allow(index) fixed pathname slot layout
        make_string(ctx, runtime, &value.chars().collect::<Vec<_>>())
    })?;
    let type_value = type_.map_or(Ok(Word::NIL), |value| {
        // check-added-lines: allow(index) fixed pathname slot layout
        // check-added-lines: allow(index) fixed pathname slot layout
        make_string(ctx, runtime, &value.chars().collect::<Vec<_>>())
    })?;
    slots[4] = type_value; // check-added-lines: allow(index) fixed pathname slot layout
    make_pathname(ctx, runtime, &slots)
}

pub fn parse_namestring_value(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    input: Word,
) -> Result<Word, ObjectError> {
    let mut source = text(ctx, input)?;
    let host = if let Some((prefix, rest)) = source
        .split_once(':')
        .map(|(a, b)| (a.to_owned(), b.to_owned()))
    {
        if !prefix.contains('/') && !prefix.is_empty() {
            source = rest;
            make_string(ctx, runtime, &prefix.chars().collect::<Vec<_>>())?
        } else {
            Word::NIL
        }
    } else {
        Word::NIL
    };
    parse_source(ctx, runtime, &source, host)
}

pub fn parse_namestring_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let input = args.required(0)?;
    if is_pathname(ctx, runtime, input) {
        values.set(&[input, Word::fixnum(0), Word::NIL]);
        return Ok(input);
    }
    let source = text(ctx, input)?;
    let start = usize::try_from(args.get(3).and_then(Word::as_fixnum).unwrap_or(0))
        .map_err(|_| ObjectError::TypeError)?;
    let end_value = match args.get(4).and_then(Word::as_fixnum) {
        Some(value) => value,
        None => i64::try_from(source.chars().count()).map_err(|_| ObjectError::Layout)?,
    };
    let end = usize::try_from(end_value).map_err(|_| ObjectError::TypeError)?;
    let chars = source.chars().collect::<Vec<_>>();
    if start > end || end > chars.len() {
        return Err(ObjectError::TypeError);
    }
    let sliced = make_string(ctx, runtime, &chars[start..end])?; // check-added-lines: allow(index) bounds checked above
    let mut pathname = parse_namestring_value(ctx, runtime, sliced)?;
    let mut slots = [Word::NIL; SLOTS];
    for (index, slot) in slots.iter_mut().enumerate() {
        *slot = structure_ref(ctx, pathname, index)?;
    }
    if let Some(host) = args.get(1).filter(|value| *value != Word::NIL) {
        slots[0] = host; // check-added-lines: allow(index) fixed pathname slot layout
    }
    if let Some(defaults) = args.get(2).filter(|value| *value != Word::NIL) {
        let defaults = if is_pathname(ctx, runtime, defaults) {
            defaults
        } else {
            return Err(ObjectError::TypeError);
        };
        for (index, slot) in slots.iter_mut().enumerate() {
            if *slot == Word::NIL || (index == 2 && relative_directory(ctx, *slot)?) {
                *slot = structure_ref(ctx, defaults, index)?;
            }
        }
    }
    pathname = make_pathname(ctx, runtime, &slots)?;
    values.set(&[
        pathname,
        Word::fixnum(i64::try_from(end).map_err(|_| ObjectError::Layout)?),
        Word::NIL,
    ]);
    Ok(pathname)
}
