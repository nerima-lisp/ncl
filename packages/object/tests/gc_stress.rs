#![allow(missing_docs)]

use ncl_object::hash_table::{HashTable, HashTest, Weakness};
use ncl_object::package::{FindStatus, Package};
use ncl_object::{
    ArrayElementType, ArrayOptions, CodeObject, Function, Runtime, ThreadContext, Word, car, code_constants,
    code_debug, code_stack_map, complex_imag, complex_real, function_code, function_lambda_list,
    function_name, make_array, make_closure, make_code_object, make_complex, make_cons,
    make_instance, make_ratio, make_readtable, make_simple_fun, make_simple_vector, make_stream,
    make_string, make_structure, make_symbol, ratio_denominator, ratio_numerator,
    set_symbol_value, simple_vector_ref, simple_vector_set, slot_ref, stream_state,
};

#[test]
fn allocation_paths_survive_collection_before_every_allocation() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    ctx.set_gc_stress(true);
    runtime.set_gc_stress(true);

    let mut string =
        make_string(&mut ctx, &runtime, &['S', 'T', 'R', 'E', 'S', 'S']).unwrap_or(Word::NIL);
    let string_token = ncl_object::push_root(&mut ctx, &mut string);
    assert_eq!(ncl_object::string_ref(&ctx, string, 0), Ok('S'));

    let mut cons = make_cons(&mut ctx, &runtime, string, Word::fixnum(7)).unwrap_or(Word::NIL);
    let cons_token = ncl_object::push_root(&mut ctx, &mut cons);
    assert_eq!(car(&mut ctx, cons), Ok(string));

    let mut vector = make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1), Word::fixnum(2)])
        .unwrap_or(Word::NIL);
    let vector_token = ncl_object::push_root(&mut ctx, &mut vector);
    simple_vector_set(&mut ctx, vector, 1, cons)
        .unwrap_or_else(|error| panic!("vector set: {error:?}"));
    assert_eq!(simple_vector_ref(&ctx, vector, 1), Ok(cons));

    let mut symbol = make_symbol(&mut ctx, &runtime, string).unwrap_or(Word::NIL);
    let symbol_token = ncl_object::push_root(&mut ctx, &mut symbol);
    set_symbol_value(&mut ctx, symbol, Word::fixnum(42))
        .unwrap_or_else(|error| panic!("symbol: {error:?}"));

    let mut table = HashTable::new(&mut ctx, &runtime, HashTest::Equal, Weakness::None)
        .unwrap_or_else(|error| panic!("table: {error:?}"))
        .as_word();
    let table_token = ncl_object::push_root(&mut ctx, &mut table);
    HashTable::from(table)
        .insert(&mut ctx, &runtime, string, symbol)
        .unwrap_or_else(|error| panic!("insert: {error:?}"));
    assert_eq!(
        HashTable::from(table).get(&mut ctx, string),
        Ok(Some(symbol))
    );
    assert_eq!(
        HashTable::from(table).remove(&mut ctx, &runtime, string),
        Ok(Some(symbol))
    );

    let mut package = Package::new(&mut ctx, &runtime, "STRESS")
        .unwrap_or_else(|error| panic!("package: {error:?}"))
        .as_word();
    let package_token = ncl_object::push_root(&mut ctx, &mut package);
    let package = Package::from(package);
    let (interned, status) = package
        .intern(&mut ctx, &runtime, "NAME")
        .unwrap_or_else(|error| panic!("intern: {error:?}"));
    assert_eq!(status, FindStatus::Internal);
    let mut interned = interned;
    let interned_token = ncl_object::push_root(&mut ctx, &mut interned);
    let lookup_name = make_string(&mut ctx, &runtime, &['N', 'A', 'M', 'E']).unwrap_or(Word::NIL);
    assert_eq!(
        package.find_symbol(&mut ctx, lookup_name),
        Ok(Some((interned, FindStatus::Internal)))
    );
    let export_name = make_string(&mut ctx, &runtime, &['N', 'A', 'M', 'E']).unwrap_or(Word::NIL);
    package
        .export(&mut ctx, &runtime, export_name)
        .unwrap_or_else(|error| panic!("export: {error:?}"));
    let unexport_name = make_string(&mut ctx, &runtime, &['N', 'A', 'M', 'E']).unwrap_or(Word::NIL);
    package
        .unexport(&mut ctx, &runtime, unexport_name)
        .unwrap_or_else(|error| panic!("unexport: {error:?}"));
    package
        .shadow(&mut ctx, &runtime, string)
        .unwrap_or_else(|error| panic!("shadow: {error:?}"));
    let gensym = package.gensym(&mut ctx, &runtime).unwrap_or(Word::NIL);
    assert_ne!(gensym, Word::NIL);
    package
        .unintern(&mut ctx, &runtime, string)
        .unwrap_or_else(|error| panic!("unintern: {error:?}"));

    let mut instance = make_instance(&mut ctx, &runtime, symbol, &[cons])
        .unwrap_or_else(|error| panic!("instance: {error:?}"))
        .into();
    let instance_token = ncl_object::push_root(&mut ctx, &mut instance);
    assert_eq!(slot_ref(&ctx, instance.into(), 0), Ok(cons));

    let class_name = "STRESS-CLASS".to_owned();
    runtime
        .define_class(class_name.clone(), symbol)
        .unwrap_or_else(|error| panic!("class: {error:?}"));
    assert_eq!(runtime.class(&class_name), Some(symbol));
    runtime
        .define_function("NCL", "STRESS-FUNCTION", symbol)
        .unwrap_or_else(|error| panic!("function: {error:?}"));
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

