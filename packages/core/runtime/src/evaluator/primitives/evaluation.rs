impl Runtime {
    fn apply_evaluation_primitive(
        &self,
        name: &str,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        match name {
            "EVAL" => {
                if arguments.len() != 1 {
                    return Err(self.arity("eval", "one", arguments.len()));
                }
                let form = self.form_from_value(&arguments[0], span)?;
                self.eval_values_in(&form, environment)
            }
            "COMPILE" => self.compile_function(arguments, environment, span),
            "LOAD" => self.load_file(arguments, span),
            _ => unreachable!("evaluation primitive group was misclassified"),
        }
    }
}
