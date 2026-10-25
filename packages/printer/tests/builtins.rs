#![allow(clippy::unwrap_used, reason = "tests assert on builtin output")]
#![allow(missing_docs)]

use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, FunctionObject, LambdaList, MultipleValues, ObjectError, Package,
    Parameter, ParameterType, Runtime, ThreadContext, Word, make_cons, make_string,
};

const STRUCTURE_PRINT_PARAMETERS: &[Parameter] = &[
    Parameter {
        name: BuiltinName::new("OBJECT"),
        ty: ParameterType::Any,
    },
    Parameter {
        name: BuiltinName::new("STREAM"),
        ty: ParameterType::Any,
    },
    Parameter {
        name: BuiltinName::new("DEPTH"),
        ty: ParameterType::Any,
    },
];

fn structure_print_callback(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    ncl_object::structure_layout(ctx, args.required(0)?)?;
    if args.required(2)?.as_fixnum() != Some(0) {
        return Err(ObjectError::TypeError);
    }
    let mut stream = args.required(1)?;
    ncl_object::with_root(ctx, &mut stream, |ctx, stream| {
        let mut text = make_string(ctx, runtime, &"PRINT-FUNCTION".chars().collect::<Vec<_>>())?;
        ncl_object::with_root(ctx, &mut text, |ctx, text| {
            let writer = runtime
                .function(ctx, "COMMON-LISP", "WRITE-STRING")
                .ok_or(ObjectError::Layout)?;
            let writer = FunctionObject::try_from(writer)?;
            runtime.call_builtin(ctx, writer, &[text.get(), stream.get()])
        })
    })
}

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

fn string_output_for(name: &str, value: &str) -> String {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_printer::register(&mut ctx, &runtime).unwrap();
    ctx.set_gc_stress(true);
    let mut object = make_string(&mut ctx, &runtime, &value.chars().collect::<Vec<_>>()).unwrap();
    let object_root = ncl_object::push_root(&mut ctx, &mut object);
    let result = call(&runtime, &mut ctx, name, &[object]);
    assert!(ncl_object::pop_root(&mut ctx, object_root));
    string_contents(&ctx, result)
}

fn string_contents(ctx: &ThreadContext, string: Word) -> String {
    let length = ncl_object::string_length(ctx, string).unwrap();
    (0..length)
        .map(|index| ncl_object::string_ref(ctx, string, index).unwrap())
        .collect()
}

fn keyword(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> Word {
    let package = runtime.ensure_package(ctx, "KEYWORD").unwrap();
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap()
        .0
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
fn string_printer_builtins_apply_their_escape_and_write_options() {
    assert_eq!(string_output_for("PRINC-TO-STRING", "x"), "x");
    assert_eq!(string_output_for("PRIN1-TO-STRING", "x"), "\"x\"");
    assert_eq!(string_output_for("WRITE-TO-STRING", "x"), "\"x\"");

    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_printer::register(&mut ctx, &runtime).unwrap();
    let object = make_string(&mut ctx, &runtime, &['x']).unwrap();
    let escape = keyword(&runtime, &mut ctx, "ESCAPE");
    let allow_other_keys = keyword(&runtime, &mut ctx, "ALLOW-OTHER-KEYS");
    let user_package = runtime
        .ensure_package(&mut ctx, "COMMON-LISP-USER")
        .unwrap();
    let unknown = Package::from_word(user_package)
        .intern(&mut ctx, &runtime, "UNKNOWN-KEY")
        .unwrap()
        .0;
    let result = call(
        &runtime,
        &mut ctx,
        "WRITE-TO-STRING",
        &[
            object,
            escape,
            Word::NIL,
            allow_other_keys,
            Word::TRUE,
            unknown,
            Word::fixnum(0),
        ],
    );
    assert_eq!(string_contents(&ctx, result), "x");

    let base = keyword(&runtime, &mut ctx, "BASE");
    let result = call(
        &runtime,
        &mut ctx,
        "WRITE-TO-STRING",
        &[
            Word::fixnum(4),
            base,
            Word::fixnum(2),
            base,
            Word::fixnum(10),
        ],
    );
    assert_eq!(string_contents(&ctx, result), "100");
}

#[test]
fn princ_to_string_disables_readability_for_unreadable_objects() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_printer::register(&mut ctx, &runtime).unwrap();
    let common_lisp = runtime.ensure_package(&mut ctx, "COMMON-LISP").unwrap();
    let print_readably = Package::from_word(common_lisp)
        .intern(&mut ctx, &runtime, "*PRINT-READABLY*")
        .unwrap()
        .0;
    ncl_object::set_symbol_value(&mut ctx, print_readably, Word::TRUE).unwrap();

    let object = runtime
        .function(&mut ctx, "COMMON-LISP", "PRINC-TO-STRING")
        .unwrap();
    let result = call(&runtime, &mut ctx, "PRINC-TO-STRING", &[object]);
    assert!(string_contents(&ctx, result).starts_with("#<FUNCTION "));

    let prin1 = function(&runtime, &mut ctx, "PRIN1-TO-STRING");
    assert_eq!(
        runtime.call_builtin(&mut ctx, prin1, &[object]),
        Err(ncl_object::ObjectError::Layout)
    );
}

#[test]
fn write_to_string_accepts_bignum_limits() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_printer::register(&mut ctx, &runtime).unwrap();
    let vector =
        ncl_object::make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1), Word::fixnum(2)])
            .unwrap();
    let large_limit: Word = ncl_object::make_bignum_from_i128(&mut ctx, &runtime, i128::MAX)
        .unwrap()
        .into();
    let length = keyword(&runtime, &mut ctx, "LENGTH");

    let result = call(
        &runtime,
        &mut ctx,
        "WRITE-TO-STRING",
        &[vector, length, large_limit],
    );
    assert_eq!(string_contents(&ctx, result), "#(1 2)");
}

