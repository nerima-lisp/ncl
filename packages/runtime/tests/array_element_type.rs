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
            (let* ((class (find-class 'ncl-array-class-type-test))
                   (instance (make-instance 'ncl-array-class-type-test)))
              (list
                (upgraded-array-element-type class)
                (upgraded-array-element-type 'ncl-array-class-type-test)
                (upgraded-array-element-type '(or ncl-array-class-type-test symbol))
                (array-element-type (make-array 0 :element-type class))
                (array-element-type (make-array 0 :element-type 'ncl-array-class-type-test))
                (array-element-type (make-array 0 :element-type '(or ncl-array-class-type-test symbol)))
                (typep instance class)
                (typep instance 'ncl-array-class-type-test))))";
        let result = if compile {
            runtime.compile(source)
        } else {
            runtime.eval(source)
        }
        .unwrap_or_else(|error| panic!("class type specifier failed: {error}"));

        assert_eq!(
            runtime.format_result(result),
            "(T T T T T T T T)",
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
