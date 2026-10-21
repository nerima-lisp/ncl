#![allow(missing_docs)]
#![allow(clippy::expect_used)]
#![cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]

use ncl_runtime::Runtime;
use std::process::Command;

fn eval(source: &str) -> String {
    let mut runtime = Runtime::new().expect("runtime");
    let value = runtime.eval(source).expect("evaluation");
    runtime.format_result(value)
}

#[test]
fn throw_resumes_after_cleanup() {
    assert_eq!(
        eval("(catch 'a (unwind-protect (throw 'a 1) (setq *uwp-c* 1)))"),
        "1"
    );
}

#[test]
fn unwind_protect_does_not_cover_a_prior_throwing_call_in_catch() {
    assert_eq!(
        eval(
            "(progn (setq *uwp-c* 0) (list (catch 'outer (progn (funcall (lambda () (throw 'outer :before))) (unwind-protect :after (setq *uwp-c* (1+ *uwp-c*))))) *uwp-c*))"
        ),
        "(:BEFORE 0)"
    );
}

#[test]
fn catch_tag_throw_does_not_enter_the_catch_body_or_run_its_cleanup() {
    assert_eq!(
        eval(
            "(progn (setq *uwp-c* 0) (list (catch 'outer (catch (throw 'outer :before) (unwind-protect :after (setq *uwp-c* (1+ *uwp-c*))))) *uwp-c*))"
        ),
        "(:BEFORE 0)"
    );
}

#[test]
fn return_from_resumes_after_cleanup() {
    assert_eq!(
        eval("(block b (unwind-protect (return-from b 5) (setq *uwp-c* 1)))"),
        "5"
    );
}

#[test]
fn escaping_lambda_return_from_runs_unwind_protect_cleanup() {
    assert_eq!(
        eval(
            "(let ((cleanup nil)) (let ((value (block done (unwind-protect (funcall (lambda () (return-from done :done))) (setq cleanup :ran))))) (list value cleanup)))"
        ),
        "(:DONE :RAN)"
    );
}

#[test]
fn return_from_through_unwind_protect_restores_special_binding_after_cleanup() {
    assert_eq!(
        eval(
            r"
(progn
  (defvar *uwp-return-from-special* :outer)
  (defvar *uwp-return-from-cleanup-value* nil)
  (defvar *uwp-return-from-cleanup-count* 0)
  (let ((result
          (block done
            (let ((*uwp-return-from-special* :inner))
              (unwind-protect
                (return-from done :returned)
                (progn
                  (setq *uwp-return-from-cleanup-value* *uwp-return-from-special*)
                  (incf *uwp-return-from-cleanup-count*)))))))
    (list result
          *uwp-return-from-cleanup-value*
          *uwp-return-from-cleanup-count*
          *uwp-return-from-special*)))
"
        ),
        "(:RETURNED :INNER 1 :OUTER)"
    );
}

#[test]
fn go_through_unwind_protect_restores_special_binding_before_cleanup() {
    assert_eq!(
        eval(
            r"
(progn
  (defvar *uwp-go-special* :outer)
  (defvar *uwp-go-cleanup-value* nil)
  (defvar *uwp-go-cleanup-count* 0)
  (tagbody
    (unwind-protect
      (let ((*uwp-go-special* :inner))
        (go done))
      (progn
        (setq *uwp-go-cleanup-value* *uwp-go-special*)
        (incf *uwp-go-cleanup-count*)))
   done)
  (list *uwp-go-cleanup-value* *uwp-go-cleanup-count* *uwp-go-special*))
"
        ),
        "(:OUTER 1 :OUTER)"
    );
}

#[test]
fn go_through_unwind_protect_can_revisit_same_tag() {
    assert_eq!(
        eval(
            r"
(progn
  (defvar *uwp-repeat-go-count* 0)
  (defvar *uwp-repeat-go-cleanups* 0)
  (setq *uwp-repeat-go-count* 0)
  (setq *uwp-repeat-go-cleanups* 0)
  (tagbody
    (unwind-protect (go again)
      (incf *uwp-repeat-go-cleanups*))
   again
    (incf *uwp-repeat-go-count*)
    (when (< *uwp-repeat-go-count* 2)
      (go again)))
  (list *uwp-repeat-go-count* *uwp-repeat-go-cleanups*))
"
        ),
        "(2 1)"
    );
}

