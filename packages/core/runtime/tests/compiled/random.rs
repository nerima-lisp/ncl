use super::*;

#[test]
fn uses_dynamic_random_state_for_random_and_make_random_state() {
    assert_eq!(
        evaluate(
            "(let ((state (make-random-state t)))
               (list
                 (let ((*random-state* (make-random-state state))
                       (copy (make-random-state)))
                   (= (random 1000000) (random 1000000 copy)))
                 (random-state-p *random-state*)))"
        )
        .to_string(),
        "(T T)"
    );
}
