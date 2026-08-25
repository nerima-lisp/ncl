use ncl_runtime::Runtime;

#[test]
fn builtin_argument_errors_are_checked_through_one_table() {
    let cases = [
        ("(length 1)", "length type error"),
        ("(nth -1 '(a b))", "nth negative index"),
        ("(elt '(a) 2)", "elt bounds"),
        ("(getf '(a) :key)", "getf malformed property list"),
        (
            "(get-properties '(a) '(:key))",
            "get-properties malformed list",
        ),
        ("(typep 1 '(integer 2 3 4))", "typep malformed integer type"),
        ("(make-array '(2 -1))", "make-array negative dimension"),
        ("(aref #(1) 2)", "aref bounds"),
        ("(gethash 'key 1)", "gethash table type"),
        ("(setf (gethash 'key 1) 2)", "setf gethash table type"),
        ("(subseq '(a b) 2 1)", "subseq reversed bounds"),
        ("(fill '(a b) 1 :start 2)", "fill bounds"),
        ("(replace '(a b) '(c) :start1 3)", "replace bounds"),
        ("(coerce 1 'list)", "coerce unsupported type"),
        ("(read-from-string \"(\")", "read-from-string syntax"),
        ("(make-string-input-stream 1)", "input stream type"),
        ("(write-string 1)", "write-string type"),
        ("(close 1)", "close stream type"),
        (
            "(write-to-string 1 :unknown t)",
            "write-to-string unknown option",
        ),
        (
            "(read-from-string \"a\" nil nil :unknown t)",
            "read-from-string unknown option",
        ),
        (
            "(make-string-input-stream \"abc\" -1)",
            "input stream negative start",
        ),
        (
            "(read-char (make-string-output-stream))",
            "read-char output stream",
        ),
        (
            "(get-output-stream-string (make-string-input-stream \"\"))",
            "get-output-stream-string input stream",
        ),
        (
            "(unread-char #\\a (make-string-input-stream \"a\"))",
            "unread-char before read",
        ),
    ];

    for (source, name) in cases {
        assert!(
            Runtime::new().eval_source(source).is_err(),
            "{name}: {source}"
        );
    }
}
