//! Tests for registered list-operation builtins.

use std::collections::HashMap;

use ncl_object::{FunctionObject, ObjectError, ObjectRef, Package, Runtime, ThreadContext, Word};

fn setup() -> (Runtime, ThreadContext, HashMap<String, FunctionObject>) {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("context: {error:?}"));
    ncl_lib_sequences::register(&runtime).unwrap_or_else(|error| panic!("sequences: {error:?}"));
    let functions = [
        "APPEND", "CAR", "CONS", "EQUAL", "LIST", "LISTP", "NCONC", "NREVERSE", "REMOVE", "SUBST",
    ]
    .into_iter()
    .map(|name| {
        let word = runtime
            .function(&mut ctx, "COMMON-LISP", name)
            .unwrap_or_else(|| panic!("missing builtin {name}"));
        (
            name.to_owned(),
            FunctionObject::try_from(word).unwrap_or_else(|_| panic!("{name} is not a function")),
        )
    })
    .collect();
    (runtime, ctx, functions)
}

fn call_result(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    functions: &HashMap<String, FunctionObject>,
    name: &str,
    args: &[Word],
) -> Result<Word, ObjectError> {
    assert!(functions.contains_key(name), "missing builtin {name}");
    let function = runtime
        .function(ctx, "COMMON-LISP", name)
        .unwrap_or_else(|| panic!("missing builtin {name}"));
    let function =
        FunctionObject::try_from(function).unwrap_or_else(|_| panic!("{name} is not a function"));
    // Builtins can allocate, so keep their arguments rooted for the call.
    ncl_object::with_roots(ctx, args, |ctx, roots| {
        let rooted_args = roots.iter().map(|root| **root).collect::<Vec<_>>();
        runtime.call_builtin(ctx, function, &rooted_args)
    })
}

fn call(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    functions: &HashMap<String, FunctionObject>,
    name: &str,
    args: &[Word],
) -> Word {
    call_result(runtime, ctx, functions, name, args)
        .unwrap_or_else(|error| panic!("{name} failed: {error:?}"))
}

fn assert_list(ctx: &ThreadContext, list: Word, expected: &[Word]) {
    let mut cursor = list;
    for &value in expected {
        assert!(cursor.is_cons());
        assert_eq!(ncl_object::car(ctx, cursor), Ok(value));
        cursor = ncl_object::cdr(ctx, cursor).unwrap_or_else(|error| panic!("cdr: {error:?}"));
    }
    assert_eq!(cursor, Word::NIL);
}

fn keyword(ctx: &mut ThreadContext, runtime: &Runtime, name: &str) -> Word {
    let package = runtime
        .find_package(ctx, "KEYWORD")
        .unwrap_or_else(|| panic!("missing KEYWORD package"));
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap_or_else(|error| panic!("intern {name}: {error:?}"))
        .0
}

fn package_symbol(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    package_name: &str,
    name: &str,
) -> Word {
    let package = runtime
        .find_package(ctx, package_name)
        .unwrap_or_else(|| panic!("missing {package_name} package"));
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap_or_else(|error| panic!("intern {package_name}:{name}: {error:?}"))
        .0
}

fn assert_subst_type_error(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    functions: &HashMap<String, FunctionObject>,
    args: &[Word],
) {
    assert_eq!(
        call_result(runtime, ctx, functions, "SUBST", args),
        Err(ObjectError::TypeError)
    );
}

fn builtin_word(ctx: &mut ThreadContext, runtime: &Runtime, name: &str) -> Word {
    runtime
        .function(ctx, "COMMON-LISP", name)
        .unwrap_or_else(|| panic!("missing builtin {name}"))
}

#[test]
fn listp_accepts_a_dotted_cons() {
    let (runtime, mut ctx, functions) = setup();
    let dotted = call(
        &runtime,
        &mut ctx,
        &functions,
        "CONS",
        &[Word::fixnum(1), Word::fixnum(2)],
    );
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "LISTP", &[dotted]),
        Word::TRUE
    );
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "LISTP", &[Word::fixnum(1)],),
        Word::NIL
    );
}

