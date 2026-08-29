use super::{
    COMMON_LISP_PACKAGE, DEFAULT_PACKAGE, KEYWORD_PACKAGE, PackageState, SymbolStatus,
    canonical_symbol_name, normalize_package_name, normalize_symbol_name, split_symbol,
};

#[test]
fn normalize_package_name_strips_colon_prefix_uppercases_and_maps_cl_alias() {
    assert_eq!(normalize_package_name("cl-user"), "CL-USER");
    assert_eq!(normalize_package_name(":cl-user"), "CL-USER");
    assert_eq!(normalize_package_name("cl"), COMMON_LISP_PACKAGE);
    assert_eq!(normalize_package_name("CL"), COMMON_LISP_PACKAGE);
}

#[test]
fn normalize_symbol_name_uppercases() {
    assert_eq!(normalize_symbol_name("Foo"), "FOO");
}

#[test]
fn canonical_symbol_name_omits_the_default_package_prefix() {
    assert_eq!(canonical_symbol_name(DEFAULT_PACKAGE, "thing"), "THING");
    assert_eq!(
        canonical_symbol_name("other-package", "thing"),
        "OTHER-PACKAGE::THING"
    );
}

#[test]
fn split_symbol_distinguishes_internal_and_external_qualifiers() {
    assert_eq!(
        split_symbol("pkg::name"),
        Some(("pkg", "name", false)),
        "double colon is an internal (unexported) reference"
    );
    assert_eq!(
        split_symbol("pkg:name"),
        Some(("pkg", "name", true)),
        "single colon is an external (exported) reference"
    );
    assert_eq!(split_symbol("bare-name"), None);
    assert_eq!(
        split_symbol("pkg:::name"),
        Some(("pkg", ":name", false)),
        "the first double colon wins over a later single colon"
    );
}

#[test]
fn intern_symbol_reports_status_and_rejects_unknown_packages() {
    let mut state = PackageState::new();
    assert_eq!(
        state.intern_symbol(DEFAULT_PACKAGE, "widget"),
        Some(SymbolStatus::Internal)
    );
    assert_eq!(state.intern_symbol("no-such-package", "widget"), None);
}

#[test]
fn interning_into_the_keyword_package_is_always_external() {
    let mut state = PackageState::new();
    assert_eq!(
        state.intern_symbol(KEYWORD_PACKAGE, "widget"),
        Some(SymbolStatus::External)
    );
}

#[test]
fn export_symbols_promotes_an_already_interned_symbol_to_external() {
    let mut state = PackageState::new();
    state.intern_symbol(DEFAULT_PACKAGE, "widget");
    assert_eq!(
        state.symbol_status(DEFAULT_PACKAGE, "widget"),
        Some(SymbolStatus::Internal)
    );

    state.export_symbols(DEFAULT_PACKAGE, &["widget".to_string()]);
    assert_eq!(
        state.symbol_status(DEFAULT_PACKAGE, "widget"),
        Some(SymbolStatus::External)
    );

    state.unexport_symbols(DEFAULT_PACKAGE, &["widget".to_string()]);
    assert_eq!(
        state.symbol_status(DEFAULT_PACKAGE, "widget"),
        Some(SymbolStatus::Internal)
    );
}

#[test]
fn shadowing_replaces_an_imported_symbol_and_clears_its_import_record() {
    let mut state = PackageState::new();
    state
        .define_package(
            "source",
            Vec::new(),
            Vec::new(),
            std::iter::once("shared".to_string()).collect(),
            None,
            std::collections::HashMap::new(),
        )
        .unwrap_or_else(|error| panic!("define_package should succeed: {error}"));
    state.export_symbols("source", &["shared".to_string()]);
    state.import_symbol("source", "shared", DEFAULT_PACKAGE, false);

    assert_eq!(
        state.imported_symbol_for(DEFAULT_PACKAGE, "shared"),
        Some("SOURCE::SHARED".to_string()),
        "importing without shadowing records where the symbol came from"
    );
    assert!(!state.is_shadowed(DEFAULT_PACKAGE, "shared"));

    state.shadow_symbol(DEFAULT_PACKAGE, "shared");

    assert!(
        state.is_shadowed(DEFAULT_PACKAGE, "shared"),
        "shadow_symbol must mark the name as shadowed"
    );
    assert_eq!(
        state.imported_symbol_for(DEFAULT_PACKAGE, "shared"),
        None,
        "shadowing a symbol clears its prior import record so it resolves locally"
    );
}

#[test]
fn unintern_removes_from_every_symbol_table_it_could_be_in() {
    let mut state = PackageState::new();

    // A symbol can simultaneously be present, exported, imported, and
    // shadowed; unintern_symbol must clear it from all four sets, not just
    // the first one it happens to check.
    state.intern_symbol(DEFAULT_PACKAGE, "multi");
    state.export_symbols(DEFAULT_PACKAGE, &["multi".to_string()]);
    state.import_symbol(COMMON_LISP_PACKAGE, "multi", DEFAULT_PACKAGE, true);

    assert!(state.symbol_exists(DEFAULT_PACKAGE, "multi"));
    assert!(state.is_exported(DEFAULT_PACKAGE, "multi"));
    assert!(state.is_shadowed(DEFAULT_PACKAGE, "multi"));
    assert!(
        state
            .imported_symbol_for(DEFAULT_PACKAGE, "multi")
            .is_some()
    );

    assert!(state.unintern_symbol(DEFAULT_PACKAGE, "multi"));

    assert!(!state.symbol_exists(DEFAULT_PACKAGE, "multi"));
    assert!(!state.is_exported(DEFAULT_PACKAGE, "multi"));
    assert!(!state.is_shadowed(DEFAULT_PACKAGE, "multi"));
    assert!(
        state
            .imported_symbol_for(DEFAULT_PACKAGE, "multi")
            .is_none()
    );
}

#[test]
fn unintern_of_a_name_never_interned_reports_no_removal() {
    let mut state = PackageState::new();
    assert!(!state.unintern_symbol(DEFAULT_PACKAGE, "never-there"));
}

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
