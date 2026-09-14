use super::*;
use ncl_ir::{Constant, FunctionBuilder, OpKind, Terminator, Ty};

#[test]
fn lowers_fixnum_return_to_decodable_x86() {
    let mut builder =
        FunctionBuilder::new(ncl_ir::FunctionId(0), "one", Vec::new(), vec![Ty::Word]);
    let constant = builder.add_constant(Constant::Fixnum(1));
    let Ok(values) = builder.push_op(OpKind::Const { result: constant }, &[Ty::Word]) else {
        assert!(false, "builder always has an entry block");
        return;
    };
    let value = values[0];
    assert!(builder
        .terminate(Terminator::Return {
            values: vec![value]
        })
        .is_ok());
    let Ok(compiled) = compile_function(&builder.finish(), &X86_64Abi) else {
        assert!(false, "valid IR lowers");
        return;
    };
    assert!(!compiled.code.is_empty());
    assert_eq!(compiled.code.last(), Some(&0xc3));
}

#[test]
fn encodes_safepoint_header_and_live_slot() {
    let Ok(map) = SafepointMap::new(7, 6, 6, &[4], &[], FLAG_CALL) else {
        assert!(false, "valid map");
        return;
    };
    let Ok(bytes) = map.encode() else {
        assert!(false, "valid wire format");
        return;
    };
    assert_eq!(&bytes[..4], &7_u32.to_le_bytes());
    assert_eq!(bytes[16] & (1 << 2), 1 << 2);
    assert_eq!(bytes[16] & (1 << 4), 1 << 4);
}
