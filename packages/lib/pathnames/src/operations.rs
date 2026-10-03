use super::{
    BuiltinArgs, MultipleValues, ObjectError, Package, Runtime, SLOTS, ThreadContext, Word, car,
    cdr, component_string, directory_text, is_pathname, make_cons, make_pathname, make_string,
    namestring_value, pathname_designator, relative_directory, string_length, structure_ref,
    symbol_text, text, with_root,
};
use std::fs;
use std::path::Path;

pub fn has_wildcards(ctx: &ThreadContext, word: Word) -> Result<bool, ObjectError> {
    if word == Word::NIL {
        return Ok(false);
    }
    if let Ok(value) = text(ctx, word) {
        return Ok(value.contains('*') || value.contains('?'));
    }
    if let Ok(value) = symbol_text(ctx, word) {
        return Ok(value.contains('*') || value.contains('?'));
    }
    let mut cursor = word;
    while cursor != Word::NIL {
        if has_wildcards(ctx, car(ctx, cursor)?)? {
            return Ok(true);
        }
        cursor = cdr(ctx, cursor)?;
    }
    Ok(false)
}

pub fn wildcard_match(pattern: &str, value: &str) -> bool {
    let pattern = pattern.chars().collect::<Vec<_>>();
    let value = value.chars().collect::<Vec<_>>();
    let mut states = vec![false; value.len() + 1];
    states[0] = true; // check-added-lines: allow(index) state vector is initialized with value.len() + 1 entries
    for token in pattern {
        let mut next = vec![false; value.len() + 1];
        match token {
            '*' => {
                let mut active = false;
                for index in 0..=value.len() {
                    active |= states[index]; // check-added-lines: allow(index) loop bounds match the state vector
                    next[index] = active; // check-added-lines: allow(index) loop bounds match the state vector
                }
            }
            '?' => {
                next[1..=value.len()].copy_from_slice(&states[..value.len()]); // check-added-lines: allow(index) both slices have value.len() elements
            }
            literal => {
                for index in 1..=value.len() {
                    next[index] = states[index - 1] && value[index - 1] == literal; // check-added-lines: allow(index) loop bounds keep all offsets in range
                }
            }
        }
        states = next;
    }
    states[value.len()] // check-added-lines: allow(index) state vector has value.len() + 1 entries
}

pub fn pathname_component_match(
    ctx: &ThreadContext,
    pattern: Word,
    value: Word,
) -> Result<bool, ObjectError> {
    if pattern == Word::NIL {
        return Ok(value == Word::NIL);
    }
    if value == Word::NIL {
        return Ok(false);
    }
    Ok(wildcard_match(&text(ctx, pattern)?, &text(ctx, value)?))
}

pub fn pathname_match_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let pathname = pathname_designator(ctx, runtime, args.required(0)?)?;
    let pattern = pathname_designator(ctx, runtime, args.required(1)?)?;
    let directory = wildcard_match(
        &directory_text(ctx, structure_ref(ctx, pattern, 2)?)?,
        &directory_text(ctx, structure_ref(ctx, pathname, 2)?)?,
    );
    let matches = directory
        && (0..SLOTS)
            .filter(|index| *index != 2)
            .try_fold(true, |matched, index| {
                Ok::<_, ObjectError>(
                    matched
                        && pathname_component_match(
                            ctx,
                            structure_ref(ctx, pattern, index)?,
                            structure_ref(ctx, pathname, index)?,
                        )?,
                )
            })?;
    Ok(if matches { Word::TRUE } else { Word::NIL })
}

pub fn filesystem_path(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    value: Word,
) -> Result<(Word, String), ObjectError> {
    let pathname = pathname_designator(ctx, runtime, value)?;
    Ok((pathname, namestring_value(ctx, pathname)?))
}

pub fn probe_file_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let (pathname, name) = filesystem_path(ctx, runtime, args.required(0)?)?;
    Ok(if Path::new(&name).exists() {
        pathname
    } else {
        Word::NIL
    })
}

pub fn truename_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let (_, name) = filesystem_path(ctx, runtime, args.required(0)?)?;
    let canonical = fs::canonicalize(name).map_err(|_| ObjectError::TypeError)?;
    let string = make_string(
        ctx,
        runtime,
        &canonical.to_string_lossy().chars().collect::<Vec<_>>(),
    )?;
    parse_namestring_value(ctx, runtime, string)
}

pub fn file_length_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let (_, name) = filesystem_path(ctx, runtime, args.required(0)?)?;
    Ok(Word::fixnum(
        i64::try_from(
            fs::metadata(name)
                .map_err(|_| ObjectError::TypeError)?
                .len(),
        )
        .map_err(|_| ObjectError::Layout)?,
    ))
}

pub fn delete_file_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let (_, name) = filesystem_path(ctx, runtime, args.required(0)?)?;
    fs::remove_file(name).map_err(|_| ObjectError::TypeError)?;
    Ok(Word::TRUE)
}