#[test]
fn prog_go_restores_a_special_binding_after_updating_live_arguments() {
    assert_eq!(
        eval(
            r"
(progn
  (defvar *uwp-prog-go-special* :outer)
  (defvar *uwp-prog-go-seen* nil)
  (defun uwp-prog-go-live-argument (items)
    (prog ((*uwp-prog-go-special* :inner)
           (iterations 0))
     again
      (setq *uwp-prog-go-seen* *uwp-prog-go-special*)
      (setq iterations (1+ iterations))
      (setq items (cdr items))
      (if items (go again))
      (return iterations)))
  (list (uwp-prog-go-live-argument '(a b))
        *uwp-prog-go-seen*
        *uwp-prog-go-special*))
"
        ),
        "(2 :INNER :OUTER)"
    );
}

#[test]
fn nested_return_from_after_escaping_tagbody_preserves_regions() {
    assert_eq!(
        eval(
            r"
(progn
  (defvar *uwp-escaping-tagbody-special* :outer)
  (defvar *uwp-escaping-tagbody-cleanup-value* nil)
  (defvar *uwp-escaping-tagbody-cleanup-count* 0)
  (defvar *uwp-escaping-tagbody-inner-result* nil)
  (setq *uwp-escaping-tagbody-cleanup-count* 0)
  (let ((outer-result
          (block outer
            (let ((*uwp-escaping-tagbody-special* :inner))
              (setq *uwp-escaping-tagbody-inner-result*
                    (block done
                      (unwind-protect
                          (tagbody
                            (flet ((jump () (go two)))
                              (unwind-protect (jump) nil))
                           one
                           two
                            (return-from done :inner))
                        (progn
                          (setq *uwp-escaping-tagbody-cleanup-value*
                                *uwp-escaping-tagbody-special*)
                          (incf *uwp-escaping-tagbody-cleanup-count*))))))
            (return-from outer :outer))))
    (list outer-result
          *uwp-escaping-tagbody-inner-result*
          *uwp-escaping-tagbody-cleanup-value*
          *uwp-escaping-tagbody-cleanup-count*
          *uwp-escaping-tagbody-special*)))
"
        ),
        "(:OUTER :INNER :INNER 1 :OUTER)"
    );
}

#[test]
fn return_from_after_terminal_tagbody_preserves_outer_exit() {
    assert_eq!(
        eval(
            r"
(progn
  (defvar *uwp-terminal-tagbody-special* :outer)
  (defvar *uwp-terminal-tagbody-cleanup-value* nil)
  (defvar *uwp-terminal-tagbody-cleanup-count* 0)
  (setq *uwp-terminal-tagbody-cleanup-count* 0)
  (let ((outer-result
          (block outer
            (let ((*uwp-terminal-tagbody-special* :inner))
              (block done
                (unwind-protect
                    (tagbody
                      two
                      (return-from done :inner))
                  (progn
                    (setq *uwp-terminal-tagbody-cleanup-value*
                          *uwp-terminal-tagbody-special*)
                    (incf *uwp-terminal-tagbody-cleanup-count*)))))
            (return-from outer :outer))))
    (list outer-result
          *uwp-terminal-tagbody-cleanup-value*
          *uwp-terminal-tagbody-cleanup-count*
          *uwp-terminal-tagbody-special*)))
"
        ),
        "(:OUTER :INNER 1 :OUTER)"
    );
}

#[test]
fn unwind_protect_inside_if_does_not_cover_the_merge_block() {
    assert_eq!(
        eval(
            "(if t (let ((cleanup nil)) (let ((value (block done (unwind-protect (funcall (lambda () (return-from done :done))) (setq cleanup :ran))))) (list value cleanup))) :other)"
        ),
        "(:DONE :RAN)"
    );
}

#[test]
fn escaping_block_inside_if_does_not_cover_the_outer_merge_block() {
    assert_eq!(
        eval(
            "(if t (let ((cleanup nil)) (let ((value (block done (unwind-protect (funcall (lambda () (setq cleanup :body) (return-from done :done))) (setq cleanup :ran))))) (list value cleanup))) :other)"
        ),
        "(:DONE :RAN)"
    );
}

#[test]
fn cleanup_non_local_exit_has_priority() {
    assert_eq!(
        eval("(catch 'a (catch 'b (unwind-protect (throw 'a 1) (throw 'b 2))))"),
        "2"
    );
}

#[test]
fn caught_cleanup_exit_preserves_outer_pending_exit() {
    assert_eq!(
        eval("(catch 'a (unwind-protect (throw 'a 1) (catch 'b (throw 'b 2))))"),
        "1"
    );
}

#[test]
fn pending_value_survives_allocating_cleanup() {
    assert_eq!(
        eval("(catch 'a (unwind-protect (throw 'a 1) (list 9)))"),
        "1"
    );
}

#[test]
fn cleanup_side_effect_is_completed_before_result() {
    let output = Command::new(env!("CARGO_BIN_EXE_ncl"))
        .args(["--eval", "(progn (setq *uwp-c* 0) (catch 'a (unwind-protect (throw 'a 1) (setq *uwp-c* 7))) *uwp-c*)"])
        .output()
        .expect("ncl executable");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "7");
}

#[test]
fn cleanup_throw_during_normal_completion_runs_once() {
    assert_eq!(
        eval(
            "(progn (setq *uwp-c* 0) (list (catch 'cleanup (unwind-protect :protected (setq *uwp-c* (1+ *uwp-c*)) (throw 'cleanup :cleanup-exit))) *uwp-c*))"
        ),
        "(:CLEANUP-EXIT 1)"
    );
}

#[test]
fn nested_cleanup_runs_in_lifo_order() {
    assert_eq!(
        eval(
            "(progn (setq *uwp-c* nil) (catch 'a (unwind-protect (unwind-protect (throw 'a 1) (setq *uwp-c* (cons :inner *uwp-c*))) (setq *uwp-c* (cons :outer *uwp-c*)))) (nreverse *uwp-c*))"
        ),
        "(:INNER :OUTER)"
    );
}

#[test]
fn cleanup_exit_wins_after_nested_cleanup() {
    assert_eq!(
        eval(
            "(catch 'a (catch 'b (unwind-protect (unwind-protect (throw 'a 1) (throw 'b 2)) (throw 'a 3))))"
        ),
        "3"
    );
}
