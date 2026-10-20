#![allow(
    clippy::unwrap_used,
    reason = "coverage tests assert on printer output"
)]

//! Fourteenth-wave broad matrix coverage selected from llvm-cov JSON.

use ncl_object::{
    Arity, ArrayElementType, ArrayOptions, Builtin, BuiltinConvention, BuiltinIdentifier,
    BuiltinImplementation, BuiltinName, BuiltinPackage, FunctionObject, LambdaList, MultipleValues,
    ObjectError, Package, Parameter, ParameterType, Runtime, ThreadContext, Word, make_array,
    make_bignum_from_i128, make_complex, make_cons, make_double, make_instance, make_simple_vector,
    make_string, make_structure, set_symbol_special, set_symbol_value,
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

fn builtin(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> FunctionObject {
    FunctionObject::try_from(runtime.function(ctx, "COMMON-LISP", name).unwrap()).unwrap()
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

fn make_structure_value(runtime: &Runtime, ctx: &mut ThreadContext) -> Word {
    let name = intern(runtime, ctx, "COMMON-LISP-USER", "W14-POINT");
    let slot = intern(runtime, ctx, "COMMON-LISP-USER", "X");
    let descriptor = make_simple_vector(ctx, runtime, &[slot, Word::NIL, Word::NIL]).unwrap();
    let slots = make_simple_vector(ctx, runtime, &[descriptor]).unwrap();
    let class = make_simple_vector(
        ctx,
        runtime,
        &[name, Word::NIL, slots, Word::fixnum(1), slots],
    )
    .unwrap();
    runtime.define_class(ctx, "W14-POINT", class).unwrap();
    let qualified = runtime.structure_class_name(ctx, name).unwrap();
    runtime.define_class(ctx, &qualified, class).unwrap();
    let layout = runtime.register_structure_layout(1).unwrap();
    runtime
        .register_structure_class_with_parent(ctx, layout, None, name)
        .unwrap();
    make_structure(ctx, runtime, layout, &[Word::fixnum(14)]).unwrap()
}

fn hook(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &ncl_object::BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ncl_object::ObjectError> {
    let text = make_string(ctx, runtime, &['W', '1', '4']).unwrap();
    let write_string = FunctionObject::try_from(
        runtime
            .function(ctx, "COMMON-LISP", "WRITE-STRING")
            .unwrap(),
    )
    .map_err(|_| ncl_object::ObjectError::UndefinedFunction)?;
    runtime.call_builtin(ctx, write_string, &[text, args.required(1)?])?;
    args.required(0)
}

#[test]
fn structure_and_instance_matrix_exercises_builtin_and_print_fallbacks() {
    let (runtime, mut ctx) = context();
    let structure = make_structure_value(&runtime, &mut ctx);
    assert_eq!(
        render(&runtime, &mut ctx, structure, PrintOptions::new()),
        Ok("#S(W14-POINT :X 14)".to_string())
    );
    let instance = make_instance(&mut ctx, &runtime, Word::NIL, &[]).unwrap();
    assert!(
        render(&runtime, &mut ctx, instance.as_word(), PrintOptions::new())
            .unwrap()
            .starts_with("#<INSTANCE ")
    );
    let make_stream = builtin(&runtime, &mut ctx, "MAKE-STRING-OUTPUT-STREAM");
    let stream = runtime.call_builtin(&mut ctx, make_stream, &[]).unwrap();
    let descriptor = Builtin {
        lambda_list: LambdaList::fixed(&[OBJECT, STREAM]),
        convention: BuiltinConvention::Direct(Arity::exact(2)),
    };
    runtime
        .register_builtin(
            &mut ctx,
            BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new("PRINT-OBJECT")),
            BuiltinImplementation::direct(descriptor, hook),
        )
        .unwrap();
    let princ = builtin(&runtime, &mut ctx, "PRINC");
    assert_eq!(
        runtime.call_builtin(&mut ctx, princ, &[structure, stream]),
        Ok(structure)
    );
    let get_output = builtin(&runtime, &mut ctx, "GET-OUTPUT-STREAM-STRING");
    let output = runtime
        .call_builtin(&mut ctx, get_output, &[stream])
        .unwrap();
    assert_eq!(ncl_object::string_length(&ctx, output).unwrap(), 3);
}

#[test]
fn circle_matrix_covers_simple_vector_array_and_dotted_cycles() {
    let (runtime, mut ctx) = context();
    let shared = make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(5)]).unwrap();
    let tail = make_cons(&mut ctx, &runtime, shared, Word::NIL).unwrap();
    let list = make_cons(&mut ctx, &runtime, shared, tail).unwrap();
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            list,
            PrintOptions::new().with_circle(true)
        ),
        Ok("(#1=#(5) #1#)".to_string())
    );
    let array = make_array(
        &mut ctx,
        &runtime,
        &[1],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::NIL,
            adjustable: false,
            fill_pointer: None,
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )
    .unwrap();
    ncl_object::array_row_major_set(&mut ctx, array, 0, array).unwrap();
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            array,
            PrintOptions::new().with_circle(true)
        ),
        Ok("#1=#(#1#)".to_string())
    );
}

