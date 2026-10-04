#![allow(clippy::unwrap_used, reason = "tests assert on builtin output")]
#![allow(missing_docs)]

use ncl_object::{
    FunctionObject, Package, Runtime, ThreadContext, Word, make_double, make_ratio,
    make_simple_vector, make_string, set_symbol_value,
};

fn function(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> FunctionObject {
    FunctionObject::try_from(runtime.function(ctx, "COMMON-LISP", name).unwrap()).unwrap()
}

fn call(runtime: &Runtime, ctx: &mut ThreadContext, name: &str, args: &[Word]) -> Word {
    let function = function(runtime, ctx, name);
    runtime.call_builtin(ctx, function, args).unwrap()
}

fn output_for(name: &str, value: &str) -> String {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_printer::register(&mut ctx, &runtime).unwrap();
    ncl_lib_streams::register(&runtime).unwrap();
    ctx.set_gc_stress(true);

    let mut stream = call(&runtime, &mut ctx, "MAKE-STRING-OUTPUT-STREAM", &[]);
    let stream_token = ncl_object::push_root(&mut ctx, &mut stream);
    let mut object = make_string(&mut ctx, &runtime, &value.chars().collect::<Vec<_>>()).unwrap();
    let object_token = ncl_object::push_root(&mut ctx, &mut object);
    call(&runtime, &mut ctx, name, &[object, stream]);
    let result = call(&runtime, &mut ctx, "GET-OUTPUT-STREAM-STRING", &[stream]);
    let _ = ncl_object::pop_root(&mut ctx, object_token);
    let _ = ncl_object::pop_root(&mut ctx, stream_token);
    let length = ncl_object::string_length(&ctx, result).unwrap();
    (0..length)
        .map(|index| ncl_object::string_ref(&ctx, result, index).unwrap())
        .collect()
}

#[test]
fn printers_use_stream_output_and_common_lisp_semantics() {
    assert_eq!(output_for("PRINC", "x"), "x");
    assert_eq!(output_for("PRIN1", "x"), "\"x\"");
    assert_eq!(output_for("PRINT", "x"), "\n\"x\"\n");
}

#[test]
fn printer_builtin_keeps_object_result() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_printer::register(&mut ctx, &runtime).unwrap();
    ncl_lib_streams::register(&runtime).unwrap();
    let stream = call(&runtime, &mut ctx, "MAKE-STRING-OUTPUT-STREAM", &[]);
    let object = make_string(&mut ctx, &runtime, &['x']).unwrap();
    let result = call(&runtime, &mut ctx, "PRINC", &[object, stream]);
    assert_eq!(result, object);
}

#[test]
fn pprint_dispatch_builtins_use_the_ambient_table() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_printer::register(&mut ctx, &runtime).unwrap();
    ncl_lib_streams::register(&runtime).unwrap();

    let object = Word::fixnum(7);
    let table = call(&runtime, &mut ctx, "COPY-PPRINT-DISPATCH", &[]);
    assert_eq!(
        call(&runtime, &mut ctx, "PPRINT-DISPATCH", &[object, table]),
        Word::NIL
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "SET-PPRINT-DISPATCH",
            &[Word::TRUE, Word::NIL]
        ),
        Word::NIL
    );
    assert_eq!(
        call(&runtime, &mut ctx, "PPRINT-DISPATCH", &[object]),
        Word::NIL
    );
}

#[test]
fn priority_dispatch_matches_basic_type_specifiers() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_printer::register(&mut ctx, &runtime).unwrap();
    let table = call(&runtime, &mut ctx, "COPY-PPRINT-DISPATCH", &[]);
    let package = runtime.ensure_package(&mut ctx, "COMMON-LISP").unwrap();
    let integer = Package::from_word(package)
        .intern(&mut ctx, &runtime, "INTEGER")
        .unwrap()
        .0;
    let table = ncl_printer::set_pprint_dispatch_with_priority(
        &mut ctx,
        &runtime,
        integer,
        Word::fixnum(11),
        10,
        table,
    )
    .unwrap();
    assert_eq!(
        ncl_printer::pprint_dispatch(&mut ctx, Word::fixnum(4), table).unwrap(),
        Word::fixnum(11)
    );
}

#[test]
fn dispatch_matches_common_numeric_and_sequence_specifiers() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_printer::register(&mut ctx, &runtime).unwrap();
    let table = call(&runtime, &mut ctx, "COPY-PPRINT-DISPATCH", &[]);
    let package = runtime.ensure_package(&mut ctx, "COMMON-LISP").unwrap();
    let number = Package::from_word(package)
        .intern(&mut ctx, &runtime, "NUMBER")
        .unwrap()
        .0;
    let sequence = Package::from_word(package)
        .intern(&mut ctx, &runtime, "SEQUENCE")
        .unwrap()
        .0;
    let ratio = make_ratio(&mut ctx, &runtime, Word::fixnum(1), Word::fixnum(2)).unwrap();
    let vector = make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1)]).unwrap();
    let table =
        ncl_printer::set_pprint_dispatch(&mut ctx, &runtime, number, Word::fixnum(1), table)
            .unwrap();
    let table =
        ncl_printer::set_pprint_dispatch(&mut ctx, &runtime, sequence, Word::fixnum(2), table)
            .unwrap();
    assert_eq!(
        ncl_printer::pprint_dispatch(&mut ctx, ratio.as_word(), table).unwrap(),
        Word::fixnum(1)
    );
    assert_eq!(
        ncl_printer::pprint_dispatch(&mut ctx, vector, table).unwrap(),
        Word::fixnum(2)
    );
    let float = make_double(&mut ctx, &runtime, 1.0).unwrap();
    assert_eq!(
        ncl_printer::pprint_dispatch(&mut ctx, float.as_word(), table).unwrap(),
        Word::fixnum(1)
    );
}

#[test]
fn pprint_layout_primitives_write_to_the_ambient_stream() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_printer::register(&mut ctx, &runtime).unwrap();
    ncl_lib_streams::register(&runtime).unwrap();
    let stream = call(&runtime, &mut ctx, "MAKE-STRING-OUTPUT-STREAM", &[]);
    let package = runtime.ensure_package(&mut ctx, "COMMON-LISP").unwrap();
    let standard_output = Package::from_word(package)
        .intern(&mut ctx, &runtime, "*STANDARD-OUTPUT*")
        .unwrap()
        .0;
    set_symbol_value(&mut ctx, standard_output, stream).unwrap();
    let keyword = runtime.ensure_package(&mut ctx, "KEYWORD").unwrap();
    let mandatory = Package::from_word(keyword)
        .intern(&mut ctx, &runtime, "MANDATORY")
        .unwrap()
        .0;
    let relative = Package::from_word(keyword)
        .intern(&mut ctx, &runtime, "RELATIVE")
        .unwrap()
        .0;
    call(&runtime, &mut ctx, "PPRINT-NEWLINE", &[mandatory]);
    call(
        &runtime,
        &mut ctx,
        "PPRINT-TAB",
        &[relative, Word::fixnum(2), Word::fixnum(0)],
    );
    let result = call(&runtime, &mut ctx, "GET-OUTPUT-STREAM-STRING", &[stream]);
    let length = ncl_object::string_length(&ctx, result).unwrap();
    let output: String = (0..length)
        .map(|index| ncl_object::string_ref(&ctx, result, index).unwrap())
        .collect();
    assert_eq!(output, "\n  ");
}