#[test]
fn nreverse_rejects_a_dotted_sequence() {
    let (runtime, mut ctx, functions) = setup();
    let dotted = call(
        &runtime,
        &mut ctx,
        &functions,
        "CONS",
        &[Word::fixnum(1), Word::fixnum(2)],
    );
    assert!(matches!(
        call_result(&runtime, &mut ctx, &functions, "NREVERSE", &[dotted]),
        Err(ObjectError::TypeError)
    ));
}

#[test]
fn nreverse_reverses_simple_vectors() {
    let (runtime, mut ctx, functions) = setup();
    let vector = ncl_object::make_simple_vector(
        &mut ctx,
        &runtime,
        &[Word::fixnum(1), Word::fixnum(2), Word::fixnum(3)],
    )
    .unwrap_or_else(|error| panic!("vector: {error:?}"));

    let reversed = call(&runtime, &mut ctx, &functions, "NREVERSE", &[vector]);

    assert!(matches!(
        ncl_object::classify_object(&ctx, reversed),
        ObjectRef::SimpleVector(_)
    ));
    assert_eq!(ncl_object::simple_vector_length(&ctx, reversed), Ok(3));
    assert_eq!(
        (0..3)
            .map(|index| ncl_object::simple_vector_ref(&ctx, reversed, index))
            .collect::<Result<Vec<_>, _>>(),
        Ok(vec![Word::fixnum(3), Word::fixnum(2), Word::fixnum(1)])
    );
}

#[test]
fn nreverse_reverses_strings() {
    let (runtime, mut ctx, functions) = setup();
    let string = ncl_object::make_string(&mut ctx, &runtime, &['a', 'b', 'c'])
        .unwrap_or_else(|error| panic!("string: {error:?}"));

    let reversed = call(&runtime, &mut ctx, &functions, "NREVERSE", &[string]);

    assert!(matches!(
        ncl_object::classify_object(&ctx, reversed),
        ObjectRef::String(_)
    ));
    assert_eq!(ncl_object::string_length(&ctx, reversed), Ok(3));
    assert_eq!(
        (0..3)
            .map(|index| ncl_object::string_ref(&ctx, reversed, index))
            .collect::<Result<Vec<_>, _>>(),
        Ok(vec!['c', 'b', 'a'])
    );
}

#[test]
fn subst_replaces_nested_and_dotted_tree_parts_without_mutating_the_input() {
    let (runtime, mut ctx, functions) = setup();
    let inner = call(
        &runtime,
        &mut ctx,
        &functions,
        "CONS",
        &[Word::fixnum(1), Word::fixnum(2)],
    );
    let tree = call(
        &runtime,
        &mut ctx,
        &functions,
        "CONS",
        &[inner, Word::fixnum(3)],
    );

    let result = call(
        &runtime,
        &mut ctx,
        &functions,
        "SUBST",
        &[Word::fixnum(9), Word::fixnum(2), tree],
    );

    let result_inner =
        ncl_object::car(&ctx, result).unwrap_or_else(|error| panic!("result car: {error:?}"));
    assert_eq!(ncl_object::cdr(&ctx, result_inner), Ok(Word::fixnum(9)));
    assert_eq!(ncl_object::cdr(&ctx, inner), Ok(Word::fixnum(2)));
    assert_eq!(ncl_object::cdr(&ctx, result), Ok(Word::fixnum(3)));
    assert_eq!(ncl_object::cdr(&ctx, tree), Ok(Word::fixnum(3)));
}

