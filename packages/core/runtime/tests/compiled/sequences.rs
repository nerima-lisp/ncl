use super::{Instruction, Runtime, RuntimeError, evaluate};

#[test]
fn compiled_arithmetic_reports_overflow_and_comparisons_require_an_argument() {
    let overflow = Runtime::new()
        .eval_compiled_source("(let ((x 9223372036854775807)) (+ x 1))")
        .unwrap_err();
    assert!(matches!(overflow, RuntimeError::NumericOverflow));

    let comparison_error = Runtime::new().eval_compiled_source("(=)").unwrap_err();
    assert!(matches!(
        comparison_error,
        RuntimeError::Arity {
            function,
            actual: 0,
            ..
        } if function == "="
    ));
}

#[test]
fn compiled_evaluates_forms_and_maps_functions_over_lists() {
    assert_eq!(evaluate("(eval '(+ 2 3))").to_string(), "5");
    assert_eq!(
        evaluate("(let ((form '(+ 2 3))) (eval form))").to_string(),
        "5"
    );
    assert_eq!(evaluate("(funcall #'eval '(+ 2 3))").to_string(), "5");
    assert_eq!(
        evaluate("(mapcar (lambda (x) (* x 2)) '(1 2 3))").to_string(),
        "(2 4 6)"
    );
    assert_eq!(
        evaluate("(mapcar (lambda (x y) (+ x y)) '(1 2) '(10 20 30))").to_string(),
        "(11 22)"
    );
    assert_eq!(
        evaluate("(funcall #'mapcar (lambda (x) (+ x 1)) '(1 2 3))").to_string(),
        "(2 3 4)"
    );
    assert_eq!(evaluate("(funcall 'car '(9 8))").to_string(), "9");
    assert_eq!(evaluate("(apply 'list 1 '(2 3))").to_string(), "(1 2 3)");
    assert_eq!(
        evaluate("(mapcar 'car '((1 2) (3 4)))").to_string(),
        "(1 3)"
    );
    assert_eq!(
        evaluate("(let ((function-name 'car)) (funcall function-name '(7 6)))").to_string(),
        "7"
    );
    assert_eq!(
        evaluate("(mapc (lambda (x) (* x 2)) '(1 2 3))").to_string(),
        "(1 2 3)"
    );
    assert_eq!(
        evaluate("(mapl (lambda (tail) (car tail)) '(1 2 3))").to_string(),
        "(1 2 3)"
    );
    assert_eq!(
        evaluate("(maplist (lambda (tail) (car tail)) '(1 2 3))").to_string(),
        "(1 2 3)"
    );
    assert_eq!(
        evaluate(
            "(maplist (lambda (left right) (list (car left) (car right)))
                      '(1 2) '(10 20 30))",
        )
        .to_string(),
        "((1 10) (2 20))"
    );
    assert_eq!(
        evaluate("(mapcan (lambda (x) (list x (* x 10))) '(1 2 3))").to_string(),
        "(1 10 2 20 3 30)"
    );
    assert_eq!(
        evaluate("(mapcon (lambda (tail) (list (car tail))) '(1 2 3))").to_string(),
        "(1 2 3)"
    );
}

#[test]
fn compiled_evaluates_map_over_sequence_types() {
    assert_eq!(
        evaluate("(map 'list (lambda (x) (* x 2)) '(1 2 3))").to_string(),
        "(2 4 6)"
    );
    assert_eq!(
        evaluate("(map 'vector #'1+ #(1 2 3))").to_string(),
        "#(2 3 4)"
    );
    assert_eq!(
        evaluate("(map 'string #'identity \"abc\")").to_string(),
        "\"abc\""
    );
    assert_eq!(
        evaluate("(map 'base-string #'identity \"abc\")").to_string(),
        "\"abc\""
    );
    assert_eq!(
        evaluate("(map 'list #'+ '(1 2) '(10 20 30))").to_string(),
        "(11 22)"
    );
    assert_eq!(
        evaluate(
            "(let ((total 0))
               (map nil (lambda (x) (incf total x)) '(1 2 3))
               total)",
        )
        .to_string(),
        "6"
    );
}

