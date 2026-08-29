#[cfg(test)]
mod tests {
    use super::super::*;

    const SPAN: Span = Span::new(0, 0);

    #[test]
    fn symbol_value_reports_an_unbound_symbol() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        let arguments = [Value::symbol("unbound-xyz")];

        let result = runtime
            .apply_symbol_value_primitive("SYMBOL-VALUE", &arguments, &environment, SPAN)
            .unwrap_or_else(|| panic!("SYMBOL-VALUE is a recognized symbol-value primitive"));
        assert!(matches!(
            result,
            Err(RuntimeError::UnboundVariable { name, .. }) if name == "UNBOUND-XYZ"
        ));
    }

    #[test]
    fn symbol_value_reports_an_unbound_exact_symbol() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        let arguments = [Value::symbol_exact("Unbound-Xyz")];

        let result = runtime
            .apply_symbol_value_primitive("SYMBOL-VALUE", &arguments, &environment, SPAN)
            .unwrap_or_else(|| panic!("SYMBOL-VALUE is a recognized symbol-value primitive"));
        assert!(matches!(
            result,
            Err(RuntimeError::UnboundVariable { name, .. }) if name == "Unbound-Xyz"
        ));
    }
}
