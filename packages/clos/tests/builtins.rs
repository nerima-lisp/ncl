#![allow(missing_docs)]
#![allow(clippy::unwrap_used, reason = "tests assert on builtin registration")]

use ncl_object::{BuiltinConvention, FunctionObject, Package, Runtime, ThreadContext, Word};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_clos::register(&runtime).unwrap();
    (runtime, ctx)
}

fn function(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> FunctionObject {
    FunctionObject::try_from(runtime.function(ctx, "COMMON-LISP", name).unwrap()).unwrap()
}

fn intern(runtime: &Runtime, ctx: &mut ThreadContext, package: &str, name: &str) -> Word {
    let package = runtime.find_package(ctx, package).unwrap();
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap()
        .0
}

fn list(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Word {
    values.iter().rev().fold(Word::NIL, |tail, value| {
        ncl_object::make_cons(ctx, runtime, *value, tail).unwrap()
    })
}

fn class_name_text(ctx: &ThreadContext, class: Word) -> String {
    let name = ncl_clos::class_name(ctx, class).unwrap();
    (0..ncl_object::string_length(ctx, name).unwrap())
        .map(|index| ncl_object::string_ref(ctx, name, index).unwrap())
        .collect()
}

#[test]
fn registration_replaces_type_class_placeholders_with_class_objects() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_types::register(&runtime).unwrap();
    assert_eq!(runtime.class(&mut ctx, "INTEGER"), Some(Word::fixnum(1)));

    ncl_clos::register(&runtime).unwrap();
    let integer = runtime.class(&mut ctx, "INTEGER").unwrap();
    assert_eq!(ncl_object::simple_vector_length(&ctx, integer), Ok(5));
    let class_of = function(&runtime, &mut ctx, "CLASS-OF");
    let built_in_class = runtime.class(&mut ctx, "BUILT-IN-CLASS").unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, class_of, &[integer]),
        Ok(built_in_class)
    );
}

#[test]
fn class_of_and_class_name_are_bound_builtins() {
    let (runtime, mut ctx) = setup();
    let class_of = function(&runtime, &mut ctx, "CLASS-OF");
    let integer = runtime.class(&mut ctx, "INTEGER").unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, class_of, &[Word::fixnum(7)]),
        Ok(integer)
    );

    let class_name = function(&runtime, &mut ctx, "CLASS-NAME");
    let name = runtime
        .call_builtin(&mut ctx, class_name, &[integer])
        .unwrap();
    assert_ne!(name, Word::UNBOUND);
}

#[test]
fn standard_classes_have_registered_metaobjects_and_supported_precedence() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_types::register(&runtime).unwrap();
    ncl_conditions::register(&runtime).unwrap();
    ncl_clos::register(&runtime).unwrap();

    let class_of = function(&runtime, &mut ctx, "CLASS-OF");
    let built_in_class = runtime.class(&mut ctx, "BUILT-IN-CLASS").unwrap();
    for name in [
        "REAL",
        "RATIONAL",
        "INTEGER",
        "BIGNUM",
        "RATIO",
        "FLOAT",
        "DOUBLE-FLOAT",
        "COMPLEX",
        "ARRAY",
        "SEQUENCE",
        "VECTOR",
        "BIT-VECTOR",
        "SIMPLE-BIT-VECTOR",
        "STRING",
        "SIMPLE-STRING",
        "SIMPLE-ARRAY",
        "BROADCAST-STREAM",
        "CONCATENATED-STREAM",
        "ECHO-STREAM",
        "FILE-STREAM",
        "STRING-STREAM",
        "SYNONYM-STREAM",
        "TWO-WAY-STREAM",
        "PATHNAME",
        "LOGICAL-PATHNAME",
        "READTABLE",
        "RANDOM-STATE",
    ] {
        let class = runtime.class(&mut ctx, name).unwrap();
        assert_eq!(
            runtime.call_builtin(&mut ctx, class_of, &[class]),
            Ok(built_in_class),
            "{name} metaclass"
        );
        assert_eq!(
            ncl_object::simple_vector_length(&ctx, class),
            Ok(5),
            "{name}"
        );
    }

    let standard_class = runtime.class(&mut ctx, "STANDARD-CLASS").unwrap();
    for name in [
        "CLASS",
        "STANDARD-OBJECT",
        "STANDARD-CLASS",
        "CONDITION",
        "ERROR",
    ] {
        let class = runtime.class(&mut ctx, name).unwrap();
        assert_eq!(
            runtime.call_builtin(&mut ctx, class_of, &[class]),
            Ok(standard_class),
            "{name} metaclass"
        );
    }

    let readtable = ncl_object::make_readtable(&mut ctx, &runtime, Word::NIL, Word::NIL, Word::NIL)
        .unwrap()
        .as_word();
    assert_eq!(
        runtime.call_builtin(&mut ctx, class_of, &[readtable]),
        Ok(runtime.class(&mut ctx, "READTABLE").unwrap())
    );

    ncl_lib_streams::register(&runtime).unwrap();
    let make_input = function(&runtime, &mut ctx, "MAKE-STRING-INPUT-STREAM");
    let source = ncl_object::make_string(&mut ctx, &runtime, &['x']).unwrap();
    let string_stream = runtime
        .call_builtin(&mut ctx, make_input, &[source])
        .unwrap();
    let string_stream_class = runtime.class(&mut ctx, "STRING-STREAM").unwrap();
    let typep = function(&runtime, &mut ctx, "TYPEP");
    assert_eq!(
        runtime.call_builtin(&mut ctx, class_of, &[string_stream]),
        Ok(string_stream_class)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, typep, &[string_stream, string_stream_class]),
        Ok(Word::TRUE)
    );
}