#[test]
fn compiled_evaluates_reduce_over_sequences() {
    assert_eq!(evaluate("(reduce #'+ '(1 2 3 4))").to_string(), "10");
    assert_eq!(
        evaluate("(reduce #'- '(1 2 3) :from-end t)").to_string(),
        "2"
    );
    assert_eq!(
        evaluate("(reduce #'+ '(1 2 3) :initial-value 10)").to_string(),
        "16"
    );
    assert_eq!(
        evaluate("(reduce #'+ '(1 2 3 4) :start 1 :end 3)").to_string(),
        "5"
    );
    assert_eq!(
        evaluate("(reduce #'+ '((1) (2) (3)) :key #'car)").to_string(),
        "6"
    );
    assert_eq!(
        evaluate("(reduce #'+ \"abc\" :key #'char-code)").to_string(),
        "294"
    );
    assert_eq!(
        evaluate("(reduce #'list '() :initial-value 42)").to_string(),
        "42"
    );
}

#[test]
fn compiled_evaluates_sequence_searches() {
    assert_eq!(evaluate("(find 2 '(1 2 3))").to_string(), "2");
    assert_eq!(evaluate("(position 2 #(1 2 3))").to_string(), "1");
    assert_eq!(evaluate("(count 2 '(1 2 2 3))").to_string(), "2");
    assert_eq!(
        evaluate("(position 2 '(1 2 3 2) :from-end t)").to_string(),
        "3"
    );
    assert_eq!(
        evaluate(
            "(find 2 '(1 2 3) :test-not (lambda (wanted candidate)\n               (= wanted (+ candidate 1))))",
        )
        .to_string(),
        "2"
    );
    assert_eq!(
        evaluate("(position 20 '(10 20 30) :start 1 :end 3)").to_string(),
        "1"
    );
    assert_eq!(
        evaluate("(find 2 '((1) (2) (3)) :key #'car)").to_string(),
        "(2)"
    );
    assert_eq!(evaluate("(count 2 '(1 2 3 2) :key #'1+)").to_string(), "1");
    assert_eq!(evaluate("(find 9 '(1 2 3))").to_string(), "NIL");
}

#[test]
fn compiled_evaluates_sequence_search_and_mismatch() {
    assert_eq!(evaluate("(search '(2 3) '(1 2 3 4))").to_string(), "1");
    assert_eq!(
        evaluate("(search '(2 3) '(1 2 3 2 3) :from-end t)").to_string(),
        "3"
    );
    assert_eq!(
        evaluate("(search '(0 1) '(2 4 6 1 3 5) :key #'oddp)").to_string(),
        "2"
    );
    assert_eq!(
        evaluate("(search \"ab\" \"xxABab\" :test #'char-equal :from-end t)").to_string(),
        "4"
    );
    assert_eq!(
        evaluate("(search '(2 3) '(0 1 2 3 4) :start2 2 :end2 5)").to_string(),
        "2"
    );
    assert_eq!(evaluate("(search '() '(1 2) :start2 1)").to_string(), "1");
    assert_eq!(
        evaluate("(search '() '(1 2) :start2 1 :from-end t)").to_string(),
        "2"
    );
    assert_eq!(evaluate("(mismatch '(1 2 9) '(1 2 3))").to_string(), "2");
    assert_eq!(
        evaluate("(mismatch '(3 2 1 1 2 3) '(1 2 3) :from-end t)").to_string(),
        "3"
    );
    assert_eq!(
        evaluate("(mismatch \"abcd\" \"ABCDE\" :test #'char-equal)").to_string(),
        "4"
    );
    assert_eq!(
        evaluate("(mismatch '(1 2 3) '(2 3 4) :test-not #'eq :key #'oddp)").to_string(),
        "NIL"
    );
    assert_eq!(
        evaluate("(mismatch \"def\" \"abcdef\" :from-end t)").to_string(),
        "0"
    );
    assert_eq!(evaluate("(funcall #'search '(2) '(0 2))").to_string(), "1");
}

