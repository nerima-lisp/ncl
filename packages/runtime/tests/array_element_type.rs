#![allow(missing_docs)]

use ncl_runtime::Runtime;

#[test]
fn make_array_canonical_nil_element_type_runs_through_reader_and_compiler() {
    let mut runtime =
        Runtime::new().unwrap_or_else(|error| panic!("runtime initialization failed: {error}"));
    let element_type = runtime
        .eval("(array-element-type (make-array '(0) :element-type nil))")
        .unwrap_or_else(|error| panic!("MAKE-ARRAY with NIL element type failed: {error}"));

    assert_eq!(runtime.format_result(element_type), "T");
}

#[test]
fn class_type_specifiers_upgrade_and_match_through_eval_and_compilation() {
    for compile in [false, true] {
        let mut runtime =
            Runtime::new().unwrap_or_else(|error| panic!("runtime initialization failed: {error}"));
        let source = "(progn
            (defclass ncl-array-class-type-test () ())
            (defclass ncl-array-unrelated-class-type-test () ())
            (let* ((class (find-class 'ncl-array-class-type-test)))
              (list
                (upgraded-array-element-type class)
                (upgraded-array-element-type 'ncl-array-class-type-test)
                (upgraded-array-element-type '(or ncl-array-class-type-test symbol))
                (array-element-type (make-array 0 :element-type class))
                (array-element-type (make-array 0 :element-type 'ncl-array-class-type-test))
                (array-element-type (make-array 0 :element-type '(or ncl-array-class-type-test symbol))))))";
        let result = if compile {
            runtime.compile(source)
        } else {
            runtime.eval(source)
        }
        .unwrap_or_else(|error| panic!("class type specifier failed: {error}"));

        assert_eq!(
            runtime.format_result(result),
            "(T T T T T T)",
            "compile={compile}"
        );

        assert_subtypep_cases(&mut runtime, compile);
    }
}

fn assert_subtypep_cases(runtime: &mut Runtime, compile: bool) {
    let subtypep_cases = [
        (
            "class object/object equality",
            "(subtypep (find-class 'ncl-array-class-type-test) (find-class 'ncl-array-class-type-test))",
            "T",
            "(nth-value 1 (subtypep (find-class 'ncl-array-class-type-test) (find-class 'ncl-array-class-type-test)))",
            "T",
        ),
        (
            "class object/name equality",
            "(subtypep (find-class 'ncl-array-class-type-test) 'ncl-array-class-type-test)",
            "T",
            "(nth-value 1 (subtypep (find-class 'ncl-array-class-type-test) 'ncl-array-class-type-test))",
            "T",
        ),
        (
            "class name/object equality",
            "(subtypep 'ncl-array-class-type-test (find-class 'ncl-array-class-type-test))",
            "T",
            "(nth-value 1 (subtypep 'ncl-array-class-type-test (find-class 'ncl-array-class-type-test)))",
            "T",
        ),
        (
            "class name/name equality",
            "(subtypep 'ncl-array-class-type-test 'ncl-array-class-type-test)",
            "T",
            "(nth-value 1 (subtypep 'ncl-array-class-type-test 'ncl-array-class-type-test))",
            "T",
        ),
        (
            "class object/superclass relation",
            "(subtypep (find-class 'ncl-array-class-type-test) 'standard-object)",
            "T",
            "(nth-value 1 (subtypep (find-class 'ncl-array-class-type-test) 'standard-object))",
            "T",
        ),
        (
            "class name/superclass relation",
            "(subtypep 'ncl-array-class-type-test 'standard-object)",
            "T",
            "(nth-value 1 (subtypep 'ncl-array-class-type-test 'standard-object))",
            "T",
        ),
        (
            "built-in equality control",
            "(subtypep 'integer 'integer)",
            "T",
            "(nth-value 1 (subtypep 'integer 'integer))",
            "T",
        ),
        (
            "unrelated classes",
            "(subtypep (find-class 'ncl-array-class-type-test) 'ncl-array-unrelated-class-type-test)",
            "NIL",
            "(nth-value 1 (subtypep (find-class 'ncl-array-class-type-test) 'ncl-array-unrelated-class-type-test))",
            "T",
        ),
        (
            "unknown class type",
            "(subtypep 'ncl-array-unknown-class-type-test 'integer)",
            "NIL",
            "(nth-value 1 (subtypep 'ncl-array-unknown-class-type-test 'integer))",
            "NIL",
        ),
    ];
    for (description, primary_form, primary_expected, certainty_form, certainty_expected) in
        subtypep_cases
    {
        for (form, expected) in [
            (primary_form, primary_expected),
            (certainty_form, certainty_expected),
        ] {
            let result = if compile {
                runtime.compile(form)
            } else {
                runtime.eval(form)
            }
            .unwrap_or_else(|error| panic!("SUBTYPEP {description} failed: {error}"));
            assert_eq!(
                runtime.format_result(result),
                expected,
                "compile={compile}, {description}: {form}"
            );
        }
    }
}

#[test]
fn typep_class_name_resolution_remains_a_regression_through_eval_and_compilation() {
    for compile in [false, true] {
        let mut runtime =
            Runtime::new().unwrap_or_else(|error| panic!("runtime initialization failed: {error}"));
        let source = "(progn
            (defclass ncl-typep-class-name-regression-test () ())
            (typep
              (make-instance 'ncl-typep-class-name-regression-test)
              'ncl-typep-class-name-regression-test))";
        let result = if compile {
            runtime.compile(source)
        } else {
            runtime.eval(source)
        }
        .unwrap_or_else(|error| panic!("existing TYPEP class-name behavior failed: {error}"));

        assert_eq!(runtime.format_result(result), "T", "compile={compile}");
    }
}

#[test]
fn runtime_class_lookup_only_applies_to_atomic_type_specifiers() {
    for compile in [false, true] {
        let mut runtime =
            Runtime::new().unwrap_or_else(|error| panic!("runtime initialization failed: {error}"));
        let source = "(progn
            (defclass ncl-array-satisfies-predicate-test () ())
            (defclass member () ())
            (list
              (upgraded-array-element-type 'ncl-array-satisfies-predicate-test)
              (upgraded-array-element-type '(satisfies ncl-array-satisfies-predicate-test))
              (upgraded-array-element-type 'member)
              (upgraded-array-element-type '(member 42))))";
        let result = if compile {
            runtime.compile(source)
        } else {
            runtime.eval(source)
        }
        .unwrap_or_else(|error| panic!("compound type specifier failed: {error}"));

        assert_eq!(
            runtime.format_result(result),
            "(T T T T)",
            "compile={compile}"
        );
    }
}

#[test]
fn make_array_rejects_an_undefined_element_type() {
    for compile in [false, true] {
        let mut runtime =
            Runtime::new().unwrap_or_else(|error| panic!("runtime initialization failed: {error}"));
        let source = "(make-array 1 :element-type 'undefined-type)";
        let result = if compile {
            runtime.compile(source)
        } else {
            runtime.eval(source)
        };

        assert!(
            matches!(
                result,
                Err(ncl_runtime::RuntimeError::Object(
                    ncl_object::ObjectError::TypeError
                ))
            ),
            "MAKE-ARRAY should signal a type error, compile={compile}: {result:?}"
        );
    }
}

#[test]
fn upgraded_array_element_type_handles_non_class_designators() {
    for compile in [false, true] {
        let mut runtime =
            Runtime::new().unwrap_or_else(|error| panic!("runtime initialization failed: {error}"));
        let source = "(list
            (upgraded-array-element-type 'fixnum)
            (upgraded-array-element-type '(unsigned-byte 8))
            (upgraded-array-element-type 'character))";
        let result = if compile {
            runtime.compile(source)
        } else {
            runtime.eval(source)
        }
        .unwrap_or_else(|error| panic!("UPGRADED-ARRAY-ELEMENT-TYPE failed: {error}"));

        assert_eq!(
            runtime.format_result(result),
            "(FIXNUM UNSIGNED-BYTE CHARACTER)",
            "compile={compile}"
        );
    }
}

#[test]
fn registered_class_specifiers_parse_in_nested_type_positions() {
    for compile in [false, true] {
        let mut runtime =
            Runtime::new().unwrap_or_else(|error| panic!("runtime initialization failed: {error}"));
        let source = "(progn
            (defclass ncl-array-type-position-test () ())
            (let ((class (find-class 'ncl-array-type-position-test)))
              (list
                (subtypep '(and ncl-array-type-position-test t) (list 'and class t))
                (subtypep '(or ncl-array-type-position-test null) (list 'or class 'null))
                (subtypep '(not ncl-array-type-position-test) (list 'not class))
                (subtypep '(cons ncl-array-type-position-test null) (list 'cons class 'null))
                (subtypep '(values ncl-array-type-position-test) (list 'values class))
                (subtypep '(array ncl-array-type-position-test *) (list 'array class '*))
                (subtypep '(simple-array ncl-array-type-position-test (*))
                          (list 'simple-array class '(*)))
                (subtypep '(vector ncl-array-type-position-test 1) (list 'vector class 1))
                (subtypep '(function (ncl-array-type-position-test)
                                      ncl-array-type-position-test)
                          (list 'function (list class) class)))))";
        let result = if compile {
            runtime.compile(source)
        } else {
            runtime.eval(source)
        }
        .unwrap_or_else(|error| panic!("nested class type specifiers failed: {error}"));

        assert_eq!(
            runtime.format_result(result),
            "(T T T T T T T T T)",
            "compile={compile}"
        );
    }
}
