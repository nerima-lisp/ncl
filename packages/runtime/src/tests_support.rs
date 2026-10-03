use super::{NativeAbi, encode_maps, resolve_constant};
use crate::{PublishedFunction, Runtime, RuntimeError};
use ncl_codegen::{ContextField, RuntimeAbi, RuntimeFunction, SafepointMap};
use ncl_ir::{Constant, ConstantIndex, FunctionId, StructureKind};
use ncl_object::{BuiltinIdentifier, BuiltinName, BuiltinPackage, Word};

fn runtime() -> Runtime {
    // check-added-lines: allow(panic) test-only
    Runtime::new().unwrap_or_else(|error| panic!("runtime setup failed: {error}"))
}

#[test]
fn resolve_constant_covers_scalars_objects_and_allocated_values() {
    let mut runtime = runtime();
    let previous = [Word::fixnum(17), Word::fixnum(25)];
    let cases = [
        (Constant::Fixnum(-4), Word::fixnum(-4)),
        (Constant::Character('A' as u32), Word::character('A' as u32)),
        (Constant::Nil, Word::NIL),
        (Constant::T, Word::TRUE),
        (Constant::Unbound, Word::UNBOUND),
        (Constant::FunctionEntry(FunctionId(3)), Word::NIL),
        (Constant::Object(ConstantIndex(1)), previous[1]),
    ];

    for (constant, expected) in cases {
        let value = resolve_constant(&mut runtime.context, &runtime.object, &constant, &previous)
            // check-added-lines: allow(panic) test-only
            .unwrap_or_else(|error| panic!("{constant:?}: {error}"));
        assert_eq!(value, expected, "{constant:?}");
    }

    let allocated = [
        Constant::StringBytes(b"support".to_vec()),
        Constant::Structure {
            kind: StructureKind::Cons,
            elements: vec![ConstantIndex(0), ConstantIndex(1)],
        },
        Constant::Structure {
            kind: StructureKind::SimpleVector,
            elements: vec![ConstantIndex(0), ConstantIndex(1)],
        },
        Constant::SingleFloat(1.5),
        Constant::DoubleFloat(2.5),
        Constant::Symbol {
            package: "COMMON-LISP".to_owned(),
            name: "CAR".to_owned(),
        },
        Constant::Bignum {
            negative: true,
            limbs: vec![1, 2],
        },
        Constant::Ratio {
            numerator: ConstantIndex(0),
            denominator: ConstantIndex(1),
        },
        Constant::Complex {
            real: ConstantIndex(0),
            imaginary: ConstantIndex(1),
        },
    ];
    for constant in allocated {
        let value = resolve_constant(&mut runtime.context, &runtime.object, &constant, &previous)
            // check-added-lines: allow(panic) test-only
            .unwrap_or_else(|error| panic!("{constant:?}: {error}"));
        assert_ne!(value, Word::NIL, "{constant:?}");
    }
}

#[test]
fn resolve_constant_reports_invalid_references_and_descriptors() {
    let mut runtime = runtime();
    let previous = [Word::fixnum(1), Word::fixnum(2)];
    let cases = [
        Constant::Object(ConstantIndex(2)),
        Constant::StringBytes(vec![0xff]),
        Constant::Structure {
            kind: StructureKind::Cons,
            elements: vec![ConstantIndex(0)],
        },
        Constant::Symbol {
            package: "MISSING-PACKAGE".to_owned(),
            name: "VALUE".to_owned(),
        },
        Constant::Ratio {
            numerator: ConstantIndex(9),
            denominator: ConstantIndex(0),
        },
        Constant::Complex {
            real: ConstantIndex(0),
            imaginary: ConstantIndex(9),
        },
    ];

    for constant in cases {
        let result = resolve_constant(&mut runtime.context, &runtime.object, &constant, &previous);
        assert!(
            matches!(result, Err(RuntimeError::Object(_))),
            "{constant:?}"
        );
    }
}

#[test]
fn native_abi_dispatches_registered_and_supported_selectors() {
    let runtime = runtime();
    let functions = std::iter::once((7, PublishedFunction { entry: 0x1234 })).collect();
    let abi = NativeAbi {
        object: &runtime.object,
        functions: &functions,
        undefined_function_stub: 0x5678,
    };
    let builtin = BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new("CAR"));

    assert!(abi.builtin_address(builtin).is_ok());
    assert!(
        abi.builtin_address(BuiltinIdentifier::new(
            BuiltinPackage::CommonLisp,
            BuiltinName::new("MISSING"),
        ))
        .is_err()
    );

    for field in [
        ContextField::TlabBump,
        ContextField::TlabLimit,
        ContextField::SafepointRequest,
        ContextField::MultipleValueArea,
        ContextField::Pending,
        ContextField::MultipleValueCount,
        ContextField::Handler,
        ContextField::Cleanup,
        ContextField::Catch,
    ] {
        assert!(abi.field_offset(field).is_ok(), "{field:?}");
    }

    for function in [
        RuntimeFunction::SafepointSlow,
        RuntimeFunction::MakeClosure,
        RuntimeFunction::MakeValueCell,
        RuntimeFunction::EnterCatch,
        RuntimeFunction::LeaveCatch,
        RuntimeFunction::EnterUnwindProtect,
        RuntimeFunction::LeaveUnwindProtect,
        RuntimeFunction::EnterProgv,
        RuntimeFunction::LeaveProgv,
    ] {
        assert!(abi.runtime_address(function).is_ok(), "{function:?}");
    }
    assert_eq!(
        abi.runtime_address(RuntimeFunction::UndefinedFunction),
        Ok(0x5678)
    );
    for function in [
        RuntimeFunction::AllocateSlow,
        RuntimeFunction::Unwind,
        RuntimeFunction::Builtin,
        RuntimeFunction::ConstantTable,
    ] {
        assert!(abi.runtime_address(function).is_err(), "{function:?}");
    }

    assert_eq!(
        abi.constant_word_named(ncl_codegen::ConstantName::new("function-entry:7")),
        Some(0x1234)
    );
    for name in [
        "function-entry:nope",
        "not-a-function-entry",
        "function-entry:8",
    ] {
        assert_eq!(
            abi.constant_word_named(ncl_codegen::ConstantName::new(name)),
            None
        );
    }
}

#[test]
fn encode_maps_concatenates_valid_maps_and_reports_invalid_maps() {
    let first = SafepointMap::new(3, 4, 4, &[2], &[0, 3], 1)
        // check-added-lines: allow(panic) test-only
        .unwrap_or_else(|error| panic!("first map: {error}"));
    let second = SafepointMap::new(8, 5, 5, &[2, 4], &[14], 2)
        // check-added-lines: allow(panic) test-only
        .unwrap_or_else(|error| panic!("second map: {error}"));
    let bytes =
        // check-added-lines: allow(panic) test-only
        encode_maps(&[first, second]).unwrap_or_else(|error| panic!("encoded maps: {error}"));
    assert!(ncl_sys::SafepointMap::decode(&bytes, 2).is_ok());

    let mut invalid = SafepointMap::new(0, 4, 4, &[], &[0, 3], 0)
        // check-added-lines: allow(panic) test-only
        .unwrap_or_else(|error| panic!("invalid map fixture: {error}"));
    invalid.registers = vec![0, 4];
    assert!(matches!(
        encode_maps(&[invalid]),
        Err(RuntimeError::Native(message)) if message == "safepoint header is inconsistent"
    ));
}
