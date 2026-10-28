use super::*;

fn setup() -> (Runtime, ThreadContext, Word) {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("context: {error:?}"));
    crate::register(&runtime).unwrap_or_else(|error| panic!("sequences: {error:?}"));
    let predicate = runtime
        .function(&mut ctx, "COMMON-LISP", "CONSP")
        .unwrap_or_else(|| panic!("missing CONSP"));
    (runtime, ctx, predicate)
}

#[test]
fn apply_tree_substitution_rejects_missing_positionals() {
    let (runtime, mut ctx, _) = setup();
    let mut caller = BuiltinFunctionCaller;
    let options = domain::selection::SelectionOptions::default();
    let empty = [];
    let one = [Word::NIL];
    let two = [Word::NIL, Word::NIL];
    let cases = [
        (
            TreeSubstitutionMode::Equal { destructive: false },
            &empty[..],
        ),
        (TreeSubstitutionMode::Equal { destructive: false }, &one[..]),
        (TreeSubstitutionMode::Equal { destructive: false }, &two[..]),
        (
            TreeSubstitutionMode::Predicate {
                destructive: false,
                negate: false,
            },
            &empty[..],
        ),
        (
            TreeSubstitutionMode::Predicate {
                destructive: false,
                negate: false,
            },
            &one[..],
        ),
        (
            TreeSubstitutionMode::Predicate {
                destructive: false,
                negate: false,
            },
            &two[..],
        ),
        (
            TreeSubstitutionMode::Alist { destructive: false },
            &empty[..],
        ),
        (TreeSubstitutionMode::Alist { destructive: false }, &one[..]),
    ];

    for (mode, positional) in cases {
        assert!(matches!(
            apply_tree_substitution(&mut ctx, &runtime, &mut caller, positional, options, mode,),
            Err(ObjectError::TypeError)
        ));
    }
}

#[test]
fn apply_tree_substitution_dispatches_all_modes() {
    let (runtime, mut ctx, predicate) = setup();
    let mut caller = BuiltinFunctionCaller;
    let options = domain::selection::SelectionOptions::default();
    let equal_args = [Word::fixnum(9), Word::fixnum(1), Word::fixnum(1)];
    let predicate_args = [Word::fixnum(9), predicate, Word::NIL];
    let alist_args = [Word::NIL, Word::fixnum(1)];
    let cases = [
        (
            TreeSubstitutionMode::Equal { destructive: false },
            &equal_args[..],
            Word::fixnum(9),
        ),
        (
            TreeSubstitutionMode::Equal { destructive: true },
            &equal_args[..],
            Word::fixnum(9),
        ),
        (
            TreeSubstitutionMode::Predicate {
                destructive: false,
                negate: false,
            },
            &predicate_args[..],
            Word::NIL,
        ),
        (
            TreeSubstitutionMode::Predicate {
                destructive: false,
                negate: true,
            },
            &predicate_args[..],
            Word::fixnum(9),
        ),
        (
            TreeSubstitutionMode::Predicate {
                destructive: true,
                negate: false,
            },
            &predicate_args[..],
            Word::NIL,
        ),
        (
            TreeSubstitutionMode::Predicate {
                destructive: true,
                negate: true,
            },
            &predicate_args[..],
            Word::fixnum(9),
        ),
        (
            TreeSubstitutionMode::Alist { destructive: false },
            &alist_args[..],
            Word::fixnum(1),
        ),
        (
            TreeSubstitutionMode::Alist { destructive: true },
            &alist_args[..],
            Word::fixnum(1),
        ),
    ];

    for (mode, positional, expected) in cases {
        assert!(matches!(
            apply_tree_substitution(
                &mut ctx,
                &runtime,
                &mut caller,
                positional,
                options,
                mode,
            ),
            Ok(result) if result == expected
        ));
    }
}