#[test]
fn number_matrix_covers_radix_float_bignum_and_complex_signs() {
    let (runtime, mut ctx) = context();
    let float = make_double(&mut ctx, &runtime, -2.0).unwrap();
    assert_eq!(
        render(&runtime, &mut ctx, float.as_word(), PrintOptions::new()),
        Ok("-2.0".into())
    );
    let big = make_bignum_from_i128(&mut ctx, &runtime, 1023).unwrap();
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            big.as_word(),
            PrintOptions::new().with_base(16)
        ),
        Ok("3FF".to_string())
    );
    let ratio =
        ncl_object::make_ratio(&mut ctx, &runtime, Word::fixnum(-1), Word::fixnum(2)).unwrap();
    let complex = make_complex(&mut ctx, &runtime, Word::fixnum(-3), ratio.as_word()).unwrap();
    assert_eq!(
        render(&runtime, &mut ctx, complex.as_word(), PrintOptions::new()),
        Ok("#C(-3 -1/2)".to_string())
    );
}

#[test]
fn option_matrix_covers_invalid_base_limits_and_circle_modes() {
    let (runtime, mut ctx) = context();
    let base = intern(&runtime, &mut ctx, "COMMON-LISP", "*PRINT-BASE*");
    let length = intern(&runtime, &mut ctx, "COMMON-LISP", "*PRINT-LENGTH*");
    let circle = intern(&runtime, &mut ctx, "COMMON-LISP", "*PRINT-CIRCLE*");
    for symbol in [base, length, circle] {
        set_symbol_special(&mut ctx, symbol, true).unwrap();
    }
    set_symbol_value(&mut ctx, base, Word::fixnum(1)).unwrap();
    set_symbol_value(&mut ctx, length, Word::fixnum(3)).unwrap();
    set_symbol_value(&mut ctx, circle, Word::TRUE).unwrap();
    let options = PrintOptions::from_specials(&mut ctx, &runtime);
    assert_eq!(options.base().get(), 10);
    assert_eq!(options.length().map(ncl_printer::NonNegative::get), Some(3));
    assert!(options.circle());
    assert_eq!(options.with_base(1).base().get(), 10);
    assert_eq!(options.try_with_base(37), None);
}

#[test]
fn array_matrix_covers_specialized_opaque_and_zero_limit_paths() {
    let (runtime, mut ctx) = context();
    let specialized = ncl_object::make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Fixnum,
        &[Word::fixnum(2), Word::fixnum(4)],
    )
    .unwrap();
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            specialized,
            PrintOptions::new().with_vector_length(Some(0)),
        ),
        Ok("#(...)".to_string())
    );
    assert_eq!(
        render(&runtime, &mut ctx, specialized, PrintOptions::new()),
        Ok("#(2 4)".into())
    );
    assert!(
        render(
            &runtime,
            &mut ctx,
            specialized,
            PrintOptions::new().with_array(false)
        )
        .unwrap()
        .starts_with("#<ARRAY ")
    );
}

#[test]
fn builtin_argument_and_readability_errors_remain_typed() {
    let (runtime, mut ctx) = context();
    let print = builtin(&runtime, &mut ctx, "PRINT");
    assert_eq!(
        runtime.call_builtin(&mut ctx, print, &[]),
        Err(ObjectError::TypeError)
    );
    let text = make_string(&mut ctx, &runtime, &['x']).unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, print, &[text, Word::fixnum(4)]),
        Err(ObjectError::TypeError)
    );
    let vector = make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1)]).unwrap();
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            vector,
            PrintOptions::new().with_readably(true).with_array(false),
        ),
        Err(PrintError::NotReadable)
    );
}
