//! Integration tests for package lookup, symbol identity, and package semantics.

use ncl_object::package::{FindStatus, Package};
use ncl_object::{
    BuiltinPackage, Runtime, ThreadContext, Word, cdr, make_string, pop_root,
    push_root, string_length, string_ref, symbol_name, symbol_package,
};
use ncl_packages::{PackageListOperations, PackageOperations};

fn string(ctx: &mut ThreadContext, runtime: &Runtime, value: &str) -> Word {
    make_string(ctx, runtime, &value.chars().collect::<Vec<_>>())
        .unwrap_or_else(|error| panic!("string allocation: {error:?}"))
}
fn string_text(ctx: &ThreadContext, value: Word) -> String {
    let length =
        string_length(ctx, value).unwrap_or_else(|error| panic!("string length: {error:?}"));
    (0..length)
        .map(|index| {
            string_ref(ctx, value, index).unwrap_or_else(|error| panic!("string ref: {error:?}"))
        })
        .collect()
}

#[test]
fn package_documentation_round_trips() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));

    let package = Package::new(&mut ctx, &runtime, "DOCUMENTATION")
        .unwrap_or_else(|error| panic!("package: {error:?}"));
    assert_eq!(
        package
            .documentation(&ctx)
            .unwrap_or_else(|error| panic!("package documentation: {error:?}")),
        Word::NIL
    );
    let documentation = string(&mut ctx, &runtime, "package docs");
    package
        .set_documentation(&mut ctx, documentation)
        .unwrap_or_else(|error| panic!("set package documentation: {error:?}"));
    assert_eq!(
        string_text(
            &ctx,
            package
                .documentation(&ctx)
                .unwrap_or_else(|error| panic!("package documentation: {error:?}"))
        ),
        "package docs"
    );
}

#[test]
fn package_documentation_survives_full_collection() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    ctx.set_strict_forwarding(true);

    let mut package_word = Package::new(&mut ctx, &runtime, "DOCUMENTATION-GC")
        .unwrap_or_else(|error| panic!("package: {error:?}"))
        .as_word();
    let package_root = push_root(&mut ctx, &mut package_word);
    let expected = {
        let documentation = string(&mut ctx, &runtime, "docs retained by package slot");
        Package::from_word(package_word)
            .set_documentation(&mut ctx, documentation)
            .unwrap_or_else(|error| panic!("set package documentation: {error:?}"));
        string_text(
            &ctx,
            Package::from_word(package_word)
                .documentation(&ctx)
                .unwrap_or_else(|error| panic!("package documentation before GC: {error:?}")),
        )
    };

    ctx.collect(true)
        .unwrap_or_else(|error| panic!("full collection: {error:?}"));
    let actual = string_text(
        &ctx,
        Package::from_word(package_word)
            .documentation(&ctx)
            .unwrap_or_else(|error| panic!("package documentation after GC: {error:?}")),
    );
    assert_eq!(actual, expected);
    assert!(pop_root(&mut ctx, package_root));
}

const SB_PACKAGES: [(BuiltinPackage, &str); 16] = [
    (BuiltinPackage::SbExt, "SB-EXT"),
    (BuiltinPackage::SbThread, "SB-THREAD"),
    (BuiltinPackage::SbAlien, "SB-ALIEN"),
    (BuiltinPackage::SbSys, "SB-SYS"),
    (BuiltinPackage::SbMop, "SB-MOP"),
    (BuiltinPackage::SbGray, "SB-GRAY"),
    (BuiltinPackage::SbUnicode, "SB-UNICODE"),
    (BuiltinPackage::SbDebug, "SB-DEBUG"),
    (BuiltinPackage::SbAlienInternals, "SB-ALIEN-INTERNALS"),
    (BuiltinPackage::SbC, "SB-C"),
    (BuiltinPackage::SbImpl, "SB-IMPL"),
    (BuiltinPackage::SbInt, "SB-INT"),
    (BuiltinPackage::SbKernel, "SB-KERNEL"),
    (BuiltinPackage::SbPcl, "SB-PCL"),
    (BuiltinPackage::SbUnix, "SB-UNIX"),
    (BuiltinPackage::SbVm, "SB-VM"),
];

