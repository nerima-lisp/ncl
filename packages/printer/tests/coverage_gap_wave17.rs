#![allow(
    clippy::unwrap_used,
    reason = "coverage tests assert on printer output"
)]

//! Seventeenth-wave coverage for dispatch wrappers, stream routing, and
//! boundary values that are reachable through the public printer API.

use ncl_object::{
    Arity, ArrayElementType, ArrayOptions, Builtin, BuiltinConvention, BuiltinIdentifier,
    BuiltinImplementation, BuiltinName, BuiltinPackage, FunctionObject, LambdaList, MultipleValues,
    Package, Parameter, ParameterType, Runtime, ThreadContext, Word, make_array, make_cons,
    make_double, make_ratio, make_simple_vector, make_string, make_structure, set_symbol_plist,
    set_symbol_special, set_symbol_value,
};
use ncl_printer::{PrintCase, PrintError, PrintOptions, StringSink, write};

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

fn builtin(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> FunctionObject {
    FunctionObject::try_from(runtime.function(ctx, "COMMON-LISP", name).unwrap()).unwrap()
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

fn qualified_structure(runtime: &Runtime, ctx: &mut ThreadContext) -> (Word, Word) {
    let name = intern(runtime, ctx, "WAVE17", "POINT");
    let slot_name = intern(runtime, ctx, "WAVE17", "X");
    let descriptor = make_simple_vector(ctx, runtime, &[slot_name, Word::NIL, Word::NIL]).unwrap();
    let slots = make_simple_vector(ctx, runtime, &[descriptor]).unwrap();
    let class = make_simple_vector(
        ctx,
        runtime,
        &[name, Word::NIL, slots, Word::fixnum(1), slots],
    )
    .unwrap();
    runtime.define_class(ctx, "WAVE17::POINT", class).unwrap();
    runtime
        .define_class(ctx, runtime.structure_class_name(ctx, name).unwrap(), class)
        .unwrap();
    let layout = runtime.register_structure_layout(1).unwrap();
    runtime
        .register_structure_class_with_parent(ctx, layout, None, name)
        .unwrap();
    (
        make_structure(ctx, runtime, layout, &[Word::fixnum(17)]).unwrap(),
        name,
    )
}

fn structure_hook(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &ncl_object::BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ncl_object::ObjectError> {
    let text = make_string(ctx, runtime, &['W', '1', '7'])?;
    let write_string = FunctionObject::try_from(
        runtime
            .function(ctx, "COMMON-LISP", "WRITE-STRING")
            .ok_or(ncl_object::ObjectError::UndefinedFunction)?,
    )
    .map_err(|_| ncl_object::ObjectError::UndefinedFunction)?;
    runtime.call_builtin(ctx, write_string, &[text, args.required(1)?])?;
    args.required(0)
}

#[test]
fn structure_hook_accepts_function_wrapper_and_skips_unmatched_property() {
    let (runtime, mut ctx) = context();
    let (structure, name) = qualified_structure(&runtime, &mut ctx);
    let function = runtime
        .register_builtin(
            &mut ctx,
            BuiltinIdentifier::new(BuiltinPackage::NclTest, BuiltinName::new("WAVE17-HOOK")),
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
    let unrelated = intern(&runtime, &mut ctx, "NCL", "%OTHER");
    let function_operator = intern(&runtime, &mut ctx, "COMMON-LISP", "FUNCTION");
    let function_tail = make_cons(&mut ctx, &runtime, function.as_word(), Word::NIL).unwrap();
    let function_form = make_cons(&mut ctx, &runtime, function_operator, function_tail).unwrap();
    let wrong_property = make_cons(&mut ctx, &runtime, unrelated, Word::NIL).unwrap();
    let right_property = make_cons(&mut ctx, &runtime, key, function_form).unwrap();
    let rest = make_cons(&mut ctx, &runtime, right_property, Word::NIL).unwrap();
    let plist = make_cons(&mut ctx, &runtime, wrong_property, rest).unwrap();
    set_symbol_plist(&mut ctx, name, plist).unwrap();

    let make_stream = builtin(&runtime, &mut ctx, "MAKE-STRING-OUTPUT-STREAM");
    let stream = runtime.call_builtin(&mut ctx, make_stream, &[]).unwrap();
    let princ = builtin(&runtime, &mut ctx, "PRINC");
    assert_eq!(
        runtime.call_builtin(&mut ctx, princ, &[structure, stream]),
        Ok(structure)
    );
    let get_output = builtin(&runtime, &mut ctx, "GET-OUTPUT-STREAM-STRING");
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
        Ok("W17".into())
    );
}

#[test]
fn print_builtin_adds_newlines_and_routes_true_nil_and_explicit_streams() {
    let (runtime, mut ctx) = context();
    let make_stream = builtin(&runtime, &mut ctx, "MAKE-STRING-OUTPUT-STREAM");
    let standard = runtime.call_builtin(&mut ctx, make_stream, &[]).unwrap();
    let terminal = runtime.call_builtin(&mut ctx, make_stream, &[]).unwrap();
    let explicit = runtime.call_builtin(&mut ctx, make_stream, &[]).unwrap();
    let standard_symbol = intern(&runtime, &mut ctx, "COMMON-LISP", "*STANDARD-OUTPUT*");
    let terminal_symbol = intern(&runtime, &mut ctx, "COMMON-LISP", "*TERMINAL-IO*");
    set_symbol_value(&mut ctx, standard_symbol, standard).unwrap();
    set_symbol_value(&mut ctx, terminal_symbol, terminal).unwrap();
    let print = builtin(&runtime, &mut ctx, "PRINT");
    let value = make_string(&mut ctx, &runtime, &['x']).unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, print, &[value, Word::NIL]),
        Ok(value)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, print, &[value, Word::TRUE]),
        Ok(value)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, print, &[value, explicit]),
        Ok(value)
    );
    let get_output = builtin(&runtime, &mut ctx, "GET-OUTPUT-STREAM-STRING");
    for stream in [standard, terminal, explicit] {
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
            Ok("\n\"x\"\n".into())
        );
    }
}

