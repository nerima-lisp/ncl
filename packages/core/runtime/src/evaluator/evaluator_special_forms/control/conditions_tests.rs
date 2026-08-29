#[cfg(test)]
mod tests {
    use crate::Runtime;

    #[test]
    fn handler_case_does_not_intercept_non_local_control_transfers() {
        let value = Runtime::new()
            .eval_source("(block outer (handler-case (return-from outer 5) (error () 9)))")
            .unwrap_or_else(|error| {
                panic!("return-from should escape handler-case instead of being caught: {error}")
            })
            .pop()
            .unwrap_or_else(|| panic!("a value"));
        assert_eq!(value.to_string(), "5");
    }
}
