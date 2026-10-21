#![allow(
    missing_docs,
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "integration tests assert on lowering results"
)]

#[path = "support/forms.rs"]
mod forms;

use forms::Fixture;

use ncl_compiler_front::lower_toplevel;
use ncl_ir::{Compare, Constant, Convert, Function, Op, OpKind, Terminator, Ty, ValueId, verify};
use ncl_object::Word;

fn assert_verifies(function: &Function) {
    if let Err(errors) = verify(function) {
        panic!("{} does not verify: {errors:?}\n{function}", function.name);
    }
}

fn only_branch(function: &Function) -> (ValueId, ncl_ir::BlockId, ncl_ir::BlockId) {
    let branches = function
        .blocks
        .iter()
        .filter_map(|block| match &block.terminator {
            Terminator::Branch {
                condition,
                then_target,
                else_target,
                ..
            } => Some((*condition, *then_target, *else_target)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(branches.len(), 1, "one conditional branch is lowered");
    branches[0]
}

fn block(function: &Function, id: ncl_ir::BlockId) -> &ncl_ir::BasicBlock {
    function
        .blocks
        .iter()
        .find(|block| block.id == id)
        .expect("branch target exists")
}

fn jump_args(function: &Function, source: ncl_ir::BlockId, target: ncl_ir::BlockId) -> &[ValueId] {
    match &block(function, source).terminator {
        Terminator::Jump {
            target: actual,
            args,
        } => {
            assert_eq!(*actual, target);
            args
        }
        other => panic!("expected jump to {target}, got {other:?}"),
    }
}

fn defining_op(function: &Function, value: ValueId) -> &Op {
    function
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .find(|op| op.results.iter().any(|(result, _)| *result == value))
        .expect("SSA value has a defining operation")
}

fn constant_for_value(function: &Function, value: ValueId) -> Option<&Constant> {
    match &defining_op(function, value).kind {
        OpKind::Const { result } => usize::try_from(result.0)
            .ok()
            .and_then(|index| function.constants.get(index)),
        OpKind::Convert {
            op: Convert::I64ToWord,
            value,
        }
        | OpKind::Move { value } => constant_for_value(function, *value),
        _ => None,
    }
}

fn assert_constant(function: &Function, value: ValueId, expected: &Constant) {
    assert_eq!(constant_for_value(function, value), Some(expected));
}

fn assert_if_assignment_state(test_is_true: bool) {
    let mut fx = Fixture::new();
    let (test, test_constant) = if test_is_true {
        (fx.cl("T"), Constant::T)
    } else {
        (Word::NIL, Constant::Nil)
    };
    let x = fx.user("X");
    let binding = fx.list(&[x, Word::fixnum(0)]);
    let bindings = fx.list(&[binding]);
    let setq = fx.cl("SETQ");
    let then_setq = fx.list(&[setq, x, Word::fixnum(11)]);
    let else_setq = fx.list(&[setq, x, Word::fixnum(22)]);
    let if_form = fx.form("IF", &[test, then_setq, else_setq]);
    let form = fx.form("LET", &[bindings, if_form, x]);

    let expr = fx.expand(form).expect("expand if assignment state");
    let lowered = lower_toplevel(&expr).expect("lower if assignment state");
    let function = &lowered.entry;
    assert_verifies(function);

    let (condition, then_block, else_block) = only_branch(function);
    match &defining_op(function, condition).kind {
        OpKind::Compare { op, left, right } => {
            assert_eq!(*op, Compare::Ne);
            assert_constant(function, *left, &test_constant);
            assert_constant(function, *right, &Constant::Nil);
        }
        other => panic!("branch condition is not a comparison: {other:?}"),
    }

    let merge = match &block(function, then_block).terminator {
        Terminator::Jump { target, .. } => *target,
        other => panic!("then arm does not jump to a merge: {other:?}"),
    };
    assert_eq!(
        match &block(function, else_block).terminator {
            Terminator::Jump { target, .. } => *target,
            other => panic!("else arm does not jump to a merge: {other:?}"),
        },
        merge,
        "both arms reach the same merge"
    );

    let then_args = jump_args(function, then_block, merge);
    let else_args = jump_args(function, else_block, merge);
    assert_eq!(then_args.len(), 2, "then passes result and updated X");
    assert_eq!(else_args.len(), 2, "else passes result and updated X");
    assert_eq!(then_args[0], then_args[1], "then SETQ result is X's value");
    assert_eq!(else_args[0], else_args[1], "else SETQ result is X's value");
    assert_constant(function, then_args[1], &Constant::Fixnum(11));
    assert_constant(function, else_args[1], &Constant::Fixnum(22));

    let merge_block = block(function, merge);
    assert_eq!(merge_block.params.len(), 2, "merge carries result and X");
    assert!(
        merge_block
            .params
            .iter()
            .all(|parameter| parameter.ty == Ty::Word)
    );
    let returned = function
        .blocks
        .iter()
        .find_map(|block| match &block.terminator {
            Terminator::Return { values } => values.first().copied(),
            _ => None,
        })
        .expect("entry returns a value");
    assert_eq!(
        returned, merge_block.params[1].value,
        "the form returns X's merged branch state"
    );
}

#[test]
fn nil_test_merges_the_else_assignment_into_returned_state() {
    assert_if_assignment_state(false);
}

#[test]
fn true_test_merges_the_then_assignment_into_returned_state() {
    assert_if_assignment_state(true);
}

#[test]
fn omitted_else_merges_nil_and_returns_it() {
    let mut fx = Fixture::new();
    let form = fx.form("IF", &[Word::NIL, Word::fixnum(11)]);
    let expr = fx.expand(form).expect("expand if without else");
    let lowered = lower_toplevel(&expr).expect("lower if without else");
    let function = &lowered.entry;
    assert_verifies(function);

    let (condition, then_block, else_block) = only_branch(function);
    match &defining_op(function, condition).kind {
        OpKind::Compare { op, left, right } => {
            assert_eq!(*op, Compare::Ne);
            assert_constant(function, *left, &Constant::Nil);
            assert_constant(function, *right, &Constant::Nil);
        }
        other => panic!("branch condition is not a comparison: {other:?}"),
    }

    let merge = match &block(function, then_block).terminator {
        Terminator::Jump { target, .. } => *target,
        other => panic!("then arm does not jump to a merge: {other:?}"),
    };
    let then_args = jump_args(function, then_block, merge);
    let else_args = jump_args(function, else_block, merge);
    assert_eq!(then_args.len(), 1);
    assert_eq!(else_args.len(), 1);
    assert_constant(function, then_args[0], &Constant::Fixnum(11));
    assert_constant(function, else_args[0], &Constant::Nil);

    let merge_block = block(function, merge);
    assert_eq!(merge_block.params.len(), 1);
    let returned = function
        .blocks
        .iter()
        .find_map(|block| match &block.terminator {
            Terminator::Return { values } => values.first().copied(),
            _ => None,
        })
        .expect("entry returns a value");
    assert_eq!(returned, merge_block.params[0].value);
}
