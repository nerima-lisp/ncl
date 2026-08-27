use super::evaluate;
use ncl_runtime::Runtime;

#[test]
fn compiled_format_accepts_boundary_directives_from_table() {
    let cases = [
        (r#"(format nil "~~")"#, "~"),
        (r#"(format nil "~~~%")"#, r"~\n"),
        (r#"(format nil "~A" nil)"#, "NIL"),
        (r#"(format nil "~S" nil)"#, "NIL"),
        (r#"(format nil "~R" 0)"#, "zero"),
        (r#"(format nil "~D" 0)"#, "0"),
        (r#"(format nil "~F" 0.0)"#, "0.0"),
        (r#"(format nil "~E" 0.0)"#, "0.0E+0"),
    ];

    for (source, expected) in cases {
        assert_eq!(
            evaluate(source).to_string(),
            format!("\"{expected}\""),
            "{source}"
        );
    }
}

#[test]
fn compiled_format_rejects_malformed_directives_from_table() {
    let cases = [
        r#"(format nil "~")"#,
        r#"(format nil "~Z")"#,
        r#"(format nil "~}")"#,
        r#"(format nil "~{~A")"#,
        r#"(format nil "~<~A")"#,
        r#"(format nil "~[~A")"#,
        r#"(format nil "~5,4,3,2,1<A>")"#,
    ];

    for source in cases {
        assert!(
            Runtime::new().eval_compiled_source(source).is_err(),
            "{source}"
        );
    }
}
