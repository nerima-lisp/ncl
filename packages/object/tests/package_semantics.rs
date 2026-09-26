#![allow(missing_docs)]

use ncl_object::package::{FindStatus, Package};
use ncl_object::{
    Runtime, ThreadContext, Word, cdr, make_string, string_ref, symbol_name, symbol_package,
};

fn string(ctx: &mut ThreadContext, runtime: &Runtime, value: &str) -> Word {
    make_string(ctx, runtime, &value.chars().collect::<Vec<_>>())
        .unwrap_or_else(|error| panic!("string allocation: {error:?}"))
}

#[test]
fn common_lisp_user_inherits_common_lisp_symbols_under_gc_stress() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);

    let mut common_word = runtime
        .find_package(&ctx, "COMMON-LISP")
        .unwrap_or_else(|| panic!("COMMON-LISP package missing"));
    let common_token = ncl_object::push_root(&mut ctx, &mut common_word);
    let mut user_word = runtime
        .find_package(&ctx, "COMMON-LISP-USER")
        .unwrap_or_else(|| panic!("COMMON-LISP-USER package missing"));
    let user_token = ncl_object::push_root(&mut ctx, &mut user_word);
    let use_list = Package::from_word(user_word)
        .use_list(&ctx)
        .unwrap_or_else(|error| panic!("use-list: {error:?}"));
    assert_ne!(use_list, Word::NIL);

    for name in ["QUOTE", "IF", "<", "DEFUN"] {
        let (common_symbol, common_status) = Package::from_word(common_word)
            .intern(&mut ctx, &runtime, name)
            .unwrap_or_else(|error| panic!("common intern {name}: {error:?}"));
        assert_eq!(common_status, FindStatus::External);
        let mut common_symbol = common_symbol;
        let common_symbol_token = ncl_object::push_root(&mut ctx, &mut common_symbol);
        let (user_symbol, user_status) = Package::from_word(user_word)
            .intern(&mut ctx, &runtime, name)
            .unwrap_or_else(|error| panic!("user intern {name}: {error:?}"));
        assert_eq!(user_symbol, common_symbol);
        assert_eq!(user_status, FindStatus::Inherited);
        let lookup_name = string(&mut ctx, &runtime, name);
        assert_eq!(
            Package::from_word(user_word).find_symbol(&mut ctx, lookup_name),
            Ok(Some((common_symbol, FindStatus::Inherited)))
        );
        assert!(ncl_object::pop_root(&mut ctx, common_symbol_token));
    }
    assert!(ncl_object::pop_root(&mut ctx, user_token));
    assert!(ncl_object::pop_root(&mut ctx, common_token));
}

#[test]
fn import_preserves_home_and_rejects_accessible_conflicts() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    let home = Package::new(&mut ctx, &runtime, "HOME")
        .unwrap_or_else(|error| panic!("test failure: {error:?}"))
        .as_word();
    let target = Package::new(&mut ctx, &runtime, "TARGET")
        .unwrap_or_else(|error| panic!("test failure: {error:?}"))
        .as_word();
    let name = string(&mut ctx, &runtime, "NAME");
    let (symbol, _) = Package::from_word(home)
        .intern(&mut ctx, &runtime, "NAME")
        .unwrap_or_else(|error| panic!("test failure: {error:?}"));
    Package::from_word(target)
        .import(&mut ctx, &runtime, name, symbol)
        .unwrap_or_else(|error| panic!("test failure: {error:?}"));
    assert_eq!(
        Package::from_word(target).find_symbol(&mut ctx, name),
        Ok(Some((symbol, FindStatus::Internal)))
    );
    let other = ncl_object::make_symbol(&mut ctx, &runtime, name)
        .unwrap_or_else(|error| panic!("test failure: {error:?}"));
    assert_eq!(
        Package::from_word(target).import(&mut ctx, &runtime, name, other),
        Err(ncl_object::ObjectError::PackageConflict)
    );
    assert_eq!(symbol_package(&ctx, other), Ok(Word::NIL));
}

#[test]
fn unintern_clears_home_and_shadowing_but_not_inherited() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("test failure: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("test failure: {error:?}"));
    let base = Package::new(&mut ctx, &runtime, "BASE")
        .unwrap_or_else(|error| panic!("test failure: {error:?}"))
        .as_word();
    let user = Package::new(&mut ctx, &runtime, "USER")
        .unwrap_or_else(|error| panic!("test failure: {error:?}"))
        .as_word();
    let name = string(&mut ctx, &runtime, "NAME");
    let (symbol, _) = Package::from_word(base)
        .intern(&mut ctx, &runtime, "NAME")
        .unwrap_or_else(|error| panic!("test failure: {error:?}"));
    Package::from_word(base)
        .export(&mut ctx, &runtime, name)
        .unwrap_or_else(|error| panic!("test failure: {error:?}"));
    Package::from_word(base)
        .shadow(&mut ctx, &runtime, name)
        .unwrap_or_else(|error| panic!("test failure: {error:?}"));
    Package::from_word(base)
        .shadow(&mut ctx, &runtime, name)
        .unwrap_or_else(|error| panic!("test failure: {error:?}"));
    let shadowing = Package::from_word(base)
        .shadowing_symbols(&ctx)
        .unwrap_or_else(|error| panic!("test failure: {error:?}"));
    assert_ne!(shadowing, Word::NIL);
    assert_eq!(cdr(&mut ctx, shadowing), Ok(Word::NIL));
    Package::from_word(user)
        .use_package(&mut ctx, &runtime, base)
        .unwrap_or_else(|error| panic!("test failure: {error:?}"));
    assert!(
        !Package::from_word(user)
            .unintern(&mut ctx, &runtime, name)
            .unwrap_or_else(|error| panic!("test failure: {error:?}"))
    );
    assert!(
        Package::from_word(base)
            .unintern(&mut ctx, &runtime, name)
            .unwrap_or_else(|error| panic!("test failure: {error:?}"))
    );
    assert_eq!(
        Package::from_word(base).find_symbol(&mut ctx, name),
        Ok(None)
    );
    assert_eq!(symbol_package(&ctx, symbol), Ok(Word::NIL));
    assert_eq!(
        Package::from_word(base).shadowing_symbols(&ctx),
        Ok(Word::NIL)
    );
}