#[test]
fn subst_uses_test_key_and_test_not_on_subtrees() {
    let (runtime, mut ctx, functions) = setup();
    let mut root_tokens = Vec::new();
    let mut test = keyword(&mut ctx, &runtime, "TEST");
    root_tokens.push(ncl_object::push_root(&mut ctx, &mut test));
    let mut key = keyword(&mut ctx, &runtime, "KEY");
    root_tokens.push(ncl_object::push_root(&mut ctx, &mut key));
    let mut test_not = keyword(&mut ctx, &runtime, "TEST-NOT");
    root_tokens.push(ncl_object::push_root(&mut ctx, &mut test_not));
    let mut equal = runtime
        .function(&mut ctx, "COMMON-LISP", "EQUAL")
        .unwrap_or_else(|| panic!("missing EQUAL builtin"));
    root_tokens.push(ncl_object::push_root(&mut ctx, &mut equal));
    let mut car = runtime
        .function(&mut ctx, "COMMON-LISP", "CAR")
        .unwrap_or_else(|| panic!("missing CAR builtin"));
    root_tokens.push(ncl_object::push_root(&mut ctx, &mut car));

    let mut old_pair = call(
        &runtime,
        &mut ctx,
        &functions,
        "CONS",
        &[Word::fixnum(1), Word::fixnum(2)],
    );
    root_tokens.push(ncl_object::push_root(&mut ctx, &mut old_pair));
    let mut tree_pair = call(
        &runtime,
        &mut ctx,
        &functions,
        "CONS",
        &[Word::fixnum(1), Word::fixnum(2)],
    );
    root_tokens.push(ncl_object::push_root(&mut ctx, &mut tree_pair));
    let mut tree = call(&runtime, &mut ctx, &functions, "LIST", &[tree_pair]);
    root_tokens.push(ncl_object::push_root(&mut ctx, &mut tree));
    let mut replacement = call(
        &runtime,
        &mut ctx,
        &functions,
        "CONS",
        &[Word::fixnum(8), Word::fixnum(9)],
    );
    root_tokens.push(ncl_object::push_root(&mut ctx, &mut replacement));
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "EQUAL",
            &[old_pair, tree_pair],
        ),
        Word::TRUE
    );
    let result = call(
        &runtime,
        &mut ctx,
        &functions,
        "SUBST",
        &[replacement, old_pair, tree, test, equal],
    );
    assert_eq!(ncl_object::car(&ctx, result), Ok(replacement));
    assert_eq!(ncl_object::car(&ctx, tree), Ok(tree_pair));

    let mut keyed_tree = call(&runtime, &mut ctx, &functions, "LIST", &[Word::NIL]);
    root_tokens.push(ncl_object::push_root(&mut ctx, &mut keyed_tree));
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "CAR", &[keyed_tree]),
        Word::NIL
    );
    let keyed_result = call(
        &runtime,
        &mut ctx,
        &functions,
        "SUBST",
        &[Word::fixnum(7), Word::NIL, keyed_tree, key, car],
    );
    assert_eq!(keyed_result, Word::fixnum(7));

    let nil_key_result = call(
        &runtime,
        &mut ctx,
        &functions,
        "SUBST",
        &[Word::fixnum(7), Word::NIL, keyed_tree, key, Word::NIL],
    );
    assert_eq!(ncl_object::car(&ctx, nil_key_result), Ok(Word::fixnum(7)));
    assert_eq!(ncl_object::cdr(&ctx, nil_key_result), Ok(Word::fixnum(7)));

    let mut old = call(&runtime, &mut ctx, &functions, "LIST", &[Word::fixnum(1)]);
    root_tokens.push(ncl_object::push_root(&mut ctx, &mut old));
    let mut test_not_tree = call(&runtime, &mut ctx, &functions, "LIST", &[Word::fixnum(1)]);
    root_tokens.push(ncl_object::push_root(&mut ctx, &mut test_not_tree));
    let test_not_result = call(
        &runtime,
        &mut ctx,
        &functions,
        "SUBST",
        &[Word::fixnum(6), old, test_not_tree, test_not, equal],
    );
    assert_eq!(ncl_object::car(&ctx, test_not_result), Ok(Word::fixnum(6)));
    assert_eq!(ncl_object::cdr(&ctx, test_not_result), Ok(Word::fixnum(6)));
    for token in root_tokens.into_iter().rev() {
        assert!(ncl_object::pop_root(&mut ctx, token));
    }
}

