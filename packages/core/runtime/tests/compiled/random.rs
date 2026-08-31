use super::*;

#[test]
fn uses_dynamic_random_state_for_random_and_make_random_state() {
    assert_eq!(
        evaluate(
            "(let ((state (make-random-state t)))
               (list
                 (let ((*random-state* (make-random-state state)))
                   (let ((copy (make-random-state)))
                     (= (random 1000000) (random 1000000 copy))))
                 (random-state-p *random-state*)))"
        )
        .to_string(),
        "(T T)"
    );
}

#[test]
fn setq_updates_compiled_dynamic_random_state() {
    assert_eq!(
        evaluate(
            "(let ((first (make-random-state t))
                    (second (make-random-state t)))
               (let ((*random-state* first))
                 (let ((expected (make-random-state second)))
                   (setq *random-state* second)
                   (= (random 1000000) (random 1000000 expected)))))"
        )
        .to_string(),
        "T"
    );
}
