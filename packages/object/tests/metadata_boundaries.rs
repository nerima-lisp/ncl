#![allow(missing_docs)]
#![allow(missing_docs)]

use ncl_object::{
    make_cons, make_string, LispError, ObjectError, Package, PackageError, Runtime, StringObject,
    ThreadContext, Word,
};

fn string(ctx: &mut ThreadContext, runtime: &Runtime, value: &str) -> StringObject {
    let word = make_string(ctx, runtime, &value.chars().collect::<Vec<_>>())
        .unwrap_or_else(|error| panic!("string allocation: {error:?}"));
    StringObject::from_word(word)
}

fn context() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    (runtime, ctx)
}

#[test]
fn try_from_word_accepts_only_package_objects() {
    let (runtime, mut ctx) = context();
    let package = Package::new(&mut ctx, &runtime, "VALID")
        .unwrap_or_else(|error| panic!("package: {error:?}"));
    let string_word = string(&mut ctx, &runtime, "not-a-package").as_word();
    let cons_word = make_cons(&mut ctx, &runtime, Word::NIL, Word::NIL)
        .unwrap_or_else(|error| panic!("cons allocation: {error:?}"));

    assert_eq!(Package::try_from_word(&ctx, package.as_word()), Ok(package));
    assert_eq!(
        Package::try_from_word(&ctx, Word::NIL),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        Package::try_from_word(&ctx, Word::fixnum(7)),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        Package::try_from_word(&ctx, string_word),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        Package::try_from_word(&ctx, cons_word),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn local_nickname_resolution_returns_the_target_or_none() {
    let (runtime, mut ctx) = context();
    let owner = Package::new(&mut ctx, &runtime, "OWNER")
        .unwrap_or_else(|error| panic!("owner: {error:?}"));
    let target = Package::new(&mut ctx, &runtime, "TARGET")
        .unwrap_or_else(|error| panic!("target: {error:?}"));
    let nickname = string(&mut ctx, &runtime, "LOCAL");
    let entry = make_cons(&mut ctx, &runtime, nickname.as_word(), target.as_word())
        .unwrap_or_else(|error| panic!("entry: {error:?}"));
    let entries = make_cons(&mut ctx, &runtime, entry, Word::NIL)
        .unwrap_or_else(|error| panic!("entries: {error:?}"));

    owner
        .set_local_nicknames(&mut ctx, entries)
        .unwrap_or_else(|error| panic!("set local nicknames: {error:?}"));
    let other = string(&mut ctx, &runtime, "OTHER");
    let longer = string(&mut ctx, &runtime, "LOCAL-LONG");
    assert_eq!(owner.local_nicknames(&ctx), Ok(entries));
    assert_eq!(
        owner.resolve_local_nickname(&ctx, nickname),
        Ok(Some(target))
    );
    assert_eq!(owner.resolve_local_nickname(&ctx, other), Ok(None));
    assert_eq!(owner.resolve_local_nickname(&ctx, longer), Ok(None));

    owner
        .set_local_nicknames(&mut ctx, Word::NIL)
        .unwrap_or_else(|error| panic!("clear local nicknames: {error:?}"));
    assert_eq!(owner.resolve_local_nickname(&ctx, nickname), Ok(None));
}

#[test]
fn malformed_local_nickname_lists_report_their_boundary_error() {
    let (runtime, mut ctx) = context();
    let owner = Package::new(&mut ctx, &runtime, "MALFORMED")
        .unwrap_or_else(|error| panic!("owner: {error:?}"));
    let wanted = string(&mut ctx, &runtime, "WANTED");

    owner
        .set_local_nicknames(&mut ctx, Word::fixnum(1))
        .unwrap_or_else(|error| panic!("set malformed outer list: {error:?}"));
    assert_eq!(
        owner.resolve_local_nickname(&ctx, wanted),
        Err(ObjectError::Layout)
    );

    let malformed_entry = make_cons(&mut ctx, &runtime, Word::fixnum(1), Word::NIL)
        .unwrap_or_else(|error| panic!("malformed entry: {error:?}"));
    let entries = make_cons(&mut ctx, &runtime, malformed_entry, Word::NIL)
        .unwrap_or_else(|error| panic!("entries: {error:?}"));
    owner
        .set_local_nicknames(&mut ctx, entries)
        .unwrap_or_else(|error| panic!("set malformed entry: {error:?}"));
    assert_eq!(
        owner.resolve_local_nickname(&ctx, wanted),
        Err(ObjectError::TypeError)
    );

    let malformed_entry = make_cons(&mut ctx, &runtime, wanted.as_word(), Word::fixnum(1))
        .unwrap_or_else(|error| panic!("malformed target entry: {error:?}"));
    let entries = make_cons(&mut ctx, &runtime, malformed_entry, Word::NIL)
        .unwrap_or_else(|error| panic!("target entries: {error:?}"));
    owner
        .set_local_nicknames(&mut ctx, entries)
        .unwrap_or_else(|error| panic!("set malformed target: {error:?}"));
    assert_eq!(
        owner.resolve_local_nickname(&ctx, wanted),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn locked_local_nickname_metadata_rejects_writes_until_unlocked() {
    let (runtime, mut ctx) = context();
    let package = Package::new(&mut ctx, &runtime, "LOCKED-METADATA")
        .unwrap_or_else(|error| panic!("package: {error:?}"));
    let nickname = string(&mut ctx, &runtime, "LOCAL");
    let entry = make_cons(&mut ctx, &runtime, nickname.as_word(), package.as_word())
        .unwrap_or_else(|error| panic!("entry: {error:?}"));

    package
        .set_locked(&mut ctx, true)
        .unwrap_or_else(|error| panic!("lock: {error:?}"));
    assert_eq!(package.is_locked(&ctx), Ok(true));
    assert_eq!(
        package.set_local_nicknames(&mut ctx, entry),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        ctx.take_pending_lisp_error(),
        Some(LispError::PackageError(PackageError::Locked))
    );
    assert_eq!(package.local_nicknames(&ctx), Ok(Word::NIL));

    package
        .set_locked(&mut ctx, false)
        .unwrap_or_else(|error| panic!("unlock: {error:?}"));
    package
        .set_local_nicknames(&mut ctx, entry)
        .unwrap_or_else(|error| panic!("set after unlock: {error:?}"));
    assert_eq!(package.is_locked(&ctx), Ok(false));
    assert_eq!(package.local_nicknames(&ctx), Ok(entry));
}
