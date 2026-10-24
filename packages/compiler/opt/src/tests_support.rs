use ncl_ir::{
    Constant, Function, FunctionBuilder, FunctionId, OpKind, Param, Terminator, Ty, ValueId,
};
use std::fmt::Debug;

pub trait Fixture<T> {
    fn fixture(self) -> T;
}

impl<T, E: Debug> Fixture<T> for Result<T, E> {
    fn fixture(self) -> T {
        self.unwrap_or_else(|error| panic!("fixture failed: {error:?}"))
    }
}

pub fn leaf() -> Function {
    let mut b = FunctionBuilder::new(
        FunctionId(2),
        "leaf",
        vec![Param {
            name: "x".into(),
            ty: Ty::Word,
        }],
        vec![Ty::Word],
    );
    let x = b
        .push_op(OpKind::Move { value: ValueId(0) }, &[Ty::Word])
        .fixture()[0];
    b.terminate(Terminator::Return { values: vec![x] })
        .fixture();
    b.finish()
}

pub fn caller() -> Function {
    let mut b = FunctionBuilder::new(
        FunctionId(1),
        "caller",
        vec![Param {
            name: "x".into(),
            ty: Ty::Word,
        }],
        vec![Ty::Word],
    );
    let entry = b.add_constant(Constant::FunctionEntry(FunctionId(2)));
    let f = b
        .push_op(OpKind::Const { result: entry }, &[Ty::Word])
        .fixture()[0];
    let call = b
        .push_op(
            OpKind::Call {
                function: f,
                args: vec![ValueId(0)],
            },
            &[Ty::Word],
        )
        .fixture()[0];
    b.terminate(Terminator::Return { values: vec![call] })
        .fixture();
    b.finish()
}

pub fn scalar_leaf() -> Function {
    let mut b = FunctionBuilder::new(
        FunctionId(12),
        "scalarleaf",
        vec![Param {
            name: "x".into(),
            ty: Ty::Word,
        }],
        vec![Ty::Word],
    );
    let x = b
        .push_op(OpKind::Move { value: ValueId(0) }, &[Ty::Word])
        .fixture()[0];
    b.terminate(Terminator::Return { values: vec![x] })
        .fixture();
    b.finish()
}

pub fn scalar_caller() -> Function {
    let mut b = FunctionBuilder::new(FunctionId(11), "scalarcaller", vec![], vec![Ty::Word]);
    let fixnum = b.add_constant(Constant::Fixnum(7));
    let value = b
        .push_op(OpKind::Const { result: fixnum }, &[Ty::I64])
        .fixture()[0];
    let argument = b
        .push_op(
            OpKind::Convert {
                op: ncl_ir::Convert::I64ToWord,
                value,
            },
            &[Ty::Word],
        )
        .fixture()[0];
    let entry = b.add_constant(Constant::FunctionEntry(FunctionId(12)));
    let function = b
        .push_op(OpKind::Const { result: entry }, &[Ty::Word])
        .fixture()[0];
    let result = b
        .push_op(
            OpKind::Call {
                function,
                args: vec![argument],
            },
            &[Ty::Word],
        )
        .fixture()[0];
    b.terminate(Terminator::Return {
        values: vec![result],
    })
    .fixture();
    b.finish()
}
