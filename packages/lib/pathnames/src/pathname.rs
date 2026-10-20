use std::path::{Path, PathBuf};

use ncl_object::{
    BuiltinArgs, LispError, MultipleValues, ObjectError, ObjectRef, ObjectType, Package, Runtime,
    Scope, ThreadContext, Word, car, cdr, classify_object, make_string, string_length, string_ref,
    symbol_name, symbol_package, symbol_value, with_root,
};

/// Convert a host path to a Lisp `PATHNAME` instance.
///
/// # Errors
///
/// Returns an error if the runtime has no `PATHNAME` class or cannot allocate the instance.
pub fn from_host_path(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    path: &Path,
) -> Result<Word, ObjectError> {
    pathname_from_path(ctx, runtime, path)
}

/// Resolve a string or `PATHNAME` instance against the current pathname defaults.
///
/// # Errors
///
/// Returns an error if the designator or current pathname defaults are invalid.
pub fn resolve_pathname(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    designator: Word,
) -> Result<PathBuf, ObjectError> {
    let path = path_from_word(ctx, runtime, designator)?;
    if path.is_absolute() {
        return Ok(path);
    }
    let defaults = default_pathname(ctx, runtime)?;
    Ok(merge_paths(path, &defaults))
}

fn pathname_class(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<Word, ObjectError> {
    runtime.class(ctx, "PATHNAME").ok_or(ObjectError::Layout)
}

pub fn pathname_from_path(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    path: &Path,
) -> Result<Word, ObjectError> {
    let mut scope = Scope::new(ctx);
    let namestring = make_string(
        scope.context_mut(),
        runtime,
        &path.to_string_lossy().chars().collect::<Vec<_>>(),
    )?;
    let namestring = scope.root::<Word>(ncl_object::Local::from_word(namestring));
    let class = pathname_class(scope.context_mut(), runtime)?;
    let class = scope.root::<Word>(ncl_object::Local::from_word(class));
    let namestring = scope.get(namestring).as_word();
    let class = scope.get(class).as_word();
    ncl_clos::make_instance(scope.context_mut(), runtime, class, &[namestring])
}

pub fn path_from_word(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    word: Word,
) -> Result<PathBuf, ObjectError> {
    let mut word = word;
    with_root(ctx, &mut word, |ctx, word| {
        match classify_object(ctx, word.get()) {
            ObjectRef::String(_) => Ok(PathBuf::from(string_to_rust(ctx, word.get())?)),
            ObjectRef::Instance(_) => {
                let mut class = pathname_class(ctx, runtime)?;
                with_root(ctx, &mut class, |ctx, class| {
                    let instance = ncl_object::Instance::from_word(word.get());
                    if ncl_object::instance_class(ctx, instance)? != class.get() {
                        return Err(type_error(ctx, word.get(), ObjectType::Instance));
                    }
                    let namestring = ncl_object::slot_ref(ctx, instance, 0)?;
                    Ok(PathBuf::from(string_to_rust(ctx, namestring)?))
                })
            }
            _ => Err(type_error(ctx, word.get(), ObjectType::String)), // check-added-lines: allow(wildcard) only strings and PATHNAME instances are accepted
        }
    })
}

const fn type_error(ctx: &mut ThreadContext, word: Word, expected: ObjectType) -> ObjectError {
    ctx.set_pending_lisp_error(LispError::TypeError {
        datum: word,
        expected,
    });
    ObjectError::TypeError
}

fn string_to_rust(ctx: &ThreadContext, word: Word) -> Result<String, ObjectError> {
    let length = string_length(ctx, word)?;
    let mut value = String::with_capacity(length);
    for index in 0..length {
        value.push(string_ref(ctx, word, index)?);
    }
    Ok(value)
}

fn string_word(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    value: &str,
) -> Result<Word, ObjectError> {
    make_string(ctx, runtime, &value.chars().collect::<Vec<_>>())
}

fn current_default_word(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<Word, ObjectError> {
    let package = runtime
        .find_package(ctx, "COMMON-LISP")
        .ok_or(ObjectError::Layout)?;
    let mut package = package;
    with_root(ctx, &mut package, |ctx, package| {
        let (symbol, _) = Package::from_word(package.get()).intern(
            ctx,
            runtime,
            "*DEFAULT-PATHNAME-DEFAULTS*",
        )?;
        symbol_value(ctx, symbol)
    })
}

fn default_pathname(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<PathBuf, ObjectError> {
    let value = current_default_word(ctx, runtime)?;
    if value == Word::NIL {
        return Ok(with_trailing_separator(
            std::env::current_dir().map_err(|_| ObjectError::Layout)?,
        ));
    }
    path_from_word(ctx, runtime, value)
}

fn base_directory(path: &Path) -> PathBuf {
    if path.to_string_lossy().ends_with(std::path::MAIN_SEPARATOR) {
        path.to_path_buf()
    } else {
        path.parent()
            .map_or_else(|| Path::new(".").to_path_buf(), Path::to_path_buf)
    }
}

fn merge_paths(path: PathBuf, defaults: &Path) -> PathBuf {
    let inherits_type =
        path.extension().is_none() && !path.to_string_lossy().ends_with(std::path::MAIN_SEPARATOR);
    let merged = if path.is_absolute() {
        path
    } else {
        base_directory(defaults).join(path)
    };
    if inherits_type && let Some(extension) = defaults.extension() {
        return merged.with_extension(extension);
    }
    merged
}

pub fn with_trailing_separator(path: PathBuf) -> PathBuf {
    if path.to_string_lossy().ends_with(std::path::MAIN_SEPARATOR) {
        path
    } else {
        PathBuf::from(format!(
            "{}{sep}",
            path.display(),
            sep = std::path::MAIN_SEPARATOR
        ))
    }
}

fn keyword_name(
    ctx: &ThreadContext,
    runtime: &Runtime,
    word: Word,
) -> Result<Option<String>, ObjectError> {
    if !matches!(classify_object(ctx, word), ObjectRef::Symbol(_)) {
        return Ok(None);
    }
    let Some(keyword) = runtime.find_package(ctx, "KEYWORD") else {
        return Ok(None);
    };
    if symbol_package(ctx, word)? != keyword {
        return Ok(None);
    }
    Ok(Some(string_to_rust(ctx, symbol_name(ctx, word)?)?))
}

pub fn pathname_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let word = args.required(0)?;
    if matches!(classify_object(ctx, word), ObjectRef::Instance(_)) {
        let class = pathname_class(ctx, runtime)?;
        if ncl_object::instance_class(ctx, ncl_object::Instance::from_word(word))? == class {
            return Ok(word);
        }
    }
    let path = path_from_word(ctx, runtime, word)?;
    pathname_from_path(ctx, runtime, &path)
}

pub fn pathnamep_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let word = args.required(0)?;
    if !matches!(classify_object(ctx, word), ObjectRef::Instance(_)) {
        return Ok(Word::NIL);
    }
    let class = pathname_class(ctx, runtime)?;
    Ok(
        if ncl_object::instance_class(ctx, ncl_object::Instance::from_word(word))? == class {
            Word::TRUE
        } else {
            Word::NIL
        },
    )
}

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
            "DEFAULTS" | "DIRECTORY" | "NAME" | "TYPE" | "VERSION"
        ) {
            return Err(ObjectError::TypeError);
        }
        options.entry(key).or_insert(index + 1);
    }
    if let Some(index) = options.get("VERSION")
        && args.get(*index) != Some(Word::NIL)
    {
        return Err(ObjectError::TypeError);
    }

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
        Some(word) => component_string(ctx, runtime, word)?,
        None => default_name,
    };
    let kind = match options
        .get("TYPE")
        .map(|index| args.get(*index).ok_or(ObjectError::Layout))
        .transpose()?
    {
        Some(word) => component_string(ctx, runtime, word)?,
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
    pathname_from_path(ctx, runtime, &path)
}