#[test]
fn compiled_evaluates_sequence_sort_and_stable_sort() {
    assert_eq!(evaluate("(sort '(3 1 2) #'<)").to_string(), "(1 2 3)");
    assert_eq!(
        evaluate("(stable-sort '(2 -2 1 -1) #'< :key #'abs)").to_string(),
        "(1 -1 2 -2)"
    );
    assert_eq!(evaluate("(sort #(3 1 2) #'<)").to_string(), "#(1 2 3)");
    assert_eq!(evaluate("(sort \"cba\" #'char<)").to_string(), "\"abc\"");
    assert_eq!(
        evaluate("(funcall #'stable-sort '(3 1 2) #'<)").to_string(),
        "(1 2 3)"
    );
}

#[test]
fn compiled_evaluates_sequence_merge() {
    assert_eq!(
        evaluate("(merge 'list '(1 3 5) '(2 4 6) #'<)").to_string(),
        "(1 2 3 4 5 6)"
    );
    assert_eq!(
        evaluate("(merge 'vector #(1 3) #(2 4) #'<)").to_string(),
        "#(1 2 3 4)"
    );
    assert_eq!(
        evaluate("(merge 'string \"ace\" \"bdf\" #'char<)").to_string(),
        "\"abcdef\""
    );
    assert_eq!(
        evaluate("(merge 'base-string \"ace\" \"bdf\" #'char<)").to_string(),
        "\"abcdef\""
    );
    assert_eq!(
        evaluate("(merge 'list '(-1 -3) '(2 4) #'< :key #'abs)").to_string(),
        "(-1 2 -3 4)"
    );
    assert_eq!(
        evaluate("(merge 'list '((1 a) (2 b)) '((1 c) (2 d)) #'< :key #'car)").to_string(),
        "((1 A) (1 C) (2 B) (2 D))"
    );
    assert_eq!(
        evaluate("(funcall #'merge 'list '(1 3) '(2 4) #'<)").to_string(),
        "(1 2 3 4)"
    );
}

#[test]
fn compiled_evaluates_sequence_quantifiers() {
    assert_eq!(evaluate("(every #'numberp '(1 2))").to_string(), "T");
    assert_eq!(evaluate("(every #'= '(1 2) #(1 2))").to_string(), "T");
    assert_eq!(evaluate("(some #'identity '(nil 2 4))").to_string(), "2");
    assert_eq!(evaluate("(notany #'evenp '(1 3 5))").to_string(), "T");
    assert_eq!(evaluate("(notevery #'evenp '(2 4 5))").to_string(), "T");
    assert_eq!(evaluate("(every #'char= \"ab\" \"ab\")").to_string(), "T");
    assert_eq!(evaluate("(every #'identity '())").to_string(), "T");
    assert_eq!(evaluate("(some #'identity '())").to_string(), "NIL");
    assert_eq!(
        evaluate("(funcall #'some #'identity '(nil 3))").to_string(),
        "3"
    );
}

#[test]
fn compiled_evaluates_list_membership_and_association_searches() {
    assert_eq!(evaluate("(member 2 '(1 2 3))").to_string(), "(2 3)");
    assert_eq!(
        evaluate("(member 2 '((1) (2) (3)) :key #'car)").to_string(),
        "((2) (3))"
    );
    assert_eq!(
        evaluate("(member-if-not #'evenp '(2 4 5 6))").to_string(),
        "(5 6)"
    );
    assert_eq!(evaluate("(adjoin 4 '(1 2 3))").to_string(), "(4 1 2 3)");
    assert_eq!(
        evaluate("(assoc 'b '((a . 1) (b . 2)))").to_string(),
        "(B . 2)"
    );
    assert_eq!(
        evaluate("(assoc-if (lambda (key) (eq key 'b)) '((a . 1) (b . 2)))").to_string(),
        "(B . 2)"
    );
    assert_eq!(
        evaluate("(rassoc-if #'evenp '((a . 1) (b . 2)))").to_string(),
        "(B . 2)"
    );
    assert_eq!(
        evaluate("(funcall #'member 2 '(1 2 3))").to_string(),
        "(2 3)"
    );
}

