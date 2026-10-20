use super::{
    base_directory, component_string, directory_from_word, make_pathname_builtin, merge_paths,
    path_from_word, path_name, path_type, with_trailing_separator,
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
    )
    .unwrap_or_else(|error| panic!("relative conversion: {error:?}"));
    assert_eq!(path, std::path::Path::new("/defaults/child/../*"));

    assert_eq!(
        directory_from_word(
            scope.context_mut(),
            &runtime,
            Word::NIL,
            std::path::Path::new("/defaults/base.lisp")
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
