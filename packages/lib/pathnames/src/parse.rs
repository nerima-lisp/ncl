use super::{
    BuiltinArgs, MultipleValues, ObjectError, Runtime, SLOTS, ThreadContext, Word, keyword_symbol,
    list, make_pathname, make_string, text,
};
use crate::is_pathname;
use ncl_object::string_length;

pub fn parse_namestring_value(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    input: Word,
) -> Result<Word, ObjectError> {
    let source = text(ctx, input)?;
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
    slots[2] = directory; // check-added-lines: allow(index) fixed pathname slot layout
    slots[3] = name.map_or(Ok(Word::NIL), |value| {
        // check-added-lines: allow(index) fixed pathname slot layout
        make_string(ctx, runtime, &value.chars().collect::<Vec<_>>())
    })?;
    // check-added-lines: allow(index) fixed pathname slot layout
    slots[4] = type_.map_or(Ok(Word::NIL), |value| {
        // check-added-lines: allow(index) fixed pathname slot layout
        // check-added-lines: allow(index) fixed pathname slot layout
        make_string(ctx, runtime, &value.chars().collect::<Vec<_>>())
    })?;
    make_pathname(ctx, runtime, &slots)
}

pub fn parse_namestring_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let input = args.required(0)?;
    let pathname = if is_pathname(ctx, runtime, input) {
        input
    } else {
        parse_namestring_value(ctx, runtime, input)?
    };
    let position =
        Word::fixnum(i64::try_from(string_length(ctx, input)?).map_err(|_| ObjectError::Layout)?);
    values.set(&[pathname, position, Word::NIL]);
    Ok(pathname)
}
