#![allow(missing_docs)]

use ncl_object::package::{FindStatus, Package};
use ncl_object::{
    BuiltinName, BuiltinPackage, ObjectError, Runtime, ThreadContext, Word, cdr, make_string,
    pop_root, push_root, string_length, string_ref, symbol_name, symbol_package,
};

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

const OBJECT_OWNERSHIP: &str = include_str!("../ownership.tsv");
const SB_EXT_SURFACE: &str = include_str!("../../../conformance/sbcl/symbols/sb-ext.tsv");

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
fn object_owned_sb_ext_symbols_share_ncl_identity_and_surface_visibility() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    ncl_object::register(&mut ctx, &runtime)
        .unwrap_or_else(|error| panic!("NCL-GC register: {error:?}"));

    let ownership = ncl_ownership::rows_for_crate_from_str(OBJECT_OWNERSHIP, "ncl-object", 1)
        .unwrap_or_else(|error| panic!("ownership rows: {error:?}"));
    let sb_ext_rows: Vec<_> = ownership
        .iter()
        .filter(|row| row.package == "SB-EXT")
        .collect();
    assert_eq!(sb_ext_rows.len(), 14);

    for row in sb_ext_rows {
        let source_rows: Vec<_> = ownership
            .iter()
            .filter(|source| source.package.starts_with("NCL-") && source.symbol == row.symbol)
            .collect();
        assert_eq!(source_rows.len(), 1, "{} ownership source", row.symbol);
        let source_package_name = &source_rows[0].package;
        let mut source_package = runtime
            .find_package(&ctx, source_package_name)
            .unwrap_or_else(|| panic!("{source_package_name} package missing"));
        let source_package_token = ncl_object::push_root(&mut ctx, &mut source_package);
        let mut sb_package = runtime
            .find_package(&ctx, "SB-EXT")
            .unwrap_or_else(|| panic!("SB-EXT package missing"));
        let sb_package_token = ncl_object::push_root(&mut ctx, &mut sb_package);
        let mut name = string(&mut ctx, &runtime, &row.symbol);
        let name_token = ncl_object::push_root(&mut ctx, &mut name);

        let (source_symbol, _) = Package::from_word(source_package)
            .find_symbol(&mut ctx, name)
            .unwrap_or_else(|error| panic!("{} source lookup: {error:?}", row.symbol))
            .unwrap_or_else(|| panic!("{} source symbol missing", row.symbol));
        let mut source_symbol = source_symbol;
        let source_symbol_token = ncl_object::push_root(&mut ctx, &mut source_symbol);
        let home_before = symbol_package(&ctx, source_symbol)
            .unwrap_or_else(|error| panic!("{} home before import: {error:?}", row.symbol));
        assert_eq!(home_before, source_package, "{} source home", row.symbol);

        let (sb_symbol, sb_status) = Package::from_word(sb_package)
            .find_symbol(&mut ctx, name)
            .unwrap_or_else(|error| panic!("{} SB lookup: {error:?}", row.symbol))
            .unwrap_or_else(|| panic!("{} SB symbol missing", row.symbol));
        assert_eq!(sb_symbol, source_symbol, "{} symbol identity", row.symbol);
        let surface_row = SB_EXT_SURFACE
            .lines()
            .skip(1)
            .map(|line| line.split('\t').collect::<Vec<_>>())
            .find(|fields| fields.len() >= 4 && fields[0] == "SB-EXT" && fields[1] == row.symbol)
            .unwrap_or_else(|| panic!("{} SBCL surface row missing", row.symbol));
        assert_eq!(surface_row[2], "yes", "{} SBCL presence", row.symbol);
        let expected_status = match surface_row[3] {
            "yes" => FindStatus::External,
            "no" => FindStatus::Internal,
            external => panic!("{} invalid SBCL external flag {external}", row.symbol),
        };
        assert_eq!(sb_status, expected_status, "{} external status", row.symbol);
        let home_after = symbol_package(&ctx, sb_symbol)
            .unwrap_or_else(|error| panic!("{} home after import: {error:?}", row.symbol));
        assert_eq!(home_after, home_before, "{} home package", row.symbol);

        assert!(ncl_object::pop_root(&mut ctx, source_symbol_token));
        assert!(ncl_object::pop_root(&mut ctx, name_token));
        assert!(ncl_object::pop_root(&mut ctx, sb_package_token));
        assert!(ncl_object::pop_root(&mut ctx, source_package_token));
    }
}

