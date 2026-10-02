#![allow(
    clippy::unwrap_used,
    reason = "coverage tests assert on printer output"
)]

//! Twelfth-wave coverage selected from crate-local llvm-cov line data.

use ncl_object::{
    Arity, ArrayElementType, ArrayOptions, Builtin, BuiltinConvention, BuiltinIdentifier,
    BuiltinImplementation, BuiltinName, BuiltinPackage, FunctionObject, LambdaList, MultipleValues,
    Package, Parameter, ParameterType, Runtime, ThreadContext, Word, make_array,
    make_bignum_from_i128, make_complex, make_cons, make_double, make_simple_vector, make_string,
    make_structure, set_symbol_special, set_symbol_value,
};
use ncl_printer::{PrintError, PrintOptions, StringSink, write};

const OBJECT: Parameter = Parameter {
    name: BuiltinName::new("OBJECT"),
    ty: ParameterType::Any,
};
const STREAM: Parameter = Parameter {
    name: BuiltinName::new("STREAM"),
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
) -> Result<String, PrintError> {
    let mut sink = StringSink::new();
    write(ctx, runtime, object, &mut sink, &options)?;
    Ok(sink.into_string())
}

fn builtin(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> FunctionObject {
    FunctionObject::try_from(runtime.function(ctx, "COMMON-LISP", name).unwrap()).unwrap()
}

fn point_structure(runtime: &Runtime, ctx: &mut ThreadContext) -> Word {
    let name = intern(runtime, ctx, "COMMON-LISP-USER", "W12-POINT");
    let slot_name = intern(runtime, ctx, "COMMON-LISP-USER", "X");
    let descriptor = make_simple_vector(ctx, runtime, &[slot_name, Word::NIL, Word::NIL]).unwrap();
    let slots = make_simple_vector(ctx, runtime, &[descriptor]).unwrap();
    let class = make_simple_vector(
        ctx,
        runtime,
        &[name, Word::NIL, slots, Word::fixnum(1), slots],
    )
    .unwrap();
    runtime.define_class(ctx, "W12-POINT", class).unwrap();
    let qualified = runtime.structure_class_name(ctx, name).unwrap();
    runtime.define_class(ctx, &qualified, class).unwrap();
    let layout = runtime.register_structure_layout(1).unwrap();
    runtime
        .register_structure_class_with_parent(ctx, layout, None, name)
        .unwrap();
    make_structure(ctx, runtime, layout, &[Word::fixnum(12)]).unwrap()
}

fn custom_print_object(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &ncl_object::BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ncl_object::ObjectError> {
    let text = make_string(ctx, runtime, &['H', 'O', 'O', 'K']).unwrap();
    let write_string = FunctionObject::try_from(
        runtime
            .function(ctx, "COMMON-LISP", "WRITE-STRING")
            .unwrap(),
    )
    .map_err(|_| ncl_object::ObjectError::UndefinedFunction)?;
    runtime.call_builtin(ctx, write_string, &[text, args.required(1)?])?;
    args.required(0)
}

fn register_custom_print_object(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
) -> Result<(), ncl_object::ObjectError> {
    runtime
        .register_builtin(
            ctx,
            BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new("PRINT-OBJECT")),
            BuiltinImplementation::direct(
                Builtin {
                    lambda_list: LambdaList::fixed(&[OBJECT, STREAM]),
                    convention: BuiltinConvention::Direct(Arity::exact(2)),
                },
                custom_print_object,
            ),
        )
        .map(|_| ())
}

#[test]
fn structure_builtins_cover_default_and_print_object_dispatch() {
    let (runtime, mut ctx) = context();
    let structure = point_structure(&runtime, &mut ctx);
    assert_eq!(
        render(&runtime, &mut ctx, structure, PrintOptions::new()),
        Ok("#S(W12-POINT :X 12)".to_string())
    );
    let make_stream = builtin(&runtime, &mut ctx, "MAKE-STRING-OUTPUT-STREAM");
    let stream = runtime.call_builtin(&mut ctx, make_stream, &[]).unwrap();
    register_custom_print_object(&mut ctx, &runtime).unwrap();
    let princ = builtin(&runtime, &mut ctx, "PRINC");
    assert_eq!(
        runtime.call_builtin(&mut ctx, princ, &[structure, stream]),
        Ok(structure)
    );
    let get_output = builtin(&runtime, &mut ctx, "GET-OUTPUT-STREAM-STRING");
    let output = runtime
        .call_builtin(&mut ctx, get_output, &[stream])
        .unwrap();
    assert_eq!(ncl_object::string_length(&ctx, output).unwrap(), 4);
}

#[test]
fn numbers_cover_nested_float_ratio_complex_and_radix_prefixes() {
    let (runtime, mut ctx) = context();
    let double = make_double(&mut ctx, &runtime, 3.0).unwrap();
    assert_eq!(
        render(&runtime, &mut ctx, double.as_word(), PrintOptions::new()),
        Ok("3.0".to_string())
    );
    let big = make_bignum_from_i128(&mut ctx, &runtime, 255).unwrap();
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            big.as_word(),
            PrintOptions::new().with_base(8).with_radix(true),
        ),
        Ok("#o377".to_string())
    );
    let ratio =
        ncl_object::make_ratio(&mut ctx, &runtime, Word::fixnum(3), Word::fixnum(4)).unwrap();
    let complex = make_complex(&mut ctx, &runtime, ratio.as_word(), Word::fixnum(5)).unwrap();
    assert_eq!(
        render(&runtime, &mut ctx, complex.as_word(), PrintOptions::new()),
        Ok("#C(3/4 5)".to_string())
    );
}

