use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;

use ncl_object::{
    BuiltinArgs, FileError, LispError, MultipleValues, ObjectError, Runtime, Scope, ThreadContext,
    Word, make_cons, with_root, with_rooted_slice,
};

use crate::pathname::{path_from_word, pathname_from_path, resolve_pathname};

pub fn probe_file_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let path = resolve_pathname(ctx, runtime, args.required(0)?)?;
    if !path.exists() {
        return Ok(Word::NIL);
    }
    let truename = std::fs::canonicalize(path).map_err(|_| ObjectError::TypeError)?;
    pathname_from_path(ctx, runtime, &truename)
}

pub fn truename_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let path = resolve_pathname(ctx, runtime, args.required(0)?)?;
    let truename = canonical_path(&path).map_err(|error| {
        let condition = match error.kind() {
            std::io::ErrorKind::NotFound => FileError::NotFound,
            std::io::ErrorKind::PermissionDenied => FileError::PermissionDenied,
            _ => FileError::InvalidPath, // check-added-lines: allow(wildcard) map remaining filesystem errors to invalid path
        };
        ctx.set_pending_lisp_error(LispError::FileError(condition));
        ObjectError::TypeError
    })?;
    pathname_from_path(ctx, runtime, &truename)
}

fn canonical_path(path: &Path) -> Result<PathBuf, std::io::Error> {
    std::fs::canonicalize(path)
}

pub fn directory_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let pattern = resolve_pathname(ctx, runtime, args.required(0)?)?;
    let mut matches = Vec::new();
    expand_pattern(&pattern, &mut matches).map_err(|_| ObjectError::TypeError)?;
    matches.sort();
    with_rooted_slice(ctx, &[Word::NIL], |ctx, roots| {
        for path in matches.iter().rev() {
            let pathname = pathname_from_path(ctx, runtime, path)?;
            let mut pathname = pathname;
            with_root(ctx, &mut pathname, |ctx, pathname| {
                let Some(root) = roots.first_mut() else {
                    return Err(ObjectError::Layout);
                };
                *root = make_cons(ctx, runtime, pathname.get(), *root)?;
                Ok(())
            })?;
        }
        roots.first().copied().ok_or(ObjectError::Layout)
    })
}

pub fn pathname_directory_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let path = path_from_word(ctx, runtime, args.required(0)?)?;
    let namestring = path.to_string_lossy();
    let directory = if namestring.ends_with(std::path::MAIN_SEPARATOR) {
        path.as_path()
    } else {
        path.parent().map_or_else(|| Path::new(""), |parent| parent)
    };
    let components = directory.components().collect::<Vec<_>>();
    let absolute = directory.is_absolute();
    build_directory_list(ctx, runtime, &components, absolute)
}

