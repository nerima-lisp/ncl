#![allow(missing_docs)]

use std::process::{Command, Output, Stdio};

use ncl_lib_streams::registration::REGISTERED_BUILTINS;

struct Case {
    builtin: &'static str,
    source: &'static str,
    expected: &'static str,
}

const ANCILLARY_BUILTINS: &[&str] = &[
    "WITH-OUTPUT-TO-STRING",
    "WITH-INPUT-FROM-STRING",
    "PRINC",
    "PRIN1",
    "PRINT",
    "FORMAT",
];

const CASES: &[Case] = &[
    Case {
        builtin: "OPEN",
        source: "(let ((s (open \"/dev/null\" :direction :input))) (prog1 (open-stream-p s) (close s)))",
        expected: "T",
    },
    Case {
        builtin: "STREAMP",
        source: "(streamp *standard-output*)",
        expected: "T",
    },
    Case {
        builtin: "INPUT-STREAM-P",
        source: "(input-stream-p *standard-input*)",
        expected: "T",
    },
    Case {
        builtin: "OUTPUT-STREAM-P",
        source: "(output-stream-p *standard-output*)",
        expected: "T",
    },
    Case {
        builtin: "OPEN-STREAM-P",
        source: "(open-stream-p *standard-output*)",
        expected: "T",
    },
    Case {
        builtin: "INTERACTIVE-STREAM-P",
        source: "(interactive-stream-p *standard-output*)",
        expected: "T",
    },
    Case {
        builtin: "STREAM-ELEMENT-TYPE",
        source: "(eq (stream-element-type *standard-output*) 'character)",
        expected: "T",
    },
    Case {
        builtin: "STREAM-EXTERNAL-FORMAT",
        source: "(stream-external-format *standard-output*)",
        expected: "NIL",
    },
    Case {
        builtin: "FILE-POSITION",
        source: "(file-position *standard-input*)",
        expected: "0",
    },
    Case {
        builtin: "FILE-LENGTH",
        source: "(let ((s (open \"/dev/null\" :direction :input))) (prog1 (file-length s) (close s)))",
        expected: "0",
    },
    Case {
        builtin: "FILE-STRING-LENGTH",
        source: "(file-string-length *standard-output* \"abc\")",
        expected: "3",
    },
    Case {
        builtin: "READ-CHAR",
        source: "(read-char (make-string-input-stream \"a\"))",
        expected: "#\\a",
    },
    Case {
        builtin: "READ-CHAR-NO-HANG",
        source: "(read-char-no-hang (make-string-input-stream \"a\"))",
        expected: "#\\a",
    },
    Case {
        builtin: "READ-CHAR-NO-HANG",
        source: "(read-char-no-hang (make-string-input-stream \"\") nil :eof)",
        expected: ":EOF",
    },
    Case {
        builtin: "PEEK-CHAR",
        source: "(peek-char t (make-string-input-stream \" a\") nil (code-char 33))",
        expected: "#\\a",
    },
    Case {
        builtin: "PEEK-CHAR",
        source: "(peek-char (code-char 90) (make-string-input-stream \"aZ\") nil (code-char 33) nil)",
        expected: "#\\Z",
    },
    Case {
        builtin: "PEEK-CHAR",
        source: "(peek-char nil (make-string-input-stream \"\") nil (code-char 33))",
        expected: "#\\!",
    },
    Case {
        builtin: "PEEK-CHAR",
        source: "(let ((s (make-string-input-stream \" a\"))) (list (peek-char t s) (read-char s)))",
        expected: "(#\\a #\\a)",
    },
    Case {
        builtin: "READ-LINE",
        source: "(multiple-value-list (read-line (make-string-input-stream \"abc\")))",
        expected: "(\"abc\" T)",
    },
    Case {
        builtin: "READ-LINE",
        source: "(read-line (make-string-input-stream \"\") nil :eof)",
        expected: ":EOF",
    },
    Case {
        builtin: "READ-BYTE",
        source: "(let ((s (open \"/dev/null\" :direction :input))) (prog1 (read-byte s nil :eof) (close s)))",
        expected: ":EOF",
    },
    Case {
        builtin: "UNREAD-CHAR",
        source: "(let ((s (make-string-input-stream \"a\"))) (read-char s) (unread-char (code-char 97) s) (read-char s))",
        expected: "#\\a",
    },
    Case {
        builtin: "WRITE-CHAR",
        source: "(let ((s (make-string-output-stream))) (write-char (code-char 65) s) (get-output-stream-string s))",
        expected: "\"A\"",
    },
    Case {
        builtin: "WRITE-BYTE",
        source: "(let ((s (open \"/dev/null\" :direction :output))) (prog1 (write-byte 65 s) (close s)))",
        expected: "65",
    },
    Case {
        builtin: "WRITE-STRING",
        source: "(let ((s (make-string-output-stream))) (write-string \"abc\" s) (get-output-stream-string s))",
        expected: "\"abc\"",
    },
    Case {
        builtin: "WRITE-STRING",
        source: "(let ((s (make-string-output-stream))) (write-string \"abc\" s :start 1 :end 2) (get-output-stream-string s))",
        expected: "\"b\"",
    },
    Case {
        builtin: "WRITE-LINE",
        source: "(let ((s (make-string-output-stream))) (write-line \"abc\" s) (get-output-stream-string s))",
        expected: "\"abc\n\"",
    },
    Case {
        builtin: "WRITE-LINE",
        source: "(let ((s (make-string-output-stream))) (write-line \"abc\" s :start 1 :end 2) (get-output-stream-string s))",
        expected: "\"b\n\"",
    },
    Case {
        builtin: "TERPRI",
        source: "(let ((s (make-string-output-stream))) (terpri s) (get-output-stream-string s))",
        expected: "\"\n\"",
    },
    Case {
        builtin: "FINISH-OUTPUT",
        source: "(let ((s (make-string-output-stream))) (finish-output s))",
        expected: "NIL",
    },
    Case {
        builtin: "FRESH-LINE",
        source: "(let ((s (make-string-output-stream))) (fresh-line s))",
        expected: "NIL",
    },
    Case {
        builtin: "FRESH-LINE",
        source: "(let ((s (make-string-output-stream))) (write-char (code-char 120) s) (list (fresh-line s) (get-output-stream-string s)))",
        expected: "(T \"x\n\")",
    },
    Case {
        builtin: "MAKE-STRING-OUTPUT-STREAM",
        source: "(streamp (make-string-output-stream))",
        expected: "T",
    },
    Case {
        builtin: "MAKE-STRING-INPUT-STREAM",
        source: "(let ((s (make-string-input-stream \"abc\" :start 1 :end 2))) (read-char s))",
        expected: "#\\b",
    },
    Case {
        builtin: "GET-OUTPUT-STREAM-STRING",
        source: "(let ((s (make-string-output-stream))) (write-string \"\" s) (get-output-stream-string s))",
        expected: "\"\"",
    },
    Case {
        builtin: "CLOSE",
        source: "(let ((s (make-string-input-stream \"\"))) (close s))",
        expected: "T",
    },
    Case {
        builtin: "PRINC",
        source: "(princ \"x\")",
        expected: "x\"x\"",
    },
    Case {
        builtin: "PRIN1",
        source: "(prin1 \"x\")",
        expected: "\"x\"\"x\"",
    },
    Case {
        builtin: "PRINT",
        source: "(print \"x\")",
        expected: "\n\"x\"\n\"x\"",
    },
    Case {
        builtin: "FORMAT",
        source: "(format nil \"~a/~s/~d~%~&\" \"x\" \"y\" 12)",
        expected: "\"x/\\\"y\\\"/12\n\"",
    },
    Case {
        builtin: "WITH-OUTPUT-TO-STRING",
        source: "(with-output-to-string (s) (write-string \"x\" s))",
        expected: "\"x\"",
    },
    Case {
        builtin: "WITH-INPUT-FROM-STRING",
        source: "(with-input-from-string (s \"x\") (read-char s))",
        expected: "#\\x",
    },
    Case {
        builtin: "INPUT-STREAM-P",
        source: "(let ((s (make-string-input-stream \"x\"))) (list (input-stream-p s) (output-stream-p s)))",
        expected: "(T NIL)",
    },
];