#[test]
fn standard_class_precedence_lists_follow_c3() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_types::register(&runtime).unwrap();
    ncl_conditions::register(&runtime).unwrap();
    ncl_clos::register(&runtime).unwrap();

    let precedence = FunctionObject::try_from(
        runtime
            .function(&mut ctx, "NCL-MOP", "CLASS-PRECEDENCE-LIST")
            .unwrap(),
    )
    .unwrap();
    for (name, expected) in [
        ("VECTOR", ["VECTOR", "ARRAY", "SEQUENCE", "T"].as_slice()),
        (
            "NULL",
            ["NULL", "SYMBOL", "LIST", "SEQUENCE", "T"].as_slice(),
        ),
        (
            "INTEGER",
            ["INTEGER", "RATIONAL", "REAL", "NUMBER", "T"].as_slice(),
        ),
        (
            "LOGICAL-PATHNAME",
            ["LOGICAL-PATHNAME", "PATHNAME", "T"].as_slice(),
        ),
    ] {
        let class = runtime.class(&mut ctx, name).unwrap();
        let result = runtime
            .call_builtin(&mut ctx, precedence, &[class])
            .unwrap();
        let names = (0..ncl_object::simple_vector_length(&ctx, result).unwrap())
            .map(|index| {
                class_name_text(
                    &ctx,
                    ncl_object::simple_vector_ref(&ctx, result, index).unwrap(),
                )
            })
            .collect::<Vec<_>>();
        let expected = expected
            .iter()
            .map(|name| (*name).to_owned())
            .collect::<Vec<_>>();
        assert_eq!(names, expected, "{name} precedence");
    }

    for (name, expected) in [
        (
            "SIMPLE-ERROR",
            [
                "SIMPLE-ERROR",
                "SIMPLE-CONDITION",
                "ERROR",
                "SERIOUS-CONDITION",
                "CONDITION",
                "T",
            ]
            .as_slice(),
        ),
        (
            "READER-ERROR",
            [
                "READER-ERROR",
                "PARSE-ERROR",
                "STREAM-ERROR",
                "ERROR",
                "SERIOUS-CONDITION",
                "CONDITION",
                "T",
            ]
            .as_slice(),
        ),
    ] {
        let class = runtime.class(&mut ctx, name).unwrap();
        let result = runtime
            .call_builtin(&mut ctx, precedence, &[class])
            .unwrap();
        let names = (0..ncl_object::simple_vector_length(&ctx, result).unwrap())
            .map(|index| {
                class_name_text(
                    &ctx,
                    ncl_object::simple_vector_ref(&ctx, result, index).unwrap(),
                )
            })
            .collect::<Vec<_>>();
        let expected = expected
            .iter()
            .map(|name| (*name).to_owned())
            .collect::<Vec<_>>();
        assert_eq!(names, expected, "{name} precedence");
    }
}

