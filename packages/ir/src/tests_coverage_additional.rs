use super::tests_text::{block, finish, op};
use crate::*;

#[test]
fn verifier_accepts_call_closure_without_a_named_symbol() {
    let function = finish(
        "anonymous-call",
        vec![
            Param {
                name: "closure".into(),
                ty: Ty::Word,
            },
            Param {
                name: "argument".into(),
                ty: Ty::Word,
            },
        ],
        vec![Ty::Word],
        Vec::new(),
        vec![block(
            0,
            vec![
                op(&[], OpKind::Safepoint),
                op(
                    &[(2, Ty::Word)],
                    OpKind::CallClosure {
                        closure: ValueId(0),
                        args: vec![ValueId(1)],
                        named_symbol: None,
                    },
                ),
            ],
            Terminator::Return {
                values: vec![ValueId(2)],
            },
        )],
        Vec::new(),
    );
    assert_eq!(verify(&function), Ok(()));
}

#[test]
fn parser_rejects_operation_indices_that_do_not_fit_in_u8() {
    let input = "fn @edge { 0,edge,0,0,0,1,0,0,1,0,ffffffffffffffff,7,100,7,0,0,0, }\n";
    assert_eq!(
        parse(input)
            .expect_err("an oversized LoadArg index must be rejected")
            .to_string(),
        "integer out of range"
    );
}