fn run(source: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ncl"))
        .args(["--eval", source])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap_or_else(|error| panic!("{source}: {error}"))
}

#[test]
fn registered_stream_builtins_have_compiled_probes() {
    assert_eq!(REGISTERED_BUILTINS.len(), 28);
    assert_eq!(
        REGISTERED_BUILTINS
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        REGISTERED_BUILTINS.len()
    );
    for builtin in REGISTERED_BUILTINS {
        assert!(
            CASES.iter().any(|case| case.builtin == *builtin),
            "{builtin} has no compiled probe"
        );
    }
    for case in CASES {
        assert!(
            REGISTERED_BUILTINS.contains(&case.builtin)
                || ANCILLARY_BUILTINS.contains(&case.builtin),
            "unexpected probe: {}",
            case.builtin
        );
    }
    for case in CASES {
        let output = run(case.source);
        assert_eq!(output.status.code(), Some(0), "{}", case.builtin);
        assert!(
            output.stderr.is_empty(),
            "{}: {:?}",
            case.builtin,
            output.stderr
        );
        let expected = format!("{}\n", case.expected);
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            expected,
            "{}",
            case.builtin
        );
    }
}

#[test]
fn stream_character_edges_run_through_compiled_code() {
    let cases = [
        (
            "(read-char-no-hang (make-string-input-stream \"\") nil :eof)",
            ":EOF",
        ),
        (
            "(read-line (make-string-input-stream \"\") nil :eof)",
            ":EOF",
        ),
        (
            "(let ((s (make-string-output-stream))) (write-line \"abc\" s :start 1 :end 2) (get-output-stream-string s))",
            "\"b\n\"",
        ),
        (
            "(let ((s (make-string-output-stream))) (write-char (code-char 120) s) (list (fresh-line s) (get-output-stream-string s)))",
            "(T \"x\n\")",
        ),
        (
            "(let ((s (make-string-input-stream \"x\"))) (list (input-stream-p s) (output-stream-p s)))",
            "(T NIL)",
        ),
    ];
    for (source, expected) in cases {
        let output = run(source);
        assert_eq!(output.status.code(), Some(0), "{source}");
        assert!(output.stderr.is_empty(), "{source}: {:?}", output.stderr);
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            format!("{expected}\n")
        );
    }
}

