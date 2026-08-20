use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use ncl_compiler::Compiler;
use ncl_syntax::{Form, OrdinaryLambdaList, Span};

use crate::environment::Environment;
use crate::error::RuntimeError;

use super::{
    ClassDefinition, Function, MacroLambdaList, MethodDefinition, StructureRepresentation, Value,
};

fn assert_display_cases(cases: impl IntoIterator<Item = (&'static str, Value, &'static str)>) {
    for (name, value, expected) in cases {
        assert_eq!(value.to_string(), expected, "display case: {name}");
        assert_eq!(
            format!("{:?}", value),
            format!("Value({expected})"),
            "debug case: {name}"
        );
    }
}

fn empty_ordinary_lambda_list() -> OrdinaryLambdaList {
    OrdinaryLambdaList {
        required: Vec::new(),
        required_escaped: Vec::new(),
        optional: Vec::new(),
        rest: None,
        rest_escaped: false,
        keywords: Vec::new(),
        has_keyword_section: false,
        allow_other_keys: false,
        auxiliary: Vec::new(),
    }
}

fn empty_macro_lambda_list() -> MacroLambdaList {
    MacroLambdaList {
        whole: None,
        environment: None,
        required: Vec::new(),
        optional: Vec::new(),
        rest: None,
        keywords: Vec::new(),
        has_keyword_section: false,
        allow_other_keys: false,
        auxiliary: Vec::new(),
    }
}

