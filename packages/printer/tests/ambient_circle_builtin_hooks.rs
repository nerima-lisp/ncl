#![allow(clippy::unwrap_used, reason = "tests assert on printer output")]

//! Fourth-wave coverage for ambient specials, circle scans, and builtin hooks.

use ncl_object::{
    Arity, Builtin, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation, BuiltinName,
    BuiltinPackage, FunctionObject, LambdaList, MultipleValues, Package, Parameter, ParameterType,
    Runtime, ThreadContext, Word, make_cons, make_simple_vector, make_string, make_structure,
    set_symbol_plist, set_symbol_special, set_symbol_value,
};
use ncl_printer::{PrintCase, PrintOptions, StringSink, write};

const OBJECT: Parameter = Parameter {
    name: BuiltinName::new("OBJECT"),
    ty: ParameterType::Any,
};
const STREAM: Parameter = Parameter {
    name: BuiltinName::new("STREAM"),
    ty: ParameterType::Any,
};
const INDEX: Parameter = Parameter {
    name: BuiltinName::new("INDEX"),
    ty: ParameterType::Any,
};

fn context() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_printer::register(&mut ctx, &runtime).unwrap();
    ncl_lib_streams::register(&runtime).unwrap();
    (runtime, ctx)
}

fn intern(runtime: &Runtime, ctx: &mut ThreadContext, package: &str, name: &str) -> Word {
    let package = runtime.ensure_package(ctx, package).unwrap();
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap()
        .0
}

fn render(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    object: Word,
    options: PrintOptions,
) -> String {
    let mut sink = StringSink::new();
    write(ctx, runtime, object, &mut sink, &options).unwrap();
    sink.into_string()
}

fn set_special(ctx: &mut ThreadContext, symbols: &[Word], index: usize, value: Word) {
    set_symbol_value(ctx, *symbols.get(index).unwrap(), value).unwrap();
}

#[test]
fn ambient_specials_cover_boolean_numeric_and_other_value_fallbacks() {
    let (runtime, mut ctx) = context();
    let names = [
        "*PRINT-ESCAPE*",
        "*PRINT-READABLY*",
        "*PRINT-RADIX*",
        "*PRINT-CIRCLE*",
        "*PRINT-PRETTY*",
        "*PRINT-ARRAY*",
        "*PRINT-GENSYM*",
        "*PRINT-BASE*",
        "*PRINT-LENGTH*",
        "*PRINT-LEVEL*",
    ];
    let symbols: Vec<_> = names
        .iter()
        .map(|name| intern(&runtime, &mut ctx, "COMMON-LISP", name))
        .collect();
    for symbol in symbols.iter().copied() {
        set_symbol_special(&mut ctx, symbol, true).unwrap();
    }
    set_special(&mut ctx, &symbols, 0, Word::TRUE);
    set_special(&mut ctx, &symbols, 1, Word::NIL);
    set_special(&mut ctx, &symbols, 2, Word::TRUE);
    set_special(&mut ctx, &symbols, 3, Word::TRUE);
    set_special(&mut ctx, &symbols, 4, Word::TRUE);
    set_special(&mut ctx, &symbols, 5, Word::NIL);
    set_special(&mut ctx, &symbols, 6, Word::NIL);
    set_special(&mut ctx, &symbols, 7, Word::fixnum(16));
    set_special(&mut ctx, &symbols, 8, Word::fixnum(3));
    set_special(&mut ctx, &symbols, 9, Word::fixnum(2));

    let options = PrintOptions::from_specials(&mut ctx, &runtime);
    assert!(options.escape());
    assert!(!options.readably());
    assert!(options.radix());
    assert!(options.circle());
    assert!(options.pretty());
    assert!(!options.array());
    assert!(!options.gensym());
    assert_eq!(options.base().get(), 16);
    assert_eq!(options.length().map(ncl_printer::NonNegative::get), Some(3));
    assert_eq!(options.level().map(ncl_printer::NonNegative::get), Some(2));

    set_special(&mut ctx, &symbols, 7, Word::TRUE);
    set_special(&mut ctx, &symbols, 8, Word::NIL);
    set_special(&mut ctx, &symbols, 9, Word::fixnum(-1));
    let fallback = PrintOptions::from_specials(&mut ctx, &runtime);
    assert_eq!(fallback.base().get(), 10);
    assert_eq!(fallback.length(), None);
    assert_eq!(fallback.level(), None);
    assert_eq!(fallback.case(), PrintCase::Upcase);
}

