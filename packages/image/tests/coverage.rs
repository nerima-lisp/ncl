#![allow(clippy::unwrap_used, reason = "tests assert on coverage failures")]

//! Ownership coverage gate for the public `NCL-IMAGE` surface.

use ncl_object::{Package, Runtime, ThreadContext, make_string, symbol_is_special};

const FUNCTIONS: &[&str] = &[
    "EXIT",
    "QUIT",
    "SAVE-LISP-AND-DIE",
    "OS-COLD-INIT-OR-REINIT",
    "OS-DEINIT",
    "OS-EXIT",
];

#[test]
fn registers_every_owned_symbol() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_image::register(&runtime).unwrap();

    let table = include_str!("../ownership.tsv");
    ncl_ownership::assert_crate_coverage_from_table(&runtime, &mut ctx, table, "ncl-image")
        .unwrap();

    let package = runtime.find_package(&ctx, "NCL-IMAGE").unwrap();
    let package = Package::from_word(package);
    let argv_name = make_string(
        &mut ctx,
        &runtime,
        &"*POSIX-ARGV*".chars().collect::<Vec<_>>(),
    )
    .unwrap();
    let (argv, _) = package.find_symbol(&mut ctx, argv_name).unwrap().unwrap();
    assert!(symbol_is_special(&ctx, argv).unwrap());

    for name in FUNCTIONS {
        assert!(
            runtime.function(&mut ctx, "NCL-IMAGE", name).is_some(),
            "{name}"
        );
    }
    assert!(runtime.class(&mut ctx, "EXIT").is_some());
}
