#![allow(missing_docs)]

use ncl_runtime::Runtime;

fn compound_accessor_names() -> Vec<String> {
    (1..=4)
        .flat_map(|depth| {
            (0..(1 << depth)).map(move |bits| {
                let path = (0..depth)
                    .map(|index| if bits & (1 << index) == 0 { 'A' } else { 'D' })
                    .collect::<String>();
                format!("C{path}R")
            })
        })
        .collect()
}

fn full_cons_tree(depth: usize) -> String {
    if depth == 0 {
        "0".to_owned()
    } else {
        let child = full_cons_tree(depth - 1);
        format!("(cons {child} {child})")
    }
}

fn updated_cons_tree(depth: usize, path: &[char]) -> String {
    if path.is_empty() {
        return "101".to_owned();
    }
    let child_depth = depth - 1;
    let child_path = &path[1..];
    let left = if path[0] == 'A' {
        updated_cons_tree(child_depth, child_path)
    } else {
        full_cons_tree(child_depth)
    };
    let right = if path[0] == 'D' {
        updated_cons_tree(child_depth, child_path)
    } else {
        full_cons_tree(child_depth)
    };
    format!("(cons {left} {right})")
}

fn assert_form(runtime: &mut Runtime, source: &str, expected: &str, place: &str) {
    let value = runtime
        .eval(source)
        .unwrap_or_else(|error| panic!("evaluation failed for {place}: {error}"));
    assert_eq!(
        runtime.format_result(value),
        expected,
        "unexpected result for {place}"
    );
}

#[test]
fn all_compound_cons_places_support_setf_and_incf() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let tree = full_cons_tree(5);

    for place in compound_accessor_names() {
        let path = place[1..place.len() - 1].chars().rev().collect::<Vec<_>>();
        let expected_tree = updated_cons_tree(5, &path);
        let source = format!(
            "(let ((x {tree}) (accesses 0)) (list (setf ({place} (progn (incf accesses) x)) 100) (incf ({place} (progn (incf accesses) x))) x accesses))"
        );
        let expected_source = format!("(list 100 101 {expected_tree} 2)");
        let expected = runtime
            .eval(&expected_source)
            .unwrap_or_else(|error| panic!("expected tree failed for {place}: {error}"));
        let expected = runtime.format_result(expected);
        assert_form(&mut runtime, &source, &expected, &place);
    }
}

#[test]
fn second_through_tenth_support_setf_and_incf() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let names = [
        "SECOND", "THIRD", "FOURTH", "FIFTH", "SIXTH", "SEVENTH", "EIGHTH", "NINTH", "TENTH",
    ];
    let initial = (0..10).map(|_| "0").collect::<Vec<_>>().join(" ");

    for (index, place) in names.into_iter().enumerate() {
        let expected = (0..10)
            .map(|position| if position == index + 1 { "101" } else { "0" })
            .collect::<Vec<_>>()
            .join(" ");
        let source = format!(
            "(let ((x (list {initial})) (accesses 0)) (list (setf ({place} (progn (incf accesses) x)) 100) (incf ({place} (progn (incf accesses) x))) x accesses))"
        );
        let expected = format!("(100 101 ({expected}) 2)");
        assert_form(&mut runtime, &source, &expected, place);
    }
}

#[test]
fn nested_the_places_update_the_underlying_variable() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    assert_form(
        &mut runtime,
        "(let ((x 1)) (list (setf (the fixnum (the integer x)) 4) (incf (the fixnum (the integer x))) x))",
        "(4 5 5)",
        "(THE FIXNUM (THE INTEGER X))",
    );
}

#[test]
fn package_documentation_place_round_trips() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    assert_form(
        &mut runtime,
        "(let ((p (make-package \"SETF-STANDARDIZED-DOC\" :documentation \"created\"))) (list (documentation p t) (setf (documentation p t) \"updated\") (documentation p t)))",
        "(\"created\" \"updated\" \"updated\")",
        "(DOCUMENTATION package T)",
    );
}

#[test]
fn remf_splices_matching_property_pairs() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    assert_form(
        &mut runtime,
        "(let* ((plist (list :a 1 :b 2 :c 3)) (alias plist)) (list (remf plist :b) plist alias))",
        "(T (:A 1 :C 3) (:A 1 :C 3))",
        "REMF destructive tail splice",
    );
    assert_form(
        &mut runtime,
        "(let* ((plist (list :a 1 :b 2)) (alias plist)) (list (remf plist :a) plist alias))",
        "(T (:B 2) (:A 1 :B 2))",
        "REMF head removal through place",
    );
}