pub fn rename_file_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let (old_pathname, old_name) = filesystem_path(ctx, runtime, args.required(0)?)?;
    let (new_pathname, new_name) = filesystem_path(ctx, runtime, args.required(1)?)?;
    fs::rename(old_name, new_name).map_err(|_| ObjectError::TypeError)?;
    values.set(&[new_pathname, old_pathname]);
    Ok(new_pathname)
}

pub fn ensure_directories_exist_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let pathname = pathname_designator(ctx, runtime, args.required(0)?)?;
    let name = namestring_value(ctx, pathname)?;
    let directory = if structure_ref(ctx, pathname, 3)? == Word::NIL {
        Path::new(&name)
    } else {
        Path::new(&name).parent().ok_or(ObjectError::TypeError)?
    };
    fs::create_dir_all(directory).map_err(|_| ObjectError::TypeError)?;
    Ok(pathname)
}

pub fn file_write_date_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let (_, name) = filesystem_path(ctx, runtime, args.required(0)?)?;
    let modified = fs::metadata(name)
        .map_err(|_| ObjectError::TypeError)?
        .modified()
        .map_err(|_| ObjectError::TypeError)?;
    let seconds = modified
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| ObjectError::TypeError)?
        .as_secs();
    Ok(Word::fixnum(
        i64::try_from(
            seconds
                .checked_add(2_208_988_800)
                .ok_or(ObjectError::Layout)?,
        )
        .map_err(|_| ObjectError::Layout)?,
    ))
}

pub fn file_author_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let (_, name) = filesystem_path(ctx, runtime, args.required(0)?)?;
    let _ = fs::metadata(name).map_err(|_| ObjectError::TypeError)?;
    Ok(Word::NIL)
}

pub fn file_error_pathname_builtin(
    _: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let _ = args.required(0)?;
    Ok(Word::NIL)
}

pub fn host_namestring_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let pathname = pathname_designator(ctx, runtime, args.required(0)?)?;
    make_string(
        ctx,
        runtime,
        &component_string(ctx, structure_ref(ctx, pathname, 0)?)?
            .chars()
            .collect::<Vec<_>>(),
    )
}

pub fn enough_namestring_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let pathname = pathname_designator(ctx, runtime, args.required(0)?)?;
    make_string(
        ctx,
        runtime,
        &namestring_value(ctx, pathname)?.chars().collect::<Vec<_>>(),
    )
}

pub fn wild_pathname_p_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let pathname = pathname_designator(ctx, runtime, args.required(0)?)?;
    let wild = (0..SLOTS).try_fold(false, |found, index| {
        Ok::<_, ObjectError>(found || has_wildcards(ctx, structure_ref(ctx, pathname, index)?)?)
    })?;
    Ok(if wild { Word::TRUE } else { Word::NIL })
}

pub fn merge_pathnames_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let pathname = pathname_designator(ctx, runtime, args.required(0)?)?;
    let defaults = if let Some(value) = args.get(1) {
        pathname_designator(ctx, runtime, value)?
    } else {
        Word::NIL
    };
    let mut slots = [Word::NIL; SLOTS];
    for (index, slot) in slots.iter_mut().enumerate() {
        let value = structure_ref(ctx, pathname, index)?;
        let use_default_directory = index == 2 && relative_directory(ctx, value)?;
        *slot = if (value == Word::NIL || use_default_directory) && defaults != Word::NIL {
            structure_ref(ctx, defaults, index)?
        } else {
            value
        };
    }
    make_pathname(ctx, runtime, &slots)
}

pub fn keyword_symbol(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
) -> Result<Word, ObjectError> {
    let package = runtime
        .find_package(ctx, "KEYWORD")
        .ok_or(ObjectError::PackageConflict)?;
    Ok(Package::from_word(package).intern(ctx, runtime, name)?.0)
}

pub fn list(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
) -> Result<Word, ObjectError> {
    let mut result = Word::NIL;
    for value in values.iter().rev().copied() {
        result = with_root(ctx, &mut result, |ctx, result| {
            make_cons(ctx, runtime, value, *result)
        })?;
    }
    Ok(result)
}

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
        _ => (Some(file), None), // check-added-lines: allow(wildcard) extension is optional
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
        let words = rooted.iter().map(|slot| slot.get()).collect::<Vec<_>>();
        list(ctx, runtime, &words)
    })?;
    let mut slots = [Word::NIL; SLOTS];
    // check-added-lines: allow(index) fixed pathname slot layout
    slots[2] = directory; // check-added-lines: allow(index) fixed pathname slot layout
    // check-added-lines: allow(index) fixed pathname slot layout
    slots[3] = match name {
        Some(value) => make_string(ctx, runtime, &value.chars().collect::<Vec<_>>())?,
        None => Word::NIL,
    };
    // check-added-lines: allow(index) fixed pathname slot layout
    slots[4] = match type_ {
        Some(value) => make_string(ctx, runtime, &value.chars().collect::<Vec<_>>())?,
        None => Word::NIL,
    };
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