#[test]
fn constructors_and_registry_survive_gc_stress() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    runtime.set_gc_stress(true);
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    ctx.set_gc_stress(true);

    let name = make_string(&mut ctx, &runtime, &['N', 'A', 'M', 'E']).unwrap_or(Word::NIL);
    let mut name = name;
    let name_token = ncl_object::push_root(&mut ctx, &mut name);
    let lambda = make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1)])
        .unwrap_or(Word::NIL);
    let mut lambda = lambda;
    let lambda_token = ncl_object::push_root(&mut ctx, &mut lambda);
    let code = make_code_object(
        &mut ctx,
        &runtime,
        10,
        2,
        name,
        lambda,
        Word::NIL,
    )
    .unwrap_or_else(|error| panic!("code: {error:?}"));
    let mut code_word = code.into();
    let code_token = ncl_object::push_root(&mut ctx, &mut code_word);
    let function = make_simple_fun(&mut ctx, &runtime, 1, name, lambda, code)
        .unwrap_or_else(|error| panic!("function: {error:?}"));
    let mut function_word = function.into();
    let function_token = ncl_object::push_root(&mut ctx, &mut function_word);
    let closure = make_closure(&mut ctx, &runtime, 2, name, lambda, code, &[name, lambda])
        .unwrap_or_else(|error| panic!("closure: {error:?}"));
    let mut closure_word = closure.into();
    let closure_token = ncl_object::push_root(&mut ctx, &mut closure_word);
    let code = CodeObject::from(code_word);
    let function = Function::from(function_word);
    let closure = Function::from(closure_word);
    assert_eq!(ncl_object::closure_ref(&ctx, closure, 0), Ok(name));
    assert_eq!(function_name(&ctx, function), Ok(name));
    assert_eq!(function_lambda_list(&ctx, function), Ok(lambda));
    assert_eq!(function_code(&ctx, function), Ok(code));
    assert_eq!(code_constants(&ctx, code), Ok(name));
    assert_eq!(code_stack_map(&ctx, code), Ok(lambda));
    assert_eq!(code_debug(&ctx, code), Ok(Word::NIL));

    let ratio = make_ratio(&mut ctx, &runtime, name, lambda)
        .unwrap_or_else(|error| panic!("ratio: {error:?}"));
    assert_eq!(ratio_numerator(&ctx, ratio), Ok(name));
    assert_eq!(ratio_denominator(&ctx, ratio), Ok(lambda));
    let complex = make_complex(&mut ctx, &runtime, name, lambda)
        .unwrap_or_else(|error| panic!("complex: {error:?}"));
    assert_eq!(complex_real(&ctx, complex), Ok(name));
    assert_eq!(complex_imag(&ctx, complex), Ok(lambda));

    let readtable = make_readtable(&mut ctx, &runtime, name, lambda, Word::fixnum(1))
        .unwrap_or_else(|error| panic!("readtable: {error:?}"));
    assert_eq!(ncl_object::readtable_syntax(&ctx, readtable), Ok(name));
    let stream = make_stream(
        &mut ctx,
        &runtime,
        Word::fixnum(1),
        name,
        lambda,
        Word::NIL,
        code.into(),
    )
    .unwrap_or_else(|error| panic!("stream: {error:?}"));
    assert_eq!(stream_state(&ctx, stream), Ok(Word::NIL));

    let layout = runtime
        .register_structure_layout(1)
        .unwrap_or_else(|error| panic!("layout: {error:?}"));
    let structure = make_structure(&mut ctx, &runtime, layout, &[name])
        .unwrap_or_else(|error| panic!("structure: {error:?}"));
    assert_eq!(ncl_object::structure_ref(&ctx, structure, 0), Ok(name));
    let array = make_array(
        &mut ctx,
        &runtime,
        &[2],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: name,
            adjustable: false,
            fill_pointer: None,
            displaced_to: Some(lambda),
            displaced_index_offset: 0,
        },
    )
    .unwrap_or_else(|error| panic!("array: {error:?}"));
    assert_eq!(ncl_object::array_row_major_ref(&ctx, array, 0), Ok(Word::fixnum(1)));

    let package = runtime
        .ensure_package("STRESS-A")
        .unwrap_or_else(|error| panic!("package: {error:?}"));
    let mut package = package;
    let package_token = ncl_object::push_root(&mut ctx, &mut package);
    let used = runtime
        .ensure_package("STRESS-B")
        .unwrap_or_else(|error| panic!("package: {error:?}"));
    let mut used = used;
    let used_token = ncl_object::push_root(&mut ctx, &mut used);
    let import_name = make_string(&mut ctx, &runtime, &['I', 'M', 'P', 'O', 'R', 'T'])
        .unwrap_or(Word::NIL);
    let imported = make_symbol(&mut ctx, &runtime, name)
        .unwrap_or_else(|error| panic!("symbol: {error:?}"));
    Package::from(used)
        .import(&mut ctx, &runtime, import_name, imported)
        .unwrap_or_else(|error| panic!("import: {error:?}"));
    let (symbol, _) = Package::from(used)
        .intern(&mut ctx, &runtime, "INHERITED")
        .unwrap_or_else(|error| panic!("intern: {error:?}"));
    let mut symbol = symbol;
    let symbol_token = ncl_object::push_root(&mut ctx, &mut symbol);
    let inherited_name = make_string(
        &mut ctx,
        &runtime,
        &['I', 'N', 'H', 'E', 'R', 'I', 'T', 'E', 'D'],
    )
    .unwrap_or(Word::NIL);
    Package::from(used)
        .export(&mut ctx, &runtime, inherited_name)
        .unwrap_or_else(|error| panic!("export: {error:?}"));
    Package::from(package)
        .use_package(&mut ctx, &runtime, used)
        .unwrap_or_else(|error| panic!("use: {error:?}"));
    let lookup = make_string(&mut ctx, &runtime, &['I', 'N', 'H', 'E', 'R', 'I', 'T', 'E', 'D'])
        .unwrap_or(Word::NIL);
    assert_eq!(Package::from(package).find_symbol(&mut ctx, lookup), Ok(Some((symbol, FindStatus::Inherited))));
    assert_eq!(runtime.ensure_package("STRESS-A"), Ok(package));
    assert!(ncl_object::pop_root(&mut ctx, symbol_token));
    assert!(ncl_object::pop_root(&mut ctx, used_token));
    assert!(ncl_object::pop_root(&mut ctx, package_token));
    assert!(ncl_object::pop_root(&mut ctx, closure_token));
    assert!(ncl_object::pop_root(&mut ctx, function_token));
    assert!(ncl_object::pop_root(&mut ctx, code_token));
    assert!(ncl_object::pop_root(&mut ctx, lambda_token));
    assert!(ncl_object::pop_root(&mut ctx, name_token));
}