#[test]
fn subst_checks_keyword_arguments() {
    let (runtime, mut ctx, functions) = setup();
    let mut root_tokens = Vec::new();
    let mut tree = call(&runtime, &mut ctx, &functions, "LIST", &[Word::fixnum(1)]);
    root_tokens.push(ncl_object::push_root(&mut ctx, &mut tree));
    let mut unknown = keyword(&mut ctx, &runtime, "UNKNOWN-KEY");
    root_tokens.push(ncl_object::push_root(&mut ctx, &mut unknown));
    let mut lower_case_key = keyword(&mut ctx, &runtime, "key");
    root_tokens.push(ncl_object::push_root(&mut ctx, &mut lower_case_key));
    let package = runtime
        .find_package(&ctx, "COMMON-LISP")
        .unwrap_or_else(|| panic!("missing COMMON-LISP package"));
    let (mut common_lisp_key, _) = Package::from_word(package)
        .intern(&mut ctx, &runtime, "KEY")
        .unwrap_or_else(|error| panic!("intern COMMON-LISP:KEY: {error:?}"));
    root_tokens.push(ncl_object::push_root(&mut ctx, &mut common_lisp_key));
    let mut key = keyword(&mut ctx, &runtime, "KEY");
    root_tokens.push(ncl_object::push_root(&mut ctx, &mut key));
    let mut test = keyword(&mut ctx, &runtime, "TEST");
    root_tokens.push(ncl_object::push_root(&mut ctx, &mut test));
    let mut test_not = keyword(&mut ctx, &runtime, "TEST-NOT");
    root_tokens.push(ncl_object::push_root(&mut ctx, &mut test_not));
    let mut atom = builtin_word(&mut ctx, &runtime, "ATOM");
    root_tokens.push(ncl_object::push_root(&mut ctx, &mut atom));
    assert_subst_type_error(
        &runtime,
        &mut ctx,
        &functions,
        &[Word::fixnum(7), Word::fixnum(1)],
    );
    assert_subst_type_error(
        &runtime,
        &mut ctx,
        &functions,
        &[Word::fixnum(7), Word::fixnum(1), tree, key],
    );
    assert_subst_type_error(
        &runtime,
        &mut ctx,
        &functions,
        &[Word::fixnum(7), Word::fixnum(1), tree, Word::fixnum(99)],
    );
    assert_subst_type_error(
        &runtime,
        &mut ctx,
        &functions,
        &[Word::fixnum(7), Word::fixnum(1), tree, unknown, Word::TRUE],
    );
    assert_subst_type_error(
        &runtime,
        &mut ctx,
        &functions,
        &[Word::fixnum(7), Word::fixnum(1), tree, lower_case_key, atom],
    );
    assert_subst_type_error(
        &runtime,
        &mut ctx,
        &functions,
        &[
            Word::fixnum(7),
            Word::fixnum(1),
            tree,
            common_lisp_key,
            atom,
        ],
    );
    assert_subst_type_error(
        &runtime,
        &mut ctx,
        &functions,
        &[
            Word::fixnum(7),
            Word::fixnum(1),
            tree,
            test,
            atom,
            test_not,
            atom,
        ],
    );

    for token in root_tokens.into_iter().rev() {
        assert!(ncl_object::pop_root(&mut ctx, token));
    }
}

#[test]
fn subst_uses_the_first_duplicate_keyword() {
    let (runtime, mut ctx, functions) = setup();
    let mut tree = call(&runtime, &mut ctx, &functions, "LIST", &[Word::fixnum(1)]);
    let tree_root = ncl_object::push_root(&mut ctx, &mut tree);
    let mut key = keyword(&mut ctx, &runtime, "KEY");
    let key_root = ncl_object::push_root(&mut ctx, &mut key);
    let mut atom = builtin_word(&mut ctx, &runtime, "ATOM");
    let atom_root = ncl_object::push_root(&mut ctx, &mut atom);
    let mut listp = builtin_word(&mut ctx, &runtime, "LISTP");
    let listp_root = ncl_object::push_root(&mut ctx, &mut listp);

    let result = call(
        &runtime,
        &mut ctx,
        &functions,
        "SUBST",
        &[Word::fixnum(7), Word::TRUE, tree, key, atom, key, listp],
    );
    assert_eq!(ncl_object::car(&ctx, result), Ok(Word::fixnum(7)));
    assert_eq!(ncl_object::cdr(&ctx, result), Ok(Word::fixnum(7)));

    assert!(ncl_object::pop_root(&mut ctx, listp_root));
    assert!(ncl_object::pop_root(&mut ctx, atom_root));
    assert!(ncl_object::pop_root(&mut ctx, key_root));
    assert!(ncl_object::pop_root(&mut ctx, tree_root));
}

