use super::super::{DEFAULT_PACKAGE, PackageState};

fn define_package(
    state: &mut PackageState,
    name: &str,
    nicknames: &[&str],
    local_nicknames: &[(&str, &str)],
) {
    let local_nicknames = local_nicknames
        .iter()
        .map(|(nickname, target)| ((*nickname).to_string(), (*target).to_string()))
        .collect();
    state
        .define_package(
            name,
            nicknames.iter().map(|name| (*name).to_string()).collect(),
            Vec::new(),
            std::collections::HashSet::new(),
            None,
            local_nicknames,
        )
        .unwrap_or_else(|error| panic!("package definition should succeed: {error}"));
}

#[test]
fn rename_package_preserves_references_and_rewrites_symbol_imports() {
    let mut state = PackageState::new();
    define_package(&mut state, "rename-target", &["rename-alias"], &[]);
    define_package(
        &mut state,
        "rename-consumer",
        &[],
        &[("target", "rename-target")],
    );
    state.use_package("rename-target", "rename-consumer");
    state.import_symbol("rename-target", "item", "rename-consumer", false);
    state.set_current("rename-target");

    let object = state
        .package_object_for("rename-alias")
        .unwrap_or_else(|| panic!("the package object should exist before renaming"));
    let renamed = state
        .rename_package(
            "rename-alias",
            "rename-new",
            vec!["rename-new-alias".to_string()],
        )
        .unwrap_or_else(|error| panic!("package rename should succeed: {error}"));

    assert_eq!(renamed, "RENAME-NEW");
    assert_eq!(object.name().as_deref(), Some("RENAME-NEW"));
    assert!(
        object.ptr_eq(
            &state
                .package_object_for("rename-new-alias")
                .unwrap_or_else(|| panic!("the new nickname should resolve"))
        )
    );
    assert_eq!(state.current(), "RENAME-NEW");
    assert_eq!(
        state.use_packages_for("rename-consumer"),
        vec!["RENAME-NEW"]
    );
    assert_eq!(
        state.package_local_nicknames_for("rename-consumer"),
        vec![("TARGET".to_string(), "RENAME-NEW".to_string())]
    );
    assert_eq!(
        state.imported_symbol_for("rename-consumer", "item"),
        Some("RENAME-NEW::ITEM".to_string())
    );
    assert!(!state.package_exists("rename-alias"));
}

#[test]
fn rename_package_without_nicknames_clears_old_nicknames() {
    let mut state = PackageState::new();
    define_package(&mut state, "rename-target", &["rename-alias"], &[]);

    state
        .rename_package("rename-target", "rename-new", Vec::new())
        .unwrap_or_else(|error| panic!("package rename should succeed: {error}"));

    assert!(state.package_nicknames_for("rename-new").is_empty());
    assert!(!state.package_exists("rename-alias"));
}

#[test]
fn rename_package_conflict_leaves_the_original_package_unchanged() {
    let mut state = PackageState::new();
    define_package(&mut state, "rename-target", &["rename-alias"], &[]);
    define_package(&mut state, "rename-conflict", &[], &[]);
    let object = state
        .package_object_for("rename-target")
        .unwrap_or_else(|| panic!("the package object should exist before renaming"));

    assert!(
        state
            .rename_package("rename-target", "rename-conflict", Vec::new())
            .is_err()
    );
    assert_eq!(object.name().as_deref(), Some("RENAME-TARGET"));
    assert!(state.package_exists("rename-target"));
    assert!(state.package_exists("rename-alias"));
}

#[test]
fn delete_package_marks_handles_deleted_and_removes_local_nickname_references() {
    let mut state = PackageState::new();
    define_package(&mut state, "delete-target", &["delete-alias"], &[]);
    define_package(
        &mut state,
        "delete-consumer",
        &[],
        &[("target", "delete-target")],
    );
    state.set_current("delete-target");
    let object = state
        .package_object_for("delete-target")
        .unwrap_or_else(|| panic!("the package object should exist before deletion"));

    state
        .delete_package("delete-alias")
        .unwrap_or_else(|error| panic!("package deletion should succeed: {error}"));

    assert!(state.package_object_for("delete-target").is_none());
    assert!(state.package_object_for("delete-alias").is_none());
    assert!(object.name().is_none());
    assert_eq!(state.current(), DEFAULT_PACKAGE);
    assert!(
        state
            .package_local_nicknames_for("delete-consumer")
            .is_empty()
    );
}

#[test]
fn delete_package_rejects_a_package_used_by_another_package() {
    let mut state = PackageState::new();
    define_package(&mut state, "delete-target", &[], &[]);
    define_package(&mut state, "delete-consumer", &[], &[]);
    state.use_package("delete-target", "delete-consumer");
    let object = state
        .package_object_for("delete-target")
        .unwrap_or_else(|| panic!("the package object should exist before deletion"));

    assert!(state.delete_package("delete-target").is_err());
    assert_eq!(object.name().as_deref(), Some("DELETE-TARGET"));
    assert!(state.package_exists("delete-target"));
    assert_eq!(
        state.use_packages_for("delete-consumer"),
        vec!["DELETE-TARGET"]
    );
}