#[test]
fn non_shared_circle_mode_omits_labels_for_repeated_non_cycles() {
    let (runtime, mut ctx) = context();
    let shared = make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(4)]).unwrap();
    let tail = make_cons(&mut ctx, &runtime, shared, Word::NIL).unwrap();
    let outer = make_cons(&mut ctx, &runtime, shared, tail).unwrap();
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            outer,
            PrintOptions::new().with_circle(true)
        ),
        "(#1=#(4) #1#)"
    );
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            outer,
            PrintOptions::new()
                .with_circle(true)
                .with_circle_not_shared(true),
        ),
        "(#(4) #(4))"
    );
}

fn qualified_structure(runtime: &Runtime, ctx: &mut ThreadContext) -> (Word, Word) {
    let name = intern(runtime, ctx, "WAVE4", "POINT");
    let slot_name = intern(runtime, ctx, "WAVE4", "X");
    let descriptor = make_simple_vector(ctx, runtime, &[slot_name, Word::NIL, Word::NIL]).unwrap();
    let slots = make_simple_vector(ctx, runtime, &[descriptor]).unwrap();
    let class = make_simple_vector(
        ctx,
        runtime,
        &[name, Word::NIL, slots, Word::fixnum(1), slots],
    )
    .unwrap();
    runtime.define_class(ctx, "WAVE4::POINT", class).unwrap();
    runtime
        .define_class(ctx, runtime.structure_class_name(ctx, name).unwrap(), class)
        .unwrap();
    let layout = runtime.register_structure_layout(1).unwrap();
    runtime
        .register_structure_class_with_parent(ctx, layout, None, name)
        .unwrap();
    (
        make_structure(ctx, runtime, layout, &[Word::fixnum(5)]).unwrap(),
        name,
    )
}

#[test]
fn structure_print_includes_non_user_package_prefix() {
    let (runtime, mut ctx) = context();
    let (structure, _name) = qualified_structure(&runtime, &mut ctx);
    assert_eq!(
        render(&runtime, &mut ctx, structure, PrintOptions::new()),
        "#S(WAVE4:POINT :X 5)"
    );
}

fn structure_hook(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &ncl_object::BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ncl_object::ObjectError> {
    let text = make_string(ctx, runtime, &"HOOKED".chars().collect::<Vec<_>>())?;
    let function = runtime
        .function(ctx, "COMMON-LISP", "WRITE-STRING")
        .ok_or(ncl_object::ObjectError::UndefinedFunction)?;
    let function = FunctionObject::try_from(function)
        .map_err(|_| ncl_object::ObjectError::UndefinedFunction)?;
    runtime.call_builtin(ctx, function, &[text, args.required(1)?])?;
    args.required(0)
}

#[test]
fn structure_print_function_property_precedes_print_object_fallback() {
    let (runtime, mut ctx) = context();
    let (structure, name) = qualified_structure(&runtime, &mut ctx);
    let function = runtime
        .register_builtin(
            &mut ctx,
            BuiltinIdentifier::new(BuiltinPackage::NclTest, BuiltinName::new("STRUCTURE-HOOK")),
            BuiltinImplementation::direct(
                Builtin {
                    lambda_list: LambdaList::fixed(&[OBJECT, STREAM, INDEX]),
                    convention: BuiltinConvention::Direct(Arity::exact(3)),
                },
                structure_hook,
            ),
        )
        .unwrap();
    let key = intern(&runtime, &mut ctx, "NCL", "%STRUCTURE-PRINT-FUNCTION");
    let property = make_cons(&mut ctx, &runtime, key, function.as_word()).unwrap();
    let plist = make_cons(&mut ctx, &runtime, property, Word::NIL).unwrap();
    set_symbol_plist(&mut ctx, name, plist).unwrap();

    let stream_function = FunctionObject::try_from(
        runtime
            .function(&mut ctx, "COMMON-LISP", "MAKE-STRING-OUTPUT-STREAM")
            .unwrap(),
    )
    .unwrap();
    let stream = runtime
        .call_builtin(&mut ctx, stream_function, &[])
        .unwrap();
    let princ =
        FunctionObject::try_from(runtime.function(&mut ctx, "COMMON-LISP", "PRINC").unwrap())
            .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, princ, &[structure, stream]),
        Ok(structure)
    );
    let get_output = FunctionObject::try_from(
        runtime
            .function(&mut ctx, "COMMON-LISP", "GET-OUTPUT-STREAM-STRING")
            .unwrap(),
    )
    .unwrap();
    let output = runtime
        .call_builtin(&mut ctx, get_output, &[stream])
        .unwrap();
    let rendered = render(
        &runtime,
        &mut ctx,
        output,
        PrintOptions::new().with_escape(false),
    );
    assert_eq!(rendered, "HOOKED");
}

