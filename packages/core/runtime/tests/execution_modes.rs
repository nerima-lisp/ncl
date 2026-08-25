use ncl_runtime::{Runtime, RuntimeError, Value};

type Evaluate = fn(&Runtime, &str) -> Result<Vec<Value>, RuntimeError>;

fn interpreted(runtime: &Runtime, source: &str) -> Result<Vec<Value>, RuntimeError> {
    runtime.eval_source(source)
}

fn compiled(runtime: &Runtime, source: &str) -> Result<Vec<Value>, RuntimeError> {
    runtime.eval_compiled_source(source)
}

#[test]
fn execution_modes_agree_on_core_language_cases() {
    let cases = [
        ("(+ 1 2 3)", "6"),
        ("(let ((x 4)) (* x x))", "16"),
        ("(if (> 3 2) :yes :no)", ":YES"),
        ("(mapcar #'1+ '(1 2 3))", "(2 3 4)"),
        ("(values 7 8)", "7"),
        (
            "(list (list* 1 2 3) (nthcdr 1 '(a . b)) (cdr nil))",
            "((1 2 . 3) B NIL)",
        ),
        (
            "(let ((array (make-array '(2 2) :initial-element 0)))\n               (setf (aref array 1 0) 7)\n               (list (array-dimensions array) (aref array 1 0)\n                     (row-major-aref array 2)))",
            "((2 2) 7 7)",
        ),
        (
            "(let ((table (make-hash-table :test #'equal)))\n               (setf (gethash '(key) table) 9)\n               (multiple-value-list (gethash '(key) table)))",
            "(9 T)",
        ),
        (
            "(list (butlast '(1 2 3) 1) (nth 1 '(a b c))\n                    (member 'b '(a b c)))",
            "((1 2) B (B C))",
        ),
        (
            "(list (subseq '(a b c d) 1 3)\n                    (fill 7 #(0 0 0) :start 1 :end 3)\n                    (replace '(a b c) '(x y) :start1 1)\n                    (copy-seq \"abc\"))",
            "((B C) #(0 7 7) (A X Y) \"abc\")",
        ),
        (
            "(list (concatenate 'list '(a b) #(c d))\n                    (concatenate 'vector '(a b) \"cd\")\n                    (make-sequence 'string 3 :initial-element #\\x)\n                    (coerce \"abc\" 'list))",
            "((A B C D) #(A B #\\c #\\d) \"xxx\" (#\\a #\\b #\\c))",
        ),
        (
            "(list (array-rank #(1 2)) (array-dimensions #(1 2))\n                    (array-total-size (make-array '(2 3)))\n                    (array-in-bounds-p (make-array 2) 1)\n                    (array-element-type #(1 2)))",
            "(1 (2) 6 T T)",
        ),
        (
            "(list (make-list 2 :initial-element 'x)\n                    (values-list '(a b))\n                    (list-length '(a b c))\n                    (acons 'k 9 '((a . 1)))\n                    (pairlis '(x y) '(1 2)))",
            "((X X) A 3 ((K . 9) (A . 1)) ((Y . 2) (X . 1)))",
        ),
        (
            "(list (reverse '(a b c)) (last '(a b c))\n                    (copy-list '(a b)) (copy-alist '((a . 1)))\n                    (copy-tree '(a (b c))))",
            "((C B A) (C) (A B) ((A . 1)) (A (B C)))",
        ),
        (
            "(list (append '(a b) '(c d))\n                    (revappend '(a b) '(c d))\n                    (nreconc '(a b) '(c d)))",
            "((A B C D) (B A C D) (B A C D))",
        ),
        (
            "(let ((array (make-array 3 :initial-element 0))\n                  (table (make-hash-table)))\n               (setf (svref array 1) 4)\n               (setf (gethash 'a table) 7)\n               (list (vector 1 2 3) (svref array 1)\n                     (array-row-major-index array 2)\n                     (hash-table-count table)\n                     (hash-table-p table)\n                     (remhash 'a table)\n                     (clrhash table)))",
            "(#(1 2 3) 4 2 1 T T #<HASH-TABLE EQL>)",
        ),
        (
            "(list (arrayp #(0 1)) (simple-array-p #(0 1))\n                    (array-dimension #(0 1) 0))",
            "(T T 2)",
        ),
        (
            "(list (subseq \"abcd\" 1 3)\n                    (fill #\\x \"abc\" :start 1)\n                    (replace \"abc\" \"XY\" :start1 1)\n                    (coerce '(#\\a #\\b) 'string)\n                    (make-sequence 'vector 2 :initial-element 9))",
            "(\"bc\" \"axx\" \"aXY\" \"ab\" #(9 9))",
        ),
        (
            "(list (list* 1 '(2 3))\n                    (append '() '(a . b))\n                    (nthcdr 2 '(a b . c))\n                    (cdr '(a . b)))",
            "((1 2 3) (A . B) C B)",
        ),
    ];
    let modes: [(&str, Evaluate); 2] = [("interpreted", interpreted), ("compiled", compiled)];

    for (source, expected) in cases {
        for (mode, evaluate) in modes {
            let runtime = Runtime::new();
            let values = evaluate(&runtime, source)
                .unwrap_or_else(|error| panic!("{mode} evaluation failed for {source}: {error}"));
            assert_eq!(
                values[0].to_string(),
                expected,
                "mode={mode}, source={source}"
            );
        }
    }
}