#[test]
fn compiled_evaluates_sequence_removals() {
    assert_eq!(evaluate("(remove 2 '(1 2 2 3))").to_string(), "(1 3)");
    assert_eq!(
        evaluate("(remove 2 '(1 2 3 2) :from-end t :count 1)").to_string(),
        "(1 2 3)"
    );
    assert_eq!(
        evaluate("(remove-if-not #'evenp '(1 2 4 3))").to_string(),
        "(2 4)"
    );
    assert_eq!(evaluate("(remove 2 #(1 2 3))").to_string(), "#(1 3)");
    assert_eq!(
        evaluate("(remove #\\a \"banana\" :count 2)").to_string(),
        "\"bnna\""
    );
    assert_eq!(
        evaluate("(remove-duplicates '(1 2 1 3 2) :from-end t)").to_string(),
        "(1 3 2)"
    );
    assert_eq!(
        evaluate("(delete-if #'evenp '(1 2 4 3))").to_string(),
        "(1 3)"
    );
    assert_eq!(
        evaluate("(funcall #'remove 2 '(1 2 3))").to_string(),
        "(1 3)"
    );
}

#[test]
fn compiled_evaluates_sequence_substitutions() {
    assert_eq!(
        evaluate("(substitute 9 2 '(1 2 2 3))").to_string(),
        "(1 9 9 3)"
    );
    assert_eq!(
        evaluate("(substitute 9 2 '(1 2 2 3) :from-end t :count 1)").to_string(),
        "(1 2 9 3)"
    );
    assert_eq!(
        evaluate("(substitute-if-not 0 #'evenp '(1 2 4 3))").to_string(),
        "(0 2 4 0)"
    );
    assert_eq!(
        evaluate("(substitute 0 2 #(1 2 3))").to_string(),
        "#(1 0 3)"
    );
    assert_eq!(
        evaluate("(substitute #\\x #\\a \"banana\" :count 2)").to_string(),
        "\"bxnxna\""
    );
    assert_eq!(
        evaluate("(substitute 9 2 '((1) (2) (2)) :key #'car :count 1)").to_string(),
        "((1) 9 (2))"
    );
    assert_eq!(
        evaluate("(nsubstitute 8 2 '(1 2 3))").to_string(),
        "(1 8 3)"
    );
    assert_eq!(
        evaluate("(funcall #'substitute 9 2 '(1 2 3))").to_string(),
        "(1 9 3)"
    );
}

#[test]
fn compiled_evaluates_list_set_operations() {
    assert_eq!(evaluate("(union '(1 2 2) '(2 3 3))").to_string(), "(1 2 3)");
    assert_eq!(
        evaluate("(intersection '(1 2 2 3) '(2 3 4))").to_string(),
        "(2 3)"
    );
    assert_eq!(
        evaluate("(set-difference '(1 2 2 3) '(2))").to_string(),
        "(1 3)"
    );
    assert_eq!(
        evaluate("(set-exclusive-or '(1 2 2 3) '(2 4))").to_string(),
        "(1 3 4)"
    );
    assert_eq!(evaluate("(subsetp '(1 2) '(3 2 1 4))").to_string(), "T");
    assert_eq!(
        evaluate("(union '(1 2) '(2 3) :test #'=)").to_string(),
        "(1 2 3)"
    );
    assert_eq!(
        evaluate("(union '((1 a) (2 b)) '((1 c) (3 d)) :key #'car)").to_string(),
        "((1 A) (2 B) (3 D))"
    );
    assert_eq!(evaluate("(nunion '(1 2) '(2 3))").to_string(), "(1 2 3)");
    assert_eq!(evaluate("(funcall #'union '(1) '(2))").to_string(), "(1 2)");
}

