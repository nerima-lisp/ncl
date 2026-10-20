use super::{base_directory, component_string, make_pathname_builtin, path_from_word};
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
