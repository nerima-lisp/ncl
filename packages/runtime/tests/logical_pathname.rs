#![allow(missing_docs)]

use ncl_runtime::Runtime;

#[test]
fn logical_pathname_coerces_registered_namestring_and_preserves_identity() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let value = runtime
        .eval(
            r#"(progn
                 (setf (logical-pathname-translations "FIXTUREHOST") nil)
                 (let* ((path (logical-pathname "fixturehost:foo.txt.12"))
                        (again (logical-pathname path))
                        (wild (logical-pathname "fixturehost:**;foo*.txt.*"))
                        (name-omitted (logical-pathname "fixturehost:.txt"))
                        (newest (logical-pathname "fixturehost:")))
                   (and (typep path 'logical-pathname)
                        (typep path 'pathname)
                        (pathnamep path)
                        (eq path again)
                        (eq path (pathname path))
                        (= (pathname-version path) 12)
                        (string= (namestring path) "FIXTUREHOST:FOO.TXT.12")
                        (eq (pathname-version wild) :wild)
                        (string= (namestring wild) "FIXTUREHOST:**;FOO*.TXT.*")
                        (string= (namestring name-omitted) "FIXTUREHOST:.TXT")
                        (eq (pathname-version newest) :newest))))"#,
        )
        .unwrap_or_else(|error| panic!("logical pathname evaluation: {error}"));

    assert_eq!(runtime.format_result(value), "T");
}

#[test]
fn logical_pathname_rejects_hostless_unknown_and_unsuitable_pathspecs() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let value = runtime
        .eval(
            r#"(progn
                 (setf (logical-pathname-translations "FIXTUREHOST") nil)
                 (and
                   (handler-case
                     (progn (logical-pathname "FOO.TXT") nil)
                     (type-error () t))
                   (handler-case
                     (progn (logical-pathname "UNKNOWNHOST:FOO.TXT") nil)
                     (type-error (condition)
                       (and
                         (string= (type-error-datum condition) "UNKNOWNHOST:FOO.TXT")
                         (eq (type-error-expected-type condition) 'string))))
                   (handler-case
                     (progn (logical-pathname "FIXTUREHOST:FOO%.TXT") nil)
                     (type-error () t))
                   (handler-case
                     (progn (logical-pathname "FIXTUREHOST:FOO**.TXT") nil)
                     (type-error () t))
                   (handler-case
                     (progn (logical-pathname "FIXTUREHOST:DIR;;FOO.TXT") nil)
                     (type-error () t))
                   (handler-case
                     (progn (logical-pathname "FIXTUREHOST:FOO.") nil)
                     (type-error () t))
                   (handler-case
                     (progn (logical-pathname "FIXTUREHOST:FOO.TXT.0") nil)
                     (type-error () t))
                   (handler-case
                     (progn (logical-pathname 17) nil)
                     (type-error () t))
                   (handler-case
                     (progn (logical-pathname (pathname "physical.txt")) nil)
                     (type-error () t))))"#,
        )
        .unwrap_or_else(|error| panic!("logical pathname type-error evaluation: {error}"));

    assert_eq!(runtime.format_result(value), "T");
}

#[test]
fn logical_pathname_recognizes_subclasses() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let value = runtime
        .eval(
            r"(progn
                 (defclass fixture-logical-pathname-child (logical-pathname) ())
                 (let ((path (make-instance 'fixture-logical-pathname-child)))
                   (and (typep path 'logical-pathname)
                        (typep path 'pathname)
                        (handler-case
                          (and (pathnamep path)
                               (eq path (pathname path))
                               (eq path (logical-pathname path)))
                          (type-error () nil)))))",
        )
        .unwrap_or_else(|error| panic!("logical pathname subclass evaluation: {error}"));

    assert_eq!(runtime.format_result(value), "T");
}
