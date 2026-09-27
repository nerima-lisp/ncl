#![allow(missing_docs)]
#![allow(clippy::needless_raw_string_hashes, clippy::unicode_not_nfc)]

use std::process::Command;

struct BuiltinEntry {
    builtin: &'static str,
    source: &'static str,
    expected: &'static str,
}

#[allow(clippy::too_many_lines)]
fn cases() -> Vec<BuiltinEntry> {
    vec![
        BuiltinEntry {
            builtin: "CHARACTERP",
            source: r#"(characterp #\A)"#,
            expected: "T",
        },
        BuiltinEntry {
            builtin: "CHARACTER",
            source: r#"(character "A")"#,
            expected: "#\\A",
        },
        BuiltinEntry {
            builtin: "CHAR",
            source: r#"(char "abc" 1)"#,
            expected: "#\\b",
        },
        BuiltinEntry {
            builtin: "SCHAR",
            source: r#"(schar "é" 1)"#,
            expected: "#\\́",
        },
        BuiltinEntry {
            builtin: "CHAR-CODE",
            source: r#"(char-code #\あ)"#,
            expected: "12354",
        },
        BuiltinEntry {
            builtin: "CHAR-NAME",
            source: r#"(char-name #\Space)"#,
            expected: "\"SPACE\"",
        },
        BuiltinEntry {
            builtin: "CODE-CHAR",
            source: r#"(code-char 12354)"#,
            expected: "#\\あ",
        },
        BuiltinEntry {
            builtin: "NAME-CHAR",
            source: r#"(name-char :tab)"#,
            expected: "#\\Tab",
        },
        BuiltinEntry {
            builtin: "DIGIT-CHAR",
            source: r#"(digit-char 10 16)"#,
            expected: "#\\A",
        },
        BuiltinEntry {
            builtin: "ALPHA-CHAR-P",
            source: r#"(alpha-char-p #\あ)"#,
            expected: "T",
        },
        BuiltinEntry {
            builtin: "ALPHANUMERICP",
            source: r#"(alphanumericp #\あ)"#,
            expected: "T",
        },
        BuiltinEntry {
            builtin: "UPPER-CASE-P",
            source: r#"(upper-case-p #\A)"#,
            expected: "T",
        },
        BuiltinEntry {
            builtin: "LOWER-CASE-P",
            source: r#"(lower-case-p #\a)"#,
            expected: "T",
        },
        BuiltinEntry {
            builtin: "BOTH-CASE-P",
            source: r#"(both-case-p #\A)"#,
            expected: "T",
        },
        BuiltinEntry {
            builtin: "DIGIT-CHAR-P",
            source: r#"(digit-char-p #\A 16)"#,
            expected: "10",
        },
        BuiltinEntry {
            builtin: "GRAPHIC-CHAR-P",
            source: r#"(graphic-char-p #\あ)"#,
            expected: "T",
        },
        BuiltinEntry {
            builtin: "STANDARD-CHAR-P",
            source: r#"(standard-char-p #\~)"#,
            expected: "T",
        },
        BuiltinEntry {
            builtin: "CHAR-UPCASE",
            source: r#"(char-upcase #\a)"#,
            expected: "#\\A",
        },
        BuiltinEntry {
            builtin: "CHAR-DOWNCASE",
            source: r#"(char-downcase #\A)"#,
            expected: "#\\a",
        },
        BuiltinEntry {
            builtin: "CHAR-INT",
            source: r#"(char-int #\あ)"#,
            expected: "12354",
        },
        BuiltinEntry {
            builtin: "CHAR=",
            source: r#"(char= #\a #\a)"#,
            expected: "T",
        },
        BuiltinEntry {
            builtin: "CHAR/=",
            source: r#"(char/= #\a #\A)"#,
            expected: "T",
        },
        BuiltinEntry {
            builtin: "CHAR<",
            source: r#"(char< #\あ #\い)"#,
            expected: "T",
        },
        BuiltinEntry {
            builtin: "CHAR>",
            source: r#"(char> #\b #\a)"#,
            expected: "T",
        },
        BuiltinEntry {
            builtin: "CHAR<=",
            source: r#"(char<= #\a #\a)"#,
            expected: "T",
        },
        BuiltinEntry {
            builtin: "CHAR>=",
            source: r#"(char>= #\a #\a)"#,
            expected: "T",
        },
        BuiltinEntry {
            builtin: "CHAR-EQUAL",
            source: r#"(char-equal #\a #\A)"#,
            expected: "T",
        },
        BuiltinEntry {
            builtin: "CHAR-NOT-EQUAL",
            source: r#"(char-not-equal #\a #\B)"#,
            expected: "T",
        },
        BuiltinEntry {
            builtin: "CHAR-LESSP",
            source: r#"(char-lessp #\a #\B)"#,
            expected: "T",
        },
        BuiltinEntry {
            builtin: "CHAR-GREATERP",
            source: r#"(char-greaterp #\B #\a)"#,
            expected: "T",
        },
        BuiltinEntry {
            builtin: "CHAR-NOT-GREATERP",
            source: r#"(char-not-greaterp #\a #\A)"#,
            expected: "T",
        },
        BuiltinEntry {
            builtin: "CHAR-NOT-LESSP",
            source: r#"(char-not-lessp #\a #\A)"#,
            expected: "T",
        },
        BuiltinEntry {
            builtin: "STRINGP",
            source: r#"(stringp "")"#,
            expected: "T",
        },
        BuiltinEntry {
            builtin: "STRING",
            source: r#"(string :foo)"#,
            expected: "\"FOO\"",
        },
        BuiltinEntry {
            builtin: "STRING=",
            source: r#"(string= :foo "FOO")"#,
            expected: "T",
        },
        BuiltinEntry {
            builtin: "STRING/=",
            source: r#"(string/= "abc" "abd")"#,
            expected: "2",
        },
        BuiltinEntry {
            builtin: "STRING<",
            source: r#"(string< "abc" "abd")"#,
            expected: "2",
        },
        BuiltinEntry {
            builtin: "STRING>",
            source: r#"(string> "abd" "abc")"#,
            expected: "2",
        },
        BuiltinEntry {
            builtin: "STRING<=",
            source: r#"(string<= "abc" "abc")"#,
            expected: "T",
        },
        BuiltinEntry {
            builtin: "STRING>=",
            source: r#"(string>= "abc" "abc")"#,
            expected: "T",
        },
        BuiltinEntry {
            builtin: "STRING-EQUAL",
            source: r#"(string-equal "Ab" "aB")"#,
            expected: "T",
        },
        BuiltinEntry {
            builtin: "STRING-NOT-EQUAL",
            source: r#"(string-not-equal "Ab" "aC")"#,
            expected: "1",
        },
        BuiltinEntry {
            builtin: "STRING-LESSP",
            source: r#"(string-lessp "abC" "abd")"#,
            expected: "2",
        },
        BuiltinEntry {
            builtin: "STRING-GREATERP",
            source: r#"(string-greaterp "abd" "abC")"#,
            expected: "2",
        },
        BuiltinEntry {
            builtin: "STRING-NOT-GREATERP",
            source: r#"(string-not-greaterp "abc" "abc")"#,
            expected: "T",
        },
        BuiltinEntry {
            builtin: "STRING-NOT-LESSP",
            source: r#"(string-not-lessp "abc" "abc")"#,
            expected: "T",
        },
        BuiltinEntry {
            builtin: "STRING-UPCASE",
            source: r#"(string-upcase "aB あ")"#,
            expected: "\"AB あ\"",
        },
        BuiltinEntry {
            builtin: "STRING-DOWNCASE",
            source: r#"(string-downcase "ABC")"#,
            expected: "\"abc\"",
        },
        BuiltinEntry {
            builtin: "STRING-CAPITALIZE",
            source: r#"(string-capitalize "aB cD")"#,
            expected: "\"Ab Cd\"",
        },
        BuiltinEntry {
            builtin: "NSTRING-UPCASE",
            source: r#"(nstring-upcase "ab")"#,
            expected: "\"AB\"",
        },
        BuiltinEntry {
            builtin: "NSTRING-DOWNCASE",
            source: r#"(nstring-downcase "AB")"#,
            expected: "\"ab\"",
        },
        BuiltinEntry {
            builtin: "NSTRING-CAPITALIZE",
            source: r#"(nstring-capitalize "aB cD")"#,
            expected: "\"Ab Cd\"",
        },
        BuiltinEntry {
            builtin: "SIMPLE-STRING-P",
            source: r#"(simple-string-p "")"#,
            expected: "T",
        },
        BuiltinEntry {
            builtin: "MAKE-STRING",
            source: r#"(make-string 3 :initial-element #\あ)"#,
            expected: "\"あああ\"",
        },
        BuiltinEntry {
            builtin: "STRING-TRIM",
            source: r#"(string-trim " " " hi ")"#,
            expected: "\"hi\"",
        },
        BuiltinEntry {
            builtin: "STRING-LEFT-TRIM",
            source: r#"(string-left-trim " " " hi ")"#,
            expected: "\"hi \"",
        },
        BuiltinEntry {
            builtin: "STRING-RIGHT-TRIM",
            source: r#"(string-right-trim " " " hi ")"#,
            expected: "\" hi\"",
        },
        BuiltinEntry {
            builtin: "NORMALIZE-NFC",
            source: r#"(ncl-unicode:normalize-nfc "é")"#,
            expected: "\"é\"",
        },
        BuiltinEntry {
            builtin: "NORMALIZE-NFD",
            source: r#"(ncl-unicode:normalize-nfd "é")"#,
            expected: "\"é\"",
        },
        BuiltinEntry {
            builtin: "NORMALIZE-NFKC",
            source: r#"(ncl-unicode:normalize-nfkc "①")"#,
            expected: "\"1\"",
        },
        BuiltinEntry {
            builtin: "NORMALIZE-NFKD",
            source: r#"(ncl-unicode:normalize-nfkd "①")"#,
            expected: "\"1\"",
        },
        BuiltinEntry {
            builtin: "FULL-UPCASE",
            source: r#"(ncl-unicode:full-upcase "straße")"#,
            expected: "\"STRASSE\"",
        },
        BuiltinEntry {
            builtin: "FULL-DOWNCASE",
            source: r#"(ncl-unicode:full-downcase "Ä")"#,
            expected: "\"ä\"",
        },
        BuiltinEntry {
            builtin: "FULL-TITLECASE",
            source: r#"(ncl-unicode:full-titlecase "hello world")"#,
            expected: "\"Hello World\"",
        },
        BuiltinEntry {
            builtin: "STRING-TO-UTF8",
            source: r#"(ncl-unicode:string-to-utf8 "A")"#,
            expected: "#(65)",
        },
        BuiltinEntry {
            builtin: "UTF8-TO-STRING",
            source: r#"(ncl-unicode:utf8-to-string #(227 129 130))"#,
            expected: "\"あ\"",
        },
        BuiltinEntry {
            builtin: "GRAPHEME-BOUNDARIES",
            source: r#"(ncl-unicode:grapheme-boundaries "é")"#,
            expected: "#(0 2)",
        },
        BuiltinEntry {
            builtin: "GENERAL-CATEGORY",
            source: r#"(ncl-unicode:general-category #\A)"#,
            expected: "\"Lu\"",
        },
    ]
}

