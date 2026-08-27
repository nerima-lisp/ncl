use super::*;

#[test]
fn evaluates_function_namespace_introspection() {
    assert_eq!(
        evaluate(
            "(progn
               (defun introspection-target (value) (+ value 1))
               (list (fboundp 'car)
                     (fboundp 'introspection-target)
                     (fboundp 'missing-function)
                     (functionp (fdefinition 'car))
                     (funcall (fdefinition 'introspection-target) 4)))",
        )
        .to_string(),
        "(T T NIL T 5)"
    );
    let error = Runtime::new()
        .eval_source("(fdefinition 'missing-function)")
        .must_fail();
    assert!(matches!(
        error,
        ncl_runtime::RuntimeError::UnboundVariable { name, .. }
            if name == "MISSING-FUNCTION"
    ));
}

#[test]
fn evaluates_compile_function() {
    assert_eq!(
        evaluate(
            "(let ((function (compile nil '(lambda (value) (+ value 1)))))
               (list (compiled-function-p function)
                     (funcall function 5)))"
        )
        .to_string(),
        "(T 6)"
    );
    assert_eq!(
        evaluate("(multiple-value-list (compile nil '(lambda () 42)))").to_string(),
        "(#<FUNCTION> NIL NIL)"
    );
    assert_eq!(
        evaluate(
            "(progn
               (compile 'compile-target '(lambda (value) (* value value)))
               (list (compiled-function-p #'compile-target)
                     (compile-target 7)))"
        )
        .to_string(),
        "(T 49)"
    );

    for source in [
        "(compile)",
        "(compile nil)",
        "(compile 1 '(lambda () 1))",
        "(compile 'missing-compile-function)",
        "(compile 'car 1)",
    ] {
        assert!(Runtime::new().eval_source(source).is_err(), "{source}");
    }
}

#[test]
fn evaluates_load_file() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/load.lisp")
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    assert_eq!(
        evaluate(&format!(
            r#"(list (load "{path}") *NCL-LOAD-VALUE* (NCL-LOAD-TARGET 1))"#
        ))
        .to_string(),
        "(T 41 42)"
    );

    for source in [
        "(load)",
        "(load 1)",
        "(load \"missing-file-for-test.lisp\")",
    ] {
        assert!(Runtime::new().eval_source(source).is_err(), "{source}");
    }
}

#[test]
fn evaluates_load_time_value() {
    assert_eq!(
        evaluate(
            "(let ((function (lambda () (load-time-value (+ 8 9)))))
               (list (funcall function) (funcall function)
                     (load-time-value (+ 1 2) nil)))",
        )
        .to_string(),
        "(17 17 3)"
    );
}

#[test]
fn evaluates_nth_value() {
    assert_eq!(
        evaluate(
            "(list
               (nth-value 0 (values 10 20))
               (nth-value 1 (values 10 20))
               (nth-value 4 (values 10 20))
               (nth-value 0 99)
               (nth-value 0 (values)))",
        )
        .to_string(),
        "(10 20 NIL 99 NIL)"
    );
}

#[test]
fn evaluates_function_and_macro_introspection() {
    assert_eq!(
        evaluate(
            "(progn
               (defmacro introspection-macro (value) (list '+ value 1))
               (defmacro local-macro-visible (&environment environment)
                 (if (functionp (macro-function 'local-macro environment))
                     '(quote t)
                     '(quote nil)))
               (list (functionp (macro-function 'introspection-macro))
                     (eq (macro-function 'missing-macro) nil)
                     (special-operator-p 'if)
                     (special-operator-p 'and)
                     (special-operator-p 'return-from)
                     (special-operator-p 'load-time-value)
                     (compiled-function-p (function +))
                     (macrolet ((local-macro (value) (list '+ value 2)))
                       (list (functionp (macro-function 'local-macro))
                             (local-macro-visible)))))",
        )
        .to_string(),
        "(T T T NIL NIL T NIL (NIL T))"
    );
}

#[test]
fn evaluates_symbol_function_and_setf() {
    assert_eq!(
        evaluate(
            "(progn
               (defun symbol-function-target (value) (+ value 2))
               (let ((name 'symbol-function-target))
                 (list (functionp (symbol-function name))
                       (funcall (symbol-function name) 5)
                       (progn
                         (setf (symbol-function name)
                               (lambda (value) (+ value 3)))
                         (funcall (symbol-function name) 5))
                       (fboundp name))))",
        )
        .to_string(),
        "(T 7 8 T)"
    );
}

#[test]
fn evaluates_symbol_values_and_property_lists_from_table_cases() {
    let cases = [
        (
            "(progn (defparameter symbol-value-target 10)\n                 (list (boundp 'symbol-value-target)\n                       (symbol-value 'symbol-value-target)\n                       (set 'symbol-value-target 11)\n                       (symbol-value 'symbol-value-target)))",
            "(T 10 11 11)",
        ),
        (
            "(let ((symbol (make-symbol \"temporary\")))\n                 (list (symbolp symbol) (symbol-name symbol)\n                       (boundp symbol) (constantp symbol)))",
            "(T \"temporary\" NIL NIL)",
        ),
        (
            "(let ((symbol 'property-target))\n                 (list (get symbol :answer)\n                       (putprop symbol 42 :answer)\n                       (get symbol :answer)\n                       (remprop symbol :answer)\n                       (get symbol :answer)\n                       (symbol-plist symbol)))",
            "(NIL 42 42 T NIL NIL)",
        ),
    ];

    assert_value_cases(evaluate, &cases);
}

#[test]
fn evaluates_function_namespace_mutation() {
    assert_eq!(
        evaluate(
            "(progn
               (defun fmakunbound-target () 42)
               (list (fboundp 'fmakunbound-target)
                     (symbolp (fmakunbound 'fmakunbound-target))
                     (fboundp 'fmakunbound-target)))",
        )
        .to_string(),
        "(T T NIL)"
    );
}

#[test]
fn evaluates_numeric_predicates_and_extrema() {
    assert_value_cases(
        evaluate,
        &[(
            "(list (zerop 0) (plusp 1) (minusp -1) (evenp 4) (oddp 3) (min 3 1 2) (max 3 1 2) (abs -5))",
            "(T T T T T 1 3 5)",
        )],
    );
}

#[test]
fn evaluates_common_lisp_integer_arithmetic_and_bit_operations() {
    assert_eq!(
        evaluate(
            "(list (mod -7 3) (mod 7 -3) (rem -7 3) (rem 7 -3)
                    (ash 3 2) (ash -8 -2)
                    (logand 7 3) (logior 4 1) (logxor 7 3) (lognot 0)
                    (logtest 6 2) (logtest 4 2)
                    (logcount 13) (logcount -8)
                    (integer-length 8) (integer-length -8)
                    (logand) (logior) (logxor))",
        )
        .to_string(),
        "(2 -2 -1 1 12 -2 3 5 4 -1 T NIL 3 3 4 3 -1 0 0)"
    );
}

#[test]
fn rejects_invalid_integer_arithmetic_arguments() {
    for source in [
        "(mod 1 0)",
        "(rem 1 0)",
        "(ash 1 1.5)",
        "(logand 1 1.0)",
        "(gcd 1 1.0)",
        "(integer-length 1.0)",
    ] {
        assert!(Runtime::new().eval_source(source).is_err(), "{source}");
    }
}

#[test]
fn evaluates_common_lisp_quotients_gcd_and_rational_parts() {
    assert_eq!(
        evaluate(
            "(list
                    (multiple-value-bind (q r) (floor 7 3) (list q r))
                    (multiple-value-bind (q r) (floor -7 3) (list q r))
                    (multiple-value-bind (q r) (ceiling -7 3) (list q r))
                    (multiple-value-bind (q r) (truncate -7 3) (list q r))
                    (multiple-value-bind (q r) (round 5 2) (list q r))
                    (multiple-value-bind (q r) (round 7 2) (list q r))
                    (multiple-value-bind (q r) (floor -7/3) (list q r))
                    (multiple-value-bind (q r) (ceiling 7/3) (list q r))
                    (multiple-value-bind (q r) (floor 3.5 2.0) (list q r))
                    (multiple-value-bind (q r) (round 2.5) (list q r))
                    (gcd 18 -24 30) (gcd) (lcm 6 -8 15) (lcm)
                    (numerator -6/8) (denominator -6/8)
                    (numerator 7) (denominator 7))",
        )
        .to_string(),
        "((2 1) (-3 2) (-2 -1) (-2 -1) (2 1) (4 -1) (-3 2/3) (3 -2/3) (1 1.5) (2 0.5) 6 0 120 1 -3 4 7 1)"
    );
}

#[test]
fn evaluates_common_lisp_expt_across_numeric_types() {
    assert_eq!(
        evaluate(
            "(list (expt 2 10) (expt 2 -3) (expt 3/2 2)
                    (= (expt 2.0 3) 8.0) (floatp (expt 2.0 3))
                    (floatp (expt 2 1/2)) (expt 0 0))",
        )
        .to_string(),
        "(1024 1/8 9/4 T T T 1)"
    );
}

#[test]
fn evaluates_common_lisp_sqrt_across_exact_and_float_numbers() {
    assert_eq!(
        evaluate(
            "(list (sqrt 0) (sqrt 4) (sqrt 1/4)
                    (rationalp (sqrt 2)) (floatp (sqrt 2))
                    (= (sqrt 4.0) 2.0))",
        )
        .to_string(),
        "(0 2 1/2 NIL T T)"
    );
}

#[test]
fn evaluates_common_lisp_signum_and_rationalize() {
    assert_eq!(
        evaluate(
            "(list (signum -7) (signum 0) (signum -5/2)
                    (signum -0.0) (signum 3.5)
                    (rationalize 2) (rationalize 3/6)
                    (rationalize 0.1) (rationalize (/ 1.0 3.0))
                    (rationalp (rationalize 0.1))
                    (floatp (signum 0.0)))",
        )
        .to_string(),
        "(-1 0 -1 -0.0 1.0 2 1/2 1/10 1/3 T T)"
    );
}

#[test]
fn evaluates_common_lisp_float_and_rational_conversion() {
    assert_eq!(
        evaluate(
            "(list (float 3) (float 1/2) (float -0.0) (float 1.25 0.0)
                    (rational 3) (rational 3/6) (rational 1.5)
                    (rational 0.1) (rationalp (rational 0.1)))",
        )
        .to_string(),
        "(3.0 0.5 -0.0 1.25 3 1/2 3/2 3602879701896397/36028797018963968 T)"
    );
}

#[test]
fn evaluates_basic_format_directives() {
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
}

#[test]
fn rejects_invalid_format_entry_arguments_and_destinations() {
    for source in [
        "(format nil)",
        "(format nil 1)",
        "(format 1 \"text\")",
        "(let ((stream (make-string-output-stream)))
           (close stream)
           (format stream \"text\"))",
    ] {
        Runtime::new().eval_source(source).must_fail();
    }
}

#[test]
fn rejects_invalid_format_directives() {
    for source in [
        r#"(format nil "~")"#,
        r#"(format nil "~Q")"#,
        r#"(format nil "~:~")"#,
        r#"(format nil "~}")"#,
        r#"(format nil "~A" )"#,
        r#"(format nil "~D" 1.5)"#,
        r#"(format nil "~[zero~;one~]")"#,
        r#"(format nil "~{~A~}" 1)"#,
    ] {
        assert!(Runtime::new().eval_source(source).is_err(), "{source}");
    }
}

#[test]
fn evaluates_plural_format_directive() {
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
fn evaluates_dollar_float_format_directive() {
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
fn evaluates_general_float_format_directive() {
    assert_eq!(
        evaluate(r#"(format nil "~G|~,3G|~10,3G|~10,3G|~10,3,0G|~10,3,1G|~10,3,2G|~@G" 12.3456 1.25 12.3456 0.0123456 12.3456 12.3456 12.3456 1.25)"#)
            .to_string(),
        r#""12.3456    |1.25    |  12.3    |  1.235e-2|    12.3  |   12.3   |  12.3    |+1.25    ""#,
    );
}

#[test]
fn rejects_invalid_general_float_format_parameters() {
    for source in [
        r#"(format nil "~,-1G" 1.25)"#,
        r#"(format nil "~,,-1G" 1.25)"#,
        r#"(format nil "~,,999999999999999999999999999999G" 1.25)"#,
        r#"(format nil "~:G" 1.25)"#,
        r#"(format nil "~,xG" 1.25)"#,
        r#"(format nil "~,,xG" 1.25)"#,
    ] {
        assert!(Runtime::new().eval_source(source).is_err(), "{source}");
    }
}

#[test]
fn evaluates_format_tabulation_modifiers() {
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
fn evaluates_format_write_directive() {
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
fn evaluates_fixed_float_format_directive() {
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
fn evaluates_exponential_float_format_directive() {
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
fn evaluates_parameterized_format_directives() {
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
fn evaluates_format_iteration_directives() {
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
fn evaluates_format_recursive_processing_directive() {
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
fn evaluates_format_justification_directive() {
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
fn evaluates_format_conditional_newline_directive() {
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
fn evaluates_format_indentation_directive() {
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
    ] {
        assert!(Runtime::new().eval_source(source).is_err(), "{source}");
    }
}

#[test]
fn evaluates_format_case_conversion_directive() {
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
fn evaluates_format_escape_upward_directive() {
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
fn evaluates_format_choice_directives() {
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
}

#[test]
fn evaluates_format_choice_parameters() {
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
fn evaluates_write_to_string() {
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
fn evaluates_write_escape_options() {
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
fn evaluates_print_variants_to_string_stream() {
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
fn evaluates_write_to_stream() {
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
fn evaluates_read_from_string() {
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
fn evaluates_read_from_string_options() {
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
fn evaluates_read_from_string_stream() {
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
fn evaluates_read_whitespace_consumption() {
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
        r"((A) #\SPACE B (A) #\SPACE B)",
    );
}

#[test]
fn evaluates_character_stream_options_and_eof() {
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

#[test]
fn evaluates_sequence_operations_and_type_predicates() {
    assert_eq!(
        evaluate("(list (first '(a b c)) (rest '(a b c)) (nth 1 '(a b c)) (elt \"abc\" 1) (subseq '(a b c d) 1 3) (subseq \"abcd\" 1 3) (member 'b '(a b c)) (assoc 'b '((a 1) (b 2))) (getf '(:a 1 :b 2) :b) (length \"abc\"))").to_string(),
        "(A (B C) B #\\b (B C) \"bc\" (B C) (B 2) 2 3)"
    );
    assert_eq!(
        evaluate("(list (typep 1 'integer) (typep \"abc\" 'sequence) (characterp #\\a) (keywordp :x) (vectorp #(1 2)) (endp nil) (endp '(1)))").to_string(),
        "(T T T T T T NIL)"
    );
}

#[test]
fn evaluates_compound_type_designators() {
    assert_eq!(
        evaluate(
            "(list
                (typep 3 '(or string (integer 0 5)))
                (typep 7 '(and integer (not (member 4 5))))
                (typep 4 '(member 3 4 5))
                (typep 4 '(eql 4))
                (typep 3 '(mod 4))
                (typep 3 '(unsigned-byte 4))
                (typep -8 '(signed-byte 4))
                (typep '(1 2) '(cons integer list))
                (typep #(1 2) '(vector integer 2))
                (typep #(1 2) '(simple-vector 2))
                (typep #(0 1) '(bit-vector 2))
                (typep #(1 2) '(array integer 1))
                (typep #(1 2) '(array integer (2)))
                (typep #(0 2) 'bit-vector)
                (the (or integer string) 7)
                (the (vector integer 2) #(1 2)))",
        )
        .to_string(),
        "(T T T T T T T T T T T T T NIL 7 #(1 2))"
    );
}

#[test]
fn evaluates_type_designator_boundaries_and_failures() {
    assert_eq!(
        evaluate(
            "(list
                (typep nil 'null)
                (typep nil 'symbol)
                (typep #(1 2) '(vector * *))
                (typep #(1 2) '(simple-vector *))
                (typep #(0 1) '(bit-vector *))
                (typep 255 '(unsigned-byte 8))
                (typep -128 '(signed-byte 8))
                (typep 256 '(unsigned-byte 8))
                (typep -129 '(signed-byte 8))
                (typep 0 '(mod 0))
                (typep #(1 2) '(simple-vector 3))
                (typep #(0 2) 'bit-vector)
                (typep #(0 2) '(bit-vector 3))
                (typep #(1 2) '(vector integer 3))
                (typep (make-array '(2 2) :initial-element 1)
                       '(array integer (2 2)))
                (typep (make-array '(2 2) :initial-element 1)
                       '(array integer (2 3)))
                (typep (make-array '(2 2) :initial-element 1)
                       '(array string (2 2))))",
        )
        .to_string(),
        "(T T T T T T T NIL NIL NIL NIL NIL NIL NIL T NIL NIL)"
    );

    let error = Runtime::new()
        .eval_source("(typep 1 '(unknown-type 1))")
        .must_fail();
    assert!(matches!(
        error,
        ncl_runtime::RuntimeError::InvalidForm { .. }
    ));
}

#[test]
fn rejects_invalid_type_designator_shapes() {
    for source in [
        "(typep 1 '(integer 0 1 2))",
        "(typep 1 '(vector integer 1 2))",
        "(typep 1 '(array integer (1 2) 3))",
        "(typep 1 '(mod -1))",
        "(typep 1 '(unsigned-byte -1))",
        "(typep 1 '(not integer extra))",
        "(subtypep 'integer '(integer 0 1 2))",
        "(subtypep '(vector integer 1 2) 'vector)",
    ] {
        assert!(Runtime::new().eval_source(source).is_err(), "{source}");
    }
}

#[test]
fn evaluates_subtypep() {
    let values = Runtime::new()
        .eval_source(
            r"(progn
                 (defclass subtypep-parent () ())
                 (defclass subtypep-child (subtypep-parent) ())
                 (defstruct subtypep-record value)
                 (list
                   (multiple-value-list (subtypep 'integer 'number))
                   (multiple-value-list (subtypep '(integer 0 5) '(integer -1 10)))
                   (multiple-value-list (subtypep '(integer 0 10) '(integer 1 5)))
                   (multiple-value-list (subtypep 'subtypep-child 'subtypep-parent))
                   (multiple-value-list (subtypep 'subtypep-record 'structure))
                   (multiple-value-list (subtypep 'string 'sequence))
                   (multiple-value-list (subtypep 'null 'symbol))
                   (multiple-value-list (subtypep '(integer 0 5) '(integer 1 10)))))",
        )
        .must_exist();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].to_string(),
        "((T T) (T T) (NIL T) (T T) (T T) (T T) (T T) (NIL T))"
    );
}

#[test]
fn evaluates_sequence_construction_and_coercion() {
    assert_eq!(
        evaluate(
            "(list (make-sequence 'list 3)
                    (make-sequence 'vector 2 :initial-element 7)
                    (make-sequence 'string 3 :initial-element #\\x)
                    (coerce '(1 2) 'vector)
                    (coerce #(1 2) 'list)
                    (coerce '(#\\a #\\b) 'string)
                    (coerce 'foo 'string)
                    (simple-string-p \"abc\"))",
        )
        .to_string(),
        "((NIL NIL NIL) #(7 7) \"xxx\" #(1 2) (1 2) \"ab\" \"FOO\" T)"
    );
}

#[test]
fn evaluates_parse_integer() {
    assert_eq!(
        evaluate(
            "(list
                (multiple-value-bind (value position)
                    (parse-integer \"  -1x\" :junk-allowed t)
                  (list value position))
                (multiple-value-bind (value position)
                    (parse-integer \"xx42yy\" :start 2 :end 4)
                  (list value position))
                (parse-integer \"ff\" :radix 16)
                (multiple-value-bind (value position)
                    (parse-integer \"no-integer\" :junk-allowed t)
                  (list value position)))",
        )
        .to_string(),
        "((-1 4) (42 4) 255 (NIL 0))"
    );
}

#[test]
fn rejects_invalid_parse_integer_arguments() {
    for source in [
        "(parse-integer)",
        "(parse-integer \"1\" :start)",
        "(parse-integer 1)",
        "(parse-integer \"1\" :unknown 1)",
        "(parse-integer \"1\" :radix 1)",
        "(parse-integer \"1\" :radix 37)",
        "(parse-integer \"1\" :start 2 :end 1)",
        "(parse-integer \"1\" :end 2)",
        "(parse-integer \"no-integer\")",
        "(parse-integer \"1x\")",
    ] {
        assert!(Runtime::new().eval_source(source).is_err(), "{source}");
    }
}

#[test]
fn evaluates_type_predicates_and_structural_equality() {
    assert_eq!(
        evaluate(
            "(list
                (null nil) (null 1)
                (atom 1) (atom '(a))
                (consp '(a)) (consp nil)
                (listp '(a b)) (listp '(a . b))
                (numberp 1) (numberp \"1\")
                (integerp 1) (integerp 1.0)
                (floatp 1.0) (rationalp 1/2)
                (stringp \"a\") (simple-string-p \"a\")
                (symbolp 'a) (packagep (find-package :cl-user))
                (functionp #'car)
                (eq 'a 'a) (eq \"a\" \"a\")
                (eql 1 1) (eql 1 1.0)
                (equal '(a (b)) '(a (b)))
                (equal '(a) '(b))
                (equalp \"AbC\" \"aBc\")
                (equalp #\\A #\\a)
                (equalp '(A #(1 2)) '(a #(1 2)))
                (equalp #(1 2) #(1 3)))",
        )
        .to_string(),
        "(T NIL T NIL T NIL T NIL T NIL T NIL T T T T T NIL T T NIL T NIL T NIL T T T NIL)"
    );
}

#[test]
fn evaluates_structural_equality_boundaries_from_table_cases() {
    let cases = [
        ("(eql 1/2 1/2)", "T"),
        ("(eql 1.0 1.0)", "T"),
        ("(equal \"abc\" \"abc\")", "T"),
        ("(equal '(a . b) '(a . b))", "T"),
        ("(equal '(a . b) '(a . c))", "NIL"),
        ("(equalp 1 1.0)", "T"),
        ("(equalp 1/2 0.5)", "T"),
        ("(equalp #(1 2) #(1 2))", "T"),
        ("(equalp #(1) '(1))", "NIL"),
        (
            "(equalp (make-array 2 :initial-element 1) (make-array 2 :initial-element 1))",
            "T",
        ),
        ("(equalp '(a . b) '(a . b))", "T"),
        ("(equalp '(a . b) '(a . c))", "NIL"),
    ];

    for (source, expected) in cases {
        assert_eq!(evaluate(source).to_string(), expected, "{source}");
    }
}

#[test]
fn evaluates_collection_and_runtime_type_predicates() {
    assert_eq!(
        evaluate(
            "(let ((array (make-array 2))
                   (table (make-hash-table))
                   (stream (make-string-output-stream)))
               (list (characterp #\\a) (characterp 1)
                     (keywordp :name) (keywordp 'name)
                     (vectorp #(1 2)) (vectorp '(1 2))
                     (arrayp array) (arrayp #(1 2))
                     (hash-table-p table) (hash-table-p array)
                     (streamp stream) (streamp table)))"
        )
        .to_string(),
        "(T NIL T NIL T NIL T T T NIL T NIL)"
    );
}

#[test]
fn evaluates_stream_predicates_for_both_directions_and_non_streams() {
    assert_eq!(
        evaluate(
            "(let ((input (make-string-input-stream \"in\"))
                   (output (make-string-output-stream)))
               (list (streamp input) (streamp 1)
                     (input-stream-p input) (input-stream-p output)
                     (output-stream-p input) (output-stream-p output)
                     (input-stream-p nil) (output-stream-p nil)))"
        )
        .to_string(),
        "(T NIL T NIL NIL T NIL NIL)"
    );
}

#[test]
fn rejects_invalid_close_argument_shapes() {
    for source in [
        "(close)",
        "(close nil nil)",
        "(close nil :unknown t)",
        "(close nil :abort)",
    ] {
        Runtime::new().eval_source(source).must_fail();
    }
}

#[test]
fn rejects_invalid_file_primitive_arguments() {
    let cases = [
        "(open)",
        "(open \"missing\" :direction)",
        "(open 1)",
        "(open \"missing\" 1 :input)",
        "(open \"missing\" :unknown :error)",
        "(open \"missing\" :direction 1)",
        "(open \"missing\" :direction :unknown)",
        "(open \"missing\" :if-does-not-exist :unknown)",
        "(open \"missing\" :if-exists :unknown)",
        "(probe-file)",
        "(probe-file 1)",
        "(delete-file)",
        "(delete-file 1)",
        "(rename-file \"old\")",
        "(rename-file 1 \"new\")",
        "(truename)",
        "(truename 1)",
        "(file-write-date)",
        "(file-write-date 1)",
    ];

    for source in cases {
        assert!(Runtime::new().eval_source(source).is_err(), "{source}");
    }
}

#[test]
fn evaluates_symbol_package_boundaries() {
    assert_eq!(
        evaluate(
            "(let ((uninterned (make-symbol \"temporary\")))
               (list (symbol-package 'name)
                     (symbol-package :name)
                     (symbol-package nil)
                     (symbol-package uninterned)))"
        )
        .to_string(),
        "(NCL-USER KEYWORD COMMON-LISP NIL)"
    );
    assert!(Runtime::new().eval_source("(symbol-package 1)").is_err());
}

#[test]
fn rejects_invalid_package_and_method_primitive_arguments_from_table() {
    let cases = [
        "(documentation 1 2)",
        "(documentation *package*)",
        "(list-all-packages 1)",
        "(next-method-p 1)",
        "(call-next-method)",
        "(use-package)",
        "(use-package 1 2 3)",
        "(use-package 1)",
        "(export 1)",
        "(import 1)",
        "(shadow 1)",
        "(unintern 1)",
    ];

    for source in cases {
        Runtime::new().eval_source(source).must_fail();
    }
}