const NCL_PACKAGES: [(BuiltinPackage, &str); 14] = [
    (BuiltinPackage::CommonLisp, "COMMON-LISP"),
    (BuiltinPackage::AsdfInterface, "ASDF/INTERFACE"),
    (BuiltinPackage::UiopDriver, "UIOP/DRIVER"),
    (BuiltinPackage::NclThreads, "NCL-THREADS"),
    (BuiltinPackage::NclFfi, "NCL-FFI"),
    (BuiltinPackage::NclMop, "NCL-MOP"),
    (BuiltinPackage::NclGray, "NCL-GRAY"),
    (BuiltinPackage::NclGc, "NCL-GC"),
    (BuiltinPackage::NclImage, "NCL-IMAGE"),
    (BuiltinPackage::NclUnicode, "NCL-UNICODE"),
    (BuiltinPackage::NclOs, "NCL-OS"),
    (BuiltinPackage::NclExt, "NCL-EXT"),
    (BuiltinPackage::NclSys, "NCL-SYS"),
    (BuiltinPackage::NclTest, "NCL-TEST"),
];

#[test]
fn runtime_registers_distinct_sb_packages() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));

    for (builtin, name) in SB_PACKAGES {
        assert_eq!(builtin.as_str(), name);
        let package = runtime
            .find_package(&ctx, name)
            .unwrap_or_else(|| panic!("{name} package missing"));
        let actual_name = Package::from_word(package)
            .name(&ctx)
            .unwrap_or_else(|error| panic!("{name} package name: {error:?}"));
        assert_eq!(string_text(&ctx, actual_name), name);
    }
}

