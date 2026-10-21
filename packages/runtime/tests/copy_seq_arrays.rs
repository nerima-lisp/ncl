#![allow(missing_docs)]

use ncl_runtime::Runtime;

fn assert_lisp_true(runtime: &mut Runtime, form: &str, check: &str) {
    let result = runtime
        .eval(form)
        .unwrap_or_else(|error| panic!("{check} evaluation failed: {error}"));
    assert_eq!(runtime.format_result(result), "T", "{check}");
}

#[test]
fn copy_seq_preserves_list_string_and_vector_behavior() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let result = runtime.eval(
        r#"(let* ((list-source (list :left :right))
                 (list-copy (copy-seq list-source))
                 (string-source "abc")
                 (string-copy (copy-seq string-source))
                 (vector-source #(:left :right))
                 (vector-copy (copy-seq vector-source)))
            (and (equal list-source list-copy)
                 (not (eq list-source list-copy))
                 (equal string-source string-copy)
                 (not (eq string-source string-copy))
                 (equalp vector-source vector-copy)
                 (not (eq vector-source vector-copy))))"#,
    );
    let result = result.unwrap_or_else(|error| panic!("COPY-SEQ compatibility failed: {error}"));
    assert_eq!(runtime.format_result(result), "T");
}

#[test]
fn copy_seq_copies_rank_one_arrays_with_displacement_and_fill_pointer() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let result = runtime.eval(
        r#"(let* ((general (make-array 3 :element-type 'character :initial-contents "abc"))
                 (array-backing (make-array 5 :element-type 'character :initial-contents "xabcx"))
                 (array-displaced (make-array 3 :element-type 'character
                                              :displaced-to array-backing
                                              :displaced-index-offset 1))
                 (string-backing "xabcx")
                 (string-displaced (make-array 3 :element-type 'character
                                               :displaced-to string-backing
                                               :displaced-index-offset 1))
                 (fill-pointer-vector (make-array 5 :element-type 'character
                                                  :initial-contents "abcde"
                                                  :fill-pointer 3))
                 (general-copy (copy-seq general))
                 (array-copy (copy-seq array-displaced))
                 (string-copy (copy-seq string-displaced))
                 (fill-pointer-copy (copy-seq fill-pointer-vector)))
            (and (equalp general-copy "abc")
                 (equalp array-copy "abc")
                 (equalp string-copy "abc")
                 (equalp fill-pointer-copy "abc")
                 (not (eq general general-copy))
                 (not (eq array-displaced array-copy))
                 (not (eq string-displaced string-copy))
                 (not (eq fill-pointer-vector fill-pointer-copy))
                 (eq (array-element-type array-copy) 'character)
                 (= (array-rank array-copy) 1)
                 (= (array-total-size fill-pointer-copy) 3)
                 (not (array-displacement array-copy))))"#,
    );
    let result = result.unwrap_or_else(|error| panic!("COPY-SEQ array evaluation failed: {error}"));
    assert_eq!(runtime.format_result(result), "T");
}

#[test]
fn copy_seq_copies_specialized_bit_vectors() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let result = runtime.eval(
        "(let* ((source (make-array 3 :element-type 'bit \
                                    :initial-contents '(1 0 1))) \
                (expected (make-array 3 :element-type 'bit \
                                      :initial-contents '(1 0 1))) \
                (copy (copy-seq source))) \
           (and (equalp copy expected) \
                (not (eq source copy)) \
                (eq (array-element-type copy) 'bit) \
                (= (array-rank copy) 1) \
                (= (array-total-size copy) 3) \
                (not (array-displacement copy))))",
    );
    let result =
        result.unwrap_or_else(|error| panic!("COPY-SEQ bit-vector evaluation failed: {error}"));
    assert_eq!(runtime.format_result(result), "T");
}

#[test]
fn copy_seq_array_elements_survive_gc_stress_and_strict_forwarding() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    runtime
        .eval(
            "(defparameter *ncl-copy-seq-gc-source* \
               (make-array 1 :initial-contents (list \"retained\")))",
        )
        .unwrap_or_else(|error| panic!("COPY-SEQ stress source setup failed: {error}"));
    runtime
        .eval("(defparameter *ncl-copy-seq-gc-copy* nil)")
        .unwrap_or_else(|error| panic!("COPY-SEQ stress destination setup failed: {error}"));
    runtime.set_gc_stress(true);
    runtime.set_strict_forwarding(true);

    runtime
        .eval(
            "(setq *ncl-copy-seq-gc-copy* \
               (copy-seq *ncl-copy-seq-gc-source*))",
        )
        .unwrap_or_else(|error| panic!("COPY-SEQ GC-stress evaluation failed: {error}"));
    runtime.set_gc_stress(false);

    for (form, check) in [
        (
            "(not (eq *ncl-copy-seq-gc-copy* *ncl-copy-seq-gc-source*))",
            "fresh array identity",
        ),
        (
            "(simple-vector-p *ncl-copy-seq-gc-copy*)",
            "simple vector result",
        ),
        (
            "(eq (array-element-type *ncl-copy-seq-gc-copy*) \
                (array-element-type *ncl-copy-seq-gc-source*))",
            "preserved element type",
        ),
        (
            "(eq (aref *ncl-copy-seq-gc-copy* 0) \
                (aref *ncl-copy-seq-gc-source* 0))",
            "element identity",
        ),
        (
            "(equalp (aref *ncl-copy-seq-gc-copy* 0) \"retained\")",
            "copied element content",
        ),
    ] {
        assert_lisp_true(&mut runtime, form, check);
    }
}

#[test]
fn copy_seq_rejects_rank_two_arrays() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let result = runtime.eval(
        "(handler-case (progn (copy-seq (make-array '(2 2))) nil) \
           (type-error () t))",
    );
    let result = result.unwrap_or_else(|error| panic!("COPY-SEQ rank validation failed: {error}"));
    assert_eq!(runtime.format_result(result), "T");
}