#[test]
fn options_specials_distinguish_symbols_other_values_and_valid_limits() {
    let (runtime, mut ctx) = context();
    let names = [
        "*PRINT-ESCAPE*",
        "*PRINT-BASE*",
        "*PRINT-LENGTH*",
        "*PRINT-CASE*",
    ];
    let symbols: Vec<_> = names
        .iter()
        .map(|name| intern(&runtime, &mut ctx, "COMMON-LISP", name))
        .collect();
    for symbol in symbols.iter().copied() {
        set_symbol_special(&mut ctx, symbol, true).unwrap();
    }
    set_symbol_value(&mut ctx, *symbols.first().unwrap(), Word::NIL).unwrap();
    set_symbol_value(&mut ctx, *symbols.get(1).unwrap(), Word::fixnum(16)).unwrap();
    set_symbol_value(&mut ctx, *symbols.get(2).unwrap(), Word::fixnum(0)).unwrap();
    let downcase = intern(&runtime, &mut ctx, "COMMON-LISP", "DOWNCASE");
    set_symbol_value(&mut ctx, *symbols.get(3).unwrap(), downcase).unwrap();
    let options = PrintOptions::from_specials(&mut ctx, &runtime);
    assert!(!options.escape());
    assert_eq!(options.base().get(), 16);
    assert_eq!(options.length().map(ncl_printer::NonNegative::get), Some(0));
    assert_eq!(options.case(), PrintCase::Upcase);
    set_symbol_value(&mut ctx, *symbols.get(1).unwrap(), Word::fixnum(1)).unwrap();
    set_symbol_value(&mut ctx, *symbols.get(2).unwrap(), Word::TRUE).unwrap();
    let fallback = PrintOptions::from_specials(&mut ctx, &runtime);
    assert_eq!(fallback.base().get(), 10);
    assert_eq!(fallback.length(), None);
}

#[test]
fn number_matrix_covers_ratio_complex_and_float_boundaries() {
    let (runtime, mut ctx) = context();
    let ratio = make_ratio(&mut ctx, &runtime, Word::fixnum(-7), Word::fixnum(3)).unwrap();
    let ratio_word = ratio.into();
    let complex =
        ncl_object::make_complex(&mut ctx, &runtime, ratio_word, Word::fixnum(2)).unwrap();
    let negative_zero = make_double(&mut ctx, &runtime, -0.0).unwrap();
    assert_eq!(
        render(&runtime, &mut ctx, ratio_word, PrintOptions::new()),
        Ok("-7/3".into())
    );
    assert_eq!(
        render(&runtime, &mut ctx, complex.into(), PrintOptions::new()),
        Ok("#C(-7/3 2)".into())
    );
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            negative_zero.as_word(),
            PrintOptions::new()
        ),
        Ok("-0.0".into())
    );
}

#[test]
fn array_matrix_obeys_length_level_and_readability_options() {
    let (runtime, mut ctx) = context();
    let array = make_array(
        &mut ctx,
        &runtime,
        &[4],
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
        render(&runtime, &mut ctx, array, PrintOptions::new()),
        Ok("#(6 6 6 6)".into())
    );
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            array,
            PrintOptions::new().with_vector_length(Some(2))
        ),
        Ok("#(6 6 ...)".into())
    );
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            array,
            PrintOptions::new().with_level(Some(0))
        ),
        Ok("#".into())
    );
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            array,
            PrintOptions::new().with_readably(true).with_array(false)
        ),
        Err(PrintError::NotReadable)
    );
}

#[test]
fn circle_matrix_labels_shared_arrays_but_reports_unlabelled_cycles() {
    let (runtime, mut ctx) = context();
    let shared = make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(9)]).unwrap();
    let list = make_cons(&mut ctx, &runtime, shared, Word::NIL).unwrap();
    ncl_object::rplacd(&mut ctx, list, list).unwrap();
    assert_eq!(
        render(&runtime, &mut ctx, list, PrintOptions::new()),
        Err(PrintError::Circularity)
    );
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            list,
            PrintOptions::new().with_circle(true)
        ),
        Ok("#1=(#(9) . #1#)".into())
    );
}
