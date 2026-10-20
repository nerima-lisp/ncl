use std::path::{Component, PathBuf};

use super::super::{
    delete_file_builtin, directory_builtin, file_write_date_builtin, pathname_directory_builtin,
    truename_builtin,
};
use super::{TestDirectory, runtime_and_context};
use ncl_object::{
    BuiltinArgs, Local, MultipleValues, ObjectRef, Scope, ThreadContext, Word, classify_object,
    string_length, string_ref, symbol_name,
};

fn rust_string(ctx: &ThreadContext, word: Word) -> String {
    let length =
        string_length(ctx, word).unwrap_or_else(|error| panic!("string length: {error:?}"));
    (0..length)
        .map(|index| {
            string_ref(ctx, word, index)
                .unwrap_or_else(|error| panic!("string character: {error:?}"))
        })
        .collect()
}

fn create_files() -> (TestDirectory, PathBuf, PathBuf) {
    let directory =
        TestDirectory::new().unwrap_or_else(|error| panic!("temporary directory: {error}"));
    let alpha = directory.0.join("alpha");
    let beta = directory.0.join("beta");
    std::fs::create_dir(&alpha).unwrap_or_else(|error| panic!("alpha directory: {error}"));
    std::fs::create_dir(&beta).unwrap_or_else(|error| panic!("beta directory: {error}"));
    let first_file = alpha.join("file1.lisp");
    let second_file = beta.join("file2.lisp");
    std::fs::write(&first_file, "first").unwrap_or_else(|error| panic!("first file: {error}"));
    std::fs::write(&second_file, "second").unwrap_or_else(|error| panic!("second file: {error}"));
    (directory, first_file, second_file)
}

#[test]
fn directory_returns_sorted_matches_for_nested_wildcards() {
    let (directory, first_file, second_file) = create_files();
    let (runtime, mut ctx) = runtime_and_context();
    let mut scope = Scope::new(&mut ctx);
    let pattern = directory.0.join("*").join("file?.lisp");
    let pattern = scope
        .make_string(
            &runtime,
            &pattern.to_string_lossy().chars().collect::<Vec<_>>(),
        )
        .unwrap_or_else(|error| panic!("directory pattern: {error:?}"));
    let pattern_args = [scope.get(pattern).as_word()];
    let mut values = MultipleValues::new();
    let matches = directory_builtin(
        scope.context_mut(),
        &runtime,
        &BuiltinArgs::new(&pattern_args),
        &mut values,
    )
    .unwrap_or_else(|error| panic!("DIRECTORY: {error:?}"));
    let matches = scope.root::<Word>(Local::from_word(matches));
    let matches_word = scope.get(matches).as_word();
    let match_handles = scope
        .list_to_handle_vec(Local::from_word(matches_word))
        .unwrap_or_else(|error| panic!("DIRECTORY result list: {error:?}"));
    let actual_paths = match_handles
        .iter()
        .map(|handle| {
            let word = scope.get(*handle).as_word();
            crate::pathname::path_from_word(scope.context_mut(), &runtime, word)
        })
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|error| panic!("DIRECTORY pathname: {error:?}"));
    assert_eq!(actual_paths, [first_file, second_file]);
}

