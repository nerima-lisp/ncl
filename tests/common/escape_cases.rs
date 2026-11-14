pub struct Case {
    pub name: &'static str,
    pub source: &'static str,
    pub expected: &'static str,
}

pub const CASES: &[Case] = &[
    Case {
        name: "flet-return-from",
        source: "(block done (flet ((escape () (return-from done 11))) (escape) 99))",
        expected: "11",
    },
    Case {
        name: "labels-return-from",
        source: "(block done (labels ((escape () (return-from done 12))) (escape) 99))",
        expected: "12",
    },
    Case {
        name: "labels-self-recursion-return-from",
        source: "(block done (labels ((walk (n) (if (= n 0) (return-from done 21) (walk (- n 1))))) (walk 3) 99))",
        expected: "21",
    },
    Case {
        name: "mapcar-go",
        source: "(progn (setq *nlx-go* 0) (tagbody (mapcar (lambda (v) (go out)) '(1)) (setq *nlx-go* 99) out) *nlx-go*)",
        expected: "0",
    },
    Case {
        name: "mapcar-return-from",
        source: "(block b (mapcar (lambda (v) (return-from b v)) '(1 2)))",
        expected: "1",
    },
    Case {
        name: "lambda-return-from",
        source: "(block done (funcall (lambda () (return-from done 18))) 99)",
        expected: "18",
    },
    Case {
        name: "lambda-return-from-through-catch",
        source: "(block done (catch 'inner (funcall (lambda () (return-from done :through-catch))) :unreachable))",
        expected: ":THROUGH-CATCH",
    },
    Case {
        name: "lambda-return-from-restores-progv",
        source: r"
(progn
  (defvar *nlx-progv* :outer)
  (let ((result
          (block done
            (progv '(*nlx-progv*) '(:inner)
              (funcall
                (lambda ()
                  (return-from done
                    (symbol-value '*nlx-progv*))))))))
    (list result (symbol-value '*nlx-progv*))))
",
        expected: "(:INNER :OUTER)",
    },
    Case {
        name: "lambda-return-from-restores-unbound-progv",
        source: r"
(let ((result
        (block done
          (progv '(*nlx-unbound-progv*) '(:inner)
            (funcall
              (lambda ()
                (return-from done
                  (symbol-value '*nlx-unbound-progv*))))))))
  (list result (boundp '*nlx-unbound-progv*)))
",
        expected: "(:INNER NIL)",
    },
    Case {
        name: "branch-return-from-restores-special-binding",
        source: r"
(progn
  (defvar *branch-special* :root)
  (list
    (let ((*branch-special* :outer))
      (block outer-probe
        (list
          (block inner-probe
            (let ((*branch-special* :inner))
              (if nil
                  (return-from outer-probe :outer-exit)
                  (return-from inner-probe :inner-exit))))
          *branch-special*)))
    *branch-special*))
",
        expected: "((:INNER-EXIT :OUTER) :ROOT)",
    },
    Case {
        name: "lambda-return-from-restores-repeated-progv-binding",
        source: r"
(progn
  (defvar *nlx-repeated-progv* :outer)
  (let ((result
          (block done
            (progv '(*nlx-repeated-progv* *nlx-repeated-progv*)
                   '(:middle :inner)
              (funcall
                (lambda ()
                  (return-from done
                    (symbol-value '*nlx-repeated-progv*))))))))
    (list result (symbol-value '*nlx-repeated-progv*))))
",
        expected: "(:INNER :OUTER)",
    },
    Case {
        name: "lambda-go",
        source: "(let ((result 0)) (tagbody (funcall (lambda () (go done))) done (setq result 13)) result)",
        expected: "13",
    },
    Case {
        name: "flet-go",
        source: "(let ((result 0)) (tagbody (flet ((jump () (go done))) (jump)) done (setq result 19)) result)",
        expected: "19",
    },
    Case {
        name: "labels-go",
        source: "(let ((result 0)) (tagbody (labels ((jump () (go done))) (jump)) done (setq result 20)) result)",
        expected: "20",
    },
    Case {
        name: "return-from-multiple-values",
        source: "(multiple-value-list (block done (return-from done (values 1 2))))",
        expected: "(1 2)",
    },
    Case {
        name: "unwind-protect-cleanup",
        source: "(progn (setq *escape-cleanup* 0) (block done (unwind-protect (return-from done 14) (setq *escape-cleanup* 15))) *escape-cleanup*)",
        expected: "15",
    },
    Case {
        name: "flet-body-return-from-runs-unwind-cleanup",
        source: "(progn (setq *escape-cleanup* 0) (block done (flet ((noop () nil)) (unwind-protect (return-from done 24) (setq *escape-cleanup* 25)))) *escape-cleanup*)",
        expected: "25",
    },
    Case {
        name: "labels-body-return-from-runs-unwind-cleanup",
        source: "(progn (setq *escape-cleanup* 0) (block done (labels ((noop () nil)) (unwind-protect (return-from done 26) (setq *escape-cleanup* 27)))) *escape-cleanup*)",
        expected: "27",
    },
    Case {
        name: "outer-frame-closure",
        source: "(block done (funcall (lambda (n) (return-from done 16)) 3) 99)",
        expected: "16",
    },
    Case {
        name: "gc-stress-allocation-pressure",
        source: "(block done (funcall (lambda () (return-from done (length (list (list 1 2 3) (list 4 5 6) (list 7 8 9)))))))",
        expected: "3",
    },
    Case {
        name: "block-assignment-survives-normal-exit",
        source: "(let ((x 0)) (block b (setq x 5) (return-from b 9)) x)",
        expected: "5",
    },
    Case {
        name: "block-assignment-preserves-unassigned-value",
        source: "(let ((x 0) (y 0)) (block b (setq x 1) (when t (return-from b nil)) (setq y 2)) (list x y))",
        expected: "(1 0)",
    },
    Case {
        name: "flet-assignment-through-closure",
        source: "(let ((x 0)) (flet ((bump () (setq x (1+ x)))) (bump) (bump)) x)",
        expected: "2",
    },
    Case {
        name: "block-lambda-return-preserves-assignment",
        source: "(let ((x 0)) (block b (setq x 1) (funcall (lambda () (return-from b nil))) (setq x 2)) x)",
        expected: "1",
    },
    Case {
        name: "block-mapcar-return-preserves-unassigned-value",
        source: "(let ((x 0) (y 0)) (block b (setq x 1) (mapcar (lambda (v) (return-from b v)) '(7)) (setq y 2)) (list x y))",
        expected: "(1 0)",
    },
    Case {
        name: "tagbody-assignment-through-lambda-go",
        source: "(let ((x 0)) (tagbody (setq x 5) (funcall (lambda () (go done))) done) x)",
        expected: "5",
    },
    Case {
        name: "tagbody-mapcar-go-preserves-unassigned-value",
        source: "(let ((x 0) (y 0)) (tagbody (setq x 1) (mapcar (lambda (v) (go done)) '(7)) (setq y 2) done) (list x y))",
        expected: "(1 0)",
    },
    Case {
        name: "dotimes-value-loop",
        source: "(dotimes (i 5) i)",
        expected: "NIL",
    },
    Case {
        name: "dotimes-setq-loop",
        source: "(let ((s 0)) (dotimes (i 10) (setq s (+ s i))) s)",
        expected: "45",
    },
    Case {
        name: "tagbody-backedge-loop",
        source: "(let ((x 0)) (tagbody top (setq x (1+ x)) (when (< x 5) (go top))) x)",
        expected: "5",
    },
    Case {
        name: "tagbody-go-preserves-unassigned-value",
        source: "(let ((x 0) (y 0)) (tagbody (setq x 1) (when t (go end)) (setq y 2) end) (list x y))",
        expected: "(1 0)",
    },
    Case {
        name: "do-parallel-updates",
        source: "(do ((i 0 (1+ i)) (acc nil (cons i acc))) ((= i 3) acc))",
        expected: "(2 1 0)",
    },
    Case {
        name: "dotimes-closures-do-not-crash",
        source: "(let ((fs nil)) (dotimes (i 3) (push (lambda () i) fs)) (mapcar #'funcall fs))",
        expected: "(3 3 3)",
    },
];