#[test]
fn figure_4_8_classes_installed_by_core_layers_have_metaobjects() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_types::register(&runtime).unwrap();
    ncl_conditions::register(&runtime).unwrap();
    ncl_clos::register(&runtime).unwrap();

    let classes = [
        "ARITHMETIC-ERROR",
        "ARRAY",
        "BIT-VECTOR",
        "BROADCAST-STREAM",
        "BUILT-IN-CLASS",
        "CELL-ERROR",
        "CHARACTER",
        "CLASS",
        "COMPLEX",
        "CONCATENATED-STREAM",
        "CONDITION",
        "CONS",
        "CONTROL-ERROR",
        "DIVISION-BY-ZERO",
        "ECHO-STREAM",
        "END-OF-FILE",
        "ERROR",
        "FILE-ERROR",
        "FILE-STREAM",
        "FLOAT",
        "FLOATING-POINT-INEXACT",
        "FLOATING-POINT-INVALID-OPERATION",
        "FLOATING-POINT-OVERFLOW",
        "FLOATING-POINT-UNDERFLOW",
        "FUNCTION",
        "GENERIC-FUNCTION",
        "HASH-TABLE",
        "INTEGER",
        "LIST",
        "LOGICAL-PATHNAME",
        "METHOD",
        "METHOD-COMBINATION",
        "NULL",
        "NUMBER",
        "PACKAGE",
        "PACKAGE-ERROR",
        "PARSE-ERROR",
        "PATHNAME",
        "PRINT-NOT-READABLE",
        "PROGRAM-ERROR",
        "RANDOM-STATE",
        "RATIO",
        "RATIONAL",
        "READER-ERROR",
        "READTABLE",
        "REAL",
        "RESTART",
        "SEQUENCE",
        "SERIOUS-CONDITION",
        "SIMPLE-CONDITION",
        "SIMPLE-ERROR",
        "SIMPLE-TYPE-ERROR",
        "SIMPLE-WARNING",
        "STANDARD-CLASS",
        "STANDARD-GENERIC-FUNCTION",
        "STANDARD-METHOD",
        "STANDARD-OBJECT",
        "STORAGE-CONDITION",
        "STREAM",
        "STREAM-ERROR",
        "STRING",
        "STRING-STREAM",
        "STRUCTURE-CLASS",
        "STRUCTURE-OBJECT",
        "STYLE-WARNING",
        "SYMBOL",
        "SYNONYM-STREAM",
        "T",
        "TWO-WAY-STREAM",
        "TYPE-ERROR",
        "UNBOUND-SLOT",
        "UNBOUND-VARIABLE",
        "UNDEFINED-FUNCTION",
        "VECTOR",
        "WARNING",
    ];
    assert_eq!(classes.len(), 75);

    let class_of = function(&runtime, &mut ctx, "CLASS-OF");
    let built_in_class = runtime.class(&mut ctx, "BUILT-IN-CLASS").unwrap();
    let standard_class = runtime.class(&mut ctx, "STANDARD-CLASS").unwrap();
    for name in classes {
        let class = runtime.class(&mut ctx, name).unwrap();
        let metaclass = runtime.call_builtin(&mut ctx, class_of, &[class]).unwrap();
        assert!(
            metaclass == built_in_class || metaclass == standard_class,
            "{name} was not replaced by a class metaobject"
        );
    }
}

