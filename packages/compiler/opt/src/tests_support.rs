use ncl_ir::{
    Constant, Function, FunctionBuilder, FunctionId, OpKind, Param, Terminator, Ty, ValueId,
};
use std::fmt::Debug;

pub trait Fixture<T> {
    fn fixture(self) -> T;
}

impl<T, E: Debug> Fixture<T> for Result<T, E> {
    fn fixture(self) -> T {
        self.unwrap_or_else(|_| std::process::exit(1))
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
