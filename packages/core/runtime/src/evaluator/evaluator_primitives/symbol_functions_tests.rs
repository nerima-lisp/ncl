#[cfg(test)]
mod tests {
    use super::super::*;

    const SPAN: Span = Span::new(0, 0);

    #[test]
    fn fboundp_checks_an_exact_symbols_function_binding() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        let arguments = [Value::symbol_exact("My-Fn")];

        let result = runtime
            .apply_symbol_function_primitive("FBOUNDP", &arguments, &environment, SPAN)
            .unwrap_or_else(|| panic!("FBOUNDP is a recognized symbol-function primitive"))
            .unwrap_or_else(|error| {
                panic!("FBOUNDP on an unbound exact symbol still succeeds: {error}")
            });
        assert!(matches!(result, Value::Nil));
    }

    #[test]
    fn macro_function_checks_an_exact_symbols_function_binding() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        let arguments = [Value::symbol_exact("My-Macro")];

        let result = runtime
            .apply_symbol_function_primitive("MACRO-FUNCTION", &arguments, &environment, SPAN)
            .unwrap_or_else(|| panic!("MACRO-FUNCTION is a recognized symbol-function primitive"))
            .unwrap_or_else(|error| {
                panic!("MACRO-FUNCTION on an unbound exact symbol still succeeds: {error}")
            });
        assert!(matches!(result, Value::Nil));
    }

    #[test]
    fn macro_function_returns_a_defined_macros_function_object() {
        let runtime = Runtime::new();
        let results = runtime
            .eval_source("(defmacro my-test-macro () 1) (macro-function 'my-test-macro)")
            .unwrap_or_else(|error| panic!("defining and looking up a macro succeeds: {error}"));
        assert_ne!(
            results
                .last()
                .unwrap_or_else(|| panic!("expected a value"))
                .to_string(),
            "NIL"
        );
    }

    #[test]
    fn fdefinition_checks_an_exact_symbols_function_binding() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        let arguments = [Value::symbol_exact("My-Fn")];

        let result =
            runtime.apply_symbol_function_primitive("FDEFINITION", &arguments, &environment, SPAN);
        assert!(matches!(
            result,
            Some(Err(RuntimeError::UnboundVariable { name, .. })) if name == "My-Fn"
        ));
    }

    #[test]
    fn fdefinition_reports_a_bound_non_function_value_as_not_callable() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        environment.define("MY-VAR", Value::Integer(5));
        let arguments = [Value::symbol("MY-VAR")];

        let result = runtime.apply_symbol_function_primitive(
            "SYMBOL-FUNCTION",
            &arguments,
            &environment,
            SPAN,
        );
        assert!(matches!(
            result,
            Some(Err(RuntimeError::NotCallable { value, .. })) if value == "5"
        ));
    }
}