#[test]
fn pop_remembers_car_before_place_store_mutates_it() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    runtime
        .eval(
            "(progn\n               (defun pop-mutating-place (holder) (car holder))\n               (defun update-pop-mutating-place (holder value)\n                 (rplaca (car holder) :changed)\n                 (rplaca holder value)\n                 value)\n               (defsetf pop-mutating-place update-pop-mutating-place))",
        )
        .unwrap_or_else(|error| panic!("define POP mutation place: {error}"));
    assert_form(
        &mut runtime,
        "(let* ((old (list 1 2)) (holder (cons old nil)))\n           (list (pop (pop-mutating-place holder)) old (car holder)))",
        "(1 (:CHANGED 2) (2))",
        "POP remembers CAR before setter mutation",
    );
}

#[test]
fn modifying_place_forms_evaluate_once_in_clhs_order() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    assert_form(
        &mut runtime,
        "(let ((x (list 1)) (events nil))\n           (list (setf (car (progn (setf events (append events '(place))) x))\n                       (progn (setf events (append events '(value))) 10))\n                 events x))",
        "(10 (COMMON-LISP-USER:PLACE COMMON-LISP-USER:VALUE) (10))",
        "SETF place-before-value order",
    );
    assert_form(
        &mut runtime,
        "(let ((x (list 1)) (events nil))\n           (list (incf (car (progn (setf events (append events '(place))) x))\n                       (progn (setf events (append events '(delta)))\n                              (setf (car x) 10)\n                              2))\n                 events x))",
        "(12 (COMMON-LISP-USER:PLACE COMMON-LISP-USER:DELTA) (12))",
        "INCF subforms and read order",
    );
    assert_form(
        &mut runtime,
        "(let ((x (list 10)) (events nil))\n           (list (decf (car (progn (setf events (append events '(place))) x))\n                       (progn (setf events (append events '(delta)))\n                              (setf (car x) 8)\n                              2))\n                 events x))",
        "(6 (COMMON-LISP-USER:PLACE COMMON-LISP-USER:DELTA) (6))",
        "DECF subforms and read order",
    );
    assert_form(
        &mut runtime,
        "(let ((x (list 0)) (y (list 0)) (events nil))\n           (psetf (car (progn (setf events (append events '(place-x))) x))\n                  (progn (setf events (append events '(value-x))) 1)\n                  (car (progn (setf events (append events '(place-y))) y))\n                  (progn (setf events (append events '(value-y)))\n                         (setf (car x) 99)\n                         2))\n           (list events x y))",
        "((COMMON-LISP-USER:PLACE-X COMMON-LISP-USER:VALUE-X COMMON-LISP-USER:PLACE-Y COMMON-LISP-USER:VALUE-Y) (1) (2))",
        "PSETF source order and delayed stores",
    );
    assert_form(
        &mut runtime,
        "(let ((x (list (list 1))) (events nil))\n           (push (progn (setf events (append events '(item))) 9)\n                 (car (progn (setf events (append events '(place))) x)))\n           (list events x))",
        "((COMMON-LISP-USER:ITEM COMMON-LISP-USER:PLACE) ((9 1)))",
        "PUSH item-before-place order",
    );
    assert_form(
        &mut runtime,
        "(let ((x (list (list 1))) (events nil))\n           (pushnew (progn (setf events (append events '(item))) 2)\n                    (car (progn (setf events (append events '(place))) x))\n                    :key (progn (setf events (append events '(key)))\n                                (setf (car x) '(9))\n                                nil))\n           (list events x))",
        "((COMMON-LISP-USER:ITEM COMMON-LISP-USER:PLACE COMMON-LISP-USER:KEY) ((2 9)))",
        "PUSHNEW item-place-key order",
    );
    assert_form(
        &mut runtime,
        "(let ((x (list (list 1 2))) (events nil))\n           (list (pop (car (progn (setf events (append events '(place))) x)))\n                 events x))",
        "(1 (COMMON-LISP-USER:PLACE) ((2)))",
        "POP place evaluation once",
    );
    assert_form(
        &mut runtime,
        "(let ((x (list 1)) (events nil))\n           (list (shiftf (car (progn (setf events (append events '(place))) x))\n                         (progn (setf events (append events '(new-value)))\n                                (setf (car x) 9)\n                                4))\n                 events x))",
        "(9 (COMMON-LISP-USER:PLACE COMMON-LISP-USER:NEW-VALUE) (4))",
        "SHIFTF argument-before-read order",
    );
    assert_form(
        &mut runtime,
        "(let ((x (list 1)) (y (list 2)) (events nil))\n           (rotatef (car (progn (setf events (append events '(place-x))) x))\n                    (car (progn (setf events (append events '(place-y)))\n                                (setf (car x) 9)\n                                y)))\n           (list events x y))",
        "((COMMON-LISP-USER:PLACE-X COMMON-LISP-USER:PLACE-Y) (2) (9))",
        "ROTATEF place-before-read order",
    );
    assert_form(
        &mut runtime,
        "(let ((x (list '(:a 1 :b 2))) (events nil))\n           (list (remf (car (progn (setf events (append events '(place))) x))\n                       (progn (setf events (append events '(property)))\n                              (setf (car x) '(:b 2))\n                              :a))\n                 events x))",
        "(NIL (COMMON-LISP-USER:PLACE COMMON-LISP-USER:PROPERTY) ((:B 2)))",
        "REMF property-before-read order",
    );
}

