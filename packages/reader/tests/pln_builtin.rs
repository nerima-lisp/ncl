#![allow(clippy::unwrap_used, reason = "tests assert on reader behavior")]
#![allow(missing_docs)]

use ncl_object::{
    FunctionObject, Runtime, ThreadContext, make_string, symbol_name, symbol_package,
};
use ncl_reader::{ReadOptions, read_from_string};

use ncl_lib_packages::register;

fn standard(runtime: &Runtime, ctx: &mut ThreadContext) -> ReadOptions {
    ReadOptions::standard(ctx, runtime).unwrap()
}

fn name_of(ctx: &ThreadContext, word: ncl_object::Word) -> String {
    let name = symbol_name(ctx, word).unwrap();
    let length = ncl_object::string_length(ctx, name).unwrap();
    (0..length)
        .map(|i| ncl_object::string_ref(ctx, name, i).unwrap())
        .collect()
}

#[test]
fn current_package_local_nickname_builtin_is_used_by_reader() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    register(&runtime).unwrap();

    let mut current = runtime.ensure_package(&mut ctx, "CURRENT-BUILTIN").unwrap();
    let current_token = ncl_object::push_root(&mut ctx, &mut current);
    let mut target = runtime.ensure_package(&mut ctx, "TARGET-BUILTIN").unwrap();
    let target_token = ncl_object::push_root(&mut ctx, &mut target);
    let mut nickname = make_string(&mut ctx, &runtime, &['L', 'O', 'C', 'A', 'L']).unwrap();
    let nickname_token = ncl_object::push_root(&mut ctx, &mut nickname);
    let add = runtime
        .function(&mut ctx, "NCL-EXT", "ADD-PACKAGE-LOCAL-NICKNAME")
        .unwrap();

    assert_eq!(
        runtime
            .call_builtin(
                &mut ctx,
                FunctionObject::try_from(add).unwrap(),
                &[nickname, target, current],
            )
            .unwrap(),
        nickname
    );

    let mut opts = standard(&runtime, &mut ctx);
    opts.set_current_package("CURRENT-BUILTIN").unwrap();
    let symbol = read_from_string(&mut ctx, &runtime, "LOCAL:ITEM", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_package(&ctx, symbol).unwrap(), target);
    assert_eq!(name_of(&ctx, symbol), "ITEM");

    assert!(ncl_object::pop_root(&mut ctx, nickname_token));
    assert!(ncl_object::pop_root(&mut ctx, target_token));
    assert!(ncl_object::pop_root(&mut ctx, current_token));
}
