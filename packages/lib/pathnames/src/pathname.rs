use std::path::{Path, PathBuf};

use ncl_object::{
    BuiltinArgs, LispError, MultipleValues, ObjectError, ObjectRef, ObjectType, Package, Runtime,
    Scope, ThreadContext, Word, bignum_sign, classify_object, make_string, string_length,
    string_ref, symbol_name, symbol_package, symbol_value, with_root,
};

#[path = "pathname/construct.rs"]
mod construct;

pub use construct::make_pathname_builtin;

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
    pathname_from_path_with_version(ctx, runtime, path, Word::NIL)
}

pub fn pathname_from_path_with_version(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    path: &Path,
    version: Word,
) -> Result<Word, ObjectError> {
    let mut scope = Scope::new(ctx);
    let version = scope.root::<Word>(ncl_object::Local::from_word(version));
    let namestring = make_string(
        scope.context_mut(),
        runtime,
        &path.to_string_lossy().chars().collect::<Vec<_>>(),
    )?;
    let namestring = scope.root::<Word>(ncl_object::Local::from_word(namestring));
    let class = pathname_class(scope.context_mut(), runtime)?;
    let class = scope.root::<Word>(ncl_object::Local::from_word(class));
    let namestring = scope.get(namestring).as_word();
    let version = scope.get(version).as_word();
    let class = scope.get(class).as_word();
    ncl_clos::make_instance(scope.context_mut(), runtime, class, &[namestring, version])
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

pub fn pathname_version_from_designator(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    word: Word,
) -> Result<Word, ObjectError> {
    let mut word = word;
    with_root(ctx, &mut word, |ctx, word| {
        match classify_object(ctx, word.get()) {
            ObjectRef::Instance(_) => {
                let instance = ncl_object::Instance::from_word(word.get());
                if crate::logical::is_logical_pathname(ctx, runtime, word.get())? {
                    return ncl_object::slot_ref(ctx, instance, 1);
                }
                let mut class = pathname_class(ctx, runtime)?;
                with_root(ctx, &mut class, |ctx, class| {
                    if ncl_object::instance_class(ctx, instance)? != class.get() {
                        return Err(type_error(ctx, word.get(), ObjectType::Instance));
                    }
                    ncl_object::slot_ref(ctx, instance, 1)
                })
            }
            ObjectRef::String(_) => {
                path_from_word(ctx, runtime, word.get())?;
                Ok(Word::NIL)
            }
            _ => Err(type_error(ctx, word.get(), ObjectType::String)), // check-added-lines: allow(wildcard) reject objects outside the pathname designator types
        }
    })
}

pub fn valid_pathname_version(
    ctx: &ThreadContext,
    runtime: &Runtime,
    word: Word,
) -> Result<Word, ObjectError> {
    if word == Word::NIL {
        return Ok(word);
    }

    match classify_object(ctx, word) {
        ObjectRef::Fixnum(value) if value >= 0 => Ok(word),
        ObjectRef::Bignum(value) if !bignum_sign(ctx, ncl_object::Bignum::from_word(value))? => {
            Ok(word)
        }
        ObjectRef::Symbol(_) => match keyword_name(ctx, runtime, word)?.as_deref() {
            Some("WILD" | "UNSPECIFIC" | "NEWEST") => Ok(word),
            _ => Err(ObjectError::TypeError), // check-added-lines: allow(wildcard) reject unsupported pathname versions
        },
        _ => Err(ObjectError::TypeError), // check-added-lines: allow(wildcard) reject unsupported pathname version objects
    }
}

const fn type_error(ctx: &mut ThreadContext, word: Word, expected: ObjectType) -> ObjectError {
    ctx.set_pending_lisp_error(LispError::TypeError {
        datum: word,
        expected,
    });
    ObjectError::TypeError
}

pub fn string_to_rust(ctx: &ThreadContext, word: Word) -> Result<String, ObjectError> {
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

fn keyword_word(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
) -> Result<Word, ObjectError> {
    let package = runtime
        .find_package(ctx, "KEYWORD")
        .ok_or(ObjectError::Layout)?;
    let mut package = package;
    with_root(ctx, &mut package, |ctx, package| {
        let (symbol, _) = Package::from_word(package.get()).intern(ctx, runtime, name)?;
        Ok(symbol)
    })
}

pub fn default_pathname(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
) -> Result<PathBuf, ObjectError> {
    let value = current_default_word(ctx, runtime)?;
    if value == Word::NIL {
        return Ok(with_trailing_separator(
            std::env::current_dir().map_err(|_| ObjectError::Layout)?,
        ));
    }
    path_from_word(ctx, runtime, value)
}

pub fn base_directory(path: &Path) -> PathBuf {
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

pub fn keyword_name(
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
        if crate::logical::is_logical_pathname(ctx, runtime, word)? {
            return Ok(word);
        }
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
        if crate::logical::is_logical_pathname(ctx, runtime, word)?
            || ncl_object::instance_class(ctx, ncl_object::Instance::from_word(word))? == class
        {
            Word::TRUE
        } else {
            Word::NIL
        },
    )
}

pub fn merge_pathnames_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    if args.len() > 3 {
        return Err(ObjectError::TypeError);
    }
    let mut scope = Scope::new(ctx);
    let path_word = scope.root::<Word>(ncl_object::Local::from_word(args.required(0)?));
    let defaults_word = if let Some(word) = args.get(1) {
        word
    } else {
        current_default_word(scope.context_mut(), runtime)?
    };
    let defaults_word = scope.root::<Word>(ncl_object::Local::from_word(defaults_word));
    let path_word_value = scope.get(path_word).as_word();
    let path = path_from_word(scope.context_mut(), runtime, path_word_value)?;
    let defaults_word_value = scope.get(defaults_word).as_word();
    let defaults = if defaults_word_value == Word::NIL {
        default_pathname(scope.context_mut(), runtime)?
    } else {
        path_from_word(scope.context_mut(), runtime, defaults_word_value)?
    };
    let path_version =
        pathname_version_from_designator(scope.context_mut(), runtime, path_word_value)?;
    let path_version = scope.root::<Word>(ncl_object::Local::from_word(path_version));
    let defaults_version = if defaults_word_value == Word::NIL {
        Word::NIL
    } else {
        pathname_version_from_designator(scope.context_mut(), runtime, defaults_word_value)?
    };
    let defaults_version = scope.root::<Word>(ncl_object::Local::from_word(defaults_version));
    let requested_version = if let Some(word) = args.get(2) {
        valid_pathname_version(scope.context(), runtime, word)?
    } else {
        keyword_word(scope.context_mut(), runtime, "NEWEST")?
    };
    let requested_version = scope.root::<Word>(ncl_object::Local::from_word(requested_version));
    let version = if scope.get(path_version).as_word() != Word::NIL {
        scope.get(path_version).as_word()
    } else if path_name(&path).is_none() && scope.get(defaults_version).as_word() != Word::NIL {
        scope.get(defaults_version).as_word()
    } else {
        scope.get(requested_version).as_word()
    };
    let merged = merge_paths(path, &defaults);
    pathname_from_path_with_version(scope.context_mut(), runtime, &merged, version)
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

pub fn pathname_version_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    pathname_version_from_designator(ctx, runtime, args.required(0)?)
}

pub fn namestring_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let pathspec = args.required(0)?;
    if crate::logical::is_logical_pathname(ctx, runtime, pathspec)? {
        return ncl_object::slot_ref(ctx, ncl_object::Instance::from_word(pathspec), 0);
    }
    let path = path_from_word(ctx, runtime, pathspec)?;
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

pub fn path_name(path: &Path) -> Option<String> {
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

pub fn path_type(path: &Path) -> Option<String> {
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
