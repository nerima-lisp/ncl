use super::construct::component_string;
use super::construct::directory_from_word;
use super::{
    base_directory, make_pathname_builtin, merge_paths, path_from_word, path_name, path_type,
    with_trailing_separator,
};
use ncl_object::{
    BuiltinArgs, Local, MultipleValues, ObjectError, Runtime, Scope, ThreadContext, Word,
};

fn runtime_and_context() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("thread: {error:?}"));
    ncl_clos::register(&runtime).unwrap_or_else(|error| panic!("CLOS: {error:?}"));
    crate::register(&mut ctx, &runtime).unwrap_or_else(|error| panic!("pathnames: {error:?}"));
    (runtime, ctx)
}

fn make_pathname_name_with_case(case: &str, name: &str) -> std::path::PathBuf {
    let (runtime, mut ctx) = runtime_and_context();
    let mut scope = Scope::new(&mut ctx);
    let case_keyword = scope
        .intern(&runtime, "KEYWORD", "CASE")
        .unwrap_or_else(|error| panic!("CASE keyword: {error:?}"));
    let case_value = scope
        .intern(&runtime, "KEYWORD", case)
        .unwrap_or_else(|error| panic!("case value: {error:?}"));
    let name_keyword = scope
        .intern(&runtime, "KEYWORD", "NAME")
        .unwrap_or_else(|error| panic!("NAME keyword: {error:?}"));
    let name = scope
        .make_string(&runtime, &name.chars().collect::<Vec<_>>())
        .unwrap_or_else(|error| panic!("name: {error:?}"));
    let arguments = [
        scope.get(case_keyword).as_word(),
        scope.get(case_value).as_word(),
        scope.get(name_keyword).as_word(),
        scope.get(name).as_word(),
    ];
    let mut values = MultipleValues::new();

    let pathname = make_pathname_builtin(
        scope.context_mut(),
        &runtime,
        &BuiltinArgs::new(&arguments),
        &mut values,
    )
    .unwrap_or_else(|error| panic!("MAKE-PATHNAME: {error:?}"));
    let pathname = scope.root::<Word>(Local::from_word(pathname));
    let pathname_word = scope.get(pathname).as_word();
    path_from_word(scope.context_mut(), &runtime, pathname_word)
        .unwrap_or_else(|error| panic!("pathname conversion: {error:?}"))
}

#[test]
fn base_directory_uses_path_syntax_for_an_existing_directory() {
    let directory =
        std::env::current_dir().unwrap_or_else(|error| panic!("current directory: {error}"));
    assert!(directory.is_dir());
    let parent = directory
        .parent()
        .unwrap_or_else(|| panic!("current directory has no parent"));

    assert_eq!(base_directory(&directory), parent);
}

#[test]
fn pathname_components_and_merging_preserve_explicit_parts() {
    let defaults = std::path::Path::new("/defaults/base.lisp");

    assert_eq!(
        path_name(std::path::Path::new("file.tar.gz")),
        Some("file.tar".into())
    );
    assert_eq!(
        path_type(std::path::Path::new("file.tar.gz")),
        Some("gz".into())
    );
    assert_eq!(
        path_name(std::path::Path::new("README")),
        Some("README".into())
    );
    assert_eq!(path_type(std::path::Path::new("README")), None);
    assert_eq!(path_name(std::path::Path::new("directory/")), None);
    assert_eq!(path_type(std::path::Path::new("directory/")), None);

    assert_eq!(
        merge_paths("relative".into(), defaults),
        std::path::Path::new("/defaults/relative.lisp")
    );
    assert_eq!(
        merge_paths("explicit.txt".into(), defaults),
        std::path::Path::new("/defaults/explicit.txt")
    );
    assert_eq!(
        merge_paths("directory/".into(), defaults),
        std::path::Path::new("/defaults/directory/")
    );

    assert_eq!(
        with_trailing_separator("directory".into()),
        std::path::Path::new("directory/")
    );
    assert_eq!(
        with_trailing_separator("directory/".into()),
        std::path::Path::new("directory/")
    );
}

