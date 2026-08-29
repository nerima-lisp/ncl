use crate::package::{
    COMMON_LISP_PACKAGE, DEFAULT_PACKAGE, KEYWORD_PACKAGE, PackageState, SymbolStatus,
};

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
