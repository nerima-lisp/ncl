#![allow(missing_docs)]

use ncl_object::hash_table::{HashTable, HashTest, Weakness};
use ncl_object::package::{FindStatus, Package};
use ncl_object::{
    Runtime, ThreadContext, Word, car, make_cons, make_instance, make_simple_vector,
    make_string, make_symbol, set_symbol_value, simple_vector_ref, simple_vector_set, slot_ref,
};

#[test]
fn allocation_paths_survive_collection_before_every_allocation() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap_or_else(|error| panic!("register: {error:?}"));
    ctx.set_gc_stress(true);

    let mut string = make_string(&mut ctx, &runtime, &['S', 'T', 'R', 'E', 'S', 'S'])
        .unwrap_or(Word::NIL);
    let string_token = ncl_object::push_root(&mut ctx, &mut string);
    assert_eq!(ncl_object::string_ref(&ctx, string, 0), Ok('S'));

    let mut cons = make_cons(&mut ctx, &runtime, string, Word::fixnum(7)).unwrap_or(Word::NIL);
    let cons_token = ncl_object::push_root(&mut ctx, &mut cons);
    assert_eq!(car(&mut ctx, cons), Ok(string));

    let mut vector = make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1), Word::fixnum(2)])
        .unwrap_or(Word::NIL);
    let vector_token = ncl_object::push_root(&mut ctx, &mut vector);
    simple_vector_set(&mut ctx, vector, 1, cons).unwrap_or_else(|error| panic!("vector set: {error:?}"));
    assert_eq!(simple_vector_ref(&ctx, vector, 1), Ok(cons));

    let mut symbol = make_symbol(&mut ctx, &runtime, string).unwrap_or(Word::NIL);
    let symbol_token = ncl_object::push_root(&mut ctx, &mut symbol);
    set_symbol_value(&mut ctx, symbol, Word::fixnum(42)).unwrap_or_else(|error| panic!("symbol: {error:?}"));

    let mut table = HashTable::new(&mut ctx, &runtime, HashTest::Equal, Weakness::None)
        .unwrap_or_else(|error| panic!("table: {error:?}"))
        .as_word();
    let table_token = ncl_object::push_root(&mut ctx, &mut table);
    HashTable::from(table)
        .insert(&mut ctx, &runtime, string, symbol)
        .unwrap_or_else(|error| panic!("insert: {error:?}"));
    assert_eq!(HashTable::from(table).get(&mut ctx, string), Ok(Some(symbol)));
    assert_eq!(HashTable::from(table).remove(&mut ctx, &runtime, string), Ok(Some(symbol)));

    let mut package = Package::new(&mut ctx, &runtime, "STRESS")
        .unwrap_or_else(|error| panic!("package: {error:?}"))
        .as_word();
    let package_token = ncl_object::push_root(&mut ctx, &mut package);
    let package = Package::from(package);
    let (interned, status) = package.intern(&mut ctx, &runtime, "NAME")
        .unwrap_or_else(|error| panic!("intern: {error:?}"));
    assert_eq!(status, FindStatus::Internal);
    let mut interned = interned;
    let interned_token = ncl_object::push_root(&mut ctx, &mut interned);
    let lookup_name = make_string(&mut ctx, &runtime, &['N', 'A', 'M', 'E']).unwrap_or(Word::NIL);
    assert_eq!(package.find_symbol(&mut ctx, lookup_name), Ok(Some((interned, FindStatus::Internal))));
    let export_name = make_string(&mut ctx, &runtime, &['N', 'A', 'M', 'E']).unwrap_or(Word::NIL);
    package.export(&mut ctx, &runtime, export_name).unwrap_or_else(|error| panic!("export: {error:?}"));
    let unexport_name = make_string(&mut ctx, &runtime, &['N', 'A', 'M', 'E']).unwrap_or(Word::NIL);
    package.unexport(&mut ctx, &runtime, unexport_name).unwrap_or_else(|error| panic!("unexport: {error:?}"));
    package.shadow(&mut ctx, &runtime, string).unwrap_or_else(|error| panic!("shadow: {error:?}"));
    let gensym = package.gensym(&mut ctx, &runtime).unwrap_or(Word::NIL);
    assert_ne!(gensym, Word::NIL);
    package.unintern(&mut ctx, &runtime, string).unwrap_or_else(|error| panic!("unintern: {error:?}"));

    let mut instance = make_instance(&mut ctx, &runtime, symbol, &[cons]).unwrap_or_else(|error| panic!("instance: {error:?}")).into();
    let instance_token = ncl_object::push_root(&mut ctx, &mut instance);
    assert_eq!(slot_ref(&ctx, instance.into(), 0), Ok(cons));

    let class_name = "STRESS-CLASS".to_owned();
    runtime.define_class(class_name.clone(), symbol).unwrap_or_else(|error| panic!("class: {error:?}"));
    assert_eq!(runtime.class(&class_name), Some(symbol));
    runtime.define_function("NCL", "STRESS-FUNCTION", symbol).unwrap_or_else(|error| panic!("function: {error:?}"));
    assert_eq!(runtime.function("NCL", "STRESS-FUNCTION"), Some(symbol));

    assert!(ncl_object::pop_root(&mut ctx, instance_token));
    assert!(ncl_object::pop_root(&mut ctx, interned_token));
    assert!(ncl_object::pop_root(&mut ctx, package_token));
    assert!(ncl_object::pop_root(&mut ctx, table_token));
    assert!(ncl_object::pop_root(&mut ctx, symbol_token));
    assert!(ncl_object::pop_root(&mut ctx, vector_token));
    assert!(ncl_object::pop_root(&mut ctx, cons_token));
    assert!(ncl_object::pop_root(&mut ctx, string_token));
}
