use super::{Runtime, evaluate};

#[test]
fn compiled_evaluates_basic_format_directives() {
    assert_eq!(
        evaluate(r#"(format nil "~A/~S" "text" "text")"#).to_string(),
        r#""text/\"text\"""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "~D/~B/~O/~X" -12 10 8 255)"#).to_string(),
        r#""-12/1010/10/FF""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "~C/~~/~%end" #\!)"#).to_string(),
        r#""!/~/\nend""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "line~&next")"#).to_string(),
        r#""line\nnext""#,
    );
    assert_eq!(evaluate(r#"(format t "")"#).to_string(), "NIL");
    assert_eq!(
        evaluate(r#"(format nil "~?/~*" "~A ~D" '(foo 7) 99 100)"#).to_string(),
        r#""FOO 7/""#,
    );
    assert_eq!(
        evaluate("(format nil \"foo~\n  bar\")",).to_string(),
        r#""foobar""#,
    );
    assert_eq!(
        evaluate(
            r#"(list (format nil "~A~:*~A" 1 2)
                       (format nil "~A~A~2:*~A" 1 2 3)
                       (format nil "~@*~A" 1 2)
                       (format nil "~2@*~A" 1 2 3)
                       (format nil "~1@*~A ~A" 1 2 3))"#,
        )
        .to_string(),
        r#"("11" "121" "1" "3" "2 3")"#,
    );
}

#[test]
fn compiled_evaluates_plural_format_directive() {
    assert_eq!(
        evaluate(r#"(format nil "~P|~P|~@P|~@P" 1 2 1 2)"#).to_string(),
        r#""|s|y|ies""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "~D~:P|~D~:@P" 1 2)"#).to_string(),
        r#""1|2ies""#,
    );
}

#[test]
fn compiled_evaluates_dollar_float_format_directive() {
    assert_eq!(
        evaluate(r#"(format nil "~$|~,3$|~,,8$|~2,4,10,'*$|~@$|~,,10:@$" 12.3456 12.3456 12.3 12.3 12.3 12.3)"#)
            .to_string(),
        r#""12.35|012.35|   12.30|***0012.30|+12.30|+    12.30""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "~0$|~0@$|~0:$" 12.3 12.3 -12.3)"#).to_string(),
        r#""12.|+12.|-12.""#,
    );
}

#[test]
fn compiled_evaluates_general_float_format_directive() {
    assert_eq!(
        evaluate(r#"(format nil "~G|~,3G|~10,3G|~10,3G|~10,3,0G|~10,3,1G|~10,3,2G|~@G" 12.3456 1.25 12.3456 0.0123456 12.3456 12.3456 12.3456 1.25)"#)
            .to_string(),
        r#""12.3456    |1.25    |  12.3    |  1.235e-2|    12.3  |   12.3   |  12.3    |+1.25    ""#,
    );
}

#[test]
fn compiled_evaluates_format_tabulation_modifiers() {
    assert_eq!(
        evaluate(
            r#"(list (format nil "x~T|")
                       (format nil "x~:T|")
                       (format nil "x~@T|")
                       (format nil "x~:@T|")
                       (format nil "x~3,4T|")
                       (format nil "x~3,4:T|")
                       (format nil "x~3,4@T|")
                       (format nil "x~3,4:@T|"))"#,
        )
        .to_string(),
        r#"("x |" "x|" "x |" "x|" "x  |" "x|" "x   |" "x|")"#,
    );
}

#[test]
fn compiled_evaluates_format_write_directive() {
    assert_eq!(
        evaluate(
            r#"(list (format nil "~W" '("abc"))
                       (format nil "~:W" "abc")
                       (format nil "~@W" "abc")
                       (format nil "~:@W" "abc"))"#,
        )
        .to_string(),
        r#"("(\"abc\")" "\"abc\"" "\"abc\"" "\"abc\"")"#,
    );
}

#[test]
fn compiled_evaluates_fixed_float_format_directive() {
    assert_eq!(
        evaluate(r#"(format nil "~F|~,2F|~10,2F|~@F|~4,2,,'*F" 1.25 1.25 1.25 1.25 123.4)"#)
            .to_string(),
        r#""1.25|1.25|      1.25|+1.25|****""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "~,0F" 1.25)"#).to_string(),
        r#""1.""#
    );
    assert_eq!(evaluate(r#"(format nil "~F" 3)"#).to_string(), r#""3.0""#);
}

#[test]
fn compiled_evaluates_exponential_float_format_directive() {
    assert_eq!(
        evaluate(r#"(format nil "~E|~,2E|~10,2E|~@E" 1.25 1.25 1.25 1.25)"#).to_string(),
        r#""1.25E+0|1.25E+0|   1.25E+0|+1.25E+0""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "~,2,3E|~,2,,0E|~,2,,-1E" 0.0125 637.5 637.5)"#).to_string(),
        r#""1.25E-002|0.64E+3|0.06E+4""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "~6,2,,,'*E" 123.4)"#).to_string(),
        r#""******""#,
    );
}

#[test]
fn compiled_evaluates_parameterized_format_directives() {
    assert_eq!(
        evaluate(r#"(format nil "~10A|~10@A|~10,'0D|~:D|~@D" "x" "y" 42 1234567 8)"#).to_string(),
        r#""x         |         y|0000000042|1,234,567|+8""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "~2{~A~}" '(a b c))"#).to_string(),
        r#""AB""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "~vA/~8T" 5 "x")"#).to_string(),
        r#""x    /  ""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "~R/~:R/~@R/~W" 42 42 4 '(a 1))"#).to_string(),
        r#""forty-two/forty-second/IV/(A 1)""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "~:C/~@C" #\Newline #\Space)"#).to_string(),
        r#""Newline/#\\Space""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "a~2%b")"#).to_string(),
        r#""a\n\nb""#,
    );
}

#[test]
fn compiled_evaluates_format_iteration_directives() {
    assert_eq!(
        evaluate(r#"(format nil "~{~A/~A~}" '(one 1 two 2))"#).to_string(),
        r#""ONE/1TWO/2""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "~:{~A=~D;~}" '((x 1) (y 2)))"#).to_string(),
        r#""X=1;Y=2;""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "~@{~A~}" 'one 'two 'three)"#).to_string(),
        r#""ONETWOTHREE""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "~{~{~A~}~}" '((one two) (three four)))"#).to_string(),
        r#""ONETWOTHREEFOUR""#,
    );
}

#[test]
fn compiled_evaluates_format_recursive_processing_directive() {
    assert_eq!(
        evaluate(
            r#"(list (format nil "~? ~D" "<~A ~D>" '("Foo" 5) 7)
                       (format nil "~@? ~D" "<~A ~D>" "Foo" 5 7)
                       (format nil "~@? ~D" "<~A ~D>" "Foo" 5 14 7))"#,
        )
        .to_string(),
        r#"("<Foo 5> 7" "<Foo 5> 7" "<Foo 5> 14")"#,
    );
    assert_eq!(
        evaluate(r#"(format nil "~:{ ~@?~:^ ...~} " '(("a") ("b")))"#,).to_string(),
        r#"" a ... b ""#,
    );
}

#[test]
fn compiled_evaluates_format_justification_directive() {
    assert_eq!(
        evaluate(
            r#"(list (format nil "~15<~S~;~^~S~;~^~S~>" 'foo)
                       (format nil "~15<~S~;~^~S~;~^~S~>" 'foo 'bar)
                       (format nil "~15<~S~;~^~S~;~^~S~>" 'foo 'bar 'baz)
                       (format nil "~10<~A~;~A~>" "a" "b")
                       (format nil "~10:<~A~;~A~>" "a" "b")
                       (format nil "~10@<~A~;~A~>" "a" "b")
                       (format nil "~10:@<~A~;~A~>" "a" "b")
                       (format nil "~10,2,1<~A~;~A~>" "a" "b")
                       (format nil "~10<~A~;~A~1,1^~>~A" "a" "b" "c"))"#,
        )
        .to_string(),
        r#"("            FOO" "FOO         BAR" "FOO   BAR   BAZ" "a        b" "    a    b" "a    b    " "  a   b   " "a        b" "         ac")"#,
    );
}

#[test]
fn compiled_evaluates_format_conditional_newline_directive() {
    assert_eq!(
        evaluate(
            r#"(list (format nil "a~_b")
                       (format nil "a~:_b")
                       (format nil "a~@_b")
                       (format nil "a~:@_b")
                       (format nil "a~_~A" 'b))"#,
        )
        .to_string(),
        r#"("ab" "ab" "ab" "ab" "aB")"#,
    );
}

#[test]
fn compiled_evaluates_format_indentation_directive() {
    assert_eq!(
        evaluate(
            r#"(list (format nil "a~I b")
                       (format nil "a~1I b")
                       (format nil "a~:I b")
                       (format nil "a~1:I b")
                       (format nil "a~I~A" 'b))"#,
        )
        .to_string(),
        r#"("a b" "a b" "a b" "a b" "aB")"#,
    );
    for source in [
        r#"(format nil "a~1,2I b")"#,
        r#"(format nil "a~@I b")"#,
        r#"(format nil "a~:@I b")"#,
        r#"(format nil "~:*~A" 1)"#,
        r#"(format nil "~:@*~A" 1)"#,
    ] {
        assert!(
            Runtime::new().eval_compiled_source(source).is_err(),
            "{source}"
        );
    }
}

#[test]
fn compiled_evaluates_format_case_conversion_directive() {
    assert_eq!(
        evaluate(
            r#"(list (format nil "~(~A~)" "MiXeD Words")
                       (format nil "~:(~A~)" "MiXeD Words")
                       (format nil "~@(~A~)" "MiXeD Words")
                       (format nil "~:@(~A~)" "MiXeD Words")
                       (format nil "~(~A ~A~)" "MiXeD" "WORDS")
                       (format nil "~:(~A ~A~)" "MiXeD" "WORDS")
                       (format nil "~:@(~A ~A~)" "MiXeD" "WORDS"))"#,
        )
        .to_string(),
        r#"("mixed words" "Mixed Words" "Mixed words" "MIXED WORDS" "mixed words" "Mixed Words" "MIXED WORDS")"#,
    );
}

#[test]
fn compiled_evaluates_format_escape_upward_directive() {
    assert_eq!(
        evaluate(r#"(format nil "~{~A~^, ~}" '(one two three))"#).to_string(),
        r#""ONE, TWO, THREE""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "done~^ignored")"#).to_string(),
        r#""done""#,
    );
    assert_eq!(evaluate(r#"(format nil "a~1,1^b")"#).to_string(), r#""a""#,);
    assert_eq!(evaluate(r#"(format nil "a~1,2^b")"#).to_string(), r#""ab""#,);
    assert_eq!(
        evaluate(r#"(format nil "~:{~A~:^, ~}" '((a) (b) (c)))"#).to_string(),
        r#""A, B, C""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "~[a~^b~;c~]" 0)"#).to_string(),
        r#""a""#,
    );
}

#[test]
fn compiled_evaluates_format_choice_directives() {
    assert_eq!(
        evaluate(r#"(format nil "~[zero~;one~;two~]" 1)"#).to_string(),
        r#""one""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "~[zero~;one~:;other~]" 9)"#).to_string(),
        r#""other""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "~[zero~;~{~A~}~]" 1 '(a b))"#).to_string(),
        r#""AB""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "~:[false~;true~]" nil)"#).to_string(),
        r#""false""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "~:[false~;true~]" t)"#).to_string(),
        r#""true""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "~@[yes~]" t)"#).to_string(),
        r#""yes""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "~@[yes~]" nil)"#).to_string(),
        r#""""#,
    );
    assert_eq!(
        evaluate(
            r#"(list (format nil "~@[~A~]~A" t 'x)
                       (format nil "~@[~A~]~A" nil 'x)
                       (format nil "~@[yes~]~A" t 'x)
                       (format nil "~@[yes~]~A" nil 'x))"#,
        )
        .to_string(),
        r#"("TX" "X" "yesT" "X")"#,
    );
    assert_eq!(
        evaluate(
            r#"(list (format nil "~[zero~@;one~]" 1)
                       (format nil "~[zero~@;one~]" 9)
                       (format nil "~[zero~;one~:@;other~]" 9))"#,
        )
        .to_string(),
        r#"("one" "" "other")"#,
    );
}

#[test]
fn compiled_evaluates_format_choice_parameters() {
    assert_eq!(
        evaluate(
            r#"(list (format nil "~2[zero~;one~;two~]~A" 'x)
                       (format nil "~v[zero~;one~;two~]~A" 2 'x)
                       (format nil "~#[zero~;one~;two~;many~]~A" 'x 'y))"#,
        )
        .to_string(),
        r#"("twoX" "twoX" "twoX")"#,
    );
}

#[test]
fn compiled_evaluates_write_to_string() {
    assert_eq!(
        evaluate("(write-to-string '(a 1))").to_string(),
        r#""(A 1)""#,
    );
    assert_eq!(
        evaluate("(write-to-string \"abc\")").to_string(),
        r#""\"abc\"""#,
    );
    assert_eq!(
        evaluate("(write-to-string #(1 2))").to_string(),
        r##""#(1 2)""##,
    );
}

#[test]
fn compiled_evaluates_write_escape_options() {
    assert_eq!(
        evaluate(
            r#"(list (write-to-string "abc")
                       (write-to-string "abc" :escape nil)
                       (write-to-string '("abc") :escape nil))"#,
        )
        .to_string(),
        r#"("\"abc\"" "abc" "(abc)")"#,
    );
}

#[test]
fn compiled_evaluates_print_variants_to_string_stream() {
    assert_eq!(
        evaluate(
            r#"(let ((output (make-string-output-stream)))
               (list (princ "a" output)
                     (prin1 "a" output)
                     (print 1 output)
                     (get-output-stream-string output)))"#,
        )
        .to_string(),
        r#"("a" "a" 1 "a\"a\"\n1\n")"#,
    );
}

#[test]
fn compiled_evaluates_write_to_stream() {
    assert_eq!(
        evaluate(
            r#"(let ((output (make-string-output-stream)))
               (list (princ "abc" output)
                     (prin1 "abc" output)
                     (write "abc" :stream output :escape nil)
                     (write "abc" :stream output :escape t)
                     (get-output-stream-string output)))"#,
        )
        .to_string(),
        r#"("abc" "abc" "abc" "abc" "abc\"abc\"abc\"abc\"")"#,
    );
}

#[test]
fn compiled_evaluates_read_from_string() {
    assert_eq!(
        evaluate(
            r#"(multiple-value-bind (value position)
                   (read-from-string "  (a 1) trailing")
                 (list value position))"#,
        )
        .to_string(),
        "((A 1) 8)",
    );
    assert_eq!(
        evaluate(
            r#"(multiple-value-bind (value position)
                   (read-from-string "42 rest")
                 (list value position))"#,
        )
        .to_string(),
        "(42 3)",
    );
    assert_eq!(
        evaluate(
            r#"(multiple-value-bind (value position)
                   (read-from-string "" nil :eof)
                 (list value position))"#,
        )
        .to_string(),
        "(:EOF 0)",
    );
}

#[test]
fn compiled_evaluates_read_from_string_options() {
    assert_eq!(
        evaluate(
            r#"(list
                   (multiple-value-bind (value position)
                       (read-from-string "  (a)  b" nil :eof :start 1 :end 8)
                     (list value position))
                   (multiple-value-bind (value position)
                       (read-from-string "  (a)  b" nil :eof :start 1 :end 8
                                         :preserve-whitespace t)
                     (list value position))
                   (multiple-value-bind (value position)
                       (read-from-string "  (a)  b" nil :eof :start 2 :end 5)
                     (list value position)))"#,
        )
        .to_string(),
        "(((A) 6) ((A) 5) ((A) 5))",
    );
}

#[test]
fn compiled_evaluates_read_from_string_stream() {
    assert_eq!(
        evaluate(
            r#"(let ((input (make-string-input-stream "  (a 1) 42  ")))
               (list (read input)
                     (read input)
                     (read-preserving-whitespace input nil :eof)
                     (read input nil :eof)))"#,
        )
        .to_string(),
        "((A 1) 42 :EOF :EOF)",
    );
}

#[test]
fn compiled_evaluates_read_whitespace_consumption() {
    assert_eq!(
        evaluate(
            r#"(let ((read-input (make-string-input-stream "(a)  b"))
                     (preserve-input (make-string-input-stream "(a)  b")))
                 (list (read read-input)
                       (read-char read-input)
                       (read read-input)
                       (read-preserving-whitespace preserve-input)
                       (read-char preserve-input)
                       (read preserve-input)))"#,
        )
        .to_string(),
        r#"((A) #\SPACE B (A) #\SPACE B)"#,
    );
}

#[test]
fn compiled_evaluates_character_stream_options_and_eof() {
    assert_eq!(
        evaluate(
            r#"(list
                 (let ((input (make-string-input-stream "a")))
                   (list (read-char input nil :eof)
                         (read-char input nil :eof)))
                 (let ((input (make-string-input-stream "  a ")))
                   (list (peek-char t input nil :eof)
                         (read-char input nil :eof)
                         (peek-char nil input nil :eof)
                         (read-char input nil :eof)
                         (read-char input nil :eof)))
                 (let ((input (make-string-input-stream "acb")))
                   (list (peek-char #\b input nil :eof)
                         (read-char input nil :eof)))
                 (let ((input (make-string-input-stream "a")))
                   (list (multiple-value-list (read-line input nil :eof))
                         (multiple-value-list (read-line input nil :eof))))
                 (let ((input (make-string-input-stream (format nil "abc~%def"))))
                   (list (multiple-value-list (read-line input nil :eof))
                         (multiple-value-list (read-line input nil :eof))
                         (multiple-value-list (read-line input nil :eof)))))"#,
        )
        .to_string(),
        r#"((#\a :EOF) (#\a #\a #\SPACE #\SPACE :EOF) (#\b #\b) (("a" T) (:EOF T)) (("abc" NIL) ("def" T) (:EOF T)))"#,
    );
}
