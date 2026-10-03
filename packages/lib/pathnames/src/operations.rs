use super::{
    BuiltinArgs, MultipleValues, ObjectError, Package, Runtime, SLOTS, ThreadContext, Word, car,
    cdr, component_string, directory_text, make_cons, make_pathname, make_string, namestring_value,
    pathname_designator, relative_directory, structure_ref, symbol_text, text, with_root,
};
use ncl_object::{FileError, LispError};
use std::env;
use std::fs;
use std::path::Path;

#[path = "parse.rs"]
mod parse;
pub use parse::{parse_namestring_builtin, parse_namestring_value};

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
    let (pathname, name) = filesystem_path(ctx, runtime, args.required(0)?)?;
    let canonical = fs::canonicalize(name).map_err(|_| {
        ctx.set_pending_lisp_error(LispError::FileError(FileError::InvalidPath { pathname }));
        ObjectError::TypeError
    })?;
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
    let (pathname, name) = filesystem_path(ctx, runtime, args.required(0)?)?;
    Ok(Word::fixnum(
        i64::try_from(
            fs::metadata(name)
                .map_err(|_| {
                    ctx.set_pending_lisp_error(LispError::FileError(FileError::InvalidPath {
                        pathname,
                    }));
                    ObjectError::TypeError
                })?
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
    let (pathname, name) = filesystem_path(ctx, runtime, args.required(0)?)?;
    fs::remove_file(name).map_err(|_| {
        ctx.set_pending_lisp_error(LispError::FileError(FileError::InvalidPath { pathname }));
        ObjectError::TypeError
    })?;
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

pub fn user_homedir_pathname_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    _: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let home = env::var("HOME").map_err(|_| ObjectError::TypeError)?;
    let value = make_string(ctx, runtime, &home.chars().collect::<Vec<_>>())?;
    parse_namestring_value(ctx, runtime, value)
}

pub fn directory_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let (pattern, name) = filesystem_path(ctx, runtime, args.required(0)?)?;
    let path = Path::new(&name);
    let parent = path.parent().unwrap_or_else(|| Path::new(".")); // check-added-lines: allow(panic) filesystem paths without a parent use current directory
    let file_pattern = path
        .file_name()
        .and_then(|part| part.to_str())
        .unwrap_or("*");
    let mut matches = Vec::new();
    for entry in fs::read_dir(parent).map_err(|_| ObjectError::TypeError)? {
        let entry = entry.map_err(|_| ObjectError::TypeError)?;
        let file_name = entry.file_name().to_string_lossy().into_owned();
        if wildcard_match(file_pattern, &file_name) {
            let found = make_string(
                ctx,
                runtime,
                &entry.path().to_string_lossy().chars().collect::<Vec<_>>(),
            )?;
            matches.push(parse_namestring_value(ctx, runtime, found)?);
        }
    }
    let _ = pattern;
    list(ctx, runtime, &matches)
}

fn replace_wildcard(pattern: &str, value: &str) -> String {
    if let Some(star) = pattern.find('*') {
        let prefix = &pattern[..star]; // check-added-lines: allow(index) star came from find
        let suffix = &pattern[star + 1..]; // check-added-lines: allow(index) star came from find
        if value.starts_with(prefix)
            && value.ends_with(suffix)
            && value.len() >= prefix.len() + suffix.len()
        {
            return format!(
                "{}{}{}",
                prefix,
                &value[prefix.len()..value.len() - suffix.len()], // check-added-lines: allow(index) validated prefix and suffix lengths
                suffix
            );
        }
    }
    value.to_owned()
}

fn wildcard_capture(pattern: &str, value: &str) -> Option<String> {
    let star = pattern.find('*')?;
    let prefix = &pattern[..star]; // check-added-lines: allow(index) star came from find
    let suffix = &pattern[star + 1..]; // check-added-lines: allow(index) star came from find
    (value.starts_with(prefix)
        && value.ends_with(suffix)
        && value.len() >= prefix.len() + suffix.len())
    .then(|| value[prefix.len()..value.len() - suffix.len()].to_owned()) // check-added-lines: allow(index) validated prefix and suffix lengths
}

fn substitute_wildcard(pattern: &str, capture: &str) -> String {
    pattern.find('*').map_or_else(
        || pattern.to_owned(),
        |index| format!("{}{}{}", &pattern[..index], capture, &pattern[index + 1..]), // check-added-lines: allow(index) index came from find
    )
}

pub fn translate_pathname_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let source = pathname_designator(ctx, runtime, args.required(0)?)?;
    let from = pathname_designator(ctx, runtime, args.required(1)?)?;
    let to = pathname_designator(ctx, runtime, args.required(2)?)?;
    let source_name = namestring_value(ctx, source)?;
    let from_name = namestring_value(ctx, from)?;
    let to_name = namestring_value(ctx, to)?;
    if !wildcard_match(&from_name, &source_name) {
        return Err(ObjectError::TypeError);
    }
    let translated = wildcard_capture(&from_name, &source_name).map_or_else(
        || replace_wildcard(&to_name, &source_name),
        |capture| substitute_wildcard(&to_name, &capture),
    );
    let string = make_string(ctx, runtime, &translated.chars().collect::<Vec<_>>())?;
    parse_namestring_value(ctx, runtime, string)
}

pub fn logical_pathname_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    parse_namestring_builtin(ctx, runtime, args, values)
}

pub fn logical_pathname_translations_builtin(
    _: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let _ = args.required(0)?;
    Ok(Word::NIL)
}

pub fn load_logical_pathname_translations_builtin(
    _: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let _ = args.required(0)?;
    Ok(Word::NIL)
}

pub fn translate_logical_pathname_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    logical_pathname_builtin(ctx, runtime, args, values)
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
