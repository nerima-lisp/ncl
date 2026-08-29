#[cfg(test)]
mod tests {
    use ncl_syntax::{Form, Span};

    use crate::Runtime;

    const SPAN: Span = Span::new(0, 1);

    fn atom(name: &str) -> Form {
        Form::atom(name, SPAN)
    }

    fn valid(result: Result<Form, crate::RuntimeError>) -> Form {
        result.unwrap_or_else(|error| panic!("expected a successful expansion: {error}"))
    }

    #[test]
    fn a_non_list_form_passes_through_unchanged() {
        let form = atom("X");
        let expanded = valid(Runtime::expand_builtin_with_slots(&form, false));
        assert_eq!(expanded.to_string(), form.to_string());
    }

    #[test]
    fn rejects_a_form_with_too_few_items() {
        let form = Form::list(vec![atom("WITH-SLOTS"), Form::list(vec![], SPAN)], SPAN);
        assert!(Runtime::expand_builtin_with_slots(&form, false).is_err());
    }

    #[test]
    fn rejects_bindings_that_are_not_a_list() {
        let form = Form::list(
            vec![atom("WITH-SLOTS"), atom("NOT-A-LIST"), atom("OBJ")],
            SPAN,
        );
        assert!(Runtime::expand_builtin_with_slots(&form, false).is_err());
    }

    #[test]
    fn rejects_a_with_slots_entry_that_is_neither_atom_nor_pair() {
        let entry = Form::list(vec![atom("A"), atom("B"), atom("C")], SPAN);
        let form = Form::list(
            vec![
                atom("WITH-SLOTS"),
                Form::list(vec![entry], SPAN),
                atom("OBJ"),
            ],
            SPAN,
        );
        assert!(Runtime::expand_builtin_with_slots(&form, false).is_err());
    }

    #[test]
    fn rejects_a_with_accessors_entry_with_the_wrong_length() {
        let entry = Form::list(vec![atom("A")], SPAN);
        let form = Form::list(
            vec![
                atom("WITH-ACCESSORS"),
                Form::list(vec![entry], SPAN),
                atom("OBJ"),
            ],
            SPAN,
        );
        assert!(Runtime::expand_builtin_with_slots(&form, true).is_err());
    }

    #[test]
    fn rejects_a_with_accessors_entry_with_an_invalid_accessor_name() {
        let entry = Form::list(vec![atom("A"), atom("1")], SPAN);
        let form = Form::list(
            vec![
                atom("WITH-ACCESSORS"),
                Form::list(vec![entry], SPAN),
                atom("OBJ"),
            ],
            SPAN,
        );
        assert!(Runtime::expand_builtin_with_slots(&form, true).is_err());
    }

    #[test]
    fn rejects_a_slot_symbol_that_is_not_an_atom() {
        let entry = Form::list(vec![Form::list(vec![], SPAN), atom("VAR")], SPAN);
        let form = Form::list(
            vec![
                atom("WITH-SLOTS"),
                Form::list(vec![entry], SPAN),
                atom("OBJ"),
            ],
            SPAN,
        );
        assert!(Runtime::expand_builtin_with_slots(&form, false).is_err());
    }

    #[test]
    fn rejects_a_slot_symbol_with_an_unterminated_escape() {
        let entry = atom("\\");
        let form = Form::list(
            vec![
                atom("WITH-SLOTS"),
                Form::list(vec![entry], SPAN),
                atom("OBJ"),
            ],
            SPAN,
        );
        assert!(Runtime::expand_builtin_with_slots(&form, false).is_err());
    }

    #[test]
    fn rejects_a_slot_symbol_that_is_a_numeric_literal() {
        let entry = atom("1");
        let form = Form::list(
            vec![
                atom("WITH-SLOTS"),
                Form::list(vec![entry], SPAN),
                atom("OBJ"),
            ],
            SPAN,
        );
        assert!(Runtime::expand_builtin_with_slots(&form, false).is_err());
    }
}
