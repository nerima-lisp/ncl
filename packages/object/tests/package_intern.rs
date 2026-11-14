//! Borrowed-name package lookup tests.

use ncl_object::hash_table::HashTable;
use ncl_object::package::{FindStatus, INTERNAL, USE_LIST};
use ncl_object::{
    Package, Runtime, ThreadContext, Word, get, make_cons, make_string, pop_root, push_root, put,
    widetag,
};

#[test]
fn intern_reuses_internal_symbols_across_string_hash_collisions() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    let mut package = Package::new(&mut ctx, &runtime, "BORROWED-LOOKUP")
        .unwrap_or_else(|error| panic!("package: {error:?}"))
        .as_word();
    let package_token = push_root(&mut ctx, &mut package);

    // These names collide in the default eight-slot table and exercise string probing.
    let (mut first, status) = Package::from_word(package)
        .intern(&mut ctx, &runtime, "A")
        .unwrap_or_else(|error| panic!("first intern: {error:?}"));
    assert_eq!(status, FindStatus::Internal);
    let first_token = push_root(&mut ctx, &mut first);
    let allocated = ctx.thread_access().bytes_cons_since_gc();
    let (same, status) = Package::from_word(package)
        .intern(&mut ctx, &runtime, "A")
        .unwrap_or_else(|error| panic!("reintern: {error:?}"));
    assert_eq!((same, status), (first, FindStatus::Internal));
    assert_eq!(ctx.thread_access().bytes_cons_since_gc(), allocated);

    let (mut collision, status) = Package::from_word(package)
        .intern(&mut ctx, &runtime, "I")
        .unwrap_or_else(|error| panic!("colliding intern: {error:?}"));
    assert_eq!(status, FindStatus::Internal);
    let collision_token = push_root(&mut ctx, &mut collision);
    assert_ne!(collision, first);
    let (different_length, status) = Package::from_word(package)
        .intern(&mut ctx, &runtime, "AH")
        .unwrap_or_else(|error| panic!("different-length collision: {error:?}"));
    assert_eq!(status, FindStatus::Internal);
    assert_ne!(different_length, first);
    assert_ne!(different_length, collision);

    let mut key =
        make_string(&mut ctx, &runtime, &['A']).unwrap_or_else(|error| panic!("key: {error:?}"));
    let key_token = push_root(&mut ctx, &mut key);
    let internal = HashTable::from_word(
        get(&ctx, package, widetag::PACKAGE, INTERNAL)
            .unwrap_or_else(|error| panic!("internal table: {error:?}")),
    );
    assert_eq!(internal.remove(&mut ctx, &runtime, key), Ok(Some(first)));
    let (same_collision, status) = Package::from_word(package)
        .intern(&mut ctx, &runtime, "I")
        .unwrap_or_else(|error| panic!("lookup after removal: {error:?}"));
    assert_eq!((same_collision, status), (collision, FindStatus::Internal));

    assert!(pop_root(&mut ctx, key_token));
    assert!(pop_root(&mut ctx, collision_token));
    assert!(pop_root(&mut ctx, first_token));
    assert!(pop_root(&mut ctx, package_token));
}

#[test]
fn intern_reuses_external_and_inherited_symbols_without_heap_allocation() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    let mut common = runtime
        .find_package(&ctx, "COMMON-LISP")
        .unwrap_or_else(|| panic!("COMMON-LISP package missing"));
    let common_token = push_root(&mut ctx, &mut common);
    let (mut external, status) = Package::from_word(common)
        .intern(&mut ctx, &runtime, "BORROWED-EXTERNAL")
        .unwrap_or_else(|error| panic!("external intern: {error:?}"));
    assert_eq!(status, FindStatus::External);
    let external_token = push_root(&mut ctx, &mut external);
    let allocated = ctx.thread_access().bytes_cons_since_gc();
    let (same_external, status) = Package::from_word(common)
        .intern(&mut ctx, &runtime, "BORROWED-EXTERNAL")
        .unwrap_or_else(|error| panic!("external reintern: {error:?}"));
    assert_eq!((same_external, status), (external, FindStatus::External));
    assert_eq!(ctx.thread_access().bytes_cons_since_gc(), allocated);

    let mut user = Package::new(&mut ctx, &runtime, "BORROWED-USER")
        .unwrap_or_else(|error| panic!("user package: {error:?}"))
        .as_word();
    let user_token = push_root(&mut ctx, &mut user);
    let mut use_list = make_cons(&mut ctx, &runtime, common, Word::NIL)
        .unwrap_or_else(|error| panic!("use list: {error:?}"));
    let use_list_token = push_root(&mut ctx, &mut use_list);
    put(&mut ctx, user, USE_LIST, use_list)
        .unwrap_or_else(|error| panic!("set use list: {error:?}"));

    let (inherited, status) = Package::from_word(user)
        .intern(&mut ctx, &runtime, "BORROWED-EXTERNAL")
        .unwrap_or_else(|error| panic!("inherited intern: {error:?}"));
    assert_eq!((inherited, status), (external, FindStatus::Inherited));

    assert!(pop_root(&mut ctx, use_list_token));
    assert!(pop_root(&mut ctx, user_token));
    assert!(pop_root(&mut ctx, external_token));
    assert!(pop_root(&mut ctx, common_token));
}