#[test]
fn subst_allows_unknown_keywords_when_requested() {
    let (runtime, mut ctx, functions) = setup();
    let mut tree = call(&runtime, &mut ctx, &functions, "LIST", &[Word::fixnum(1)]);
    let tree_root = ncl_object::push_root(&mut ctx, &mut tree);
    let mut unknown = keyword(&mut ctx, &runtime, "UNKNOWN-KEY");
    let unknown_root = ncl_object::push_root(&mut ctx, &mut unknown);
    let mut allow_other_keys = keyword(&mut ctx, &runtime, "ALLOW-OTHER-KEYS");
    let allow_root = ncl_object::push_root(&mut ctx, &mut allow_other_keys);

    let result = call(
        &runtime,
        &mut ctx,
        &functions,
        "SUBST",
        &[
            Word::fixnum(7),
            Word::fixnum(2),
            tree,
            unknown,
            Word::TRUE,
            allow_other_keys,
            Word::TRUE,
        ],
    );
    assert_eq!(ncl_object::car(&ctx, result), Ok(Word::fixnum(1)));
    assert_eq!(ncl_object::cdr(&ctx, result), Ok(Word::NIL));

    assert!(ncl_object::pop_root(&mut ctx, allow_root));
    assert!(ncl_object::pop_root(&mut ctx, unknown_root));
    assert!(ncl_object::pop_root(&mut ctx, tree_root));
}

#[test]
fn append_returns_an_atomic_last_argument_and_rejects_dotted_prefixes() {
    let (runtime, mut ctx, functions) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let three = Word::fixnum(3);
    let tail = Word::fixnum(9);

    assert_eq!(
        call(&runtime, &mut ctx, &functions, "APPEND", &[]),
        Word::NIL
    );
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "APPEND", &[tail]),
        tail
    );

    let first = call(&runtime, &mut ctx, &functions, "LIST", &[one, two]);
    let mut first = first;
    let first_root = ncl_object::push_root(&mut ctx, &mut first);
    let second = call(
        &runtime,
        &mut ctx,
        &functions,
        "LIST",
        &[three, Word::fixnum(4)],
    );
    let mut second = second;
    let second_root = ncl_object::push_root(&mut ctx, &mut second);
    let result = call(
        &runtime,
        &mut ctx,
        &functions,
        "APPEND",
        &[first, second, tail],
    );
    assert_eq!(ncl_object::car(&ctx, result), Ok(one));
    let result_second =
        ncl_object::cdr(&ctx, result).unwrap_or_else(|error| panic!("cdr: {error:?}"));
    assert_eq!(ncl_object::car(&ctx, result_second), Ok(two));
    let result_third =
        ncl_object::cdr(&ctx, result_second).unwrap_or_else(|error| panic!("cdr: {error:?}"));
    assert_eq!(ncl_object::car(&ctx, result_third), Ok(three));
    let result_fourth =
        ncl_object::cdr(&ctx, result_third).unwrap_or_else(|error| panic!("cdr: {error:?}"));
    assert_eq!(ncl_object::car(&ctx, result_fourth), Ok(Word::fixnum(4)));
    assert_eq!(ncl_object::cdr(&ctx, result_fourth), Ok(tail));
    assert_list(&ctx, first, &[one, two]);
    assert_list(&ctx, second, &[three, Word::fixnum(4)]);

    let dotted = call(&runtime, &mut ctx, &functions, "CONS", &[one, tail]);
    assert!(matches!(
        call_result(
            &runtime,
            &mut ctx,
            &functions,
            "APPEND",
            &[dotted, Word::NIL]
        ),
        Err(ObjectError::TypeError)
    ));
    assert!(ncl_object::pop_root(&mut ctx, second_root));
    assert!(ncl_object::pop_root(&mut ctx, first_root));
}

