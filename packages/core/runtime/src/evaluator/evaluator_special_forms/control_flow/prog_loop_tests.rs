#[cfg(test)]
mod tests {
    use crate::Runtime;

    fn eval(source: &str) -> String {
        Runtime::new()
            .eval_source(source)
            .unwrap_or_else(|error| panic!("{source}: expected success, got {error:?}"))
            .pop()
            .unwrap_or_else(|| panic!("a value"))
            .to_string()
    }

    #[test]
    fn prog_accepts_bare_symbol_and_escaped_bindings() {
        assert_eq!(eval("(prog (x) x)"), "NIL");
        assert_eq!(eval("(prog ((|x|)) x)"), "NIL");
    }

    #[test]
    fn prog_rejects_a_binding_that_is_neither_a_symbol_nor_a_list() {
        assert!(Runtime::new().eval_source("(prog (\"bad\") x)").is_err());
    }

    #[test]
    fn prog_and_prog_star_propagate_errors_from_init_forms_and_the_body() {
        for source in [
            "(prog* ((x (car 5))) x)",
            "(prog ((x (car 5))) x)",
            "(prog () (car 5))",
        ] {
            assert!(Runtime::new().eval_source(source).is_err(), "{source}");
        }
    }
}