#[test]
fn builtin_package_names_match_existing_ncl_packages() {
    for (builtin, name) in NCL_PACKAGES {
        assert_eq!(std::hint::black_box(builtin).as_str(), name);
    }
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
        let (reinterned_symbol, reinterned_status) = Package::from_word(user_word)
            .intern(&mut ctx, &runtime, name)
            .unwrap_or_else(|error| panic!("user reintern {name}: {error:?}"));
        assert_eq!(reinterned_symbol, common_symbol);
        assert_eq!(reinterned_status, FindStatus::Inherited);
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

    Package::from_word(target)
        .shadowing_import(&mut ctx, &runtime, other)
        .unwrap_or_else(|error| panic!("shadowing import: {error:?}"));
    assert_eq!(
        Package::from_word(target).find_symbol(&mut ctx, name),
        Ok(Some((other, FindStatus::Internal)))
    );
    assert_eq!(symbol_package(&ctx, symbol), Ok(home));
    assert_eq!(symbol_package(&ctx, other), Ok(target));
    let shadowing = Package::from_word(target)
        .shadowing_symbols(&ctx)
        .unwrap_or_else(|error| panic!("shadowing symbols: {error:?}"));
    assert_eq!(ncl_object::car(&ctx, shadowing), Ok(other));
    assert_eq!(ncl_object::cdr(&ctx, shadowing), Ok(Word::NIL));
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
    assert_eq!(cdr(&ctx, shadowing), Ok(Word::NIL));
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
fn package_use_export_shadow_and_unexport_are_idempotent() {
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

#[test]
fn package_nickname_intern_and_import_update_metadata() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    let package = Package::new(&mut ctx, &runtime, "METADATA")
        .unwrap_or_else(|error| panic!("package: {error:?}"));

    let (interned, status) = package
        .intern(&mut ctx, &runtime, "INTERNED")
        .unwrap_or_else(|error| panic!("intern: {error:?}"));
    assert_eq!(status, FindStatus::Internal);
    assert_eq!(symbol_package(&ctx, interned), Ok(package.as_word()));
    let (reinterned, reinterned_status) = package
        .intern(&mut ctx, &runtime, "INTERNED")
        .unwrap_or_else(|error| panic!("reintern: {error:?}"));
    assert_eq!(reinterned, interned);
    assert_eq!(reinterned_status, FindStatus::Internal);
    let (unicode_symbol, unicode_status) = package
        .intern(&mut ctx, &runtime, "λ-名前")
        .unwrap_or_else(|error| panic!("unicode intern: {error:?}"));
    assert_eq!(unicode_status, FindStatus::Internal);
    let (reinterned_unicode, reinterned_unicode_status) = package
        .intern(&mut ctx, &runtime, "λ-名前")
        .unwrap_or_else(|error| panic!("unicode reintern: {error:?}"));
    assert_eq!(reinterned_unicode, unicode_symbol);
    assert_eq!(reinterned_unicode_status, FindStatus::Internal);

    let mut nickname = string(&mut ctx, &runtime, "META");
    let nickname_token = push_root(&mut ctx, &mut nickname);
    assert_eq!(package.add_nickname(&mut ctx, &runtime, nickname), Ok(true));
    let nicknames = package
        .nicknames(&ctx)
        .unwrap_or_else(|error| panic!("nicknames: {error:?}"));
    assert_eq!(ncl_object::car(&ctx, nicknames), Ok(nickname));
    assert_eq!(
        package.remove_nickname(&mut ctx, &runtime, nickname),
        Ok(true)
    );
    assert_eq!(
        package.remove_nickname(&mut ctx, &runtime, nickname),
        Ok(false)
    );
    assert_eq!(package.nicknames(&ctx), Ok(Word::NIL));
    assert!(pop_root(&mut ctx, nickname_token));

    let other_target = Package::new(&mut ctx, &runtime, "LOCAL-OTHER")
        .unwrap_or_else(|error| panic!("other target: {error:?}"));
    let local_target = Package::new(&mut ctx, &runtime, "LOCAL-TARGET")
        .unwrap_or_else(|error| panic!("local target: {error:?}"));
    let other_nickname = string(&mut ctx, &runtime, "OTHER");
    let local_nickname = string(&mut ctx, &runtime, "LOCAL");
    let other_entry =
        ncl_object::make_cons(&mut ctx, &runtime, other_nickname, other_target.as_word())
            .unwrap_or_else(|error| panic!("other local nickname: {error:?}"));
    let local_entry =
        ncl_object::make_cons(&mut ctx, &runtime, local_nickname, local_target.as_word())
            .unwrap_or_else(|error| panic!("local nickname: {error:?}"));
    let local_tail = ncl_object::make_cons(&mut ctx, &runtime, local_entry, Word::NIL)
        .unwrap_or_else(|error| panic!("local nickname tail: {error:?}"));
    let local_entries = ncl_object::make_cons(&mut ctx, &runtime, other_entry, local_tail)
        .unwrap_or_else(|error| panic!("local nickname entries: {error:?}"));
    package
        .set_local_nicknames(&mut ctx, local_entries)
        .unwrap_or_else(|error| panic!("set local nicknames: {error:?}"));
    assert_eq!(
        package.resolve_local_nickname(&ctx, ncl_object::StringObject::from_word(local_nickname)),
        Ok(Some(local_target))
    );
    let missing_nickname = string(&mut ctx, &runtime, "MISSING");
    assert_eq!(
        package.resolve_local_nickname(&ctx, ncl_object::StringObject::from_word(missing_nickname)),
        Ok(None)
    );

    let name = string(&mut ctx, &runtime, "IMPORTED");
    let uninterned = ncl_object::make_symbol(&mut ctx, &runtime, name)
        .unwrap_or_else(|error| panic!("uninterned symbol: {error:?}"));
    assert_eq!(symbol_package(&ctx, uninterned), Ok(Word::NIL));
    package
        .import(&mut ctx, &runtime, name, uninterned)
        .unwrap_or_else(|error| panic!("import: {error:?}"));
    assert_eq!(symbol_package(&ctx, uninterned), Ok(package.as_word()));
    assert_eq!(
        package.find_symbol(&mut ctx, name),
        Ok(Some((uninterned, FindStatus::Internal)))
    );
}
