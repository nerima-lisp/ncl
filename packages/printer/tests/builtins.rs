#![allow(clippy::unwrap_used, reason = "tests assert on builtin output")]
#![allow(missing_docs)]

use ncl_object::{FunctionObject, ObjectError, Runtime, ThreadContext, Word, make_string};

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
fn registration_installs_print_builtins_and_leaves_other_owned_functions_unbound() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_printer::register(&mut ctx, &runtime).unwrap();

    for name in ["PRINC", "PRIN1", "PRINT"] {
        let word = runtime.function(&mut ctx, "COMMON-LISP", name).unwrap();
        assert!(
            !FunctionObject::try_from(word).unwrap().is_unbound(),
            "{name}"
        );
    }
    for name in [
        "COPY-PPRINT-DISPATCH",
        "PPRINT",
        "PPRINT-DISPATCH",
        "PPRINT-FILL",
        "PPRINT-INDENT",
        "PPRINT-LINEAR",
        "PPRINT-NEWLINE",
        "PPRINT-TAB",
        "PPRINT-TABULAR",
        "PRIN1-TO-STRING",
        "PRINC-TO-STRING",
        "PRINT-NOT-READABLE-OBJECT",
        "PRINT-OBJECT",
        "SET-PPRINT-DISPATCH",
        "WRITE-TO-STRING",
    ] {
        assert_eq!(
            runtime.function(&mut ctx, "COMMON-LISP", name),
            Some(Word::UNBOUND),
            "{name}"
        );
    }
}

#[test]
fn print_builtins_report_argument_and_stream_errors() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_printer::register(&mut ctx, &runtime).unwrap();
    ncl_lib_streams::register(&runtime).unwrap();

    for name in ["PRINC", "PRIN1", "PRINT"] {
        let function = function(&runtime, &mut ctx, name);
        assert_eq!(
            runtime.call_builtin(&mut ctx, function, &[]),
            Err(ObjectError::TypeError),
            "{name}"
        );
        let object = Word::fixnum(7);
        assert_eq!(
            runtime.call_builtin(&mut ctx, function, &[object, Word::fixnum(8)]),
            Err(ObjectError::TypeError),
            "{name} rejects a non-stream output designator"
        );
    }
}
