#![allow(clippy::unwrap_used, reason = "tests assert on reader behavior")]
#![allow(missing_docs)]

use ncl_object::{
    Package, Runtime, ThreadContext, Word, make_simple_vector, simple_vector_ref, structure_ref,
};
use ncl_reader::{ReadOptions, read_from_string};

#[test]
fn reads_structure_dispatch_literal_into_registered_layout() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let user = runtime
        .ensure_package(&mut ctx, "COMMON-LISP-USER")
        .unwrap();
    let (name, _) = Package::from_word(user)
        .intern(&mut ctx, &runtime, "POINT")
        .unwrap();
    let (slot_name, _) = Package::from_word(user)
        .intern(&mut ctx, &runtime, "X")
        .unwrap();
    let descriptor =
        make_simple_vector(&mut ctx, &runtime, &[slot_name, Word::NIL, Word::NIL]).unwrap();
    let slots = make_simple_vector(&mut ctx, &runtime, &[descriptor]).unwrap();
    let effective = make_simple_vector(&mut ctx, &runtime, &[descriptor]).unwrap();
    let class = make_simple_vector(
        &mut ctx,
        &runtime,
        &[name, Word::NIL, slots, Word::fixnum(1), effective],
    )
    .unwrap();
    runtime.define_class(&mut ctx, "POINT", class).unwrap();
    let qualified = runtime.structure_class_name(&ctx, name).unwrap();
    runtime.define_class(&mut ctx, qualified, class).unwrap();
    let layout = runtime.register_structure_layout(1).unwrap();
    runtime
        .register_structure_class_with_parent(&ctx, layout, None, name)
        .unwrap();
    let opts = ReadOptions::standard(&mut ctx, &runtime).unwrap();
    let structure = read_from_string(&mut ctx, &runtime, "#S(POINT :X 8)", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(structure_ref(&ctx, structure, 0).unwrap(), Word::fixnum(8));
}

#[test]
fn structure_registry_keeps_same_named_packages_distinct() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let mut registrations = Vec::new();
    for package_name in ["STRUCT-ONE", "STRUCT-TWO"] {
        let package = runtime.ensure_package(&mut ctx, package_name).unwrap();
        let (name, _) = Package::from_word(package)
            .intern(&mut ctx, &runtime, "POINT")
            .unwrap();
        let (slot_name, _) = Package::from_word(package)
            .intern(&mut ctx, &runtime, "X")
            .unwrap();
        let descriptor =
            make_simple_vector(&mut ctx, &runtime, &[slot_name, Word::NIL, Word::NIL]).unwrap();
        let slots = make_simple_vector(&mut ctx, &runtime, &[descriptor]).unwrap();
        let class = make_simple_vector(
            &mut ctx,
            &runtime,
            &[name, Word::NIL, slots, Word::fixnum(1), slots],
        )
        .unwrap();
        runtime.define_class(&mut ctx, "POINT", class).unwrap();
        let qualified = runtime.structure_class_name(&ctx, name).unwrap();
        runtime.define_class(&mut ctx, qualified, class).unwrap();
        let layout = runtime.register_structure_layout(1).unwrap();
        runtime
            .register_structure_class_with_parent(&ctx, layout, None, name)
            .unwrap();
        registrations.push((name, layout));
    }
    for (name, layout) in registrations {
        let class = runtime.structure_class(&mut ctx, layout).unwrap();
        assert_eq!(simple_vector_ref(&ctx, class, 0).unwrap(), name);
    }
}
