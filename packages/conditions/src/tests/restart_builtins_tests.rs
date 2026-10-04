#![allow(
    clippy::all,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::similar_names,
    clippy::indexing_slicing
)]
#![allow(
    clippy::panic,
    clippy::unwrap_used,
    reason = "coverage tests assert on internal helper results"
)]

include!("restart_builtins.rs");

#[test]
fn condition_and_named_restart_builtins_cover_success_and_missing_paths() {
    let (runtime, mut ctx) = setup();
    crate::register(&runtime).unwrap();
    let class = Package::from_word(runtime.ensure_package(&mut ctx, "COMMON-LISP").unwrap())
        .intern(&mut ctx, &runtime, "SIMPLE-CONDITION")
        .unwrap()
        .0;
    let condition = make_condition_builtin(
        &mut ctx,
        &runtime,
        &BuiltinArgs::new(&[class]),
        &mut MultipleValues::new(),
    )
    .unwrap();
    assert!(crate::condition_class_of(&ctx, condition).is_ok());
    assert_eq!(
        make_condition_builtin(
            &mut ctx,
            &runtime,
            &BuiltinArgs::new(&[Word::fixnum(1)]),
            &mut MultipleValues::new(),
        ),
        Err(ObjectError::TypeError)
    );

    let missing = Package::from_word(runtime.ensure_package(&mut ctx, "COMMON-LISP").unwrap())
        .intern(&mut ctx, &runtime, "MISSING-RESTART")
        .unwrap()
        .0;
    assert_eq!(
        find_restart_builtin(
            &mut ctx,
            &runtime,
            &BuiltinArgs::new(&[missing]),
            &mut MultipleValues::new(),
        ),
        Ok(Word::NIL)
    );
    assert_eq!(
        invoke_restart_builtin(
            &mut ctx,
            &runtime,
            &BuiltinArgs::new(&[missing]),
            &mut MultipleValues::new(),
        ),
        Err(ObjectError::ControlError)
    );
}

#[test]
fn named_restart_shorthands_forward_values_and_identity_adapter_preserves_args() {
    let (runtime, mut ctx) = setup();
    assert_eq!(
        identity_adapter(&BuiltinArgs::new(&[Word::fixnum(1)])),
        Ok(vec![Word::fixnum(1)])
    );
    let name = make_string(
        &mut ctx,
        &runtime,
        &['U', 'S', 'E', '-', 'V', 'A', 'L', 'U', 'E'],
    )
    .unwrap();
    let token = push_restart_builtin(
        &mut ctx,
        &runtime,
        &BuiltinArgs::new(&[name, Word::fixnum(4), Word::NIL, Word::NIL, Word::NIL]),
        &mut MultipleValues::new(),
    )
    .unwrap();
    assert_eq!(
        use_value_builtin(
            &mut ctx,
            &runtime,
            &BuiltinArgs::new(&[Word::fixnum(99)]),
            &mut MultipleValues::new(),
        ),
        Ok(Word::fixnum(4))
    );
    assert!(ctx.take_non_local_exit());
    pop_restart_builtin(
        &mut ctx,
        &runtime,
        &BuiltinArgs::new(&[token]),
        &mut MultipleValues::new(),
    )
    .unwrap();
    assert_eq!(
        symbol_text(&ctx, Word::fixnum(1)),
        Err(ObjectError::TypeError)
    );
}
