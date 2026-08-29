#[cfg(test)]
mod tests {
    use crate::Runtime;

    #[test]
    fn and_returns_the_final_falsy_value_when_it_is_last() {
        let value = Runtime::new()
            .eval_source("(and 1 nil)")
            .unwrap_or_else(|error| panic!("and should evaluate all of its forms: {error}"))
            .pop()
            .unwrap_or_else(|| panic!("a value"));
        assert_eq!(value.to_string(), "NIL");
    }

    #[test]
    fn and_and_or_propagate_errors_from_their_forms() {
        for source in ["(and (car 5))", "(or (car 5))"] {
            assert!(Runtime::new().eval_source(source).is_err(), "{source}");
        }
    }
}
