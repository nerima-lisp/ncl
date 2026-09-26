#![allow(missing_docs)]
#![allow(clippy::unwrap_used, reason = "tests assert on builtin behavior")]

use ncl_object::{
    FunctionObject, ObjectRef, Package, Runtime, ThreadContext, Word, classify_object,
    stream_direction, symbol_is_special, symbol_value,
};

const STANDARD_STREAMS: [(&str, bool); 7] = [
    ("*STANDARD-OUTPUT*", false),
    ("*STANDARD-INPUT*", true),
    ("*ERROR-OUTPUT*", false),
    ("*TERMINAL-IO*", true),
    ("*QUERY-IO*", true),
    ("*DEBUG-IO*", true),
    ("*TRACE-OUTPUT*", false),
];

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_lib_streams::register(&runtime).unwrap();
    (runtime, ctx)
}

fn symbol(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> Word {
    let package = runtime.ensure_package(ctx, "COMMON-LISP").unwrap();
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap()
        .0
}

fn function(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> FunctionObject {
    FunctionObject::try_from(runtime.function(ctx, "COMMON-LISP", name).unwrap()).unwrap()
}

#[test]
fn standard_stream_variables_are_typed_special_streams() {
    let (runtime, mut ctx) = setup();
    for (name, input) in STANDARD_STREAMS {
        let stream_symbol = symbol(&runtime, &mut ctx, name);
        assert!(symbol_is_special(&ctx, stream_symbol).unwrap(), "{name}");
        let stream = symbol_value(&ctx, stream_symbol).unwrap();
        assert!(matches!(
            classify_object(&ctx, stream),
            ObjectRef::Stream(_)
        ));
        let direction = stream_direction(&ctx, ncl_object::Stream::from_word(stream)).unwrap();
        let direction_name = match name {
            "*TERMINAL-IO*" | "*QUERY-IO*" | "*DEBUG-IO*" => "IO",
            _ if input => "INPUT",
            _ => "OUTPUT",
        };
        let direction_symbol = symbol(&runtime, &mut ctx, direction_name);
        assert_eq!(direction, direction_symbol, "{name}");
    }
}

#[test]
fn default_output_uses_standard_output_under_gc_stress_and_strict_forwarding() {
    let (runtime, mut ctx) = setup();
    let write_char = function(&runtime, &mut ctx, "WRITE-CHAR");
    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);

    assert_eq!(
        runtime.call_builtin(&mut ctx, write_char, &[Word::character(u32::from('x'))]),
        Ok(Word::character(u32::from('x')))
    );
}