#[test]
fn fdefinition_and_macro_function_place_arguments_evaluate_once() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    assert_form(
        &mut runtime,
        "(let ((fdefinition-accesses 0) (macro-accesses 0))
           (psetf (fdefinition
                    (progn (incf fdefinition-accesses) 'ncl-fdefinition-place-once))
                  #'identity)
           (setf (macro-function
                   (progn (incf macro-accesses) 'ncl-macro-function-place-once))
                 #'identity)
           (list fdefinition-accesses
                 (funcall (fdefinition 'ncl-fdefinition-place-once) 7)
                 macro-accesses
                 (not (null (macro-function 'ncl-macro-function-place-once)))))",
        "(1 7 1 T)",
        "FDEFINITION and MACRO-FUNCTION place argument evaluation once",
    );
}

#[test]
fn macro_function_place_accepts_nil_environment_and_evaluates_it_once() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    assert_form(
        &mut runtime,
        "(let ((environment-accesses 0))
           (setf (macro-function
                   'ncl-macro-function-place-environment
                   (progn (incf environment-accesses) nil))
                 #'identity)
           (list environment-accesses
                 (not (null
                       (macro-function 'ncl-macro-function-place-environment)))))",
        "(1 T)",
        "MACRO-FUNCTION place evaluates a NIL environment once",
    );
}

#[test]
fn macro_function_returns_nil_for_an_ordinary_function() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    assert_form(
        &mut runtime,
        "(progn
           (setf (fdefinition 'ncl-macro-function-normal-function) #'identity)
           (macro-function 'ncl-macro-function-normal-function))",
        "NIL",
        "MACRO-FUNCTION on an ordinary function",
    );
}

#[test]
fn fdefinition_replaces_a_macro_definition_with_a_function() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    runtime
        .eval(
            "(setf (macro-function 'ncl-fdefinition-replaces-macro)
                  #'(lambda (form environment) form))",
        )
        .unwrap_or_else(|error| panic!("macro installation failed: {error}"));
    runtime
        .eval("(setf (fdefinition 'ncl-fdefinition-replaces-macro) #'identity)")
        .unwrap_or_else(|error| panic!("function replacement failed: {error}"));
    assert_form(
        &mut runtime,
        "(macro-function 'ncl-fdefinition-replaces-macro)",
        "NIL",
        "MACRO-FUNCTION after FDEFINITION replaces a macro",
    );
    assert_form(
        &mut runtime,
        "(funcall 'ncl-fdefinition-replaces-macro 9)",
        "9",
        "function call after FDEFINITION replaces a macro",
    );
}

#[test]
fn fdefinition_place_rejects_a_non_symbol_name() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let result = runtime.eval(
        "(setf (fdefinition
                  (cons 'ncl-invalid-fdefinition-place 'ncl-tail))
              #'identity)",
    );

    assert!(
        matches!(
            result,
            Err(ncl_runtime::RuntimeError::Object(
                ncl_object::ObjectError::TypeError
            ))
        ),
        "expected TypeError for a non-symbol function name, got {result:?}"
    );
}
