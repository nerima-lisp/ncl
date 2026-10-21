#![allow(missing_docs)]

use ncl_runtime::Runtime;

#[test]
fn make_pathname_preserves_wild_inferiors_directory() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let value = runtime
        .eval(
            r"(equal (pathname-directory
                (make-pathname :directory '(:absolute :wild-inferiors)
                               :name :wild :type :wild))
               '(:absolute :wild-inferiors))",
        )
        .unwrap_or_else(|error| panic!("wild-inferiors pathname evaluation: {error}"));

    assert_eq!(runtime.format_result(value), "T");
}
