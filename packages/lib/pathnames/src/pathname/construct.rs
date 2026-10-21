use std::path::{Path, PathBuf};

use ncl_object::{
    BuiltinArgs, MultipleValues, ObjectError, ObjectRef, Runtime, ThreadContext, Word, car, cdr,
    classify_object, with_root,
};

use super::{
    base_directory, default_pathname, keyword_name, path_from_word, path_name, path_type,
    pathname_from_path_with_version, pathname_version_from_designator, string_to_rust,
    valid_pathname_version, with_trailing_separator,
};

pub fn make_pathname_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    if !args.len().is_multiple_of(2) {
        return Err(ObjectError::TypeError);
    }
    let mut options = std::collections::HashMap::new();
    for index in (0..args.len()).step_by(2) {
        let key = keyword_name(ctx, runtime, args.get(index).ok_or(ObjectError::Layout)?)?
            .ok_or(ObjectError::TypeError)?;
        if !matches!(
            key.as_str(),
            "CASE" | "DEFAULTS" | "DIRECTORY" | "NAME" | "TYPE" | "VERSION"
        ) {
            return Err(ObjectError::TypeError);
        }
        options.entry(key).or_insert(index + 1);
    }
    let common_case = if let Some(index) = options.get("CASE") {
        let case = keyword_name(ctx, runtime, args.get(*index).ok_or(ObjectError::Layout)?)?
            .ok_or(ObjectError::TypeError)?;
        match case.as_str() {
            "LOCAL" => false,
            "COMMON" => true,
            _ => return Err(ObjectError::TypeError), // check-added-lines: allow(wildcard) reject unsupported CASE values
        }
    } else {
        false
    };
    let version = if let Some(index) = options.get("VERSION") {
        valid_pathname_version(ctx, runtime, args.get(*index).ok_or(ObjectError::Layout)?)?
    } else if let Some(index) = options.get("DEFAULTS") {
        pathname_version_from_designator(
            ctx,
            runtime,
            args.get(*index).ok_or(ObjectError::Layout)?,
        )?
    } else {
        Word::NIL
    };

    let defaults = if let Some(index) = options.get("DEFAULTS") {
        path_from_word(ctx, runtime, args.get(*index).ok_or(ObjectError::Layout)?)?
    } else {
        default_pathname(ctx, runtime)?
    };
    let directory = if let Some(index) = options.get("DIRECTORY") {
        directory_from_word(
            ctx,
            runtime,
            args.get(*index).ok_or(ObjectError::Layout)?,
            &defaults,
            common_case,
        )?
    } else {
        base_directory(&defaults)
    };
    let default_name = path_name(&defaults);
    let default_type = path_type(&defaults);
    let name = match options
        .get("NAME")
        .map(|index| args.get(*index).ok_or(ObjectError::Layout))
        .transpose()?
    {
        Some(word) => component_string(ctx, runtime, word)?
            .map(|component| common_case_to_local(component, common_case)),
        None => default_name,
    };
    let kind = match options
        .get("TYPE")
        .map(|index| args.get(*index).ok_or(ObjectError::Layout))
        .transpose()?
    {
        Some(word) => component_string(ctx, runtime, word)?
            .map(|component| common_case_to_local(component, common_case)),
        None => default_type,
    };
    let mut path = directory;
    if let Some(name) = name {
        let filename = match kind {
            Some(kind) => format!("{name}.{kind}"),
            None => name,
        };
        path.push(filename);
    } else if path != Path::new("/") {
        path = with_trailing_separator(path);
    }
    pathname_from_path_with_version(ctx, runtime, &path, version)
}

fn common_case_to_local(component: String, common_case: bool) -> String {
    if !common_case {
        return component;
    }

    let has_uppercase = component.chars().any(char::is_uppercase);
    let has_lowercase = component.chars().any(char::is_lowercase);
    match (has_uppercase, has_lowercase) {
        (true, false) => component.to_lowercase(),
        (false, true) => component.to_uppercase(),
        _ => component, // check-added-lines: allow(wildcard) mixed-case components are preserved
    }
}

pub(super) fn component_string(
    ctx: &ThreadContext,
    runtime: &Runtime,
    word: Word,
) -> Result<Option<String>, ObjectError> {
    if word == Word::NIL {
        return Ok(None);
    }
    if let Some(name) = keyword_name(ctx, runtime, word)? {
        return match name.as_str() {
            "WILD" => Ok(Some("*".to_owned())),
            "UNSPECIFIC" => Ok(None),
            _ => Err(ObjectError::TypeError), // check-added-lines: allow(wildcard) reject unsupported pathname keywords
        };
    }
    if matches!(classify_object(ctx, word), ObjectRef::String(_)) {
        return Ok(Some(string_to_rust(ctx, word)?));
    }
    Err(ObjectError::TypeError)
}

pub(super) fn directory_from_word(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    word: Word,
    defaults: &Path,
    common_case: bool,
) -> Result<PathBuf, ObjectError> {
    if word == Word::NIL {
        return Ok(base_directory(defaults));
    }

    if keyword_name(ctx, runtime, word)?.as_deref() == Some("WILD") {
        return Ok(PathBuf::from(std::path::MAIN_SEPARATOR.to_string()).join("*"));
    }

    let mut initial_cursor = word;
    let (mode, mut cursor) = with_root(ctx, &mut initial_cursor, |ctx, cursor| {
        let mut first = car(ctx, cursor.get())?;
        let mode = with_root(ctx, &mut first, |ctx, first| {
            keyword_name(ctx, runtime, first.get())?.ok_or(ObjectError::TypeError)
        })?;
        Ok((mode, cdr(ctx, cursor.get())?))
    })?;
    let mut path = match mode.as_str() {
        "ABSOLUTE" => PathBuf::from(std::path::MAIN_SEPARATOR.to_string()),
        "RELATIVE" => base_directory(defaults),
        _ => return Err(ObjectError::TypeError), // check-added-lines: allow(wildcard) reject unsupported directory modes
    };

    while cursor != Word::NIL {
        let (component, next_cursor) = with_root(ctx, &mut cursor, |ctx, cursor| {
            let mut item = car(ctx, cursor.get())?;
            let component = with_root(ctx, &mut item, |ctx, item| {
                if let Some(keyword) = keyword_name(ctx, runtime, item.get())? {
                    match keyword.as_str() {
                        "UP" | "BACK" => Ok(PathBuf::from("..")),
                        "WILD" => Ok(PathBuf::from("*")),
                        "WILD-INFERIORS" => Ok(PathBuf::from("**")),
                        _ => Err(ObjectError::TypeError), // check-added-lines: allow(wildcard) reject unsupported directory keywords
                    }
                } else if matches!(classify_object(ctx, item.get()), ObjectRef::String(_)) {
                    let component = string_to_rust(ctx, item.get())?;
                    Ok(PathBuf::from(common_case_to_local(component, common_case)))
                } else {
                    Err(ObjectError::TypeError)
                }
            })?;
            Ok((component, cdr(ctx, cursor.get())?))
        })?;
        path.push(component);
        cursor = next_cursor;
    }
    Ok(path)
}