#[test]
fn nconc_links_multi_cons_dotted_prefixes_and_skips_nil() {
    let (runtime, mut ctx, functions) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let three = Word::fixnum(3);
    let four = Word::fixnum(4);
    let five = Word::fixnum(5);
    let six = Word::fixnum(6);

    assert_eq!(
        call(&runtime, &mut ctx, &functions, "NCONC", &[]),
        Word::NIL
    );
    assert_eq!(call(&runtime, &mut ctx, &functions, "NCONC", &[two]), two);

    let first_dotted_last_cons = call(
        &runtime,
        &mut ctx,
        &functions,
        "CONS",
        &[two, Word::fixnum(90)],
    );
    let first_dotted_prefix = call(
        &runtime,
        &mut ctx,
        &functions,
        "CONS",
        &[one, first_dotted_last_cons],
    );
    let mut first_dotted_prefix = first_dotted_prefix;
    let first_root = ncl_object::push_root(&mut ctx, &mut first_dotted_prefix);
    let first_dotted_last_cons =
        ncl_object::cdr(&ctx, first_dotted_prefix).unwrap_or_else(|error| panic!("cdr: {error:?}"));
    assert_eq!(
        ncl_object::cdr(&ctx, first_dotted_last_cons),
        Ok(Word::fixnum(90))
    );

    let second_dotted_last_cons = call(
        &runtime,
        &mut ctx,
        &functions,
        "CONS",
        &[four, Word::fixnum(91)],
    );
    let second_dotted_prefix = call(
        &runtime,
        &mut ctx,
        &functions,
        "CONS",
        &[three, second_dotted_last_cons],
    );
    let mut second_dotted_prefix = second_dotted_prefix;
    let second_root = ncl_object::push_root(&mut ctx, &mut second_dotted_prefix);
    let second_dotted_last_cons = ncl_object::cdr(&ctx, second_dotted_prefix)
        .unwrap_or_else(|error| panic!("cdr: {error:?}"));
    assert_eq!(
        ncl_object::cdr(&ctx, second_dotted_last_cons),
        Ok(Word::fixnum(91))
    );

    let tail = call(&runtime, &mut ctx, &functions, "LIST", &[five, six]);
    let mut tail = tail;
    let tail_root = ncl_object::push_root(&mut ctx, &mut tail);
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "NCONC", &[Word::NIL, tail]),
        tail
    );

    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "NCONC",
            &[first_dotted_prefix, Word::NIL, second_dotted_prefix, tail]
        ),
        first_dotted_prefix
    );
    let first_last_cons =
        ncl_object::cdr(&ctx, first_dotted_prefix).unwrap_or_else(|error| panic!("cdr: {error:?}"));
    let first_link =
        ncl_object::cdr(&ctx, first_last_cons).unwrap_or_else(|error| panic!("cdr: {error:?}"));
    assert_eq!(first_link, second_dotted_prefix);
    let second_last_cons = ncl_object::cdr(&ctx, second_dotted_prefix)
        .unwrap_or_else(|error| panic!("cdr: {error:?}"));
    let second_link =
        ncl_object::cdr(&ctx, second_last_cons).unwrap_or_else(|error| panic!("cdr: {error:?}"));
    assert_eq!(second_link, tail);
    assert_list(
        &ctx,
        first_dotted_prefix,
        &[one, two, three, four, five, six],
    );

    assert!(matches!(
        call_result(&runtime, &mut ctx, &functions, "NCONC", &[two, tail]),
        Err(ObjectError::TypeError)
    ));
    assert!(ncl_object::pop_root(&mut ctx, tail_root));
    assert!(ncl_object::pop_root(&mut ctx, second_root));
    assert!(ncl_object::pop_root(&mut ctx, first_root));
}

#[test]
fn remove_preserves_positional_symbols_named_like_keywords() {
    let (runtime, mut ctx, functions) = setup();
    let mut root_tokens = Vec::new();
    let mut key = package_symbol(&mut ctx, &runtime, "COMMON-LISP-USER", "KEY");
    root_tokens.push(ncl_object::push_root(&mut ctx, &mut key));
    let mut a = package_symbol(&mut ctx, &runtime, "COMMON-LISP-USER", "A");
    root_tokens.push(ncl_object::push_root(&mut ctx, &mut a));
    let mut sequence = call(&runtime, &mut ctx, &functions, "LIST", &[key, a]);
    root_tokens.push(ncl_object::push_root(&mut ctx, &mut sequence));

    let result = call(&runtime, &mut ctx, &functions, "REMOVE", &[key, sequence]);
    assert_eq!(ncl_object::car(&ctx, result), Ok(a));
    assert_eq!(ncl_object::cdr(&ctx, result), Ok(Word::NIL));

    for token in root_tokens.into_iter().rev() {
        assert!(ncl_object::pop_root(&mut ctx, token));
    }
}

