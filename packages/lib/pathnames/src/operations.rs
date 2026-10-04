use super::{
    BuiltinArgs, MultipleValues, ObjectError, Package, Runtime, SLOTS, ThreadContext, Word, car,
    cdr, component_string, make_cons, make_pathname, make_string, namestring_value,
    pathname_designator, relative_directory, structure_ref, symbol_text, text, with_root,
};
use ncl_object::{FileError, LispError};
use std::env;
use std::fs;
use std::path::Path;

use super::wildcard::translate_wildcards;

pub use super::logical::{
    compile_file_pathname_builtin, load_logical_pathname_translations_builtin,
    logical_pathname_builtin, logical_pathname_translations_builtin,
    translate_logical_pathname_builtin,
};

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
        return Ok(value.contains('*')
            || value.contains('?')
            || value.eq_ignore_ascii_case("WILD")
            || value.eq_ignore_ascii_case("WILD-INFERIORS"));
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
        return Ok(true);
    }
    if value == Word::NIL {
        return Ok(false);
    }
    if let Ok(name) = symbol_text(ctx, pattern) {
        if name.eq_ignore_ascii_case("WILD") || name.eq_ignore_ascii_case("WILD-INFERIORS") {
            return Ok(true);
        }
        if name.eq_ignore_ascii_case("NEWEST") {
            return Ok(true);
        }
        if name.eq_ignore_ascii_case("UNSPECIFIC") {
            return Ok(
                symbol_text(ctx, value).is_ok_and(|value| value.eq_ignore_ascii_case("UNSPECIFIC"))
            );
        }
    }
    if let Ok(name) = symbol_text(ctx, value)
        && name.eq_ignore_ascii_case("UNSPECIFIC")
    {
        return Ok(false);
    }
    if let (Ok(pattern), Ok(value)) = (symbol_text(ctx, pattern), symbol_text(ctx, value)) {
        return Ok(pattern.eq_ignore_ascii_case(&value));
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
    let pattern_directory = structure_ref(ctx, pattern, 2)?;
    let directory = directory_match(ctx, pattern_directory, structure_ref(ctx, pathname, 2)?)?;
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
    let full = namestring_value(ctx, pathname)?;
    let result = if let Some(defaults) = args.get(1) {
        let defaults = pathname_designator(ctx, runtime, defaults)?;
        let base = namestring_value(ctx, defaults)?;
        full.strip_prefix(&base)
            .unwrap_or(&full)
            .trim_start_matches('/')
            .to_owned()
    } else {
        full
    };
    make_string(ctx, runtime, &result.chars().collect::<Vec<_>>())
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
    let translated =
        translate_wildcards(&from_name, &to_name, &source_name).ok_or(ObjectError::TypeError)?;
    let string = make_string(ctx, runtime, &translated.chars().collect::<Vec<_>>())?;
    parse_namestring_value(ctx, runtime, string)
}

pub fn wild_pathname_p_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let pathname = pathname_designator(ctx, runtime, args.required(0)?)?;
    let wild = if let Some(field) = args.get(1) {
        let field = super::keyword_name(ctx, field)?;
        let index = match field.as_str() {
            "HOST" => 0,
            "DEVICE" => 1,
            "DIRECTORY" | "WILD-INFERIORS" => 2,
            "NAME" => 3,
            "TYPE" => 4,
            "VERSION" => 5,
            _ => return Err(ObjectError::TypeError), // check-added-lines: allow(wildcard) reject unknown field keys
        };
        has_wildcards(ctx, structure_ref(ctx, pathname, index)?)?
    } else {
        (0..SLOTS).try_fold(false, |found, index| {
            Ok::<_, ObjectError>(found || has_wildcards(ctx, structure_ref(ctx, pathname, index)?)?)
        })?
    };
    Ok(if wild { Word::TRUE } else { Word::NIL })
}

fn directory_parts(ctx: &ThreadContext, directory: Word) -> Result<Vec<Word>, ObjectError> {
    let mut parts = Vec::new();
    let mut cursor = directory;
    while cursor != Word::NIL {
        parts.push(car(ctx, cursor)?);
        cursor = cdr(ctx, cursor)?;
    }
    Ok(parts)
}

fn is_symbol(ctx: &ThreadContext, word: Word, name: &str) -> bool {
    symbol_text(ctx, word).is_ok_and(|value| value.eq_ignore_ascii_case(name))
}

fn directory_match(ctx: &ThreadContext, pattern: Word, value: Word) -> Result<bool, ObjectError> {
    if pattern == Word::NIL {
        return Ok(true);
    }
    let pattern = directory_parts(ctx, pattern)?;
    let value = directory_parts(ctx, value)?;
    fn match_parts(
        ctx: &ThreadContext,
        pattern: &[Word],
        value: &[Word],
    ) -> Result<bool, ObjectError> {
        if pattern.is_empty() {
            return Ok(value.is_empty());
        }
        // check-added-lines: allow(index) pattern is nonempty here
        if is_symbol(ctx, pattern[0], "WILD-INFERIORS") {
            // check-added-lines: allow(index) pattern is nonempty here
            // check-added-lines: allow(index) pattern is nonempty here
            // check-added-lines: allow(index) recursive slices are bounded by their lengths
            for consumed in 0..=value.len() {
                // check-added-lines: allow(index) inclusive bound is value.len()
                if match_parts(ctx, &pattern[1..], &value[consumed..])? {
                    // check-added-lines: allow(index) consumed is bounded by value.len()
                    // check-added-lines: allow(index) consumed is bounded by value.len()
                    return Ok(true);
                }
            }
            return Ok(false);
        }
        // check-added-lines: allow(index) value nonempty guard protects index zero
        if value.is_empty() || !pathname_component_match(ctx, pattern[0], value[0])? {
            // check-added-lines: allow(index) value nonempty guard protects index zero
            // check-added-lines: allow(index) value nonempty guard protects index zero
            // check-added-lines: allow(index) nonempty guard protects index zero
            return Ok(false);
        }
        match_parts(ctx, &pattern[1..], &value[1..]) // check-added-lines: allow(index) nonempty guards protect slice starts
    }
    match_parts(ctx, &pattern, &value)
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
            if use_default_directory {
                let base = structure_ref(ctx, defaults, index)?;
                append_relative_directory(ctx, runtime, base, value)?
            } else {
                structure_ref(ctx, defaults, index)?
            }
        } else {
            value
        };
    }
    make_pathname(ctx, runtime, &slots)
}

fn append_relative_directory(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    base: Word,
    relative: Word,
) -> Result<Word, ObjectError> {
    let mut values = Vec::new();
    let mut cursor = base;
    while cursor != Word::NIL {
        values.push(car(ctx, cursor)?);
        cursor = cdr(ctx, cursor)?;
    }
    let mut tail = relative;
    let mut relative_values = Vec::new();
    while tail != Word::NIL {
        relative_values.push(car(ctx, tail)?);
        tail = cdr(ctx, tail)?;
    }
    if !relative_values.is_empty() {
        let _ = relative_values.remove(0);
    }
    values.extend(relative_values);
    list(ctx, runtime, &values)
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
