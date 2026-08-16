use super::{Runtime, evaluate};

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
fn evaluates_bit_vector_dispatch_literals() {
    assert_eq!(
        evaluate("(list #*101 (typep #*101 'bit-vector) (aref #*101 1))").to_string(),
        "(#(1 0 1) T 0)"
    );
}

#[test]
fn evaluates_radix_integer_dispatch_literals() {
    assert_eq!(
        evaluate("(list #b1010 #o17 #xff #b-11)").to_string(),
        "(10 15 255 -3)"
    );
}

#[test]
fn evaluates_complex_literal_dispatch() {
    assert_eq!(
        evaluate(
            "(list (complexp #C(1 2))
                   (realpart #C(1 2))
                   (imagpart #C(1 2))
                   (typep #C(1 2) 'number)
                   (typep #C(1 2) 'complex)
                   (typep #C(1 2) 'real)
                   (realpart 3)
                   (imagpart 3.0))",
        )
        .to_string(),
        "(T 1 2 T T NIL 3 0.0)",
    );
}

#[test]
fn evaluates_complex_arithmetic() {
    assert_eq!(
        evaluate(
            "(list (+ #C(1 2) 3)
                   (- #C(1 2) #C(3 4))
                   (* #C(1 2) #C(3 4))
                   (/ #C(1 2) #C(3 -4))
                   (= #C(1 2) #C(1 2) #C(1 2))
                   (= #C(1 2) 3)
                   (= #C(3 0) 3))",
        )
        .to_string(),
        "(#C(4 2) #C(-2 -2) #C(-5 10) #C(-1/5 2/5) T NIL T)",
    );
}

#[test]
fn evaluates_complex_polar_operations() {
    assert_eq!(
        evaluate(
            "(list (conjugate #C(3 4))
                   (conjugate 5)
                   (phase 1)
                   (phase -1)
                   (phase #C(0 2))
                   (phase #C(0 0)))",
        )
        .to_string(),
        "(#C(3 -4) 5 0 3.141592653589793 1.5707963267948966 0)",
    );
}

#[test]
fn evaluates_complex_exponential_and_logarithm_operations() {
    assert_eq!(
        evaluate(
            "(list (exp 0)
                   (exp #C(0 3.141592653589793))
                   (log 1)
                   (log -1)
                   (log 8 2)
                   (cis 0)
                   (cis 3.141592653589793))",
        )
        .to_string(),
        "(1.0 #C(-1.0 0) 0 #C(0 3.141592653589793) 3.0 #C(1.0 0) #C(-1.0 0))",
    );
}

#[test]
fn evaluates_complex_trigonometric_operations() {
    assert_eq!(
        evaluate(
            "(list (sin 0)
                   (cos 0)
                   (tan 0)
                   (asin 0)
                   (asin 1)
                   (acos 1)
                   (atan 0)
                   (atan 1)
                   (atan 0 1)
                   (atan 1 1)
                   (atan -1 1)
                   (atan 0 -1)
                   (sinh 0)
                   (cosh 0)
                   (tanh 0)
                   (asinh 0)
                   (asinh 1)
                   (acosh 1)
                   (atanh 0)
                   (atanh 0.5)
                   (sin #C(0 1))
                   (cos #C(0 1))
                   (tan #C(0 1))
                   (asin #C(0 1))
                   (acos #C(0 1))
                   (atan #C(0 0.5))
                   (sinh #C(0 1))
                   (cosh #C(0 1))
                   (tanh #C(0 1))
                   (asinh #C(0 1))
                   (acosh #C(0 1))
                   (atanh #C(0 0.5))
                   (sin #C(1 0))
                   (cos #C(1 0))
                   (sinh #C(1 0))
                   (cosh #C(1 0)))",
        )
        .to_string(),
        "(0.0 1.0 0.0 0 1.5707963267948966 0 0 0.7853981633974483 0 0.7853981633974483 -0.7853981633974483 3.141592653589793 0.0 1.0 0.0 0 0.8813735870195429 0 0 0.5493061443340548 #C(0 1.1752011936438014) 1.5430806348152437 #C(0.0 0.7615941559557649) #C(0 0.8813735870195428) #C(1.5707963267948966 -0.8813735870195428) #C(0 0.5493061443340548) #C(0 0.8414709848078965) 0.5403023058681398 #C(0.0 1.557407724654902) #C(0 1.5707963267948966) #C(0.8813735870195432 1.5707963267948966) #C(0 0.4636476090008061) 0.8414709848078965 0.5403023058681398 1.1752011936438014 1.5430806348152437)",
    );
}

#[test]
fn evaluates_array_literal_dispatch() {
    assert_eq!(
        evaluate(
            "(list (array-dimensions #2A((1 2) (3 4)))
                   (aref #2A((1 2) (3 4)) 1 0)
                   (aref #1A(5 6 7) 2))",
        )
        .to_string(),
        "((2 2) 3 7)"
    );
}

#[test]
fn evaluates_subtypep() {
    let values = Runtime::new()
        .eval_source(
            r#"(progn
                 (defclass subtypep-parent () ())
                 (defclass subtypep-child (subtypep-parent) ())
                 (defstruct subtypep-record value)
                 (list
                   (multiple-value-list (subtypep 'integer 'number))
                   (multiple-value-list (subtypep '(integer 0 5) '(integer -1 10)))
                   (multiple-value-list (subtypep '(integer 0 10) '(integer 1 5)))
                   (multiple-value-list (subtypep 'subtypep-child 'subtypep-parent))
                   (multiple-value-list (subtypep 'subtypep-record 'structure))
                   (multiple-value-list (subtypep 'string 'sequence))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].to_string(),
        "((T T) (T T) (NIL T) (T T) (T T) (T T))"
    );
}

#[test]
fn evaluates_sequence_construction_and_coercion() {
    assert_eq!(
        evaluate(
            "(list (make-sequence 'list 3)
                    (make-sequence 'vector 2 :initial-element 7)
                    (make-sequence 'string 3 :initial-element #\\x)
                    (make-sequence 'base-string 2 :initial-element #\\y)
                    (coerce '(1 2) 'vector)
                    (coerce #(1 2) 'list)
                    (coerce '(#\\a #\\b) 'string)
                    (coerce '(#\\c #\\d) 'base-string)
                    (coerce 'foo 'string)
                    (coerce 'bar 'base-string)
                    (simple-string-p \"abc\"))",
        )
        .to_string(),
        "((NIL NIL NIL) #(7 7) \"xxx\" \"yy\" #(1 2) (1 2) \"ab\" \"cd\" \"FOO\" \"BAR\" T)"
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
