#[cfg(test)]
mod tests {
    use super::super::*;

    const SPAN: Span = Span::new(0, 0);

    #[test]
    fn eval_rejects_any_argument_count_other_than_one() {
        let runtime = Runtime::new();
        let environment = Environment::new();

        for arguments in [Vec::new(), vec![Value::Integer(1), Value::Integer(2)]] {
            let result = runtime
                .apply_evaluation_primitive("EVAL", &arguments, &environment, SPAN)
                .unwrap_or_else(|| panic!("EVAL is a recognized evaluation primitive"));
            assert!(matches!(
                result,
                Err(RuntimeError::Arity { function, expected, actual })
                    if function == "eval" && expected == "one" && actual == arguments.len()
            ));
        }
    }
}