#[test]
fn pathname_directory_returns_absolute_mode_and_parent_components() {
    let (_directory, first_file, _second_file) = create_files();
    let (runtime, mut ctx) = runtime_and_context();
    let mut scope = Scope::new(&mut ctx);
    let mut values = MultipleValues::new();
    let pathname = crate::pathname::pathname_from_path(scope.context_mut(), &runtime, &first_file)
        .unwrap_or_else(|error| panic!("pathname: {error:?}"));
    let pathname = scope.root::<Word>(Local::from_word(pathname));
    let pathname_args = [scope.get(pathname).as_word()];
    let components = pathname_directory_builtin(
        scope.context_mut(),
        &runtime,
        &BuiltinArgs::new(&pathname_args),
        &mut values,
    )
    .unwrap_or_else(|error| panic!("PATHNAME-DIRECTORY: {error:?}"));
    let components = scope.root::<Word>(Local::from_word(components));
    let components_word = scope.get(components).as_word();
    let component_handles = scope
        .list_to_handle_vec(Local::from_word(components_word))
        .unwrap_or_else(|error| panic!("pathname directory list: {error:?}"));
    let actual_components = component_handles
        .as_slice()
        .iter()
        .map(|handle| {
            let value = scope.get(*handle).as_word();
            let name = match classify_object(scope.context(), value) {
                ObjectRef::Symbol(_) => symbol_name(scope.context(), value)
                    .unwrap_or_else(|error| panic!("directory mode name: {error:?}")),
                _ => value,
            };
            rust_string(scope.context(), name)
        })
        .collect::<Vec<_>>();
    let expected_components = first_file
        .parent()
        .unwrap_or_else(|| panic!("file has no parent directory"))
        .components()
        .filter_map(|component| match component {
            Component::Normal(name) => Some(name.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        actual_components,
        std::iter::once("ABSOLUTE".to_owned())
            .chain(expected_components)
            .collect::<Vec<_>>()
    );
}

#[test]
fn truename_returns_the_canonical_existing_path() {
    let (_directory, first_file, _second_file) = create_files();
    let (runtime, mut ctx) = runtime_and_context();
    let mut scope = Scope::new(&mut ctx);
    let mut values = MultipleValues::new();
    let pathname = crate::pathname::pathname_from_path(scope.context_mut(), &runtime, &first_file)
        .unwrap_or_else(|error| panic!("pathname: {error:?}"));
    let pathname = scope.root::<Word>(Local::from_word(pathname));
    let file_args = [scope.get(pathname).as_word()];
    let truename = truename_builtin(
        scope.context_mut(),
        &runtime,
        &BuiltinArgs::new(&file_args),
        &mut values,
    )
    .unwrap_or_else(|error| panic!("TRUENAME: {error:?}"));
    assert_eq!(
        crate::pathname::path_from_word(scope.context_mut(), &runtime, truename)
            .unwrap_or_else(|error| panic!("truename pathname: {error:?}")),
        std::fs::canonicalize(first_file)
            .unwrap_or_else(|error| panic!("canonical first file: {error}"))
    );
}

#[test]
fn file_write_date_returns_the_file_time_in_universal_time() {
    let (_directory, first_file, _second_file) = create_files();
    let (runtime, mut ctx) = runtime_and_context();
    let mut scope = Scope::new(&mut ctx);
    let mut values = MultipleValues::new();
    let modified = std::fs::metadata(&first_file)
        .unwrap_or_else(|error| panic!("file metadata: {error}"))
        .modified()
        .unwrap_or_else(|error| panic!("modified time: {error}"));
    let unix_seconds = modified
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_else(|error| panic!("modified before Unix epoch: {error}"))
        .as_secs();
    let expected = i64::try_from(unix_seconds)
        .unwrap_or_else(|error| panic!("modified timestamp out of range: {error}"))
        + 2_208_988_800;
    let pathname = crate::pathname::pathname_from_path(scope.context_mut(), &runtime, &first_file)
        .unwrap_or_else(|error| panic!("pathname: {error:?}"));
    let pathname = scope.root::<Word>(Local::from_word(pathname));
    let file_args = [scope.get(pathname).as_word()];
    let write_date = file_write_date_builtin(
        scope.context_mut(),
        &runtime,
        &BuiltinArgs::new(&file_args),
        &mut values,
    )
    .unwrap_or_else(|error| panic!("FILE-WRITE-DATE: {error:?}"));
    assert_eq!(write_date.as_fixnum(), Some(expected));
}

#[test]
fn delete_file_removes_the_path_and_file_write_date_rejects_it() {
    let (_directory, first_file, _second_file) = create_files();
    let (runtime, mut ctx) = runtime_and_context();
    let mut scope = Scope::new(&mut ctx);
    let mut values = MultipleValues::new();
    let pathname = crate::pathname::pathname_from_path(scope.context_mut(), &runtime, &first_file)
        .unwrap_or_else(|error| panic!("pathname: {error:?}"));
    let pathname = scope.root::<Word>(Local::from_word(pathname));
    let file_args = [scope.get(pathname).as_word()];
    assert_eq!(
        delete_file_builtin(
            scope.context_mut(),
            &runtime,
            &BuiltinArgs::new(&file_args),
            &mut values,
        ),
        Ok(Word::TRUE)
    );
    assert!(!first_file.exists());
    assert_eq!(
        file_write_date_builtin(
            scope.context_mut(),
            &runtime,
            &BuiltinArgs::new(&file_args),
            &mut values,
        ),
        Err(ncl_object::ObjectError::TypeError)
    );
}
