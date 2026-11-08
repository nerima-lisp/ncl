//! Integration tests for object survival during garbage-collection stress.

use ncl_object::{
    CodeObject, Function, Runtime, ThreadContext, Word, car, cdr, code_constants, code_debug,
    code_stack_map, complex_imag, complex_real, function_code, function_lambda_list, function_name,
    make_closure, make_code_object, make_complex, make_cons, make_instance, make_ratio,
    make_simple_fun, make_simple_vector, make_string, make_structure, make_symbol,
    ratio_denominator, ratio_numerator, set_symbol_value, simple_vector_ref, simple_vector_set,
    slot_ref,
};
use ncl_readtables::{make_readtable, readtable_syntax};
use ncl_streams::{
    make_stream, stream_element_type, stream_external_format, stream_implementation, stream_state,
};

fn assert_registry_entries(ctx: &mut ThreadContext, runtime: &Runtime, symbol: Word) {
    let class_name = "STRESS-CLASS";
    runtime
        .define_class(ctx, class_name, symbol)
        .unwrap_or_else(|error| panic!("class: {error:?}"));
    assert_eq!(runtime.class(ctx, class_name), Some(symbol));
    runtime
        .define_function(ctx, "NCL", "STRESS-FUNCTION", symbol)
        .unwrap_or_else(|error| panic!("function: {error:?}"));
    assert_eq!(
        runtime.function(ctx, "NCL", "STRESS-FUNCTION"),
        Some(symbol)
    );
}

#[test]
fn make_rest_list_builtin_survives_gc_stress_and_strict_forwarding() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));

    let mut first =
        make_string(&mut ctx, &runtime, &['A']).unwrap_or_else(|error| panic!("first: {error:?}"));
    let first_token = ncl_object::push_root(&mut ctx, &mut first);
    let mut second =
        make_string(&mut ctx, &runtime, &['B']).unwrap_or_else(|error| panic!("second: {error:?}"));
    let second_token = ncl_object::push_root(&mut ctx, &mut second);
    let mut third =
        make_string(&mut ctx, &runtime, &['C']).unwrap_or_else(|error| panic!("third: {error:?}"));
    let third_token = ncl_object::push_root(&mut ctx, &mut third);

    let function = runtime
        .function(&mut ctx, "NCL-EXT", "MAKE-REST-LIST")
        .and_then(|word| ncl_object::FunctionObject::try_from(word).ok())
        .unwrap_or_else(|| panic!("MAKE-REST-LIST not registered"));
    let descriptor = runtime
        .builtin_descriptor(&ctx, function)
        .unwrap_or_else(|| panic!("MAKE-REST-LIST descriptor missing"));
    assert_eq!(descriptor.lambda_list.required.len(), 2);
    assert_eq!(
        descriptor.lambda_list.required[0].ty,
        ncl_object::ParameterType::Fixnum
    );
    assert_eq!(
        descriptor.lambda_list.required[1].ty,
        ncl_object::ParameterType::Fixnum
    );
    assert!(descriptor.lambda_list.rest.is_some());

    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);

    let list = runtime
        .call_builtin(
            &mut ctx,
            function,
            &[Word::fixnum(3), Word::fixnum(1), first, second, third],
        )
        .unwrap_or_else(|error| panic!("make rest list: {error:?}"));
    let first_result = car(&ctx, list).unwrap_or_else(|error| panic!("first result: {error:?}"));
    assert_eq!(ncl_object::string_ref(&ctx, first_result, 0), Ok('B'));
    let tail = cdr(&ctx, list).unwrap_or_else(|error| panic!("first tail: {error:?}"));
    let second_result = car(&ctx, tail).unwrap_or_else(|error| panic!("second result: {error:?}"));
    assert_eq!(ncl_object::string_ref(&ctx, second_result, 0), Ok('C'));
    assert_eq!(
        cdr(&ctx, tail),
        Ok(Word::NIL),
        "rest list has an unexpected tail"
    );

    assert!(ncl_object::pop_root(&mut ctx, third_token));
    assert!(ncl_object::pop_root(&mut ctx, second_token));
    assert!(ncl_object::pop_root(&mut ctx, first_token));
}

