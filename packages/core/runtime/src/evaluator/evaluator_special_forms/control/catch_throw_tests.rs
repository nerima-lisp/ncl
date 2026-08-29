#[cfg(test)]
mod tests {
    use crate::Runtime;

    #[test]
    fn catch_returns_the_bodys_value_when_nothing_is_thrown() {
        let value = Runtime::new()
            .eval_source("(catch 'tag 42)")
            .unwrap_or_else(|error| {
                panic!("catch without a throw should return the body's value: {error}")
            })
            .pop()
            .unwrap_or_else(|| panic!("a value"));
        assert_eq!(value.to_string(), "42");
    }

    #[test]
    fn catch_and_throw_propagate_errors_from_their_tag_and_value_forms() {
        for source in [
            "(catch (car 5) 1)",
            "(throw (car 5) 1)",
            "(throw 'tag (car 5))",
        ] {
            assert!(Runtime::new().eval_source(source).is_err(), "{source}");
        }
    }
}
