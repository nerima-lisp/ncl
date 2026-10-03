use super::*;
use crate::find_restart;
use ncl_object::{
    Package, car, cdr, make_instance, make_simple_vector, make_string, string_length, string_ref,
};

    fn setup() -> (Runtime, ThreadContext) {
        let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)
            .unwrap_or_else(|error| panic!("register: {error:?}"));
        (runtime, ctx)
    }

    fn text(ctx: &ThreadContext, word: Word) -> String {
        let length = string_length(ctx, word).unwrap_or_else(|error| panic!("length: {error:?}"));
        (0..length)
            .map(|index| {
                string_ref(ctx, word, index).unwrap_or_else(|error| panic!("ref: {error:?}"))
            })
            .collect()
    }

    #[test]
    fn find_restart_returns_non_symbol_designator_unchanged() {
        let (runtime, mut ctx) = setup();
        let value = Word::fixnum(42);
        let words = [value];
        let args = BuiltinArgs::new(&words);
        let result = find_restart_builtin(&mut ctx, &runtime, &args, &mut MultipleValues::new());
        assert_eq!(result, Ok(value));
    }

    #[test]
    fn missing_named_restart_shorthands_return_nil() {
        let (runtime, mut ctx) = setup();
        let args = BuiltinArgs::new(&[]);
        assert_eq!(
            continue_builtin(&mut ctx, &runtime, &args, &mut MultipleValues::new()),
            Ok(Word::NIL)
        );
        assert_eq!(
            abort_builtin(&mut ctx, &runtime, &args, &mut MultipleValues::new()),
            Ok(Word::NIL)
        );
        assert_eq!(
            muffle_warning_builtin(&mut ctx, &runtime, &args, &mut MultipleValues::new()),
            Ok(Word::NIL)
        );
    }

    #[test]
    fn restart_chain_find_compute_name_and_pop_are_observable() {
        let (runtime, mut ctx) = setup();
        let name = make_string(
            &mut ctx,
            &runtime,
            &['C', 'O', 'N', 'T', 'I', 'N', 'U', 'E'],
        )
        .unwrap_or_else(|error| panic!("name: {error:?}"));
        let token = push_restart_builtin(
            &mut ctx,
            &runtime,
            &BuiltinArgs::new(&[name, Word::fixnum(7), Word::NIL, Word::NIL, Word::NIL]),
            &mut MultipleValues::new(),
        )
        .unwrap_or_else(|error| panic!("push: {error:?}"));

        assert_eq!(find_restart(&ctx, name), Ok(Some(token)));
        let restarts = compute_restarts_builtin(
            &mut ctx,
            &runtime,
            &BuiltinArgs::new(&[]),
            &mut MultipleValues::new(),
        )
        .unwrap_or_else(|error| panic!("compute: {error:?}"));
        assert_eq!(car(&ctx, restarts), Ok(token));
        assert_eq!(cdr(&ctx, restarts), Ok(Word::NIL));

        let named = restart_name_builtin(
            &mut ctx,
            &runtime,
            &BuiltinArgs::new(&[token]),
            &mut MultipleValues::new(),
        )
        .unwrap_or_else(|error| panic!("restart name: {error:?}"));
        let common_lisp = runtime.ensure_package(&mut ctx, "COMMON-LISP").unwrap();
        let (expected, _) = Package::from_word(common_lisp)
            .intern(&mut ctx, &runtime, "CONTINUE")
            .unwrap();
        assert_eq!(named, expected);

        assert_eq!(
            pop_restart_builtin(
                &mut ctx,
                &runtime,
                &BuiltinArgs::new(&[token]),
                &mut MultipleValues::new()
            ),
            Ok(Word::NIL)
        );
        assert_eq!(find_restart(&ctx, name), Ok(None));
        assert_eq!(text(&ctx, name), "CONTINUE");
    }

    #[test]
    fn invoke_restart_accepts_a_record_designator_and_forwards_arguments() {
        let (runtime, mut ctx) = setup();
        let name = make_string(
            &mut ctx,
            &runtime,
            &['U', 'S', 'E', '-', 'V', 'A', 'L', 'U', 'E'],
        )
        .unwrap();
        let token = push_restart_builtin(
            &mut ctx,
            &runtime,
            &BuiltinArgs::new(&[name, Word::fixnum(9), Word::NIL, Word::NIL, Word::NIL]),
            &mut MultipleValues::new(),
        )
        .unwrap();
        let result = invoke_restart_builtin(
            &mut ctx,
            &runtime,
            &BuiltinArgs::new(&[token, Word::fixnum(5)]),
            &mut MultipleValues::new(),
        );

        assert_eq!(result, Ok(Word::fixnum(9)));
        assert!(ctx.take_non_local_exit());
        pop_restart_builtin(
            &mut ctx,
            &runtime,
            &BuiltinArgs::new(&[token]),
            &mut MultipleValues::new(),
        )
        .unwrap();
    }

    #[test]
    fn restart_name_returns_nil_for_anonymous_restart_and_accessors_read_slots() {
        let (runtime, mut ctx) = setup();
        let token = push_restart_builtin(
            &mut ctx,
            &runtime,
            &BuiltinArgs::new(&[Word::NIL, Word::NIL, Word::NIL, Word::NIL, Word::NIL]),
            &mut MultipleValues::new(),
        )
        .unwrap();
        assert_eq!(
            restart_name_builtin(
                &mut ctx,
                &runtime,
                &BuiltinArgs::new(&[token]),
                &mut MultipleValues::new(),
            ),
            Ok(Word::NIL)
        );
        pop_restart_builtin(
            &mut ctx,
            &runtime,
            &BuiltinArgs::new(&[token]),
            &mut MultipleValues::new(),
        )
        .unwrap();

        let class =
            make_simple_vector(&mut ctx, &runtime, &[Word::NIL, Word::NIL, Word::NIL]).unwrap();
        let instance = make_instance(
            &mut ctx,
            &runtime,
            class,
            &[Word::fixnum(3), Word::fixnum(4)],
        )
        .unwrap();
        assert_eq!(
            slot0_builtin(
                &mut ctx,
                &runtime,
                &BuiltinArgs::new(&[instance.as_word()]),
                &mut MultipleValues::new()
            ),
            Ok(Word::fixnum(3))
        );
        assert_eq!(
            slot1_builtin(
                &mut ctx,
                &runtime,
                &BuiltinArgs::new(&[instance.as_word()]),
                &mut MultipleValues::new()
            ),
            Ok(Word::fixnum(4))
        );
    }