fn build_directory_list(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    components: &[Component<'_>],
    absolute: bool,
) -> Result<Word, ObjectError> {
    let mut scope = Scope::new(ctx);
    let mut list = scope.root::<Word>(ncl_object::Local::from_word(Word::NIL));
    for component in components.iter().rev() {
        let value = match component {
            Component::Normal(value) => {
                let value = value.to_string_lossy();
                scope.make_string(runtime, &value.chars().collect::<Vec<_>>())?
            }
            Component::ParentDir => scope.intern(runtime, "KEYWORD", "UP")?,
            Component::CurDir | Component::RootDir | Component::Prefix(_) => continue,
        };
        list = scope.make_cons(runtime, value, list)?;
    }
    let marker = if absolute { "ABSOLUTE" } else { "RELATIVE" };
    let marker = scope.intern(runtime, "KEYWORD", marker)?;
    list = scope.make_cons(runtime, marker, list)?;
    Ok(scope.get(list).as_word())
}

fn expand_pattern(pattern: &Path, matches: &mut Vec<PathBuf>) -> std::io::Result<()> {
    let mut current = if pattern.is_absolute() {
        PathBuf::from(std::path::MAIN_SEPARATOR.to_string())
    } else {
        PathBuf::from(".")
    };
    let components = pattern
        .components()
        .filter_map(|component| match component {
            Component::RootDir | Component::Prefix(_) => None,
            Component::Normal(value) => Some(value.to_string_lossy().into_owned()),
            Component::ParentDir => Some("..".to_owned()),
            Component::CurDir => Some(".".to_owned()),
        })
        .collect::<Vec<_>>();
    expand_pattern_components(&components, 0, &mut current, matches)
}

fn expand_pattern_components(
    components: &[String],
    index: usize,
    current: &mut PathBuf,
    matches: &mut Vec<PathBuf>,
) -> std::io::Result<()> {
    let Some(component) = components.get(index) else {
        if current.exists() {
            matches.push(current.clone());
        }
        return Ok(());
    };
    let is_last = index + 1 == components.len();
    if component.contains('*') || component.contains('?') {
        let entries = match std::fs::read_dir(&*current) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error),
        };
        for entry in entries {
            let entry = entry?;
            if !wildcard_match(component, &entry.file_name().to_string_lossy()) {
                continue;
            }
            let mut path = entry.path();
            if is_last {
                matches.push(path);
            } else if path.is_dir() {
                expand_pattern_components(components, index + 1, &mut path, matches)?;
            }
        }
        return Ok(());
    }
    current.push(component);
    if is_last {
        if current.exists() {
            matches.push(current.clone());
        }
        current.pop();
        return Ok(());
    }
    if current.is_dir() {
        expand_pattern_components(components, index + 1, current, matches)?;
    }
    current.pop();
    Ok(())
}

fn wildcard_match(pattern: &str, value: &str) -> bool {
    let pattern = pattern.chars().collect::<Vec<_>>();
    let value = value.chars().collect::<Vec<_>>();
    let mut previous = vec![false; value.len() + 1];
    if let Some(first) = previous.first_mut() {
        *first = true;
    }
    for character in pattern {
        let mut current =
            vec![character == '*' && previous.first().is_some_and(|matched| *matched)];
        for (index, value_character) in value.iter().enumerate() {
            let diagonal = previous.get(index).is_some_and(|matched| *matched);
            let above = previous.get(index + 1).is_some_and(|matched| *matched);
            let left = current.last().is_some_and(|matched| *matched);
            current.push(match character {
                '*' => above || left,
                '?' => diagonal,
                literal => diagonal && literal == *value_character,
            });
        }
        previous = current;
    }
    previous.last().is_some_and(|matched| *matched)
}

pub fn delete_file_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let path = resolve_pathname(ctx, runtime, args.required(0)?)?;
    std::fs::remove_file(path).map_err(|_| ObjectError::TypeError)?;
    Ok(Word::TRUE)
}

