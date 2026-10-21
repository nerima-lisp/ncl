#![allow(missing_docs)]

use ncl_runtime::Runtime;

#[test]
fn pathname_version_round_trips_supported_components() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let value = runtime
        .eval(
            "(and \
               (eq (pathname-version (make-pathname :version :wild)) :wild) \
               (eq (pathname-version (make-pathname :version :unspecific)) :unspecific) \
               (eq (pathname-version (make-pathname :version :newest)) :newest) \
               (= (pathname-version (make-pathname :version 17)) 17) \
               (= (pathname-version \
                   (make-pathname :version 18446744073709551616)) \
                  18446744073709551616) \
               (null (pathname-version (make-pathname :version nil))) \
               (null (pathname-version (make-pathname))) \
               (eq (pathname-version \
                   (make-pathname :defaults (make-pathname :version :newest))) \
                   :newest) \
               (null (pathname-version \"file.txt\")))",
        )
        .unwrap_or_else(|error| panic!("pathname version evaluation: {error}"));

    assert_eq!(runtime.format_result(value), "T");
}

#[test]
fn pathname_version_accepts_zero() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let value = runtime
        .eval("(= (pathname-version (make-pathname :version 0)) 0)")
        .unwrap_or_else(|error| panic!("zero pathname version evaluation: {error}"));

    assert_eq!(runtime.format_result(value), "T");
}

#[test]
fn merge_pathnames_preserves_and_defaults_version_components() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let value = runtime
        .eval(
            r#"(and
                 (eq (pathname-version
                      (merge-pathnames (make-pathname :name "foo")
                                       (make-pathname)))
                     :newest)
                 (eq (pathname-version
                      (merge-pathnames (make-pathname :version :unspecific)
                                       (make-pathname :version :newest)))
                     :unspecific)
                 (eq (pathname-version
                      (merge-pathnames (make-pathname)
                                       (make-pathname :version :newest)))
                     :newest)
                 (eq (pathname-version
                      (merge-pathnames (make-pathname :name "foo" :version 0)
                                       (make-pathname :version :newest)))
                     0)
                 (eq (pathname-version
                      (merge-pathnames (make-pathname)
                                       (make-pathname :version :newest)
                                       nil))
                     :newest)
                 (null (pathname-version
                        (merge-pathnames (make-pathname :name "foo")
                                         (make-pathname :version :newest)
                                         nil))))"#,
        )
        .unwrap_or_else(|error| panic!("MERGE-PATHNAMES version evaluation: {error}"));

    assert_eq!(runtime.format_result(value), "T");
}