#[test]
fn compiled_evaluates_list_construction_and_partitioning() {
    assert_eq!(evaluate("(list* 1 2 '(3 4))").to_string(), "(1 2 3 4)");
    assert_eq!(evaluate("(list* 1 2 3)").to_string(), "(1 2 . 3)");
    assert_eq!(
        evaluate("(make-list 3 :initial-element 'x)").to_string(),
        "(X X X)"
    );
    assert_eq!(
        evaluate("(copy-tree '((1) (2 3)))").to_string(),
        "((1) (2 3))"
    );
    assert_eq!(evaluate("(list-length '(1 2 3))").to_string(), "3");
    assert_eq!(evaluate("(nthcdr 2 '(1 2 3))").to_string(), "(3)");
    assert_eq!(evaluate("(nthcdr 3 '(1 2 3))").to_string(), "NIL");
    assert_eq!(evaluate("(nthcdr 1 '(1 . 2))").to_string(), "2");
    assert_eq!(
        evaluate("(acons 'a 1 '((b . 2)))").to_string(),
        "((A . 1) (B . 2))"
    );
    assert_eq!(
        evaluate("(pairlis '(a b) '(1 2) '((c . 3)))").to_string(),
        "((B . 2) (A . 1) (C . 3))"
    );
    assert_eq!(
        evaluate("(copy-alist '((a . 1) (b 2)))").to_string(),
        "((A . 1) (B 2))"
    );
    assert_eq!(
        evaluate("(multiple-value-list (get-properties '(:a 1 :b 2) '(:b :a)))").to_string(),
        "(:A 1 (:A 1 :B 2))"
    );
    assert_eq!(
        evaluate("(multiple-value-list (get-properties '(:a 1) '(:z)))").to_string(),
        "(NIL NIL NIL)"
    );
    assert_eq!(evaluate("(last '(1 2 3) 2)").to_string(), "(2 3)");
    assert_eq!(evaluate("(butlast '(1 2 3))").to_string(), "(1 2)");
    assert_eq!(evaluate("(nreverse '(1 2 3))").to_string(), "(3 2 1)");
    assert_eq!(evaluate("(nconc '(1 2) '(3 4))").to_string(), "(1 2 3 4)");
    assert_eq!(
        evaluate("(revappend '(1 2) '(3 4))").to_string(),
        "(2 1 3 4)"
    );
    assert_eq!(
        evaluate("(funcall #'list* 1 '(2 3))").to_string(),
        "(1 2 3)"
    );
    assert_eq!(evaluate("(funcall #'nthcdr 1 '(4 5))").to_string(), "(5)");
}

#[test]
fn compiled_evaluates_sequence_fill_replace_and_concatenate() {
    assert_eq!(
        evaluate("(fill 0 '(1 2 3 4) :start 1 :end 3)").to_string(),
        "(1 0 0 4)"
    );
    assert_eq!(
        evaluate("(fill #\\x \"abcd\" :start 1)").to_string(),
        "\"axxx\""
    );
    assert_eq!(evaluate("(fill 9 #(1 2 3) :end 2)").to_string(), "#(9 9 3)");
    assert_eq!(
        evaluate("(replace '(9 9 9) '(1 2 3 4) :start1 1 :end1 3 :start2 0 :end2 2)").to_string(),
        "(9 1 2)"
    );
    assert_eq!(
        evaluate("(replace \"xxxx\" \"abcd\" :start1 1 :end1 3 :start2 0 :end2 2)").to_string(),
        "\"xabx\""
    );
    assert_eq!(evaluate("(copy-seq #(1 2))").to_string(), "#(1 2)");
    assert_eq!(
        evaluate("(concatenate 'list '(1 2) #(3) \"4\")").to_string(),
        "(1 2 3 #\\4)"
    );
    assert_eq!(
        evaluate("(concatenate 'string \"ab\" '(#\\c #\\d))").to_string(),
        "\"abcd\""
    );
    assert_eq!(
        evaluate("(concatenate 'base-string \"ab\" '(#\\c #\\d))").to_string(),
        "\"abcd\""
    );
    assert_eq!(
        evaluate("(funcall #'fill 0 '(1 2) :start 1)").to_string(),
        "(1 0)"
    );
}

