#![allow(clippy::all, clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::similar_names, clippy::indexing_slicing)]
    use super::*;

    #[test]
    fn next_method_p_distinguishes_nil_and_non_nil_continuations() {
        let mut values = MultipleValues::new();
        let nil_args = BuiltinArgs::new(&[Word::NIL]);
        let present_args = BuiltinArgs::new(&[Word::TRUE]);
        assert_eq!(
            clos_next_method_p_builtin(
                &mut ThreadContext::new(),
                &Runtime::new().expect("runtime"),
                &nil_args,
                &mut values,
            ),
            Ok(Word::NIL)
        );
        assert_eq!(
            clos_next_method_p_builtin(
                &mut ThreadContext::new(),
                &Runtime::new().expect("runtime"),
                &present_args,
                &mut values,
            ),
            Ok(Word::TRUE)
        );
    }

    #[test]
    fn class_designator_rejects_non_class_non_symbol_values() {
        let runtime = Runtime::new().expect("runtime");
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime).expect("context");
        assert_eq!(
            class_designator(&mut ctx, &runtime, Word::fixnum(7)),
            Err(ObjectError::TypeError)
        );
    }