#[test]
fn class_of_and_typep_recognize_file_streams() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_types::register(&runtime).unwrap();
    ncl_types::builtins::register(&runtime).unwrap();
    ncl_conditions::register(&runtime).unwrap();
    ncl_clos::register(&runtime).unwrap();
    ncl_lib_streams::register(&runtime).unwrap();

    let open = function(&runtime, &mut ctx, "OPEN");
    let close = function(&runtime, &mut ctx, "CLOSE");
    let class_of = function(&runtime, &mut ctx, "CLASS-OF");
    let typep = function(&runtime, &mut ctx, "TYPEP");
    let subtypep = function(&runtime, &mut ctx, "SUBTYPEP");
    let keyword = runtime.find_package(&ctx, "KEYWORD").unwrap();
    let direction = Package::from_word(keyword)
        .intern(&mut ctx, &runtime, "DIRECTION")
        .unwrap()
        .0;
    let output = Package::from_word(keyword)
        .intern(&mut ctx, &runtime, "OUTPUT")
        .unwrap()
        .0;
    let path = std::env::temp_dir()
        .join(format!("ncl-clos-file-stream-{}", std::process::id()))
        .to_string_lossy()
        .into_owned();
    let path_word =
        ncl_object::make_string(&mut ctx, &runtime, &path.chars().collect::<Vec<_>>()).unwrap();
    let stream = runtime
        .call_builtin(&mut ctx, open, &[path_word, direction, output])
        .unwrap();
    let file_stream_class = runtime.class(&mut ctx, "FILE-STREAM").unwrap();
    let stream_class = runtime.class(&mut ctx, "STREAM").unwrap();
    let terminal_io_symbol = intern(&runtime, &mut ctx, "COMMON-LISP", "*TERMINAL-IO*");
    let terminal_io = ncl_object::symbol_value(&ctx, terminal_io_symbol).unwrap();
    let two_way_stream_class = runtime.class(&mut ctx, "TWO-WAY-STREAM").unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, class_of, &[terminal_io]),
        Ok(two_way_stream_class)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, class_of, &[stream]),
        Ok(file_stream_class)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, typep, &[stream, file_stream_class]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, subtypep, &[file_stream_class, stream_class]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, close, &[stream]),
        Ok(Word::TRUE)
    );
    std::fs::remove_file(path).unwrap();
}

#[test]
fn typep_class_objects_compound_specs_and_nil_empty_type_agree() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_types::register(&runtime).unwrap();
    ncl_conditions::register(&runtime).unwrap();
    ncl_clos::register(&runtime).unwrap();

    let typep = function(&runtime, &mut ctx, "TYPEP");
    let integer = runtime.class(&mut ctx, "INTEGER").unwrap();
    let null = runtime.class(&mut ctx, "NULL").unwrap();
    let t_class = runtime.class(&mut ctx, "T").unwrap();
    let built_in_class = intern(&runtime, &mut ctx, "COMMON-LISP", "BUILT-IN-CLASS");
    let integer_name = intern(&runtime, &mut ctx, "COMMON-LISP", "INTEGER");
    let null_name = intern(&runtime, &mut ctx, "COMMON-LISP", "NULL");
    let and = intern(&runtime, &mut ctx, "COMMON-LISP", "AND");
    let or = intern(&runtime, &mut ctx, "COMMON-LISP", "OR");
    let and_type = list(&mut ctx, &runtime, &[and, integer, t_class]);
    let or_type = list(&mut ctx, &runtime, &[or, null, integer]);

    assert_eq!(
        runtime.call_builtin(&mut ctx, typep, &[integer, built_in_class]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, typep, &[Word::fixnum(7), integer_name]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, typep, &[Word::fixnum(7), and_type]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, typep, &[Word::fixnum(7), or_type]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, typep, &[Word::NIL, Word::NIL]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, typep, &[Word::NIL, null_name]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, typep, &[Word::NIL, null]),
        Ok(Word::TRUE)
    );

    let arbitrary_vector =
        ncl_object::make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1)]).unwrap();
    let simple_vector = runtime.class(&mut ctx, "SIMPLE-VECTOR").unwrap();
    let class_of = function(&runtime, &mut ctx, "CLASS-OF");
    assert_eq!(
        runtime.call_builtin(&mut ctx, class_of, &[arbitrary_vector]),
        Ok(simple_vector)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, typep, &[Word::fixnum(7), arbitrary_vector]),
        Ok(Word::NIL)
    );
}