#[test]
fn register_sb_symbol_honors_external_flag_and_rejects_invalid_inputs() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));

    let mut source = runtime
        .ensure_package(&mut ctx, BuiltinPackage::NclTest.as_str())
        .unwrap_or_else(|error| panic!("NCL-TEST package: {error:?}"));
    let source_token = push_root(&mut ctx, &mut source);
    let mut name = string(&mut ctx, &runtime, "R6-PRIVATE-SYMBOL");
    let name_token = push_root(&mut ctx, &mut name);
    let (mut source_symbol, _) = Package::from_word(source)
        .intern(&mut ctx, &runtime, "R6-PRIVATE-SYMBOL")
        .unwrap_or_else(|error| panic!("source intern: {error:?}"));
    let symbol_token = push_root(&mut ctx, &mut source_symbol);

    runtime
        .register_sb_symbol(
            &mut ctx,
            BuiltinPackage::NclTest,
            BuiltinPackage::SbDebug,
            BuiltinName::new("R6-PRIVATE-SYMBOL"),
            false,
        )
        .unwrap_or_else(|error| panic!("SB import: {error:?}"));
    let mut target = runtime
        .find_package(&ctx, BuiltinPackage::SbDebug.as_str())
        .unwrap_or_else(|| panic!("SB-DEBUG package missing"));
    let target_token = push_root(&mut ctx, &mut target);
    assert_eq!(
        Package::from_word(target).find_symbol(&mut ctx, name),
        Ok(Some((source_symbol, FindStatus::Internal)))
    );
    assert_eq!(symbol_package(&ctx, source_symbol), Ok(source));

    for (source_package, target_package, symbol_name) in [
        (
            BuiltinPackage::CommonLisp,
            BuiltinPackage::SbDebug,
            "R6-PRIVATE-SYMBOL",
        ),
        (
            BuiltinPackage::NclTest,
            BuiltinPackage::NclFfi,
            "R6-PRIVATE-SYMBOL",
        ),
        (
            BuiltinPackage::NclTest,
            BuiltinPackage::SbDebug,
            "R6-ABSENT-SYMBOL",
        ),
    ] {
        assert_eq!(
            runtime.register_sb_symbol(
                &mut ctx,
                source_package,
                target_package,
                BuiltinName::new(symbol_name),
                false,
            ),
            Err(ObjectError::Unsupported)
        );
    }

    assert!(pop_root(&mut ctx, target_token));
    assert!(pop_root(&mut ctx, symbol_token));
    assert!(pop_root(&mut ctx, name_token));
    assert!(pop_root(&mut ctx, source_token));
}

#[test]
fn register_sb_symbol_rejects_distinct_target_symbol() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    let source = runtime
        .ensure_package(&mut ctx, BuiltinPackage::NclTest.as_str())
        .unwrap_or_else(|error| panic!("NCL-TEST package: {error:?}"));
    let target = runtime
        .ensure_package(&mut ctx, BuiltinPackage::SbDebug.as_str())
        .unwrap_or_else(|error| panic!("SB-DEBUG package: {error:?}"));
    Package::from_word(source)
        .intern(&mut ctx, &runtime, "R6-CONFLICTING-SYMBOL")
        .unwrap_or_else(|error| panic!("NCL-TEST intern: {error:?}"));
    Package::from_word(target)
        .intern(&mut ctx, &runtime, "R6-CONFLICTING-SYMBOL")
        .unwrap_or_else(|error| panic!("SB-DEBUG intern: {error:?}"));

    assert_eq!(
        runtime.register_sb_symbol(
            &mut ctx,
            BuiltinPackage::NclTest,
            BuiltinPackage::SbDebug,
            BuiltinName::new("R6-CONFLICTING-SYMBOL"),
            false,
        ),
        Err(ObjectError::PackageConflict)
    );
}

#[test]
fn register_sb_symbol_propagates_unregistered_context_error() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();

    assert_eq!(
        runtime.register_sb_symbol(
            &mut ctx,
            BuiltinPackage::NclTest,
            BuiltinPackage::SbDebug,
            BuiltinName::new("R6-UNREGISTERED-CONTEXT"),
            false,
        ),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn register_sb_symbol_propagates_oversized_name_error() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    let name = Box::leak("A".repeat(8_500_000).into_boxed_str());

    assert!(matches!(
        runtime.register_sb_symbol(
            &mut ctx,
            BuiltinPackage::NclTest,
            BuiltinPackage::SbDebug,
            BuiltinName::new(name),
            false,
        ),
        Err(ObjectError::Storage(_))
    ));
}

#[test]
fn gc_registration_rejects_conflicting_sb_ext_symbol() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    let sb_ext = runtime
        .find_package(&ctx, BuiltinPackage::SbExt.as_str())
        .unwrap_or_else(|| panic!("SB-EXT package missing"));
    Package::from_word(sb_ext)
        .intern(&mut ctx, &runtime, "*AFTER-GC-HOOKS*")
        .unwrap_or_else(|error| panic!("SB-EXT intern: {error:?}"));

    assert_eq!(
        ncl_object::register(&mut ctx, &runtime),
        Err(ObjectError::PackageConflict)
    );
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