#[test]
fn directory_components_apply_modes_and_parent_markers() {
    let (runtime, mut ctx) = runtime_and_context();
    let mut scope = Scope::new(&mut ctx);
    let mode = scope
        .intern(&runtime, "KEYWORD", "ABSOLUTE")
        .unwrap_or_else(|error| panic!("mode: {error:?}"));
    let child = scope
        .make_string(&runtime, &['c', 'h', 'i', 'l', 'd'])
        .unwrap_or_else(|error| panic!("child: {error:?}"));
    let up = scope
        .intern(&runtime, "KEYWORD", "UP")
        .unwrap_or_else(|error| panic!("UP keyword: {error:?}"));
    let wild = scope
        .intern(&runtime, "KEYWORD", "WILD")
        .unwrap_or_else(|error| panic!("WILD keyword: {error:?}"));
    let tail = scope.root::<Word>(Local::from_word(Word::NIL));
    let tail = scope
        .make_cons(&runtime, wild, tail)
        .unwrap_or_else(|error| panic!("wild tail: {error:?}"));
    let tail = scope
        .make_cons(&runtime, up, tail)
        .unwrap_or_else(|error| panic!("UP tail: {error:?}"));
    let tail = scope
        .make_cons(&runtime, child, tail)
        .unwrap_or_else(|error| panic!("child tail: {error:?}"));
    let directory = scope
        .make_cons(&runtime, mode, tail)
        .unwrap_or_else(|error| panic!("directory: {error:?}"));
    let directory_word = scope.get(directory).as_word();
    let path = directory_from_word(
        scope.context_mut(),
        &runtime,
        directory_word,
        std::path::Path::new("/defaults/base.lisp"),
        false,
    )
    .unwrap_or_else(|error| panic!("directory conversion: {error:?}"));

    assert_eq!(path, std::path::Path::new("/child/../*"));

    let relative_mode = scope
        .intern(&runtime, "KEYWORD", "RELATIVE")
        .unwrap_or_else(|error| panic!("RELATIVE keyword: {error:?}"));
    let relative = scope
        .make_cons(&runtime, relative_mode, tail)
        .unwrap_or_else(|error| panic!("relative directory: {error:?}"));
    let relative_word = scope.get(relative).as_word();
    let path = directory_from_word(
        scope.context_mut(),
        &runtime,
        relative_word,
        std::path::Path::new("/defaults/base.lisp"),
        false,
    )
    .unwrap_or_else(|error| panic!("relative conversion: {error:?}"));
    assert_eq!(path, std::path::Path::new("/defaults/child/../*"));

    assert_eq!(
        directory_from_word(
            scope.context_mut(),
            &runtime,
            Word::NIL,
            std::path::Path::new("/defaults/base.lisp"),
            false,
        )
        .unwrap_or_else(|error| panic!("nil directory: {error:?}")),
        std::path::Path::new("/defaults")
    );
}

#[test]
fn make_pathname_uses_the_leftmost_duplicate_keyword() {
    let (runtime, mut ctx) = runtime_and_context();
    let mut scope = Scope::new(&mut ctx);
    let name_keyword = scope
        .intern(&runtime, "KEYWORD", "NAME")
        .unwrap_or_else(|error| panic!("keyword: {error:?}"));
    let first_name = scope
        .make_string(&runtime, &['f', 'i', 'r', 's', 't'])
        .unwrap_or_else(|error| panic!("first name: {error:?}"));
    let second_name = scope
        .make_string(&runtime, &['s', 'e', 'c', 'o', 'n', 'd'])
        .unwrap_or_else(|error| panic!("second name: {error:?}"));
    let arguments = [
        scope.get(name_keyword).as_word(),
        scope.get(first_name).as_word(),
        scope.get(name_keyword).as_word(),
        scope.get(second_name).as_word(),
    ];
    let mut values = MultipleValues::new();

    let pathname = make_pathname_builtin(
        scope.context_mut(),
        &runtime,
        &BuiltinArgs::new(&arguments),
        &mut values,
    )
    .unwrap_or_else(|error| panic!("MAKE-PATHNAME: {error:?}"));
    let pathname = scope.root::<Word>(Local::from_word(pathname));
    let word = scope.get(pathname).as_word();
    let path = path_from_word(scope.context_mut(), &runtime, word)
        .unwrap_or_else(|error| panic!("pathname conversion: {error:?}"));

    assert_eq!(
        path.file_name().and_then(|name| name.to_str()),
        Some("first")
    );
}

#[test]
fn make_pathname_common_case_converts_uppercase_to_local_case() {
    assert_eq!(
        make_pathname_name_with_case("COMMON", "FOO")
            .file_name()
            .and_then(|name| name.to_str()),
        Some("foo")
    );
}

