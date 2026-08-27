use super::*;

fn assert_rejected_sequence_calls(cases: &[&str]) {
    for source in cases {
        Runtime::new().eval_source(source).must_fail();
    }
}

#[test]
fn rejects_malformed_sequence_option_lists() {
    assert_rejected_sequence_calls(&[
        "(member 1 '(1 2) :start)",
        "(member 1 '(1 2) :unknown t)",
        "(member 1 '(1 2) 1 t)",
        "(member 1 '(1 2) :start -1)",
        "(member 1 '(1 2) :test #'= :test-not #'=)",
        "(assoc 1 '((1 a)) :unknown t)",
        "(substitute 9 1 '(1 2) :start nope)",
        "(search '(1) '(1 2) :start1 nope)",
        "(mismatch '(1) '(1 2) :end2 nope)",
        "(reduce #'+ '(1 2) :unknown t)",
        "(sort '(1 2) #'< :unknown t)",
        "(merge 'list '(1) '(2) #'< :unknown t)",
        "(remove 1 '(1 2) :test #'= :test-not #'=)",
        "(delete-duplicates '(1 1) :count 1)",
        "(member 1 '(1 2) :key)",
        "(member 1 '(1 2) :test-not #'= :test #'=)",
        "(assoc 1 '((1 a)) :key)",
        "(substitute 9 1 '(1 2) 1 t)",
        "(substitute 9 1 '(1 2) :end -1)",
        "(substitute 9 1 '(1 2) :count -1)",
        "(search '(1) '(1 2) 1 t)",
        "(search '(1) '(1 2) :end -1)",
        "(search '(1) '(1 2) :test #'= :test-not #'=)",
        "(reduce #'+ '(1 2) 1 t)",
        "(reduce #'+ '(1 2) :start -1)",
        "(reduce #'+ '(1 2) :end nope)",
        "(sort '(1 2) #'< 1 t)",
        "(merge 'list '(1) '(2) #'< 1 t)",
        "(remove 1 '(1 2) :start nope)",
        "(remove 1 '(1 2) :end -1)",
        "(remove-duplicates '(1 1) 1 t)",
        "(remove-duplicates '(1 1) :test #'= :test-not #'=)",
    ]);
}

#[test]
fn rejects_sequence_collection_calls_with_too_few_arguments() {
    assert_rejected_sequence_calls(&[
        "(map)",
        "(map 'list)",
        "(reduce)",
        "(merge)",
        "(merge 'list '(1) '(2))",
        "(map-into)",
        "(map-into '(1))",
    ]);
}
