use ncl_runtime::Runtime;
use rstest::rstest;

use super::support::evaluate_with;
use super::EvalFn;

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_common_lisp_complement(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate("(list (funcall (complement #'null) nil) (funcall (complement #'null) 1))")
            .to_string(),
        "(NIL T)",
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_common_lisp_bitfield_operations(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            r#"(let ((b (byte 4 4)))
                 (list b (ldb b #xabc) (mask-field b #xabc)
                       (dpb 2 b #xabc) (deposit-field #x050 b #xabc)))"#,
        )
        .to_string(),
        "((4 4) 11 176 2604 2652)",
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_basic_format_directives(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
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

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_character_stream_options_and_eof(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
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

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_common_lisp_expt_across_numeric_types(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
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

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_common_lisp_float_and_rational_conversion(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
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

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_common_lisp_integer_arithmetic_and_bit_operations(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(list (mod -7 3) (mod 7 -3) (rem -7 3) (rem 7 -3)
                        (ash 3 2) (ash -8 -2)
                        (logand 7 3) (logior 4 1) (logxor 7 3) (lognot 0)
                        (logtest 6 2) (logtest 4 2)
                        (logbitp 0 0) (logbitp 2 4) (logbitp 100 -1)
                        (logandc1 10 6) (logandc2 10 6) (logeqv 10 6)
                        (lognand 10 6) (lognor 10 6) (logorc1 10 6) (logorc2 10 6)
                        (logcount 13) (logcount -8)
                        (integer-length 8) (integer-length -8)
                        (logand) (logior) (logxor))",
        )
        .to_string(),
        "(2 -2 -1 1 12 -2 3 5 4 -1 T NIL NIL T T 4 8 -13 -3 -15 -9 -5 3 3 4 3 -1 0 0)"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_common_lisp_float_operations(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(list (float-sign -2.5) (float-sign -2.5 -1) (float-digits 1.0) (float-precision 0.0) (float-radix 1.0) (scale-float 1.5 2))"
        )
        .to_string(),
        "(-2.5 -2.5 53 0 2 6.0)",
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_common_lisp_phase(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate("(list (phase -1) (phase (complex -1 1)))").to_string(),
        "(3.141592653589793 2.356194490192345)"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_common_lisp_real_transcendental_functions(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate("(list (sin 0) (cos 0) (tan 0) (exp 1) (log 1) (log 8 2) (sqrt 4) (asin 0) (acos 0) (atan 0) (sinh 0) (cosh 0) (tanh 0))").to_string(),
        "(0.0 1.0 0.0 2.718281828459045 0.0 3.0 2 0.0 1.5707963267948966 0.0 0.0 1.0 0.0)",
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_common_lisp_complex_unit_circle_function(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate("(list (cis 0) (realpart (cis 1.5707963267948966)) (imagpart (cis 1.5707963267948966)))").to_string(),
        "(#C(1.0 0.0) 0.00000000000000006123233995736766 1.0)"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_common_lisp_two_argument_arctangent(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate("(list (atan 1 0) (atan 1 -1))").to_string(),
        "(1.5707963267948966 2.356194490192345)"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_common_lisp_complex_logarithm(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate("(log (complex -1 0))").to_string(),
        "#C(0.0 3.141592653589793)"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_common_lisp_complex_trigonometry(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate("(list (sin (complex 0 1)) (cos (complex 0 1)) (tan (complex 0 1)))").to_string(),
        "(#C(0.0 1.1752011936438014) #C(1.5430806348152437 -0.0) #C(0.0 0.7615941559557649))"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_common_lisp_complex_inverse_trigonometry(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate("(list (asin (complex 0 1)) (acos (complex 0 1)) (atan (complex 0 0)))")
            .to_string(),
        "(#C(0.0 0.8813735870195428) #C(1.5707963267948966 -0.8813735870195428) #C(0.0 0.0))"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_common_lisp_float_decoding(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(list (multiple-value-list (decode-float 1.5)) (multiple-value-list (integer-decode-float 1.5)) (multiple-value-list (decode-float -0.0)))"
        )
        .to_string(),
        "((0.75 1 1.0) (6755399441055744 -52 1) (-0.0 0 -1.0))"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_integer_bit_operations_on_bignums(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(let ((big (expt 2 70)))
                (list (logand big 3) (logior big 3) (logxor big 3)
                      (lognot big) (logtest big 1)
                      (ash big 2) (ash big -70)
                      (logcount big) (logcount (- big))
                      (integer-length big) (integer-length (- big))))",
        )
        .to_string(),
        "(0 1180591620717411303427 1180591620717411303427 -1180591620717411303425 NIL 4722366482869645213696 1 1 70 71 70)"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_integer_remainders_on_bignums(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(let ((big (expt 2 70)))
                (list (mod (- big) 3) (rem (- big) 3)))",
        )
        .to_string(),
        "(2 -1)"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_bignum_exponents_exactly(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate("(list (expt 1 (expt 2 63)) (integerp (expt 1 (expt 2 63))))").to_string(),
        "(1 T)"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_common_lisp_quotients_gcd_and_rational_parts(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
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
                        (multiple-value-bind (q r) (floor (expt 2 70) 3) (list q r))
                        (multiple-value-bind (q r) (ceiling (- (expt 2 70)) 3) (list q r))
                        (multiple-value-bind (q r) (truncate (expt 2 70) 3) (list q r))
                        (multiple-value-bind (q r) (round (expt 2 70) 3) (list q r))
                        (gcd 18 -24 30) (gcd) (lcm 6 -8 15) (lcm)
                        (gcd (expt 2 70) 6) (lcm (expt 2 70) 3)
                        (numerator -6/8) (denominator -6/8)
                        (numerator 7) (denominator 7))",
        )
        .to_string(),
        "((2 1) (-3 2) (-2 -1) (-2 -1) (2 1) (4 -1) (-3 2/3) (3 -2/3) (1 1.5) (2 0.5) (393530540239137101141 1) (-393530540239137101141 -1) (393530540239137101141 1) (393530540239137101141 1) 6 0 120 1 2 3541774862152233910272 -3 4 7 1)"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_common_lisp_signum_and_rationalize(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
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

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_common_lisp_sqrt_across_exact_and_float_numbers(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(list (sqrt 0) (sqrt 4) (sqrt 1/4)
                        (rationalp (sqrt 2)) (floatp (sqrt 2))
                        (= (sqrt 4.0) 2.0)
                        (sqrt (expt 2 100)) (typep (sqrt (expt 2 100)) 'fixnum)
                        (floatp (sqrt (expt 2 101))))",
        )
        .to_string(),
        "(0 2 1/2 NIL T T 1125899906842624 T T)"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_common_lisp_integer_square_root(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate("(list (isqrt 0) (isqrt 15) (isqrt (expt 2 100)))").to_string(),
        "(0 3 1125899906842624)"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_common_lisp_abs_across_numeric_types(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(list (abs -5) (abs 5) (abs -1/2) (abs 2.5)
                        (abs (expt 2 100)) (abs (- (expt 2 100))))",
        )
        .to_string(),
        "(5 5 1/2 2.5 1267650600228229401496703205376 1267650600228229401496703205376)"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_compound_type_designators(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
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

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_dollar_float_format_directive(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
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

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_exponential_float_format_directive(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
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

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_fixed_float_format_directive(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
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

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_format_case_conversion_directive(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
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

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_format_choice_directives(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
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

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_format_choice_parameters(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
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

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_format_conditional_newline_directive(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
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

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_format_newline_directive(#[case] eval_fn: EvalFn) {
    assert_eq!(
        evaluate_with(
            eval_fn,
            r#"(format nil "a~
b")"#
        )
        .to_string(),
        r#""ab""#
    );
    assert_eq!(
        evaluate_with(
            eval_fn,
            r#"(format nil "a~:
b")"#
        )
        .to_string(),
        r#""a\nb""#
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_format_escape_upward_directive(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
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

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_format_iteration_directives(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
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

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_format_justification_directive(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
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

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_format_recursive_processing_directive(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
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

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_format_tabulation_modifiers(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
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

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_format_write_directive(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
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

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_function_and_macro_introspection(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
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

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_function_namespace_mutation(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
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

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_general_float_format_directive(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
            evaluate(r#"(format nil "~G|~,3G|~10,3G|~10,3G|~10,3,0G|~10,3,1G|~10,3,2G|~@G" 12.3456 1.25 12.3456 0.0123456 12.3456 12.3456 12.3456 1.25)"#)
                .to_string(),
            r#""12.3456    |1.25    |  12.3    |  1.235e-2|    12.3  |   12.3   |  12.3    |+1.25    ""#,
        );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_load_time_value(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
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
fn load_time_value_cache_does_not_cross_source_evaluations() {
    let runtime = Runtime::new();
    let Ok(_) = runtime.eval_source("(defvar *load-time-value-source-counter* 0)") else {
        panic!("counter definition should succeed");
    };
    let Ok(first) =
        runtime.eval_source("(load-time-value (incf *load-time-value-source-counter*))")
    else {
        panic!("first source evaluation should succeed");
    };
    assert_eq!(first[0].to_string(), "1");
    let Ok(second) =
        runtime.eval_source("(load-time-value (incf *load-time-value-source-counter*))")
    else {
        panic!("second source evaluation should succeed");
    };
    assert_eq!(second[0].to_string(), "2");
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_nth_value(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
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

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_parameterized_format_directives(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
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

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_parse_integer(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
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

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_plural_format_directive(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(r#"(format nil "~P|~P|~@P|~@P" 1 2 1 2)"#).to_string(),
        r#""|s|y|ies""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "~D~:P|~D~:@P" 1 2)"#).to_string(),
        r#""1|2ies""#,
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_print_variants_to_string_stream(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
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

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_read_from_string(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
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

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_read_from_string_options(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
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

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_read_from_string_stream(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
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

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_read_whitespace_consumption(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
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

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_sequence_construction_and_coercion(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
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

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_sequence_operations_and_type_predicates(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
            evaluate("(list (first '(a b c)) (rest '(a b c)) (nth 1 '(a b c)) (elt \"abc\" 1) (subseq '(a b c d) 1 3) (subseq \"abcd\" 1 3) (member 'b '(a b c)) (assoc 'b '((a 1) (b 2))) (getf '(:a 1 :b 2) :b) (length \"abc\"))").to_string(),
            "(A (B C) B #\\b (B C) \"bc\" (B C) (B 2) 2 3)"
        );
    assert_eq!(
            evaluate("(list (typep 1 'integer) (typep \"abc\" 'sequence) (characterp #\\a) (keywordp :x) (vectorp #(1 2)) (bit-vector-p #(0 1 1)) (simple-bit-vector-p #(0 1 1)) (bit-vector-p #(0 2)) (endp nil) (endp '(1)))").to_string(),
            "(T T T T T T T NIL T NIL)"
        );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_write_escape_options(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
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

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_write_to_stream(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
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

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_write_to_string(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
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