#[test]
fn hash_table_resize_tombstones_and_reinsertion_survive_gc_stress() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    ctx.set_gc_stress(true);
    let mut table = HashTable::new(&mut ctx, &runtime, HashTest::Equal, Weakness::None)
        .unwrap_or_else(|error| panic!("table: {error:?}"))
        .as_word();
    let table_token = ncl_object::push_root(&mut ctx, &mut table);
    for index in 0..16 {
        HashTable::from(table)
            .insert(
                &mut ctx,
                &runtime,
                Word::fixnum(index),
                Word::fixnum(index + 100),
            )
            .unwrap_or_else(|error| panic!("insert {index}: {error:?}"));
    }
    for index in (0..16).step_by(2) {
        assert_eq!(
            HashTable::from(table).remove(&mut ctx, &runtime, Word::fixnum(index)),
            Ok(Some(Word::fixnum(index + 100)))
        );
    }
    for index in 16..24 {
        HashTable::from(table)
            .insert(
                &mut ctx,
                &runtime,
                Word::fixnum(index),
                Word::fixnum(index + 100),
            )
            .unwrap_or_else(|error| panic!("reinsert {index}: {error:?}"));
    }
    assert_eq!(HashTable::from(table).capacity(&ctx), Ok(32));
    assert!(ncl_object::pop_root(&mut ctx, table_token));
}