#[test]
fn make_pathname_common_case_preserves_mixed_and_inverts_lowercase() {
    let mixed = make_pathname_name_with_case("COMMON", "FoO");
    let lowercase = make_pathname_name_with_case("COMMON", "foo");

    assert_eq!(
        mixed.file_name().and_then(|name| name.to_str()),
        Some("FoO")
    );
    assert_eq!(
        lowercase.file_name().and_then(|name| name.to_str()),
        Some("FOO")
    );
}

#[test]
fn make_pathname_common_case_converts_directory_and_type() {
    let (runtime, mut ctx) = runtime_and_context();
    let mut scope = Scope::new(&mut ctx);
    let case_keyword = scope
        .intern(&runtime, "KEYWORD", "CASE")
        .unwrap_or_else(|error| panic!("CASE keyword: {error:?}"));
    let common_keyword = scope
        .intern(&runtime, "KEYWORD", "COMMON")
        .unwrap_or_else(|error| panic!("COMMON keyword: {error:?}"));
    let directory_keyword = scope
        .intern(&runtime, "KEYWORD", "DIRECTORY")
        .unwrap_or_else(|error| panic!("DIRECTORY keyword: {error:?}"));
    let absolute_keyword = scope
        .intern(&runtime, "KEYWORD", "ABSOLUTE")
        .unwrap_or_else(|error| panic!("ABSOLUTE keyword: {error:?}"));
    let name_keyword = scope
        .intern(&runtime, "KEYWORD", "NAME")
        .unwrap_or_else(|error| panic!("NAME keyword: {error:?}"));
    let type_keyword = scope
        .intern(&runtime, "KEYWORD", "TYPE")
        .unwrap_or_else(|error| panic!("TYPE keyword: {error:?}"));
    let directory_component = scope
        .make_string(&runtime, &['D', 'I', 'R'])
        .unwrap_or_else(|error| panic!("directory component: {error:?}"));
    let name = scope
        .make_string(&runtime, &['F', 'O', 'O'])
        .unwrap_or_else(|error| panic!("name: {error:?}"));
    let kind = scope
        .make_string(&runtime, &['T', 'X', 'T'])
        .unwrap_or_else(|error| panic!("type: {error:?}"));
    let directory_items = [
        Local::from_word(scope.get(absolute_keyword).as_word()),
        Local::from_word(scope.get(directory_component).as_word()),
    ];
    let directory_handles = scope.root_many(&directory_items);
    let directory = scope
        .make_list(&runtime, &directory_handles)
        .unwrap_or_else(|error| panic!("directory: {error:?}"));
    let arguments = [
        scope.get(case_keyword).as_word(),
        scope.get(common_keyword).as_word(),
        scope.get(directory_keyword).as_word(),
        scope.get(directory).as_word(),
        scope.get(name_keyword).as_word(),
        scope.get(name).as_word(),
        scope.get(type_keyword).as_word(),
        scope.get(kind).as_word(),
    ];
    let mut values = MultipleValues::new();

    let pathname = make_pathname_builtin(
        scope.context_mut(),
        &runtime,
        &BuiltinArgs::new(&arguments),
        &mut values,
    )
    .unwrap_or_else(|error| panic!("MAKE-PATHNAME: {error:?}"));
    let pathname = scope.root::<Word>(Local::from_word(pathname));
    let pathname_word = scope.get(pathname).as_word();
    let path = path_from_word(scope.context_mut(), &runtime, pathname_word)
        .unwrap_or_else(|error| panic!("pathname conversion: {error:?}"));

    assert_eq!(path, std::path::PathBuf::from("/dir/foo.txt"));
}

#[test]
fn make_pathname_local_case_preserves_component_case() {
    let path = make_pathname_name_with_case("LOCAL", "FOO");

    assert_eq!(path.file_name().and_then(|name| name.to_str()), Some("FOO"));
}

#[test]
fn component_string_rejects_unsupported_keywords() {
    let (runtime, mut ctx) = runtime_and_context();
    let mut scope = Scope::new(&mut ctx);
    let keyword = scope
        .intern(&runtime, "KEYWORD", "UNKNOWN-COMPONENT")
        .unwrap_or_else(|error| panic!("keyword: {error:?}"));

    assert!(matches!(
        component_string(scope.context(), &runtime, scope.get(keyword).as_word()),
        Err(ObjectError::TypeError)
    ));
}

