#![allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
    pub(crate) fn apply_evaluation_primitive(
        &self,
        name: &str,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Option<Result<Value, RuntimeError>> {
        if !matches!(name, "EVAL" | "COMPILE" | "LOAD" | "MAKE-INSTANCE") {
            return None;
        }
        let result = match name {
            "EVAL" => match arguments.len() {
                1 => Self::form_from_value(&arguments[0], span)
                    .and_then(|form| self.eval_values_in(&form, environment)),
                _ => Err(Self::arity("eval", "one", arguments.len())),
            },
            "COMPILE" => self.compile_function(arguments, environment, span),
            "LOAD" => self.load_file(arguments, span),
            "MAKE-INSTANCE" => self.make_instance(arguments, environment, span),
            _ => unreachable!("evaluation primitive name was prevalidated"),
        };
        Some(result)
    }
}
