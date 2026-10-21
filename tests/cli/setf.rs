use super::{ncl, output};

#[test]
fn setf_get_retains_the_written_value_and_evaluates_place_once() {
    let direct = output(ncl().args(["--eval", "(progn (setf (get 'x 'y) 42) (get 'x 'y))"]));
    assert!(direct.status.success(), "{direct:?}");
    assert_eq!(String::from_utf8_lossy(&direct.stdout).trim(), "42");

    let result = output(ncl().args([
        "--eval",
        "(let ((n 0)) (setf (get (progn (incf n) 'foo) (progn (incf n) 'bar)) 7) (list n (get 'foo 'bar)))",
    ]));
    assert!(result.status.success(), "{result:?}");
    assert_eq!(String::from_utf8_lossy(&result.stdout).trim(), "(2 7)");
}

#[test]
fn setf_supports_defsetf_and_generalized_sequence_places() {
    let result = output(ncl().args([
        "--eval",
        concat!(
            "(progn ",
            "(defun short-place (object) (car object)) ",
            "(defun short-update (object new) (progn (rplaca object new) new)) ",
            "(defsetf short-place short-update \"short place documentation\") ",
            "(defun long-place (object) (car object)) ",
            "(defsetf long-place (object) (new) \"long place documentation\" ",
            "(declare (ignorable object new)) (if object ",
            "`(progn `(ignored ,,object ,,new) ,@(list nil) ",
            "(rplaca ,object ,new) ,new) '(progn))) ",
            "(let ((count 0) (items (list 1 2 3)) (vec #(3 4)) (text (copy-seq \"abcd\"))) ",
            "(list (setf (short-place (progn (incf count) items)) 9) ",
            "(setf (long-place (progn (incf count) items)) 7) ",
            "(setf (cadr (progn (incf count) items)) 8) ",
            "(setf (elt items 2) 11) (setf (elt vec 1) 12) ",
            "(setf (subseq (progn (incf count) text) ",
            "(progn (incf count) 1) (progn (incf count) 3)) \"XY\") ",
            "(documentation 'short-place 'setf) ",
            "(documentation 'long-place 'setf) count items vec text)))",
        ),
    ]));
    assert!(result.status.success(), "{result:?}");
    assert_eq!(
        String::from_utf8_lossy(&result.stdout).trim(),
        "(9 7 8 11 12 \"XY\" \"short place documentation\" \"long place documentation\" 6 (7 8 11) #(3 12) \"aXYd\")"
    );
}

#[test]
fn psetf_returns_nil_after_storing_every_value() {
    let result = output(ncl().args([
        "--eval",
        "(let ((left 0) (right 0)) (list (psetf left 1 right 2) left right))",
    ]));
    assert!(result.status.success(), "{result:?}");
    assert_eq!(String::from_utf8_lossy(&result.stdout).trim(), "(NIL 1 2)");
}

#[test]
fn short_defsetf_preserves_the_updater_multiple_values() {
    let result = output(ncl().args([
        "--eval",
        concat!(
            "(progn ",
            "(defun short-place (object) (car object)) ",
            "(defun short-update (object new) (progn (rplaca object new) (values new 22))) ",
            "(defsetf short-place short-update) ",
            "(multiple-value-list (setf (short-place (list 1)) 9)))",
        ),
    ]));
    assert!(result.status.success(), "{result:?}");
    assert_eq!(String::from_utf8_lossy(&result.stdout).trim(), "(9 22)");
}

#[test]
fn defsetf_rejects_declarations_whose_semantics_are_not_supported() {
    let result = output(ncl().args([
        "--eval",
        concat!(
            "(progn ",
            "(defun long-place (object) (car object)) ",
            "(defsetf long-place (object) (new) ",
            "(declare (special object)) (list object new)) ",
            "(setf (long-place (list 1)) 2))",
        ),
    ]));
    assert!(!result.status.success(), "{result:?}");
}
