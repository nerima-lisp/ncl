use ncl_runtime::Runtime;

#[test]
fn symbol_accessors_cover_symbol_kinds_through_one_table() {
    let cases = [
        ("(symbol-name 'name)", "\"NAME\""),
        ("(symbol-name :name)", "\"NAME\""),
        ("(symbol-name '#:name)", "\"NAME\""),
        ("(symbol-package 'name)", "NCL-USER"),
        ("(symbol-package :name)", "KEYWORD"),
        ("(symbol-package nil)", "COMMON-LISP"),
        ("(symbol-package '#:name)", "NIL"),
    ];

    for (source, expected) in cases {
        let actual = Runtime::new()
            .eval_source(source)
            .unwrap_or_else(|error| panic!("{source}: {error}"));
        assert_eq!(actual[0].to_string(), expected, "{source}");
    }
}

#[test]
fn symbol_accessors_reject_non_symbols() {
    for source in ["(symbol-name 1)", "(symbol-package 1)"] {
        assert!(Runtime::new().eval_source(source).is_err(), "{source}");
    }
}
