use crate::package::{
    COMMON_LISP_PACKAGE, DEFAULT_PACKAGE, canonical_symbol_name, normalize_package_name,
    normalize_symbol_name, split_symbol,
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
fn split_symbol_rejects_empty_qualified_names() {
    assert_eq!(split_symbol(""), None);
    assert_eq!(split_symbol(":name"), None);
    assert_eq!(split_symbol("pkg:"), None);
    assert_eq!(split_symbol("pkg::"), None);
}
