#![allow(clippy::unwrap_used, reason = "tests assert on printer output")]

//! Reachable opaque, structure, instance, and PRINT-OBJECT paths.

use ncl_object::{
    Arity, Builtin, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation, BuiltinName,
    BuiltinPackage, FunctionObject, LambdaList, MultipleValues, Package, Parameter, ParameterType,
    Runtime, ThreadContext, Word, make_cons, make_instance, make_simple_vector, make_string,
    make_structure,
};
use ncl_printer::{PrintOptions, StringSink, write};

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
) -> String {
    let mut sink = StringSink::new();
    write(ctx, runtime, object, &mut sink, &options).unwrap();
    sink.into_string()
}

fn render_maybe_error(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    object: Word,
    options: PrintOptions,
) -> Result<String, ncl_printer::PrintError> {
    let mut sink = StringSink::new();
    write(ctx, runtime, object, &mut sink, &options)?;
    Ok(sink.into_string())
}

fn write_custom_print_object(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &ncl_object::BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ncl_object::ObjectError> {
    let message = make_string(
        ctx,
        runtime,
        &"CUSTOM-PRINT-OBJECT".chars().collect::<Vec<_>>(),
    )?;
    let write_string = runtime
        .function(ctx, "COMMON-LISP", "WRITE-STRING")
        .ok_or(ncl_object::ObjectError::UndefinedFunction)?;
    let write_string = FunctionObject::try_from(write_string)
        .map_err(|_| ncl_object::ObjectError::UndefinedFunction)?;
    runtime.call_builtin(ctx, write_string, &[message, args.required(1)?])?;
    args.required(0)
}

fn register_custom_print_object(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
) -> Result<(), ncl_object::ObjectError> {
    let descriptor = Builtin {
        lambda_list: LambdaList::fixed(&[OBJECT, STREAM]),
        convention: BuiltinConvention::Direct(Arity::exact(2)),
    };
    runtime
        .register_builtin(
            ctx,
            BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new("PRINT-OBJECT")),
            BuiltinImplementation::direct(descriptor, write_custom_print_object),
        )
        .map(|_| ())
}

fn point_structure(runtime: &Runtime, ctx: &mut ThreadContext) -> Word {
    let name = intern(runtime, ctx, "COMMON-LISP-USER", "POINT");
    let slot_name = intern(runtime, ctx, "COMMON-LISP-USER", "X");
    let descriptor = make_simple_vector(ctx, runtime, &[slot_name, Word::NIL, Word::NIL]).unwrap();
    let slots = make_simple_vector(ctx, runtime, &[descriptor]).unwrap();
    let class = make_simple_vector(
        ctx,
        runtime,
        &[name, Word::NIL, slots, Word::fixnum(1), slots],
    )
    .unwrap();
    runtime.define_class(ctx, "POINT", class).unwrap();
    let qualified = runtime.structure_class_name(ctx, name).unwrap();
    runtime.define_class(ctx, &qualified, class).unwrap();
    let layout = runtime.register_structure_layout(1).unwrap();
    runtime
        .register_structure_class_with_parent(ctx, layout, None, name)
        .unwrap();
    make_structure(ctx, runtime, layout, &[Word::fixnum(8)]).unwrap()
}

#[test]
fn opaque_function_and_unregistered_structure_have_stable_prefixes() {
    let (runtime, mut ctx) = context();
    let function = runtime.function(&mut ctx, "COMMON-LISP", "PRINC").unwrap();
    let function_output = render(&runtime, &mut ctx, function, PrintOptions::new());
    assert!(
        function_output.starts_with("#<FUNCTION "),
        "{function_output}"
    );

    let layout = runtime.register_structure_layout(1).unwrap();
    let structure = make_structure(&mut ctx, &runtime, layout, &[Word::fixnum(8)]).unwrap();
    let structure_output = render(&runtime, &mut ctx, structure, PrintOptions::new());
    assert!(
        structure_output.starts_with("#<STRUCTURE "),
        "{structure_output}"
    );

    let instance = make_instance(&mut ctx, &runtime, Word::NIL, &[])
        .unwrap()
        .as_word();
    let instance_output = render(&runtime, &mut ctx, instance, PrintOptions::new());
    assert!(
        instance_output.starts_with("#<INSTANCE "),
        "{instance_output}"
    );
}

#[test]
fn registered_structure_prints_name_slot_and_value() {
    let (runtime, mut ctx) = context();
    let structure = point_structure(&runtime, &mut ctx);
    assert_eq!(
        render(&runtime, &mut ctx, structure, PrintOptions::new()),
        "#S(POINT :X 8)"
    );
}

#[test]
fn condition_instances_use_report_for_princ_and_opaque_for_prin1() {
    let (runtime, mut ctx) = context();
    ncl_conditions::register(&runtime).unwrap();
    let class = ncl_conditions::condition_class(&mut ctx, &runtime, "SIMPLE-CONDITION").unwrap();
    let control = make_string(&mut ctx, &runtime, &"bad ~a".chars().collect::<Vec<_>>()).unwrap();
    let arguments = make_cons(&mut ctx, &runtime, Word::fixnum(9), Word::NIL).unwrap();
    let condition =
        ncl_conditions::make_condition(&mut ctx, &runtime, class, &[control, arguments]).unwrap();
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            condition,
            PrintOptions::new().with_escape(false)
        ),
        "bad 9"
    );
    let escaped = render(&runtime, &mut ctx, condition, PrintOptions::new());
    assert!(escaped.starts_with("#<INSTANCE "), "{escaped}");
}

#[test]
fn print_object_method_handles_structure_before_default_rendering() {
    let (runtime, mut ctx) = context();
    let structure = point_structure(&runtime, &mut ctx);
    register_custom_print_object(&mut ctx, &runtime).unwrap();
    let make_stream = FunctionObject::try_from(
        runtime
            .function(&mut ctx, "COMMON-LISP", "MAKE-STRING-OUTPUT-STREAM")
            .unwrap(),
    )
    .unwrap();
    let stream = runtime.call_builtin(&mut ctx, make_stream, &[]).unwrap();
    let princ =
        FunctionObject::try_from(runtime.function(&mut ctx, "COMMON-LISP", "PRINC").unwrap())
            .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, princ, &[structure, stream]),
        Ok(structure)
    );
    let get_stream = FunctionObject::try_from(
        runtime
            .function(&mut ctx, "COMMON-LISP", "GET-OUTPUT-STREAM-STRING")
            .unwrap(),
    )
    .unwrap();
    let output = runtime
        .call_builtin(&mut ctx, get_stream, &[stream])
        .unwrap();
    let rendered = render_maybe_error(&runtime, &mut ctx, output, PrintOptions::new()).unwrap();
    assert_eq!(rendered, "\"CUSTOM-PRINT-OBJECT\"");
}
