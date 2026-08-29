#[cfg(test)]
mod tests {
    use crate::Runtime;

    #[test]
    fn prog1_and_prog2_propagate_errors_from_any_of_their_forms() {
        for source in [
            "(prog1 (car 5))",
            "(prog1 1 (car 5))",
            "(prog2 (car 5) 2)",
            "(prog2 1 (car 5))",
            "(prog2 1 2 (car 5))",
        ] {
            assert!(Runtime::new().eval_source(source).is_err(), "{source}");
        }
    }
}