#[test]
fn execution_modes_agree_on_type_and_arity_errors() {
    let cases = [
        ("(+ 1 nil)", "type error"),
        ("(car 1)", "type error"),
        ("(=)", "arity error"),
    ];
    let modes: [(&str, Evaluate); 2] = [("interpreted", interpreted), ("compiled", compiled)];

    for (source, description) in cases {
        let mut errors = Vec::new();
        for (mode, evaluate) in modes {
            let runtime = Runtime::new();
            let error = match evaluate(&runtime, source) {
                Ok(values) => panic!("{mode} unexpectedly succeeded for {source}: {values:?}"),
                Err(error) => error,
            };
            errors.push((mode, error));
        }

        assert_eq!(
            errors[0].1.to_string(),
            errors[1].1.to_string(),
            "{description}: {source}"
        );
    }
}

#[test]
fn execution_modes_agree_on_collection_boundary_errors() {
    let cases = [
        "(aref #(1) 2)",
        "(row-major-aref (make-array 2) 2)",
        "(gethash 1 42)",
        "(elt 1 0)",
        "(subseq '(a b) 2 1)",
        "(fill 1 #(0) :start 2)",
        "(replace #(1) #(2) :start1 2)",
        "(make-sequence 'unknown 1)",
    ];
    let modes: [(&str, Evaluate); 2] = [("interpreted", interpreted), ("compiled", compiled)];

    for source in cases {
        let mut errors = Vec::new();
        for (mode, evaluate) in modes {
            let runtime = Runtime::new();
            let error = match evaluate(&runtime, source) {
                Ok(values) => panic!("{mode} unexpectedly succeeded for {source}: {values:?}"),
                Err(error) => error,
            };
            errors.push(error.to_string());
        }
        assert_eq!(errors[0], errors[1], "source={source}");
    }
}

