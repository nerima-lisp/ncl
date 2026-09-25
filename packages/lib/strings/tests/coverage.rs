//! Strict ownership coverage for the strings registration boundary.

use ncl_object::{symbol_is_constant, FunctionObject, Runtime, ThreadContext};

const TABLE: &str = include_str!("../ownership.tsv");

#[test]
fn strings_registration_covers_every_owned_symbol() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut context = ThreadContext::new();
    context
        .register(&runtime)
        .unwrap_or_else(|error| panic!("context: {error:?}"));
    ncl_lib_strings::register(&runtime).unwrap_or_else(|error| panic!("strings: {error:?}"));

    let mut rows = 0;
    for line in TABLE.lines().skip(1) {
        let columns: Vec<_> = line.split('\t').collect();
        if columns.len() != 7 || columns[3] != "ncl-lib-strings" || columns[4] != "1" {
            continue;
        }
        rows += 1;
        let package = columns[0];
        let symbol = columns[1];
        let kind = columns[2];
        if kind.contains("function") {
            let word = runtime
                .function(&mut context, package, symbol)
                .unwrap_or_else(|| panic!("missing function {package}::{symbol}"));
            assert!(
                FunctionObject::try_from(word).is_ok(),
                "unbound function {package}::{symbol}"
            );
        }
        if kind.contains("class") {
            assert!(
                runtime.class(&mut context, symbol).is_some(),
                "missing class {package}::{symbol}"
            );
        }
        if kind.contains("constant") {
            let package_word = runtime
                .find_package(&context, package)
                .unwrap_or_else(|| panic!("package missing: {package}"));
            let (word, _) = ncl_object::Package::from_word(package_word)
                .intern(&mut context, &runtime, symbol)
                .unwrap_or_else(|error| panic!("intern: {error:?}"));
            assert!(
                symbol_is_constant(&context, word)
                    .unwrap_or_else(|error| panic!("constant flag: {error:?}")),
                "missing constant flag {package}::{symbol}"
            );
        }
    }
    assert!(rows > 0);
}
