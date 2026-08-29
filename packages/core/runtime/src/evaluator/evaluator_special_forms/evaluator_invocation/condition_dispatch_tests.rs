#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use ncl_syntax::Span;

    use crate::{Environment, Function, Runtime, RuntimeError, Value};

    const SPAN: Span = Span::new(0, 1);

    fn reader() -> Value {
        Value::Function(Rc::new(Function::ConditionReader {
            condition_name: "MY-CONDITION".to_string(),
            slot_name: "DETAIL".to_string(),
        }))
    }

    fn writer() -> Value {
        Value::Function(Rc::new(Function::ConditionWriter {
            condition_name: "MY-CONDITION".to_string(),
            slot_name: "DETAIL".to_string(),
        }))
    }

    fn empty_condition() -> Value {
        Value::condition_from_parts("MY-CONDITION".to_string(), String::new(), None, Vec::new())
    }

    #[test]
    fn apply_in_dispatches_condition_reader_arity_and_missing_slot_errors() {
        let runtime = Runtime::new();
        let environment = Environment::new();

        let arity_error = runtime
            .apply_in(&reader(), &[], SPAN, &environment)
            .map_or_else(
                |error| error,
                |value| panic!("expected an error, got {value:?}"),
            );
        assert!(
            matches!(&arity_error, RuntimeError::Arity { function, .. } if function == "condition reader")
        );

        let missing_slot = runtime
            .apply_in(&reader(), &[empty_condition()], SPAN, &environment)
            .map_or_else(
                |error| error,
                |value| panic!("expected an error, got {value:?}"),
            );
        assert!(matches!(
            &missing_slot,
            RuntimeError::InvalidForm { message, .. } if message == "condition slot is not defined"
        ));
    }

    #[test]
    fn apply_in_dispatches_condition_writer_arity_and_missing_slot_errors() {
        let runtime = Runtime::new();
        let environment = Environment::new();

        let arity_error = runtime
            .apply_in(&writer(), &[Value::Integer(1)], SPAN, &environment)
            .map_or_else(
                |error| error,
                |value| panic!("expected an error, got {value:?}"),
            );
        assert!(
            matches!(&arity_error, RuntimeError::Arity { function, .. } if function == "condition writer")
        );

        let missing_slot = runtime
            .apply_in(
                &writer(),
                &[Value::Integer(9), empty_condition()],
                SPAN,
                &environment,
            )
            .map_or_else(
                |error| error,
                |value| panic!("expected an error, got {value:?}"),
            );
        assert!(matches!(
            &missing_slot,
            RuntimeError::InvalidForm { message, .. } if message == "condition slot is not defined"
        ));
    }
}