#[test]
fn all_registered_string_builtins_work_through_compiled_code() {
    let cases = cases();
    assert_eq!(cases.len(), 68);
    for case in cases {
        let output = Command::new(env!("CARGO_BIN_EXE_ncl"))
            .args(["--eval", case.source])
            .output()
            .unwrap_or_else(|error| panic!("{}: spawn failed: {error}", case.builtin));
        assert!(
            output.status.success(),
            "{}: {} exited {:?}: {}",
            case.builtin,
            case.source,
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            case.expected,
            "{}: {}",
            case.builtin,
            case.source
        );
    }
}

#[test]
fn string_builtins_return_typed_conditions_for_invalid_arguments_and_ranges() {
    for (builtin, source) in [
        ("CHAR", r#"(char "" 0)"#),
        ("CHAR-CODE", r#"(char-code "A")"#),
        ("STRING=", r#"(string= t "A")"#),
        ("STRING-UPCASE", r#"(string-upcase "A" :start 2)"#),
        ("MAKE-STRING", r#"(make-string -1)"#),
        (
            "NCL-UNICODE:UTF8-TO-STRING",
            r#"(ncl-unicode:utf8-to-string #(255))"#,
        ),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_ncl"))
            .args(["--eval", source])
            .output()
            .unwrap_or_else(|error| panic!("{builtin}: spawn failed: {error}"));
        assert!(
            !output.status.success(),
            "{builtin}: {source} unexpectedly succeeded"
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("object error: TypeError"),
            "{builtin}: {source} did not report TypeError: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn keyword_calls_over_the_native_argument_limit_remain_args_pending() {
    let source = r#"(string-upcase "aB あ" :start 1 :end 2)"#;
    let output = Command::new(env!("CARGO_BIN_EXE_ncl"))
        .args(["--eval", source])
        .output()
        .unwrap_or_else(|error| panic!("{source}: spawn failed: {error}"));
    assert!(!output.status.success(), "{source} unexpectedly succeeded");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("at most four register arguments"),
        "{source}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