pub fn file_write_date_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let path = resolve_pathname(ctx, runtime, args.required(0)?)?;
    let modified = std::fs::metadata(path)
        .map_err(|_| ObjectError::TypeError)?
        .modified()
        .map_err(|_| ObjectError::TypeError)?;
    let unix_seconds = modified
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ObjectError::TypeError)?
        .as_secs();
    let lisp_seconds = i64::try_from(unix_seconds)
        .map_err(|_| ObjectError::TypeError)?
        .saturating_add(2_208_988_800);
    Ok(Word::fixnum(lisp_seconds))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::{canonical_path, expand_pattern, probe_file_builtin, wildcard_match};
    use crate::pathname::pathname_from_path;
    use ncl_object::{
        BuiltinArgs, Local, MultipleValues, Package, Runtime, Scope, ThreadContext, Word,
        set_symbol_value,
    };

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Result<Self, std::io::Error> {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            loop {
                let index = NEXT.fetch_add(1, Ordering::Relaxed);
                let path = std::env::temp_dir()
                    .join(format!("ncl-pathnames-{}-{index}", std::process::id()));
                match std::fs::create_dir(&path) {
                    Ok(()) => return Ok(Self(path)),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(error) => return Err(error),
                }
            }
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _cleanup = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn canonical_path_resolves_an_existing_path() -> Result<(), std::io::Error> {
        let path = std::env::current_dir()?;
        assert_eq!(canonical_path(&path)?, std::fs::canonicalize(path)?);
        Ok(())
    }

    #[test]
    fn wildcard_match_handles_stars_questions_and_unicode() {
        assert!(wildcard_match("*", ""));
        assert!(wildcard_match("prefix*suffix", "prefix-middle-suffix"));
        assert!(wildcard_match("file?.lisp", "file2.lisp"));
        assert!(!wildcard_match("file?.lisp", "file12.lisp"));
        assert!(wildcard_match("λ?.lisp", "λ語.lisp"));
        assert!(!wildcard_match("file.lisp", "files.lisp"));
    }

    #[test]
    fn expand_pattern_matches_nested_wildcards_and_ignores_missing_paths()
    -> Result<(), std::io::Error> {
        let directory = TestDirectory::new()?;
        let alpha = directory.0.join("alpha");
        let beta = directory.0.join("beta");
        std::fs::create_dir(&alpha)?;
        std::fs::create_dir(&beta)?;
        std::fs::write(alpha.join("file1.lisp"), "")?;
        std::fs::write(beta.join("file2.lisp"), "")?;
        std::fs::write(directory.0.join("not-a-directory"), "")?;

        let mut matches = Vec::new();
        expand_pattern(&directory.0.join("*").join("file?.lisp"), &mut matches)?;
        matches.sort();
        assert_eq!(matches, [alpha.join("file1.lisp"), beta.join("file2.lisp")]);

        let mut literal_match = Vec::new();
        expand_pattern(&alpha.join("file1.lisp"), &mut literal_match)?;
        assert_eq!(literal_match, [alpha.join("file1.lisp")]);

        let mut missing_matches = Vec::new();
        expand_pattern(
            &directory.0.join("missing-directory").join("file?.lisp"),
            &mut missing_matches,
        )?;
        assert!(missing_matches.is_empty());
        expand_pattern(
            &directory.0.join("*").join("missing?.lisp"),
            &mut missing_matches,
        )?;
        assert!(missing_matches.is_empty());
        Ok(())
    }

    #[test]
    fn probe_file_uses_parent_of_directory_defaults_without_separator() {
        let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)
            .unwrap_or_else(|error| panic!("thread: {error:?}"));
        ncl_clos::register(&runtime).unwrap_or_else(|error| panic!("CLOS: {error:?}"));
        crate::register(&mut ctx, &runtime).unwrap_or_else(|error| panic!("pathnames: {error:?}"));

        let directory =
            std::env::current_dir().unwrap_or_else(|error| panic!("current directory: {error}"));
        assert!(directory.is_dir());
        let basename = directory
            .file_name()
            .unwrap_or_else(|| panic!("current directory has no basename"))
            .to_string_lossy()
            .into_owned();
        let mut scope = Scope::new(&mut ctx);
        let defaults = pathname_from_path(scope.context_mut(), &runtime, &directory)
            .unwrap_or_else(|error| panic!("pathname defaults: {error:?}"));
        let defaults = scope.root::<Word>(Local::from_word(defaults));
        let common = runtime
            .find_package(scope.context_mut(), "COMMON-LISP")
            .unwrap_or_else(|| panic!("COMMON-LISP package is missing"));
        let (defaults_symbol, _) = Package::from_word(common)
            .intern(scope.context_mut(), &runtime, "*DEFAULT-PATHNAME-DEFAULTS*")
            .unwrap_or_else(|error| panic!("defaults symbol: {error:?}"));
        let defaults_word = scope.get(defaults).as_word();
        set_symbol_value(scope.context_mut(), defaults_symbol, defaults_word)
            .unwrap_or_else(|error| panic!("set defaults: {error:?}"));
        let relative_name = scope
            .make_string(&runtime, &basename.chars().collect::<Vec<_>>())
            .unwrap_or_else(|error| panic!("relative name: {error:?}"));
        let arguments = [scope.get(relative_name).as_word()];
        let mut values = MultipleValues::new();

        let result = probe_file_builtin(
            scope.context_mut(),
            &runtime,
            &BuiltinArgs::new(&arguments),
            &mut values,
        )
        .unwrap_or_else(|error| panic!("PROBE-FILE: {error:?}"));

        assert_ne!(result, Word::NIL);
    }
}
