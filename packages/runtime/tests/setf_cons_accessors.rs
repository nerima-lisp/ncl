#![allow(missing_docs)]

use ncl_runtime::Runtime;

#[test]
fn setf_compound_cons_accessors_update_and_return_the_place_value() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let value = runtime
        .eval(
            "(let ((x (list 1 2 3 4))) \
               (list (setf (cadr x) 9) \
                     (setf (caddr x) 8) \
                     (setf (cdddr x) '(7)) \
                     x))",
        )
        .unwrap_or_else(|error| panic!("compound SETF accessors failed: {error}"));

    assert_eq!(runtime.format_result(value), "(9 8 (7) (1 9 8 7))");
}