#[test]
fn structure_print_function_reads_rooted_values_after_package_allocations()
-> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_printer::register(&mut ctx, &runtime)?;
    ncl_lib_streams::register(&runtime)?;

    let user_package = runtime.ensure_package(&mut ctx, "COMMON-LISP-USER")?;
    let (mut name, _) = Package::from_word(user_package).intern(&mut ctx, &runtime, "PRINTED")?;
    let name_token = ncl_object::push_root(&mut ctx, &mut name);
    let ncl_package = runtime.ensure_package(&mut ctx, "NCL")?;
    let (mut key, _) =
        Package::from_word(ncl_package).intern(&mut ctx, &runtime, "%STRUCTURE-PRINT-FUNCTION")?;
    let key_token = ncl_object::push_root(&mut ctx, &mut key);

    let print_function = runtime.register_builtin(
        &mut ctx,
        BuiltinIdentifier::new(BuiltinPackage::NclTest, BuiltinName::new("STRUCTURE-PRINT")),
        BuiltinImplementation::direct(
            Builtin {
                lambda_list: LambdaList::fixed(STRUCTURE_PRINT_PARAMETERS),
                convention: BuiltinConvention::Direct(Arity::exact(3)),
            },
            structure_print_callback,
        ),
    )?;
    let mut print_function_word = print_function.as_word();
    let print_function_token = ncl_object::push_root(&mut ctx, &mut print_function_word);

    let layout = runtime.register_structure_layout(0)?;
    let mut class = ncl_object::make_simple_vector(&mut ctx, &runtime, &[name])?;
    let class_token = ncl_object::push_root(&mut ctx, &mut class);
    let class_name = runtime.structure_class_name(&ctx, name)?;
    runtime.define_class(&mut ctx, class_name, class)?;
    runtime.register_structure_class_with_parent(&ctx, layout, None, name)?;

    let mut property = make_cons(&mut ctx, &runtime, print_function_word, Word::NIL)?;
    let property_token = ncl_object::push_root(&mut ctx, &mut property);
    let mut plist = make_cons(&mut ctx, &runtime, key, property)?;
    let plist_token = ncl_object::push_root(&mut ctx, &mut plist);
    let mut unrelated_value = make_cons(&mut ctx, &runtime, Word::NIL, plist)?;
    let unrelated_value_token = ncl_object::push_root(&mut ctx, &mut unrelated_value);
    plist = make_cons(&mut ctx, &runtime, name, unrelated_value)?;
    ncl_object::set_symbol_plist(&mut ctx, name, plist)?;

    let mut structure = ncl_object::make_structure(&mut ctx, &runtime, layout, &[])?;
    let structure_token = ncl_object::push_root(&mut ctx, &mut structure);
    let mut stream = call(&runtime, &mut ctx, "MAKE-STRING-OUTPUT-STREAM", &[]);
    let stream_token = ncl_object::push_root(&mut ctx, &mut stream);

    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);
    let result = ncl_object::with_roots(&mut ctx, &[structure, stream], |ctx, roots| {
        let princ = runtime
            .function(ctx, "COMMON-LISP", "PRINC")
            .ok_or(ObjectError::Layout)?;
        let princ = FunctionObject::try_from(princ)?;
        runtime.call_builtin(
            ctx,
            princ,
            &[
                roots.first().ok_or(ObjectError::Layout)?.get(),
                roots.get(1).ok_or(ObjectError::Layout)?.get(),
            ],
        )
    })?;
    assert_eq!(result, structure);

    let get_output = runtime
        .function(&mut ctx, "COMMON-LISP", "GET-OUTPUT-STREAM-STRING")
        .ok_or(ObjectError::Layout)?;
    let get_output = FunctionObject::try_from(get_output)?;
    let output = runtime.call_builtin(&mut ctx, get_output, &[stream])?;
    assert_eq!(string_contents(&ctx, output), "PRINT-FUNCTION");

    assert!(ncl_object::pop_root(&mut ctx, stream_token));
    assert!(ncl_object::pop_root(&mut ctx, structure_token));
    assert!(ncl_object::pop_root(&mut ctx, unrelated_value_token));
    assert!(ncl_object::pop_root(&mut ctx, plist_token));
    assert!(ncl_object::pop_root(&mut ctx, property_token));
    assert!(ncl_object::pop_root(&mut ctx, class_token));
    assert!(ncl_object::pop_root(&mut ctx, print_function_token));
    assert!(ncl_object::pop_root(&mut ctx, key_token));
    assert!(ncl_object::pop_root(&mut ctx, name_token));
    Ok(())
}