#[test]
fn component_string_keeps_supported_keywords() {
    let (runtime, mut ctx) = runtime_and_context();
    let mut scope = Scope::new(&mut ctx);
    let wild = scope
        .intern(&runtime, "KEYWORD", "WILD")
        .unwrap_or_else(|error| panic!("WILD keyword: {error:?}"));
    let unspecified = scope
        .intern(&runtime, "KEYWORD", "UNSPECIFIC")
        .unwrap_or_else(|error| panic!("UNSPECIFIC keyword: {error:?}"));

    assert!(matches!(
        component_string(scope.context(), &runtime, scope.get(wild).as_word()),
        Ok(Some(value)) if value == "*"
    ));
    assert!(matches!(
        component_string(scope.context(), &runtime, scope.get(unspecified).as_word()),
        Ok(None)
    ));
}

#[test]
fn make_pathname_preserves_valid_version_components() {
    let (runtime, mut ctx) = runtime_and_context();
    let mut scope = Scope::new(&mut ctx);
    let version_keyword = scope
        .intern(&runtime, "KEYWORD", "VERSION")
        .unwrap_or_else(|error| panic!("VERSION keyword: {error:?}"));

    for name in ["WILD", "UNSPECIFIC", "NEWEST"] {
        let version = scope
            .intern(&runtime, "KEYWORD", name)
            .unwrap_or_else(|error| panic!("{name} version: {error:?}"));
        let expected = scope.get(version).as_word();
        let arguments = [scope.get(version_keyword).as_word(), expected];
        let mut values = MultipleValues::new();
        let pathname = make_pathname_builtin(
            scope.context_mut(),
            &runtime,
            &BuiltinArgs::new(&arguments),
            &mut values,
        )
        .unwrap_or_else(|error| panic!("MAKE-PATHNAME :VERSION {name}: {error:?}"));
        let pathname = scope.root::<Word>(Local::from_word(pathname));
        let pathname_word = scope.get(pathname).as_word();
        let version = ncl_object::slot_ref(
            scope.context(),
            ncl_object::Instance::from_word(pathname_word),
            1,
        )
        .unwrap_or_else(|error| panic!("PATHNAME version slot: {error:?}"));

        assert_eq!(version, expected);
    }

    let arguments = [scope.get(version_keyword).as_word(), Word::fixnum(1)];
    let mut values = MultipleValues::new();
    let pathname = make_pathname_builtin(
        scope.context_mut(),
        &runtime,
        &BuiltinArgs::new(&arguments),
        &mut values,
    )
    .unwrap_or_else(|error| panic!("MAKE-PATHNAME :VERSION 1: {error:?}"));
    let pathname = scope.root::<Word>(Local::from_word(pathname));
    let pathname_word = scope.get(pathname).as_word();
    let version = ncl_object::slot_ref(
        scope.context(),
        ncl_object::Instance::from_word(pathname_word),
        1,
    )
    .unwrap_or_else(|error| panic!("PATHNAME version slot: {error:?}"));

    assert_eq!(version, Word::fixnum(1));

    let arguments = [scope.get(version_keyword).as_word(), Word::fixnum(0)];
    let mut values = MultipleValues::new();
    let pathname = make_pathname_builtin(
        scope.context_mut(),
        &runtime,
        &BuiltinArgs::new(&arguments),
        &mut values,
    )
    .unwrap_or_else(|error| panic!("MAKE-PATHNAME :VERSION 0: {error:?}"));
    let pathname = scope.root::<Word>(Local::from_word(pathname));
    let pathname_word = scope.get(pathname).as_word();
    let version = ncl_object::slot_ref(
        scope.context(),
        ncl_object::Instance::from_word(pathname_word),
        1,
    )
    .unwrap_or_else(|error| panic!("PATHNAME version slot: {error:?}"));

    assert_eq!(version, Word::fixnum(0));
}

#[test]
fn make_pathname_rejects_invalid_version_components() {
    let (runtime, mut ctx) = runtime_and_context();
    let mut scope = Scope::new(&mut ctx);
    let version_keyword = scope
        .intern(&runtime, "KEYWORD", "VERSION")
        .unwrap_or_else(|error| panic!("VERSION keyword: {error:?}"));
    let invalid_keyword = scope
        .intern(&runtime, "KEYWORD", "NOT-A-VERSION")
        .unwrap_or_else(|error| panic!("invalid version: {error:?}"));

    for version in [Word::fixnum(-1), scope.get(invalid_keyword).as_word()] {
        let arguments = [scope.get(version_keyword).as_word(), version];
        let mut values = MultipleValues::new();
        assert!(matches!(
            make_pathname_builtin(
                scope.context_mut(),
                &runtime,
                &BuiltinArgs::new(&arguments),
                &mut values,
            ),
            Err(ObjectError::TypeError)
        ));
    }
}
