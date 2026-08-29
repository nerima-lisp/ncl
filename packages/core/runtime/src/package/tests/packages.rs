use crate::package::{COMMON_LISP_PACKAGE, DEFAULT_PACKAGE, KEYWORD_PACKAGE, PackageState};

#[test]
fn use_package_is_idempotent_and_unuse_package_reverses_it() {
    let mut state = PackageState::new();
    state.use_package(COMMON_LISP_PACKAGE, DEFAULT_PACKAGE);
    state.use_package(COMMON_LISP_PACKAGE, DEFAULT_PACKAGE);
    assert_eq!(
        state
            .use_packages_for(DEFAULT_PACKAGE)
            .iter()
            .filter(|name| *name == COMMON_LISP_PACKAGE)
            .count(),
        1,
        "using the same package twice must not duplicate the use-list entry"
    );

    state.unuse_package(COMMON_LISP_PACKAGE, DEFAULT_PACKAGE);
    assert!(
        !state
            .use_packages_for(DEFAULT_PACKAGE)
            .contains(&COMMON_LISP_PACKAGE.to_string())
    );
}

#[test]
fn define_package_rejects_a_nickname_that_collides_with_an_existing_package_name() {
    let mut state = PackageState::new();
    let result = state.define_package(
        "brand-new",
        vec![DEFAULT_PACKAGE.to_string()],
        Vec::new(),
        std::collections::HashSet::new(),
        None,
        std::collections::HashMap::new(),
    );
    assert!(result.is_err());
}

#[test]
fn define_package_rejects_a_name_that_conflicts_with_an_existing_nickname() {
    let mut state = PackageState::new();
    state
        .define_package(
            "first",
            vec!["alias".to_string()],
            Vec::new(),
            std::collections::HashSet::new(),
            None,
            std::collections::HashMap::new(),
        )
        .unwrap_or_else(|error| panic!("first define_package should succeed: {error}"));

    let result = state.define_package(
        "alias",
        Vec::new(),
        Vec::new(),
        std::collections::HashSet::new(),
        None,
        std::collections::HashMap::new(),
    );
    assert!(result.is_err());
}

#[test]
fn define_package_rejects_a_local_nickname_for_an_unknown_target() {
    let mut state = PackageState::new();
    let mut local_nicknames = std::collections::HashMap::new();
    local_nicknames.insert("short".to_string(), "no-such-package".to_string());

    let result = state.define_package(
        "with-bad-nickname",
        Vec::new(),
        Vec::new(),
        std::collections::HashSet::new(),
        None,
        local_nicknames,
    );
    assert!(result.is_err());
}

#[test]
fn canonical_package_name_resolves_through_a_global_nickname() {
    let mut state = PackageState::new();
    state
        .define_package(
            "long-package-name",
            vec!["short".to_string()],
            Vec::new(),
            std::collections::HashSet::new(),
            None,
            std::collections::HashMap::new(),
        )
        .unwrap_or_else(|error| panic!("define_package should succeed: {error}"));

    assert_eq!(state.canonical_package_name("short"), "LONG-PACKAGE-NAME");
}

#[test]
fn canonical_package_name_for_prefers_a_local_nickname_over_the_global_one() {
    let mut state = PackageState::new();
    state
        .define_package(
            "global-target",
            vec!["shared-alias".to_string()],
            Vec::new(),
            std::collections::HashSet::new(),
            None,
            std::collections::HashMap::new(),
        )
        .unwrap_or_else(|error| panic!("global-target define_package should succeed: {error}"));
    let mut local_nicknames = std::collections::HashMap::new();
    local_nicknames.insert("shared-alias".to_string(), DEFAULT_PACKAGE.to_string());
    state
        .define_package(
            "consumer",
            Vec::new(),
            Vec::new(),
            std::collections::HashSet::new(),
            None,
            local_nicknames,
        )
        .unwrap_or_else(|error| panic!("consumer define_package should succeed: {error}"));

    assert_eq!(
        state.canonical_package_name_for("consumer", "shared-alias"),
        DEFAULT_PACKAGE,
        "consumer's local nickname must shadow the global one"
    );
    assert_eq!(
        state.canonical_package_name("shared-alias"),
        "GLOBAL-TARGET",
        "the global nickname is unaffected by another package's local one"
    );
}

#[test]
fn common_lisp_and_keyword_packages_report_every_symbol_as_exported() {
    let state = PackageState::new();
    assert!(state.is_exported(COMMON_LISP_PACKAGE, "anything"));
    assert!(state.is_exported(KEYWORD_PACKAGE, "anything"));
}

#[test]
fn all_package_names_includes_the_three_built_in_packages_sorted() {
    let state = PackageState::new();
    let names = state.all_package_names();
    assert!(names.contains(&COMMON_LISP_PACKAGE.to_string()));
    assert!(names.contains(&DEFAULT_PACKAGE.to_string()));
    assert!(names.contains(&KEYWORD_PACKAGE.to_string()));
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(names, sorted);
}