#[allow(
    clippy::missing_const_for_fn,
    reason = "the callback must match the registered builtin hook signature"
)]
fn failing_print_object(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    _args: &ncl_object::BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ncl_object::ObjectError> {
    Err(ncl_object::ObjectError::TypeError)
}

#[test]
fn structure_builtin_falls_back_when_print_object_is_unbound() {
    let (runtime, mut ctx) = context();
    let (structure, _name) = qualified_structure(&runtime, &mut ctx);
    let stream_function = FunctionObject::try_from(
        runtime
            .function(&mut ctx, "COMMON-LISP", "MAKE-STRING-OUTPUT-STREAM")
            .unwrap(),
    )
    .unwrap();
    let stream = runtime
        .call_builtin(&mut ctx, stream_function, &[])
        .unwrap();
    let princ =
        FunctionObject::try_from(runtime.function(&mut ctx, "COMMON-LISP", "PRINC").unwrap())
            .unwrap();

    assert_eq!(
        runtime.call_builtin(&mut ctx, princ, &[structure, stream]),
        Ok(structure)
    );
    let get_output = FunctionObject::try_from(
        runtime
            .function(&mut ctx, "COMMON-LISP", "GET-OUTPUT-STREAM-STRING")
            .unwrap(),
    )
    .unwrap();
    let output = runtime
        .call_builtin(&mut ctx, get_output, &[stream])
        .unwrap();
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            output,
            PrintOptions::new().with_escape(false),
        ),
        "#S(WAVE4:POINT :X 5)"
    );
}

#[test]
fn structure_builtin_propagates_print_object_errors() {
    let (runtime, mut ctx) = context();
    let (structure, _name) = qualified_structure(&runtime, &mut ctx);
    runtime
        .register_builtin(
            &mut ctx,
            BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new("PRINT-OBJECT")),
            BuiltinImplementation::direct(
                Builtin {
                    lambda_list: LambdaList::fixed(&[OBJECT, STREAM]),
                    convention: BuiltinConvention::Direct(Arity::exact(2)),
                },
                failing_print_object,
            ),
        )
        .unwrap();
    let stream_function = FunctionObject::try_from(
        runtime
            .function(&mut ctx, "COMMON-LISP", "MAKE-STRING-OUTPUT-STREAM")
            .unwrap(),
    )
    .unwrap();
    let stream = runtime
        .call_builtin(&mut ctx, stream_function, &[])
        .unwrap();
    let princ =
        FunctionObject::try_from(runtime.function(&mut ctx, "COMMON-LISP", "PRINC").unwrap())
            .unwrap();

    assert_eq!(
        runtime.call_builtin(&mut ctx, princ, &[structure, stream]),
        Err(ncl_object::ObjectError::TypeError)
    );
}