#[test]
fn subtypep_uses_registered_class_descriptors_and_keeps_nil_empty() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_types::register(&runtime).unwrap();
    ncl_types::builtins::register(&runtime).unwrap();
    ncl_conditions::register(&runtime).unwrap();
    ncl_clos::register(&runtime).unwrap();

    let subtypep = function(&runtime, &mut ctx, "SUBTYPEP");
    let integer = runtime.class(&mut ctx, "INTEGER").unwrap();
    let number = runtime.class(&mut ctx, "NUMBER").unwrap();
    let error = runtime.class(&mut ctx, "ERROR").unwrap();
    let condition = runtime.class(&mut ctx, "CONDITION").unwrap();
    let null = runtime.class(&mut ctx, "NULL").unwrap();
    let symbol = runtime.class(&mut ctx, "SYMBOL").unwrap();

    assert_eq!(
        runtime.call_builtin(&mut ctx, subtypep, &[integer, number]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, subtypep, &[error, condition]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, subtypep, &[null, symbol]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, subtypep, &[Word::NIL, null]),
        Ok(Word::TRUE)
    );

    let or = intern(&runtime, &mut ctx, "COMMON-LISP", "OR");
    let compound = list(&mut ctx, &runtime, &[or, integer, null]);
    let t_class = runtime.class(&mut ctx, "T").unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, subtypep, &[compound, t_class]),
        Ok(Word::TRUE)
    );
}

#[test]
fn slot_builtins_round_trip_and_reject_non_instances() {
    let (runtime, mut ctx) = setup();
    let class = runtime.class(&mut ctx, "STANDARD-OBJECT").unwrap();
    let instance = ncl_clos::make_instance(&mut ctx, &runtime, class, &[Word::UNBOUND]).unwrap();
    let slot_value = function(&runtime, &mut ctx, "SLOT-VALUE");
    let slot_set = function(&runtime, &mut ctx, "SLOT-VALUE-SET");
    assert_eq!(
        runtime
            .builtin_descriptor(&ctx, slot_set)
            .unwrap()
            .convention,
        BuiltinConvention::Direct(ncl_object::Arity::exact(3))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, slot_set, &[instance, Word::fixnum(0), Word::TRUE]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, slot_value, &[instance, Word::fixnum(0)]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, slot_value, &[Word::NIL, Word::fixnum(0)]),
        Err(ncl_object::ObjectError::TypeError)
    );
}

#[test]
fn slot_exists_p_recognizes_direct_and_inherited_slots() {
    let (runtime, mut ctx) = setup();
    let inherited_name = Word::fixnum(17);
    let direct_name = Word::fixnum(23);
    let inherited_slot =
        ncl_object::make_simple_vector(&mut ctx, &runtime, &[inherited_name, Word::fixnum(0)])
            .unwrap();
    let parent_slots =
        ncl_object::make_simple_vector(&mut ctx, &runtime, &[inherited_slot]).unwrap();
    let parent = ncl_clos::make_class(
        &mut ctx,
        &runtime,
        Word::NIL,
        Word::NIL,
        parent_slots,
        Word::fixnum(0),
    )
    .unwrap();
    let child_slots = ncl_object::make_simple_vector(&mut ctx, &runtime, &[direct_name]).unwrap();
    let child = ncl_clos::make_class(
        &mut ctx,
        &runtime,
        Word::NIL,
        parent,
        child_slots,
        Word::fixnum(0),
    )
    .unwrap();
    let instance = ncl_clos::make_instance(&mut ctx, &runtime, child, &[]).unwrap();
    let slot_exists = function(&runtime, &mut ctx, "SLOT-EXISTS-P");

    assert_eq!(
        runtime.call_builtin(&mut ctx, slot_exists, &[instance, inherited_name]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, slot_exists, &[instance, direct_name]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, slot_exists, &[instance, Word::fixnum(99)]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, slot_exists, &[Word::NIL, inherited_name]),
        Err(ncl_object::ObjectError::TypeError)
    );
}

#[test]
fn production_table_contains_only_bound_builtins() {
    let (runtime, mut ctx) = setup();
    let production = ncl_clos::production_function_names();
    assert!(!production.is_empty());
    for identifier in production {
        let word = runtime
            .function(
                &mut ctx,
                identifier.package.as_str(),
                identifier.name.as_str(),
            )
            .unwrap_or_else(|| panic!("production builtin is not registered: {identifier:?}"));
        let function = FunctionObject::try_from(word).unwrap_or_else(|_| {
            panic!("production builtin is not a FunctionObject: {identifier:?}")
        });
        assert!(
            runtime.builtin_descriptor(&ctx, function).is_some(),
            "production builtin has no descriptor: {identifier:?}"
        );
    }
}
