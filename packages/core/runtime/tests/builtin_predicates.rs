use ncl_runtime::Runtime;

#[test]
fn predicates_are_checked_through_a_shared_examples_table() {
    let cases = [
        ("(null nil)", "T"),
        ("(null 1)", "NIL"),
        ("(null ())", "T"),
        ("(atom 1)", "T"),
        ("(atom '(a))", "NIL"),
        ("(consp '(a))", "T"),
        ("(consp nil)", "NIL"),
        ("(listp nil)", "T"),
        ("(listp #(1))", "NIL"),
        ("(numberp 1/2)", "T"),
        ("(numberp 'number)", "NIL"),
        ("(integerp 1)", "T"),
        ("(integerp 1.0)", "NIL"),
        ("(floatp 1.0)", "T"),
        ("(floatp 1)", "NIL"),
        ("(rationalp 1/2)", "T"),
        ("(rationalp 1.0)", "NIL"),
        ("(stringp \"text\")", "T"),
        ("(stringp 'text)", "NIL"),
        ("(simple-string-p \"text\")", "T"),
        ("(simple-string-p '(text))", "NIL"),
        ("(characterp #\\A)", "T"),
        ("(characterp \"A\")", "NIL"),
        ("(symbolp 'name)", "T"),
        ("(symbolp \"name\")", "NIL"),
        ("(keywordp :name)", "T"),
        ("(keywordp 'name)", "NIL"),
        ("(vectorp #(1 2))", "T"),
        ("(vectorp '(1 2))", "NIL"),
        ("(simple-vector-p #(1 2))", "T"),
        ("(simple-vector-p '(1 2))", "NIL"),
        ("(functionp #'car)", "T"),
        ("(functionp 1)", "NIL"),
        ("(eq 'name 'name)", "T"),
        ("(eq 'name 'other)", "NIL"),
        ("(eql 1 1)", "T"),
        ("(eql 1 2)", "NIL"),
        ("(equal '(a) '(a))", "T"),
        ("(equal '(a) '(b))", "NIL"),
        ("(equalp \"Text\" \"text\")", "T"),
        ("(equalp \"Text\" \"other\")", "NIL"),
    ];

    for (source, expected) in cases {
        let actual = Runtime::new()
            .eval_source(source)
            .unwrap_or_else(|error| panic!("{source}: {error}"));
        assert_eq!(actual[0].to_string(), expected, "{source}");
    }
}

#[test]
fn subtype_relations_are_checked_through_a_shared_examples_table() {
    let cases = [
        (
            "(multiple-value-list (subtypep 'integer 'integer))",
            "(T T)",
        ),
        (
            "(multiple-value-list (subtypep '(or integer string) 'object))",
            "(T T)",
        ),
        (
            "(multiple-value-list (subtypep '(member 1 2) 'integer))",
            "(T T)",
        ),
        (
            "(multiple-value-list (subtypep '(eql 1) 'integer))",
            "(T T)",
        ),
        (
            "(multiple-value-list (subtypep '(integer 0 5) '(integer -1 10)))",
            "(T T)",
        ),
        (
            "(multiple-value-list (subtypep '(integer 0 10) '(integer 1 5)))",
            "(NIL T)",
        ),
        (
            "(multiple-value-list (subtypep '(or integer string) '(or integer string)))",
            "(T T)",
        ),
        (
            "(multiple-value-list (subtypep '(and integer number) '(and integer number)))",
            "(T T)",
        ),
        (
            "(multiple-value-list (subtypep '(not integer) '(not integer)))",
            "(T T)",
        ),
        (
            "(multiple-value-list (subtypep '(member 1 2) '(member 1 2)))",
            "(T T)",
        ),
        (
            "(multiple-value-list (subtypep '(mod 4) '(mod 4)))",
            "(T T)",
        ),
        (
            "(multiple-value-list (subtypep '(unsigned-byte 8) '(unsigned-byte 8)))",
            "(T T)",
        ),
        (
            "(multiple-value-list (subtypep '(signed-byte 8) '(signed-byte 8)))",
            "(T T)",
        ),
        (
            "(multiple-value-list (subtypep '(cons integer list) '(cons integer list)))",
            "(T T)",
        ),
        (
            "(multiple-value-list (subtypep '(vector integer 2) '(vector integer 2)))",
            "(T T)",
        ),
        (
            "(multiple-value-list (subtypep '(simple-vector 2) '(simple-vector 2)))",
            "(T T)",
        ),
        (
            "(multiple-value-list (subtypep '(bit-vector 2) '(bit-vector 2)))",
            "(T T)",
        ),
        (
            "(multiple-value-list (subtypep '(array integer (2)) 'array))",
            "(T T)",
        ),
    ];

    for (source, expected) in cases {
        let actual = Runtime::new()
            .eval_source(source)
            .unwrap_or_else(|error| panic!("{source}: {error}"));
        assert_eq!(actual[0].to_string(), expected, "{source}");
    }
}
