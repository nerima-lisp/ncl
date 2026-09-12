use super::*;

#[test]
fn symbol_macrolet_defines_escaped_names_case_sensitively() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let form = list(vec![
        atom("SYMBOL-MACROLET"),
        list(vec![list(vec![atom("|s|"), atom("42")])]),
        atom("|s|"),
    ]);

    let result = runtime
        .prepare_compiled_symbol_macrolet(&form, &environment)
        .unwrap_or_else(|error| panic!("an escaped symbol macro name compiles: {error}"));
    assert_eq!(result.to_string(), "(PROGN 42)");
}