fn component_string(
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

fn directory_from_word(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    word: Word,
    defaults: &Path,
) -> Result<PathBuf, ObjectError> {
    if word == Word::NIL {
        return Ok(base_directory(defaults));
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
                        _ => Err(ObjectError::TypeError), // check-added-lines: allow(wildcard) reject unsupported directory keywords
                    }
                } else if matches!(classify_object(ctx, item.get()), ObjectRef::String(_)) {
                    Ok(PathBuf::from(string_to_rust(ctx, item.get())?))
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

pub fn merge_pathnames_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    if args.len() > 2 {
        return Err(ObjectError::TypeError);
    }
    let path = path_from_word(ctx, runtime, args.required(0)?)?;
    let defaults = if let Some(word) = args.get(1) {
        path_from_word(ctx, runtime, word)?
    } else {
        default_pathname(ctx, runtime)?
    };
    pathname_from_path(ctx, runtime, &merge_paths(path, &defaults))
}

pub fn pathname_name_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let path = path_from_word(ctx, runtime, args.required(0)?)?;
    path_name(&path).map_or(Ok(Word::NIL), |name| string_word(ctx, runtime, &name))
}

pub fn pathname_type_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let path = path_from_word(ctx, runtime, args.required(0)?)?;
    path_type(&path).map_or(Ok(Word::NIL), |kind| string_word(ctx, runtime, &kind))
}

pub fn namestring_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let path = path_from_word(ctx, runtime, args.required(0)?)?;
    string_word(ctx, runtime, &path.to_string_lossy())
}

pub fn compile_file_pathname_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let mut path = resolve_pathname(ctx, runtime, args.required(0)?)?;
    path.set_extension("fasl");
    pathname_from_path(ctx, runtime, &path)
}

fn path_name(path: &Path) -> Option<String> {
    let namestring = path.to_string_lossy();
    if namestring.ends_with(std::path::MAIN_SEPARATOR) {
        return None;
    }
    let filename = path.file_name()?.to_string_lossy();
    Some(match path.extension() {
        Some(extension) => {
            let extension = extension.to_string_lossy();
            filename
                .strip_suffix(&format!(".{extension}"))
                .unwrap_or(&filename)
                .to_owned()
        }
        None => filename.into_owned(),
    })
}

fn path_type(path: &Path) -> Option<String> {
    let namestring = path.to_string_lossy();
    if namestring.ends_with(std::path::MAIN_SEPARATOR) {
        return None;
    }
    path.extension()
        .map(|extension| extension.to_string_lossy().into_owned())
}

#[cfg(test)]
#[path = "pathname/tests_pathname.rs"]
mod tests;