#[test]
fn remove_matches_option_names_only_in_the_keyword_package() {
    let (runtime, mut ctx, functions) = setup();
    let mut root_tokens = Vec::new();
    let mut a = package_symbol(&mut ctx, &runtime, "COMMON-LISP-USER", "A");
    root_tokens.push(ncl_object::push_root(&mut ctx, &mut a));
    let mut b = package_symbol(&mut ctx, &runtime, "COMMON-LISP-USER", "B");
    root_tokens.push(ncl_object::push_root(&mut ctx, &mut b));
    let mut non_keyword_key = package_symbol(&mut ctx, &runtime, "COMMON-LISP-USER", "KEY");
    root_tokens.push(ncl_object::push_root(&mut ctx, &mut non_keyword_key));
    let mut sequence = call(&runtime, &mut ctx, &functions, "LIST", &[a, b]);
    root_tokens.push(ncl_object::push_root(&mut ctx, &mut sequence));
    let mut car = builtin_word(&mut ctx, &runtime, "CAR");
    root_tokens.push(ncl_object::push_root(&mut ctx, &mut car));
    let mut allow_other_keys = keyword(&mut ctx, &runtime, "ALLOW-OTHER-KEYS");
    root_tokens.push(ncl_object::push_root(&mut ctx, &mut allow_other_keys));

    let result = call(
        &runtime,
        &mut ctx,
        &functions,
        "REMOVE",
        &[
            a,
            sequence,
            non_keyword_key,
            car,
            allow_other_keys,
            Word::TRUE,
        ],
    );
    assert_eq!(ncl_object::car(&ctx, result), Ok(b));
    assert_eq!(ncl_object::cdr(&ctx, result), Ok(Word::NIL));

    for token in root_tokens.into_iter().rev() {
        assert!(ncl_object::pop_root(&mut ctx, token));
    }
}

#[test]
fn selection_rejects_non_symbol_keyword_arguments_as_program_error() {
    let (runtime, mut ctx, functions) = setup();
    ncl_conditions::register(&runtime).unwrap_or_else(|error| panic!("conditions: {error:?}"));
    let mut root_tokens = Vec::new();
    let mut sequence = call(
        &runtime,
        &mut ctx,
        &functions,
        "LIST",
        &[Word::fixnum(1), Word::fixnum(2)],
    );
    root_tokens.push(ncl_object::push_root(&mut ctx, &mut sequence));
    let mut allow_other_keys = keyword(&mut ctx, &runtime, "ALLOW-OTHER-KEYS");
    root_tokens.push(ncl_object::push_root(&mut ctx, &mut allow_other_keys));

    for (name, args) in [
        (
            "REMOVE",
            vec![
                Word::fixnum(1),
                sequence,
                Word::fixnum(3),
                Word::fixnum(4),
                allow_other_keys,
                Word::TRUE,
            ],
        ),
        (
            "SUBST",
            vec![
                Word::fixnum(9),
                Word::fixnum(1),
                sequence,
                Word::fixnum(3),
                Word::fixnum(4),
                allow_other_keys,
                Word::TRUE,
            ],
        ),
    ] {
        assert_eq!(
            call_result(&runtime, &mut ctx, &functions, name, &args),
            Err(ObjectError::TypeError),
            "{name} should reject a non-symbol keyword argument",
        );
        let mut condition = ctx
            .take_pending_condition()
            .unwrap_or_else(|| panic!("{name} did not signal a PROGRAM-ERROR condition"));
        let condition_token = ncl_object::push_root(&mut ctx, &mut condition);
        let class = ncl_conditions::condition_class_of(&ctx, condition)
            .unwrap_or_else(|error| panic!("condition class: {error:?}"));
        let program_error = ncl_conditions::condition_class(&mut ctx, &runtime, "PROGRAM-ERROR")
            .unwrap_or_else(|| panic!("missing PROGRAM-ERROR class"));
        assert_eq!(class, program_error, "{name} condition class");
        assert!(ncl_object::pop_root(&mut ctx, condition_token));
    }

    for token in root_tokens.into_iter().rev() {
        assert!(ncl_object::pop_root(&mut ctx, token));
    }
}