#[test]
fn compiled_evaluates_map_into_over_sequences() {
    assert_eq!(
        evaluate(
            "(let ((result (vector 0 0 0)))
               (map-into result #'+ '(1 2)))",
        )
        .to_string(),
        "#(1 2 0)"
    );
    assert_eq!(
        evaluate(
            "(let ((result (list 9 9 9)))
               (map-into result #'1+ '(1 2))
               result)",
        )
        .to_string(),
        "(2 3 9)"
    );
    assert_eq!(
        evaluate(
            "(let ((result \"xxx\"))
               (map-into result #'identity \"ab\")
               result)",
        )
        .to_string(),
        "\"abx\""
    );
    assert_eq!(
        evaluate(
            "(let ((result (vector 0 0)))
               (map-into result (lambda () 7))
               result)",
        )
        .to_string(),
        "#(7 7)"
    );
    assert_eq!(
        evaluate("(map-into (vector 0 0) #'1+ '(1 2))").to_string(),
        "#(2 3)"
    );
    assert_eq!(
        evaluate("(map-into \"xx\" #'identity \"ab\")").to_string(),
        "\"ab\""
    );
    assert_eq!(evaluate("(map-into nil #'1+ '(1 2))").to_string(), "NIL");
    assert_eq!(
        evaluate(
            "(let ((container (vector (vector 0 0))))
               (map-into (aref container 0) #'1+ '(1 2))
               container)",
        )
        .to_string(),
        "#(#(2 3))"
    );
}

#[test]
fn compiled_evaluates_function_namespace_introspection() {
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
        .eval_compiled_source("(fdefinition 'missing-function)")
        .unwrap_err();
    assert!(matches!(
        error,
        RuntimeError::UnboundVariable { name, .. } if name == "MISSING-FUNCTION"
    ));
}

#[test]
fn compiled_evaluates_compile_function() {
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
               (compile 'compiled-compile-target '(lambda (value) (* value value)))
               (list (compiled-function-p #'compiled-compile-target)
                     (compiled-compile-target 7)))"
        )
        .to_string(),
        "(T 49)"
    );
}

#[test]
fn compiled_evaluates_load_file() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/load.lisp")
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    assert_eq!(
        evaluate(&format!(
            r#"(list (load "{}") *NCL-LOAD-VALUE* (NCL-LOAD-TARGET 1))"#,
            path
        ))
        .to_string(),
        "(T 41 42)"
    );
}

#[test]
fn compiled_evaluates_load_time_value() {
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
fn compiled_evaluates_nth_value() {
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
fn compiled_lowers_nth_value_to_native_instruction() {
    let compiled = Runtime::new()
        .compile_source("(nth-value 1 (values 10 20))")
        .expect("source should compile");

    assert!(
        compiled
            .iter()
            .flat_map(|form| form.program().functions.iter())
            .flat_map(|function| function.instructions.iter())
            .any(|instruction| matches!(instruction, Instruction::NthValue(_)))
    );
}

#[test]
fn compiled_lowers_simple_push_and_pop_without_eval() {
    let compiled = Runtime::new()
        .compile_source("(let ((xs (list 2 3))) (list (push 1 xs) (pop xs)))")
        .expect("source should compile");
    let instructions = compiled
        .iter()
        .flat_map(|form| form.program().functions.iter())
        .flat_map(|function| function.instructions.iter())
        .collect::<Vec<_>>();

    assert!(
        instructions
            .iter()
            .any(|instruction| { matches!(instruction, Instruction::Push(_)) })
    );
    assert!(
        instructions
            .iter()
            .any(|instruction| { matches!(instruction, Instruction::PopPlace(_)) })
    );
    assert!(
        !instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::Eval(_)))
    );
}