#[test]
fn execution_modes_agree_on_lambda_list_shapes() {
    let cases = [
        (
            "((lambda (required &optional (optional 2)) (list required optional)) 1)",
            "(1 2)",
        ),
        (
            "((lambda (required &optional (optional 2)) (list required optional)) 1 9)",
            "(1 9)",
        ),
        ("((lambda (&rest values) values) 1 2 3)", "(1 2 3)"),
        (
            "((lambda (&key (left 1) (right 2)) (list left right)) :right 8)",
            "(1 8)",
        ),
        (
            "((lambda (&key (left 1) &allow-other-keys) left) :other 8)",
            "1",
        ),
        ("((lambda (&aux (computed (+ 2 3))) computed))", "5"),
    ];
    let modes: [(&str, Evaluate); 2] = [("interpreted", interpreted), ("compiled", compiled)];

    for (source, expected) in cases {
        for (mode, evaluate) in modes {
            let runtime = Runtime::new();
            let values = evaluate(&runtime, source)
                .unwrap_or_else(|error| panic!("{mode} evaluation failed for {source}: {error}"));
            assert_eq!(
                values[0].to_string(),
                expected,
                "mode={mode}, source={source}"
            );
        }
    }
}

#[test]
fn execution_modes_agree_on_lambda_list_errors() {
    let cases = [
        "((lambda (required) required))",
        "((lambda (required) required) 1 2)",
        "((lambda (&key known) known) :unknown 1)",
        "((lambda (&key known) known) :known)",
        "((lambda (&key known) known) 1 2)",
    ];
    let modes: [(&str, Evaluate); 2] = [("interpreted", interpreted), ("compiled", compiled)];

    for source in cases {
        let mut errors = Vec::new();
        for (mode, evaluate) in modes {
            let runtime = Runtime::new();
            let error = match evaluate(&runtime, source) {
                Ok(values) => panic!("{mode} unexpectedly succeeded for {source}: {values:?}"),
                Err(error) => error,
            };
            errors.push(error.to_string());
        }
        assert!(
            errors
                .iter()
                .all(|error| error.contains("argument") || error.contains("keyword")),
            "both modes must report a lambda-list argument error for {source}: {errors:?}"
        );
    }
}

#[test]
fn execution_modes_agree_on_setf_updates() {
    let cases = [
        (
            "(let ((xs (list 1 2 3))) (setf (car xs) 9 (nth 2 xs) 7) xs)",
            "(9 2 7)",
        ),
        (
            "(let ((text \"abc\")) (setf (char text 1) #\\X) text)",
            "\"aXc\"",
        ),
        (
            "(let ((array (make-array '(2 2)))) (setf (aref array 1 0) 7) (aref array 1 0))",
            "7",
        ),
        (
            "(let ((table (make-hash-table :test #'equal))) (setf (gethash '(key) table) 9) (gethash '(key) table))",
            "9",
        ),
        (
            "(let ((plist (list :key 1))) (setf (getf plist :key) 2) plist)",
            "(:KEY 2)",
        ),
    ];
    let modes: [(&str, Evaluate); 2] = [("interpreted", interpreted), ("compiled", compiled)];

    for (source, expected) in cases {
        for (mode, evaluate) in modes {
            let runtime = Runtime::new();
            let values = evaluate(&runtime, source)
                .unwrap_or_else(|error| panic!("{mode} evaluation failed for {source}: {error}"));
            assert_eq!(
                values[0].to_string(),
                expected,
                "mode={mode}, source={source}"
            );
        }
    }
}

#[test]
fn execution_modes_agree_on_setf_errors() {
    let cases = [
        "(let ((xs nil)) (setf (car xs) 1))",
        "(let ((xs (list 1 2))) (setf (nth 2 xs) 1))",
        "(let ((text \"abc\")) (setf (char text 1) 1))",
        "(let ((array (make-array 2))) (setf (aref array 2) 1))",
        "(let ((xs (list 1 2))) (setf (nth -1 xs) 1))",
    ];
    let modes: [(&str, Evaluate); 2] = [("interpreted", interpreted), ("compiled", compiled)];

    for source in cases {
        let mut errors = Vec::new();
        for (mode, evaluate) in modes {
            let runtime = Runtime::new();
            let error = match evaluate(&runtime, source) {
                Ok(values) => panic!("{mode} unexpectedly succeeded for {source}: {values:?}"),
                Err(error) => error,
            };
            errors.push(error.to_string());
        }
        assert_eq!(errors[0], errors[1], "source={source}");
    }
}