#[test]
fn allocation_paths_survive_collection_before_every_allocation() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);
    let mut string = make_string(&mut ctx, &runtime, &['S', 'T', 'R', 'E', 'S', 'S'])
        .unwrap_or_else(|error| panic!("allocation: {error:?}"));
    let string_token = ncl_object::push_root(&mut ctx, &mut string);
    assert_eq!(ncl_object::string_ref(&ctx, string, 0), Ok('S'));
    let mut cons = make_cons(&mut ctx, &runtime, string, Word::fixnum(7))
        .unwrap_or_else(|error| panic!("allocation: {error:?}"));
    let cons_token = ncl_object::push_root(&mut ctx, &mut cons);
    assert_eq!(car(&ctx, cons), Ok(string));
    let mut vector = make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1), Word::fixnum(2)])
        .unwrap_or_else(|error| panic!("allocation: {error:?}"));
    let vector_token = ncl_object::push_root(&mut ctx, &mut vector);
    simple_vector_set(&mut ctx, vector, 1, cons)
        .unwrap_or_else(|error| panic!("vector set: {error:?}"));
    assert_eq!(simple_vector_ref(&ctx, vector, 1), Ok(cons));
    let mut symbol = make_symbol(&mut ctx, &runtime, string)
        .unwrap_or_else(|error| panic!("allocation: {error:?}"));
    let symbol_token = ncl_object::push_root(&mut ctx, &mut symbol);
    set_symbol_value(&mut ctx, symbol, Word::fixnum(42))
        .unwrap_or_else(|error| panic!("symbol: {error:?}"));
    let mut instance = make_instance(&mut ctx, &runtime, symbol, &[cons])
        .unwrap_or_else(|error| panic!("instance: {error:?}"))
        .into();
    let instance_token = ncl_object::push_root(&mut ctx, &mut instance);
    assert_eq!(
        slot_ref(&ctx, ncl_object::Instance::from_word(instance), 0),
        Ok(cons)
    );
    assert_registry_entries(&mut ctx, &runtime, symbol);
    assert!(ncl_object::pop_root(&mut ctx, instance_token));
    assert!(ncl_object::pop_root(&mut ctx, symbol_token));
    assert!(ncl_object::pop_root(&mut ctx, vector_token));
    assert!(ncl_object::pop_root(&mut ctx, cons_token));
    assert!(ncl_object::pop_root(&mut ctx, string_token));
}
#[test]
fn constructors_and_registry_survive_gc_stress() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);
    let name = make_string(&mut ctx, &runtime, &['N', 'A', 'M', 'E'])
        .unwrap_or_else(|error| panic!("allocation: {error:?}"));
    let mut name = name;
    let name_token = ncl_object::push_root(&mut ctx, &mut name);
    let lambda = make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1), Word::fixnum(1)])
        .unwrap_or_else(|error| panic!("allocation: {error:?}"));
    let mut lambda = lambda;
    let lambda_token = ncl_object::push_root(&mut ctx, &mut lambda);
    let code = make_code_object(&mut ctx, &runtime, 10, 2, name, lambda, Word::NIL)
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
    let code = CodeObject::from_word(code_word);
    let function = Function::from_word(function_word);
    let closure = Function::from_word(closure_word);
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
    assert_eq!(readtable_syntax(&ctx, readtable), Ok(name));
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
    assert_eq!(stream_element_type(&ctx, stream), Ok(name));
    assert_eq!(stream_external_format(&ctx, stream), Ok(lambda));
    assert_eq!(stream_implementation(&ctx, stream), Ok(code.into()));
    let layout = runtime
        .register_structure_layout(1)
        .unwrap_or_else(|error| panic!("layout: {error:?}"));
    let structure = make_structure(&mut ctx, &runtime, layout, &[name])
        .unwrap_or_else(|error| panic!("structure: {error:?}"));
    assert_eq!(ncl_object::structure_ref(&ctx, structure, 0), Ok(name));
    assert!(ncl_object::pop_root(&mut ctx, closure_token));
    assert!(ncl_object::pop_root(&mut ctx, function_token));
    assert!(ncl_object::pop_root(&mut ctx, code_token));
    assert!(ncl_object::pop_root(&mut ctx, lambda_token));
    assert!(ncl_object::pop_root(&mut ctx, name_token));
}