#[test]
fn compiled_evaluates_function_and_macro_introspection() {
    assert_eq!(
        evaluate(
            "(progn
               (defmacro introspection-macro (value) (list '+ value 1))
               (define-compiler-macro introspection-function (value) (list '+ value 3))
               (defmacro local-macro-visible (&environment environment)
                 (if (functionp (macro-function 'local-macro environment))
                     '(quote t)
                     '(quote nil)))
               (list (functionp (macro-function 'introspection-macro))
                     (eq (macro-function 'missing-macro) nil)
                     (functionp (compiler-macro-function 'introspection-function))
                     (eq (compiler-macro-function 'missing-compiler-macro) nil)
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
        "(T T T T T NIL NIL T NIL (NIL T))"
    );
    assert_eq!(
        evaluate("(multiple-value-list (function-lambda-expression (lambda (value) (+ value 1))))")
            .to_string(),
        "(NIL NIL NIL)"
    );
    assert_eq!(
        evaluate("(multiple-value-list (function-lambda-expression #'car))").to_string(),
        "(NIL NIL NIL)"
    );
}

#[test]
fn compiled_evaluates_define_compiler_macro() {
    assert_eq!(
        evaluate(
            "(progn
               (defun compiled-compiler-macro-target (value) (+ value 100))
               (define-compiler-macro compiled-compiler-macro-target (value)
                 (list '+ value 1))
               (compiled-compiler-macro-target 5))",
        )
        .to_string(),
        "6"
    );
}

#[test]
fn compiled_evaluates_symbol_function_and_setf() {
    assert_eq!(
        evaluate(
            "(progn
               (defun compiled-symbol-function-target (value) (+ value 2))
               (let ((name 'compiled-symbol-function-target))
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
fn compiled_evaluates_macro_function_and_setf() {
    assert_eq!(
        evaluate(
            "(progn
               (defmacro compiled-source-macro-function (value) (list '+ value 4))
               (let ((name 'compiled-target-macro-function))
                 (list (eq (macro-function name) nil)
                       (progn
                         (setf (macro-function name)
                               (macro-function 'compiled-source-macro-function))
                         (functionp (macro-function name)))
                       (macroexpand-1 '(compiled-target-macro-function 5))
                       (eval '(compiled-target-macro-function 5))
                       (progn
                         (setf (macro-function name) nil)
                         (eq (macro-function name) nil))
                       (fboundp name))))",
        )
        .to_string(),
        "(T T (+ 5 4) 9 T NIL)"
    );
}

#[test]
fn compiled_evaluates_compiler_macro_function_and_setf() {
    let values = Runtime::new()
        .eval_compiled_source(
            "(defun compiled-target-compiler-macro-function (value) (+ value 100))
             (define-compiler-macro compiled-source-compiler-macro-function (value) (list '+ value 4))
             (let ((name 'compiled-target-compiler-macro-function))
               (list (eq (compiler-macro-function name) nil)
                     (progn
                       (setf (compiler-macro-function name)
                             (compiler-macro-function 'compiled-source-compiler-macro-function))
                       (functionp (compiler-macro-function name))))
             )
             (compiled-target-compiler-macro-function 5)
             (let ((name 'compiled-target-compiler-macro-function))
               (progn
                 (setf (compiler-macro-function name) nil)
                 (eq (compiler-macro-function name) nil)))
             (compiled-target-compiler-macro-function 5)",
        )
        .unwrap();

    assert_eq!(values[2].to_string(), "(T T)");
    assert_eq!(values[3].to_string(), "9");
    assert_eq!(values[4].to_string(), "T");
    assert_eq!(values[5].to_string(), "105");
}

#[test]
fn compiled_evaluates_function_namespace_mutation() {
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
fn compiled_evaluates_numeric_predicates_and_extrema() {
    assert_eq!(
        evaluate("(list (zerop 0) (zerop #C(0 0)) (zerop #C(0 1)) (plusp 1) (minusp -1) (evenp 4) (oddp 3) (min 3 1 2) (max 3 1 2) (abs -5) (abs #C(3 4)))").to_string(),
        "(T T NIL T T T T 1 3 5 5)"
    );
}

#[test]
fn compiled_evaluates_common_lisp_integer_arithmetic_and_bit_operations() {
    assert_eq!(
        evaluate(
            "(list (mod -7 3) (mod 7 -3) (rem -7 3) (rem 7 -3)
                    (ash 3 2) (ash -8 -2)
                    (logand 7 3) (logior 4 1) (logxor 7 3) (lognot 0)
                    (lognand 6 3) (lognor 6 3)
                    (logandc1 6 3) (logandc2 6 3)
                    (logorc1 6 3) (logorc2 6 3)
                    (logeqv 7 3)
                    (boole boole-and 6 3) (boole boole-ior 4 1)
                    (boole boole-xor 7 3) (boole boole-eqv 7 3)
                    (boole boole-andc1 6 3) (boole boole-andc2 6 3)
                    (logbitp 1 10) (logbitp 0 10)
                    (logtest 6 2) (logtest 4 2)
                    (ldb-test (byte 3 1) 10) (ldb-test (byte 3 1) 1)
                    (mask-field (byte 3 1) 15) (deposit-field 10 (byte 3 1) 1)
                    (logcount 13) (logcount -8)
                    (integer-length 8) (integer-length -8)
                    (logand) (logior) (logxor))",
        )
        .to_string(),
        "(2 -2 -1 1 12 -2 3 5 4 -1 -3 -8 1 4 -5 -2 -5 2 5 4 -5 1 4 T NIL T NIL T NIL 14 11 3 3 4 3 -1 0 0)"
    );
}

#[test]
fn compiled_evaluates_common_lisp_quotients_gcd_and_rational_parts() {
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
fn compiled_evaluates_common_lisp_expt_across_numeric_types() {
    assert_eq!(
        evaluate(
            "(list (expt 2 10) (expt 2 -3) (expt 3/2 2)
                    (= (expt 2.0 3) 8.0) (floatp (expt 2.0 3))
                    (floatp (expt 2 1/2)) (expt 0 0)
                    (expt -4 1/2) (expt #C(1 1) 2))",
        )
        .to_string(),
        "(1024 1/8 9/4 T T T 1 #C(0 2.0) #C(0 2.0))"
    );
}

#[test]
fn compiled_evaluates_common_lisp_sqrt_across_exact_and_float_numbers() {
    assert_eq!(
        evaluate(
            "(list (sqrt 0) (sqrt 4) (sqrt 1/4)
                    (rationalp (sqrt 2)) (floatp (sqrt 2))
                    (= (sqrt 4.0) 2.0)
                    (sqrt -4) (sqrt -1/4) (sqrt #C(3 4)))",
        )
        .to_string(),
        "(0 2 1/2 NIL T T #C(0 2) #C(0 1/2) #C(2.0 1.0))"
    );
}

#[test]
fn compiled_evaluates_common_lisp_signum_and_rationalize() {
    assert_eq!(
        evaluate(
            "(list (signum -7) (signum 0) (signum -5/2)
                    (signum -0.0) (signum 3.5)
                    (signum #C(3 4)) (signum #C(0 0))
                    (rationalize 2) (rationalize 3/6)
                    (rationalize 0.1) (rationalize (/ 1.0 3.0))
                    (rationalp (rationalize 0.1))
                    (floatp (signum 0.0)))",
        )
        .to_string(),
        "(-1 0 -1 -0.0 1.0 #C(3/5 4/5) #C(0 0) 2 1/2 1/10 1/3 T T)"
    );
}

#[test]
fn compiled_evaluates_common_lisp_float_and_rational_conversion() {
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
