use ncl_syntax::Span;

use crate::{Runtime, Value};

const SPAN: Span = Span::new(0, 1);

fn valid<T, E>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|_| panic!("expected a valid designator list"))
}

#[test]
fn package_and_symbol_lists_are_table_driven() {
    let runtime = Runtime::new();
    let packages = Value::list(vec![
        Value::String("ncl-user".into()),
        Value::symbol("keyword"),
    ]);
    assert_eq!(
        valid(runtime.package_names_from_value(&packages, SPAN)),
        ["NCL-USER", "KEYWORD"]
    );

    let symbols = Value::list(vec![Value::symbol("one"), Value::keyword("two")]);
    assert_eq!(
        valid(Runtime::symbol_names_from_value(&symbols, SPAN)),
        ["ONE", "TWO"]
    );

    let invalid = Value::Integer(1);
    assert!(runtime.package_names_from_value(&invalid, SPAN).is_err());
    assert!(Runtime::symbol_names_from_value(&invalid, SPAN).is_err());
}

#[test]
fn import_references_resolve_keyword_qualified_and_current_symbols() {
    let runtime = Runtime::new();
    let references = Value::list(vec![
        Value::keyword("key"),
        Value::symbol("common-lisp:car"),
        Value::symbol("local"),
    ]);
    assert_eq!(
        valid(runtime.symbol_import_references_from_value(&references, SPAN)),
        [
            ("KEYWORD".into(), "KEY".into()),
            ("COMMON-LISP".into(), "CAR".into()),
            ("NCL-USER".into(), "LOCAL".into())
        ]
    );
    assert!(
        runtime
            .symbol_import_references_from_value(&Value::Integer(1), SPAN)
            .is_err()
    );
    assert!(
        runtime
            .symbol_import_references_from_value(
                &Value::list(vec![Value::UninternedSymbol("x".into())]),
                SPAN
            )
            .is_err()
    );
    assert!(
        runtime
            .symbol_import_references_from_value(&Value::list(vec![Value::Integer(1)]), SPAN)
            .is_err()
    );
}
