#[cfg(test)]
mod tests {
    use super::super::reservations::CompilationReservations;
    use super::super::{Environment, Form, Runtime, RuntimeError, Span};
    use std::collections::HashSet;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    const SPAN: Span = Span::new(0, 0);

    fn atom(name: &str) -> Form {
        Form::atom(name, SPAN)
    }

    fn list(items: Vec<Form>) -> Form {
        Form::list(items, SPAN)
    }

    fn assert_names(runtime: &Runtime, expected: &[&str]) {
        let expected: HashSet<String> = expected.iter().map(|name| (*name).to_owned()).collect();
        assert_eq!(*runtime.compilation_reserved_names.borrow(), expected);
    }

    #[test]
    fn nested_guards_restore_inherited_names_and_remove_only_new_names() {
        let runtime = Runtime::new();
        let mut inherited = CompilationReservations::new(&runtime);
        inherited.reserve(&atom("inherited"));
        assert_names(&runtime, &["INHERITED"]);

        {
            let mut outer = CompilationReservations::new(&runtime);
            outer.reserve(&list(vec![atom("inherited"), atom("outer"), atom("outer")]));
            assert_names(&runtime, &["INHERITED", "OUTER"]);
            {
                let mut inner = CompilationReservations::new(&runtime);
                inner.reserve(&list(vec![
                    atom("inherited"),
                    atom("outer"),
                    atom("inner"),
                    atom("inner"),
                ]));
                assert_names(&runtime, &["INHERITED", "OUTER", "INNER"]);
            }
            assert_names(&runtime, &["INHERITED", "OUTER"]);
        }
        assert_names(&runtime, &["INHERITED"]);
        drop(inherited);
        assert_names(&runtime, &[]);
    }

    #[test]
    fn prepare_compiled_form_error_restores_inherited_reservations() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        let mut inherited = CompilationReservations::new(&runtime);
        inherited.reserve(&atom("INHERITED"));
        let form = list(vec![
            atom("PROGN"),
            atom("INHERITED"),
            list(vec![atom("INCF")]),
        ]);

        let result = runtime.prepare_compiled_form(&form, &environment);

        assert!(matches!(
            result,
            Err(RuntimeError::Arity { function, expected, actual: 0 })
                if function == "INCF" && expected == "one or two"
        ));
        assert_names(&runtime, &["INHERITED"]);
        drop(inherited);
        assert_names(&runtime, &[]);
    }

    #[test]
    fn prepare_compiled_form_success_restores_inherited_reservations() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        let mut inherited = CompilationReservations::new(&runtime);
        inherited.reserve(&atom("INHERITED"));
        let form = list(vec![atom("PROGN"), atom("INHERITED"), atom("NEW")]);

        let prepared = runtime
            .prepare_compiled_form(&form, &environment)
            .unwrap_or_else(|error| panic!("PROGN preparation failed: {error}"));

        assert_eq!(prepared.to_string(), "(PROGN INHERITED NEW)");
        assert_names(&runtime, &["INHERITED"]);
        drop(inherited);
        assert_names(&runtime, &[]);
    }

    #[test]
    fn panic_unwinding_restores_inherited_reservations() {
        let runtime = Runtime::new();
        let mut inherited = CompilationReservations::new(&runtime);
        inherited.reserve(&atom("INHERITED"));

        let result = catch_unwind(AssertUnwindSafe(|| {
            let mut outer = CompilationReservations::new(&runtime);
            outer.reserve(&list(vec![atom("INHERITED"), atom("OUTER")]));
            let mut inner = CompilationReservations::new(&runtime);
            inner.reserve(&list(vec![atom("OUTER"), atom("INNER")]));
            assert_names(&runtime, &["INHERITED", "OUTER", "INNER"]);
            panic!("reservation cleanup probe");
        }));

        let payload = result.expect_err("the cleanup probe must unwind");
        assert_eq!(
            payload.downcast_ref::<&str>(),
            Some(&"reservation cleanup probe")
        );
        assert_names(&runtime, &["INHERITED"]);
        drop(inherited);
        assert_names(&runtime, &[]);
    }
}