fn display_cases() -> Vec<(&'static str, Value, &'static str)> {
    let form = Form::atom("NIL", Span::new(0, 3));
    let macro_lambda_list = empty_macro_lambda_list();
    let compiled_program = Rc::new(Compiler::compile_form(&form).expect("NIL compiles"));

    vec![
        ("nil", Value::Nil, "NIL"),
        ("unbound", Value::Unbound, "#<UNBOUND>"),
        ("true", Value::Boolean(true), "T"),
        ("false", Value::Boolean(false), "NIL"),
        ("integer", Value::Integer(-42), "-42"),
        (
            "rational",
            Value::rational(3, 2).expect("valid rational"),
            "3/2",
        ),
        ("integral float", Value::Float(2.0), "2.0"),
        ("fractional float", Value::Float(2.5), "2.5"),
        (
            "complex",
            Value::complex(Value::Integer(1), Value::Float(2.5)),
            "#C(1 2.5)",
        ),
        ("string", Value::string("line\n"), r#""line\n""#),
        ("character space", Value::Character(' '), r"#\SPACE"),
        ("character newline", Value::Character('\n'), r"#\NEWLINE"),
        ("character tab", Value::Character('\t'), r"#\TAB"),
        ("character return", Value::Character('\r'), r"#\RETURN"),
        ("character", Value::Character('A'), r"#\A"),
        (
            "string input stream",
            Value::string_input_stream("input", 0, 5),
            "#<STRING-INPUT-STREAM>",
        ),
        (
            "string output stream",
            Value::string_output_stream(),
            "#<STRING-OUTPUT-STREAM>",
        ),
        (
            "file input stream",
            Value::file_input_stream("input".to_string()),
            "#<FILE-INPUT-STREAM>",
        ),
        (
            "file output stream",
            Value::file_output_stream(PathBuf::from("output"), String::new()),
            "#<FILE-OUTPUT-STREAM>",
        ),
        (
            "file io stream",
            Value::file_io_stream(PathBuf::from("io"), String::new(), false),
            "#<FILE-IO-STREAM>",
        ),
        (
            "package",
            Value::package("COMMON-LISP"),
            "#<PACKAGE \"COMMON-LISP\">",
        ),
        (
            "environment",
            Value::environment(Environment::new()),
            "#<ENVIRONMENT>",
        ),
        ("symbol", Value::symbol("name"), "NAME"),
        ("exact symbol", Value::symbol_exact(r"a|b\c"), r"|a\|b\\c|"),
        (
            "uninterned symbol",
            Value::uninterned_symbol("temp"),
            "#:temp",
        ),
        ("keyword", Value::keyword(":name"), ":NAME"),
        (
            "exact keyword",
            Value::keyword_exact("key name"),
            ":|key name|",
        ),
        (
            "list",
            Value::list(vec![Value::Integer(1), Value::string("x")]),
            r#"(1 "x")"#,
        ),
        (
            "empty dotted list",
            Value::dotted_list(Vec::new(), Value::Integer(1)),
            "(. 1)",
        ),
        (
            "dotted list",
            Value::dotted_list(vec![Value::Integer(1)], Value::string("tail")),
            r#"(1 . "tail")"#,
        ),
        (
            "vector",
            Value::vector(vec![Value::Integer(1), Value::string("x")]),
            r#"#(1 "x")"#,
        ),
        (
            "array",
            Value::array(vec![2, 1], vec![Value::Integer(1), Value::Integer(2)]),
            "#<ARRAY [2, 1]>",
        ),
        ("hash table", Value::hash_table("eql"), "#<HASH-TABLE eql>"),
        ("method", method(1), "#<METHOD>"),
        ("empty values", Value::values(Vec::new()), "#<VALUES>"),
        (
            "multiple values",
            Value::values(vec![Value::Integer(1), Value::string("x")]),
            r#"#<VALUES 1 "x">"#,
        ),
        (
            "condition",
            Value::condition_from_parts(
                "ERROR".to_string(),
                "message".to_string(),
                None,
                Vec::new(),
            ),
            "#<CONDITION message>",
        ),
        ("restart", Value::restart("ABORT"), "#<RESTART ABORT>"),
        (
            "structure",
            Value::structure_with_types(
                "POINT",
                vec![("X".to_string(), Value::Integer(1))],
                Vec::new(),
            ),
            "#S(POINT :X 1)",
        ),
        (
            "class",
            Value::class_object(class("POINT")),
            "#<CLASS POINT>",
        ),
        (
            "instance",
            Value::instance(class("POINT"), Vec::new()),
            "#<POINT INSTANCE>",
        ),
        (
            "builtin",
            Value::builtin("TEST-BUILTIN", test_builtin),
            "#<BUILTIN TEST-BUILTIN>",
        ),
        (
            "primitive",
            Value::primitive("TEST-PRIMITIVE"),
            "#<PRIMITIVE TEST-PRIMITIVE>",
        ),
        (
            "structure constructor",
            Value::Function(Rc::new(Function::StructureConstructor {
                name: "MAKE-POINT".to_string(),
                slots: Vec::new(),
                structure_types: vec!["POINT".to_string()],
                representation: StructureRepresentation::Record,
                constructor_lambda_list: None,
                environment: Environment::new(),
            })),
            "#<STRUCTURE-CONSTRUCTOR MAKE-POINT>",
        ),
        (
            "structure predicate",
            Value::Function(Rc::new(Function::StructurePredicate {
                name: "POINT-P".to_string(),
            })),
            "#<STRUCTURE-PREDICATE POINT-P>",
        ),
        (
            "structure accessor",
            Value::Function(Rc::new(Function::StructureAccessor {
                structure_name: "POINT".to_string(),
                slot_name: "X".to_string(),
                slot_index: 0,
                read_only: false,
            })),
            "#<STRUCTURE-ACCESSOR POINT-X>",
        ),
        (
            "structure copier",
            Value::Function(Rc::new(Function::StructureCopier {
                name: "COPY-POINT".to_string(),
            })),
            "#<STRUCTURE-COPIER COPY-POINT>",
        ),
        (
            "generic",
            Value::generic("PRINT-OBJECT", empty_ordinary_lambda_list()),
            "#<GENERIC-FUNCTION PRINT-OBJECT>",
        ),
        (
            "slot reader",
            Value::slot_reader("POINT", "X"),
            "#<SLOT-READER POINT-X>",
        ),
        (
            "slot writer",
            Value::slot_writer("POINT", "X"),
            "#<SLOT-WRITER POINT-X>",
        ),
        (
            "condition reader",
            Value::condition_reader("ERROR", "MESSAGE"),
            "#<CONDITION-READER ERROR-MESSAGE>",
        ),
        (
            "condition writer",
            Value::condition_writer("ERROR", "MESSAGE"),
            "#<CONDITION-WRITER ERROR-MESSAGE>",
        ),
        (
            "closure",
            Value::closure(vec!["X".to_string()], Vec::new(), Environment::new()),
            "#<FUNCTION>",
        ),
        (
            "macro",
            Value::macro_function(macro_lambda_list.clone(), Vec::new(), Environment::new()),
            "#<MACRO>",
        ),
        (
            "long defsetf",
            Value::long_defsetf_function(
                macro_lambda_list.clone(),
                "STORE".to_string(),
                Vec::new(),
                Environment::new(),
            ),
            "#<MACRO>",
        ),
        (
            "modify macro",
            Value::modify_macro_function(macro_lambda_list, form.clone(), Environment::new()),
            "#<MACRO>",
        ),
        (
            "compiled",
            Value::compiled(compiled_program, 0, Environment::new()),
            "#<FUNCTION>",
        ),
    ]
}

fn test_builtin(_: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Nil)
}

fn class(name: &str) -> Rc<ClassDefinition> {
    Rc::new(ClassDefinition {
        name: name.to_string(),
        precedence: Vec::new(),
        slots: Vec::new(),
        default_initargs: Vec::new(),
        documentation: Rc::new(RefCell::new(None)),
    })
}

fn method(id: u64) -> Value {
    Value::Method(Rc::new(MethodDefinition {
        id,
        generic_function: "TEST-GENERIC".to_string(),
        lambda_list: Value::Nil,
        qualifiers: Vec::new(),
        specializers: Vec::new(),
        function: Value::builtin("TEST-BUILTIN", test_builtin),
    }))
}

#[test]
fn value_display_covers_all_variants() {
    assert_display_cases(display_cases());
}
