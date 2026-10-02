#![allow(missing_docs)]
#![allow(clippy::unwrap_used, reason = "tests assert on DEFSTRUCT metadata")]

use ncl_object::{FunctionObject, Package, Runtime, ThreadContext, Word};

fn list(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Word {
    let mut scope = ncl_object::Scope::new(ctx);
    let roots = scope.root_many(
        &values
            .iter()
            .copied()
            .map(ncl_object::Local::from_word)
            .collect::<Vec<_>>(),
    );
    let result = scope.make_list(runtime, &roots).unwrap();
    scope.get(result).as_word()
}

#[test]
fn defstruct_include_registers_layout_and_overrides_inherited_default() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_clos::register(&runtime).unwrap();
    let package = runtime.find_package(&ctx, "COMMON-LISP-USER").unwrap();
    let intern = |name: &str, ctx: &mut ThreadContext| {
        Package::from_word(package)
            .intern(ctx, &runtime, name)
            .unwrap()
            .0
    };
    let defstruct = FunctionObject::try_from(
        runtime
            .function(&mut ctx, "COMMON-LISP", "DEFSTRUCT")
            .unwrap(),
    )
    .unwrap();
    let base = intern("N26-BASE", &mut ctx);
    let child = intern("N26-CHILD", &mut ctx);
    let value = intern("VALUE", &mut ctx);
    let include = intern(":INCLUDE", &mut ctx);

    let base_slot = list(&mut ctx, &runtime, &[value, Word::fixnum(7)]);
    let base_form = list(&mut ctx, &runtime, &[defstruct.as_word(), base, base_slot]);
    runtime
        .call_builtin(&mut ctx, defstruct, &[base_form])
        .unwrap();

    let override_slot = list(&mut ctx, &runtime, &[value, Word::fixnum(9)]);
    let include_form = list(&mut ctx, &runtime, &[include, base, override_slot]);
    let child_header = list(&mut ctx, &runtime, &[child, include_form]);
    let child_form = list(&mut ctx, &runtime, &[defstruct.as_word(), child_header]);
    runtime
        .call_builtin(&mut ctx, defstruct, &[child_form])
        .unwrap();

    let child_class = runtime.class(&mut ctx, "N26-CHILD").unwrap();
    let effective = ncl_object::simple_vector_ref(&ctx, child_class, 4).unwrap();
    let descriptor = ncl_object::simple_vector_ref(&ctx, effective, 0).unwrap();
    assert_eq!(
        ncl_object::simple_vector_ref(&ctx, descriptor, 0),
        Ok(value)
    );
    assert_eq!(
        ncl_object::simple_vector_ref(&ctx, descriptor, 1),
        Ok(Word::fixnum(9))
    );
    assert_eq!(ncl_object::simple_vector_length(&ctx, effective), Ok(1));

    let base_layout = runtime.structure_layout_for_symbol(&ctx, base).unwrap();
    let child_layout = runtime.structure_layout_for_symbol(&ctx, child).unwrap();
    assert!(runtime.structure_layout_is_a(child_layout, base_layout));
    assert_eq!(
        runtime.structure_class(&mut ctx, child_layout),
        Some(child_class)
    );
}