#[test]
fn export_unexport_use_unuse_shadow_and_nickname_are_idempotent() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("test failure: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("test failure: {error:?}"));
    let base = Package::new(&mut ctx, &runtime, "BASE")
        .unwrap_or_else(|error| panic!("test failure: {error:?}"))
        .as_word();
    let user = Package::new(&mut ctx, &runtime, "USER")
        .unwrap_or_else(|error| panic!("test failure: {error:?}"))
        .as_word();
    let name = string(&mut ctx, &runtime, "NAME");
    let (symbol, _) = Package::from_word(base)
        .intern(&mut ctx, &runtime, "NAME")
        .unwrap_or_else(|error| panic!("test failure: {error:?}"));
    assert!(
        Package::from_word(base)
            .export(&mut ctx, &runtime, name)
            .unwrap_or_else(|error| panic!("test failure: {error:?}"))
    );
    assert!(
        Package::from_word(base)
            .export(&mut ctx, &runtime, name)
            .unwrap_or_else(|error| panic!("test failure: {error:?}"))
    );
    assert!(
        Package::from_word(user)
            .use_package(&mut ctx, &runtime, base)
            .unwrap_or_else(|error| panic!("test failure: {error:?}"))
    );
    assert!(
        !Package::from_word(user)
            .use_package(&mut ctx, &runtime, base)
            .unwrap_or_else(|error| panic!("test failure: {error:?}"))
    );
    assert_eq!(
        Package::from_word(user).find_symbol(&mut ctx, name),
        Ok(Some((symbol, FindStatus::Inherited)))
    );
    assert!(
        Package::from_word(user)
            .export(&mut ctx, &runtime, name)
            .unwrap_or_else(|error| panic!("test failure: {error:?}"))
    );
    assert_eq!(
        Package::from_word(user).find_symbol(&mut ctx, name),
        Ok(Some((symbol, FindStatus::External)))
    );
    assert!(
        Package::from_word(user)
            .unuse_package(&mut ctx, base)
            .unwrap_or_else(|error| panic!("test failure: {error:?}"))
    );
    assert!(
        !Package::from_word(user)
            .unuse_package(&mut ctx, base)
            .unwrap_or_else(|error| panic!("test failure: {error:?}"))
    );
    Package::from_word(base)
        .shadow(&mut ctx, &runtime, name)
        .unwrap_or_else(|error| panic!("test failure: {error:?}"));
    Package::from_word(base)
        .shadow(&mut ctx, &runtime, name)
        .unwrap_or_else(|error| panic!("test failure: {error:?}"));
    assert_eq!(
        Package::from_word(base).find_symbol(&mut ctx, name),
        Ok(Some((symbol, FindStatus::External)))
    );
    assert!(
        Package::from_word(base)
            .unexport(&mut ctx, &runtime, name)
            .unwrap_or_else(|error| panic!("test failure: {error:?}"))
    );
    assert!(
        !Package::from_word(base)
            .unexport(&mut ctx, &runtime, name)
            .unwrap_or_else(|error| panic!("test failure: {error:?}"))
    );

    let nickname = string(&mut ctx, &runtime, "B");
    Package::from_word(base)
        .add_nickname(&mut ctx, &runtime, nickname)
        .unwrap_or_else(|error| panic!("test failure: {error:?}"));
    assert!(
        !Package::from_word(base)
            .add_nickname(&mut ctx, &runtime, nickname)
            .unwrap_or_else(|error| panic!("test failure: {error:?}"))
    );
    let mut registered = runtime
        .ensure_package(&mut ctx, "REGISTERED")
        .unwrap_or_else(|error| panic!("test failure: {error:?}"));
    let token = ncl_object::push_root(&mut ctx, &mut registered);
    Package::from_word(registered)
        .add_nickname(&mut ctx, &runtime, nickname)
        .unwrap_or_else(|error| panic!("test failure: {error:?}"));
    assert_eq!(runtime.find_package(&ctx, "B"), Some(registered));
    assert!(ncl_object::pop_root(&mut ctx, token));
}

#[test]
fn symbol_name_round_trip_is_preserved() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("test failure: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("test failure: {error:?}"));
    let package = Package::new(&mut ctx, &runtime, "NAMES")
        .unwrap_or_else(|error| panic!("test failure: {error:?}"));
    let (symbol, _) = package
        .intern(&mut ctx, &runtime, "ROUNDTRIP")
        .unwrap_or_else(|error| panic!("test failure: {error:?}"));
    let name = symbol_name(&ctx, symbol).unwrap_or_else(|error| panic!("test failure: {error:?}"));
    assert_eq!(ncl_object::string_length(&ctx, name), Ok(9));
    for (index, expected) in "ROUNDTRIP".chars().enumerate() {
        assert_eq!(string_ref(&ctx, name, index), Ok(expected));
    }
}