#[test]
fn arrays_cover_rank_two_limits_and_circle_shared_elements() {
    let (runtime, mut ctx) = context();
    let array = make_array(
        &mut ctx,
        &runtime,
        &[2, 2],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::fixnum(6),
            adjustable: false,
            fill_pointer: None,
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )
    .unwrap();
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            array,
            PrintOptions::new().with_length(Some(2)),
        ),
        Ok("#2A(6 6 ...)".to_string())
    );
    let shared = make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(9)]).unwrap();
    let tail = make_cons(&mut ctx, &runtime, shared, Word::NIL).unwrap();
    let outer = make_cons(&mut ctx, &runtime, shared, tail).unwrap();
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            outer,
            PrintOptions::new().with_circle(true),
        ),
        Ok("(#1=#(9) #1#)".to_string())
    );
}

#[test]
fn print_level_and_readability_take_precedence_over_object_rendering() {
    let (runtime, mut ctx) = context();
    let inner = make_cons(&mut ctx, &runtime, Word::fixnum(1), Word::NIL).unwrap();
    let outer = make_cons(&mut ctx, &runtime, inner, Word::NIL).unwrap();
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            outer,
            PrintOptions::new().with_level(Some(1)),
        ),
        Ok("(#)".to_string())
    );
    let function = builtin(&runtime, &mut ctx, "PRINC");
    assert!(
        render(&runtime, &mut ctx, function.as_word(), PrintOptions::new(),)
            .unwrap()
            .starts_with("#<FUNCTION ")
    );
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            function.as_word(),
            PrintOptions::new().with_readably(true),
        ),
        Err(PrintError::NotReadable)
    );
}

#[test]
fn ambient_specials_keep_defaults_for_unbound_and_invalid_lengths() {
    let (runtime, mut ctx) = context();
    let base = intern(&runtime, &mut ctx, "COMMON-LISP", "*PRINT-BASE*");
    let length = intern(&runtime, &mut ctx, "COMMON-LISP", "*PRINT-LENGTH*");
    let circle = intern(&runtime, &mut ctx, "COMMON-LISP", "*PRINT-CIRCLE*");
    for symbol in [base, length, circle] {
        set_symbol_special(&mut ctx, symbol, true).unwrap();
    }
    set_symbol_value(&mut ctx, base, Word::fixnum(37)).unwrap();
    set_symbol_value(&mut ctx, length, Word::fixnum(-2)).unwrap();
    set_symbol_value(&mut ctx, circle, Word::NIL).unwrap();
    let options = PrintOptions::from_specials(&mut ctx, &runtime);
    assert_eq!(options.base().get(), 10);
    assert_eq!(options.length(), None);
    assert!(!options.circle());
}

#[test]
fn print_builtin_rejects_missing_object_without_touching_stream() {
    let (runtime, mut ctx) = context();
    let print = builtin(&runtime, &mut ctx, "PRINT");
    assert_eq!(
        runtime.call_builtin(&mut ctx, print, &[]),
        Err(ncl_object::ObjectError::TypeError)
    );
}