#[test]
fn file_stream_round_trip_uses_a_temporary_file() {
    let path = std::env::temp_dir().join(format!("ncl-e2estream-{}", std::process::id()));
    std::fs::write(&path, "abc\n").unwrap_or_else(|error| panic!("{path:?}: {error}"));
    let path = path.to_string_lossy().into_owned();
    let source = format!(
        "(let ((s (open \"{path}\" :direction :input))) (list (open-stream-p s) (file-length s) (read-line s) (close s)))"
    );
    let output = run(&source);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "(T 4 \"abc\" T)\n");

    let source = format!(
        "(let ((s (open \"{path}\" :direction :io))) (list (file-position s) (file-length s) (read-byte s) (file-position s) (close s)))"
    );
    let output = run(&source);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "(0 4 97 1 T)\n");

    let source = format!(
        "(let ((s (open \"{path}\" :direction :output :if-exists :supersede))) (write-string \"z\" s) (close s))"
    );
    let output = run(&source);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "T\n");

    let source =
        format!("(let ((s (open \"{path}\" :direction :input))) (prog1 (read-line s) (close s)))");
    let output = run(&source);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "\"z\"\n");
    std::fs::remove_file(&path).unwrap_or_else(|error| panic!("{path}: {error}"));
}

#[test]
fn format_t_writes_to_standard_output() {
    let output = run("(format t \"hi~%\")");
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "hi\nNIL\n");
}
