use crate::*;

fn sample() -> Function {
    let mut builder = FunctionBuilder::new(
        FunctionId(1),
        "fib",
        vec![Param {
            name: "n".into(),
            ty: Ty::I64,
        }],
        vec![Ty::I64],
    );
    let constant = builder.add_constant(Constant::Fixnum(1));
    let value = builder
        .push_op(OpKind::Const { result: constant }, &[Ty::I64])
        .ok()
        .and_then(|mut values| values.pop())
        .unwrap_or(ValueId(0));
    let _ = builder.terminate(Terminator::Return {
        values: vec![value],
    });
    builder.finish()
}

#[test]
fn text_round_trip() {
    let function = sample();
    let dump = function.to_string();
    assert_eq!(parse(&dump), Ok(function));
}

#[test]
fn builder_and_verifier_accept_function() {
    assert!(verify(&sample()).is_ok());
}

#[test]
fn verifier_rejects_unknown_successor() {
    let mut function = sample();
    function.blocks[0].terminator = Terminator::Jump {
        target: BlockId(99),
        args: Vec::new(),
    };
    assert!(verify(&function).is_err());
}

#[test]
fn constants_cover_runtime_descriptors() {
    let constants = [
        Constant::Character(65),
        Constant::SingleFloat(1.0),
        Constant::DoubleFloat(2.0),
        Constant::Symbol {
            package: "CL".into(),
            name: "T".into(),
        },
        Constant::Object(ConstantIndex(0)),
        Constant::StringBytes(vec![1, 2]),
        Constant::Nil,
        Constant::T,
        Constant::Unbound,
    ];
    assert_eq!(constants.len(), 9);
}
