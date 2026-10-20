#![allow(
    missing_docs,
    clippy::expect_used,
    clippy::too_many_lines,
    clippy::unwrap_used
)]

use crate::tests_x86_64_fixture::X86_64FixtureAbi;
use crate::{AllocationTarget, Location, allocate, compile_function_x86_64};
use ncl_ir::{Constant, Convert, FunctionBuilder, OpKind, Terminator, Ty};

#[test]
#[cfg(test)]
fn x86_64_lowering_spills_values_across_safepoints() -> Result<(), String> {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(71),
        "allocated-root",
        Vec::new(),
        vec![Ty::Word],
    );
    let constant = builder.add_constant(Constant::Fixnum(9));
    let unboxed = match builder.push_op(OpKind::Const { result: constant }, &[Ty::I64]) {
        Ok(ids) => ids[0],
        Err(error) => unreachable!("constant: {error:?}"),
    };
    let value = match builder.push_op(
        OpKind::Convert {
            op: Convert::I64ToWord,
            value: unboxed,
        },
        &[Ty::Word],
    ) {
        Ok(ids) => ids[0],
        Err(error) => unreachable!("fixnum conversion: {error:?}"),
    };
    assert!(builder.push_op(OpKind::Safepoint, &[]).is_ok(), "safepoint");
    assert!(
        // check-added-lines: allow(panic) test-only assertion
        builder
            .terminate(Terminator::Return {
                values: vec![value],
            })
            .is_ok(),
        "return"
    );
    let function = builder.finish();
    let allocation = allocate(&function, AllocationTarget::X86_64);
    let Some(Location::Spill(spill)) = allocation.location(value) else {
        return Err("safepoint-crossing value was allocated to a register".into());
    };
    let compiled = match compile_function_x86_64(&function, &X86_64FixtureAbi) {
        Ok(compiled) => compiled,
        Err(error) => unreachable!("{error:?}"),
    };
    let Some(map) = compiled.safepoint_maps.first() else {
        return Err("safepoint map missing".into());
    };
    assert!(map.registers.is_empty()); // check-added-lines: allow(panic) test-only assertion
    let slot = 4 + 2 + usize::try_from(spill).map_err(|_| "spill slot overflow")?;
    let slot_is_live = map
        .bitmap
        .get(slot / 8)
        .is_some_and(|bits| bits & (1 << (slot % 8)) != 0);
    let slot_message = format!("spill slot {slot} is missing from the safepoint map");
    // check-added-lines: allow(panic) test-only assertion
    assert!(slot_is_live, "{}", slot_message); // check-added-lines: allow(panic) test-only assertion
    assert!(!compiled.code.is_empty());
    Ok(())
}

#[test]
fn x86_64_lowering_reserves_allocator_spills_in_frame() -> Result<(), String> {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(72),
        "allocated-spills",
        Vec::new(),
        vec![Ty::Word; 8],
    );
    let mut values = Vec::new();
    for index in 0..8u32 {
        let constant = builder.add_constant(Constant::Fixnum(i64::from(index)));
        let unboxed = match builder.push_op(OpKind::Const { result: constant }, &[Ty::I64]) {
            Ok(ids) => ids[0],
            Err(error) => unreachable!("constant: {error:?}"),
        };
        values.push(
            match builder.push_op(
                OpKind::Convert {
                    op: Convert::I64ToWord,
                    value: unboxed,
                },
                &[Ty::Word],
            ) {
                Ok(ids) => ids[0],
                Err(error) => unreachable!("fixnum conversion: {error:?}"),
            },
        );
    }
    assert!(builder.push_op(OpKind::Safepoint, &[]).is_ok(), "safepoint");
    assert!(
        builder.terminate(Terminator::Return { values }).is_ok(),
        "return"
    );
    let function = builder.finish();
    let allocation = allocate(&function, AllocationTarget::X86_64);
    assert!(allocation.spill_words > 0);
    let compiled = match compile_function_x86_64(&function, &X86_64FixtureAbi) {
        Ok(compiled) => compiled,
        Err(error) => unreachable!("{error:?}"),
    };
    assert!(compiled.frame_size >= (4 + 16 + allocation.spill_words) * 8);
    assert!(compiled.safepoint_maps[0].bitmap.len() > 1);
    let Some(map) = compiled.safepoint_maps.first() else {
        return Err("safepoint map missing".into());
    };
    for interval in &allocation.intervals {
        if interval.ty != Ty::Word || interval.start > 16 || 16 > interval.end {
            continue;
        }
        let Some(Location::Spill(spill)) = allocation.location(interval.value) else {
            continue;
        };
        let spill = usize::try_from(spill).map_err(|_| "spill slot")?;
        let slot = 4 + 16 + spill; // check-added-lines: allow(panic) regression assertion for map coverage.
        assert!(
            map.bitmap
                .get(slot / 8)
                .is_some_and(|bits| bits & (1 << (slot % 8)) != 0),
            "spill slot {slot} is missing from the header-inclusive map"
        );
    }
    Ok(())
}

#[path = "tests_x86_64_calls_handlers.rs"]
mod calls_handlers;

#[test]
fn x86_64_prologue_spills_argc_from_rdi_before_arguments() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(75),
        "argc-prologue",
        vec![
            ncl_ir::Param {
                name: "argc".into(),
                ty: Ty::Word,
            },
            ncl_ir::Param {
                name: "argument".into(),
                ty: Ty::Word,
            },
        ],
        vec![],
    );
    assert!(
        builder
            .terminate(Terminator::Return { values: Vec::new() })
            .is_ok()
    );

    let function = builder.finish();
    let allocation = allocate(&function, AllocationTarget::X86_64);
    assert_eq!(
        allocation.location(ncl_ir::ValueId(0)),
        Some(Location::Register(11))
    );
    assert_eq!(
        allocation.location(ncl_ir::ValueId(1)),
        Some(Location::Register(12))
    );
    let compiled = match compile_function_x86_64(&function, &X86_64FixtureAbi) {
        Ok(compiled) => compiled,
        Err(error) => unreachable!("argc lowering: {error:?}"),
    };
    assert!(
        compiled
            .code
            .windows(3)
            .any(|bytes| bytes == [0x49, 0x89, 0xfc])
    );
    assert!(
        compiled
            .code
            .windows(3)
            .any(|bytes| bytes == [0x49, 0x89, 0xf5])
    );
}

#[test]
fn x86_64_prologue_loads_overflow_arguments_from_r9() {
    let params = (0..6)
        .map(|index| ncl_ir::Param {
            name: format!("arg{index}"),
            ty: Ty::Word,
        })
        .collect();
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(76),
        "overflow-arguments",
        params,
        Vec::new(),
    );
    assert!(
        builder
            .terminate(Terminator::Return { values: Vec::new() })
            .is_ok()
    );
    let function = builder.finish();
    let allocation = allocate(&function, AllocationTarget::X86_64);
    assert!(matches!(
        allocation.location(ncl_ir::ValueId(4)),
        Some(Location::Spill(_))
    ));
    let compiled = match compile_function_x86_64(&function, &X86_64FixtureAbi) {
        Ok(compiled) => compiled,
        Err(error) => unreachable!("overflow argument lowering: {error:?}"),
    };
    assert!(
        compiled
            .code
            .windows(4)
            .any(|bytes| bytes == [0x4d, 0x8b, 0x19, 0x4c])
    );
    assert!(
        compiled
            .code
            .windows(4)
            .any(|bytes| bytes == [0x4d, 0x8b, 0x59, 0x08])
    );
}

#[test]
fn x86_64_prologue_initializes_load_arg_sources() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(78),
        "load-arg-result-values",
        vec![
            ncl_ir::Param {
                name: "left".into(),
                ty: Ty::Word,
            },
            ncl_ir::Param {
                name: "right".into(),
                ty: Ty::Word,
            },
            ncl_ir::Param {
                name: "third".into(),
                ty: Ty::Word,
            },
        ],
        vec![Ty::Word, Ty::Word],
    );
    let left = builder
        .push_op(OpKind::LoadArg { index: 1 }, &[Ty::Word])
        .unwrap_or_else(|error| panic!("left LoadArg: {error}"))[0];
    let right = builder
        .push_op(OpKind::LoadArg { index: 2 }, &[Ty::Word])
        .unwrap_or_else(|error| panic!("right LoadArg: {error}"))[0];
    assert!(
        builder
            .terminate(Terminator::Return {
                values: vec![left, right],
            })
            .is_ok()
    );
    let function = builder.finish();
    let allocation = allocate(&function, AllocationTarget::X86_64);
    assert_eq!(
        allocation.location(ncl_ir::ValueId(1)),
        Some(Location::Register(12))
    );
    assert_eq!(
        allocation.location(ncl_ir::ValueId(2)),
        Some(Location::Register(13))
    );
    let compiled = compile_function_x86_64(&function, &X86_64FixtureAbi)
        .unwrap_or_else(|error| panic!("entry parameter lowering: {error:?}"));
    assert!(
        compiled
            .code
            .windows(3)
            .any(|bytes| bytes == [0x49, 0x89, 0xf4])
    );
    assert!(
        compiled
            .code
            .windows(3)
            .any(|bytes| bytes == [0x49, 0x89, 0xd5])
    );
}

#[test]
fn x86_64_load_heap_constant_untags_each_indirection() -> Result<(), String> {
    let mut builder = ncl_ir::FunctionBuilder::new(
        ncl_ir::FunctionId(77),
        "untag-heap-constant",
        Vec::new(),
        vec![Ty::Word],
    );
    let constant = builder.add_constant(Constant::Symbol {
        package: "COMMON-LISP".to_owned(),
        name: "T".to_owned(),
    });
    let value = builder
        .push_op(OpKind::Const { result: constant }, &[Ty::Word])
        .map_err(|error| format!("heap constant: {error:?}"))?[0];
    builder
        .terminate(Terminator::Return {
            values: vec![value],
        })
        .map_err(|error| format!("return: {error:?}"))?;

    let compiled = compile_function_x86_64(&builder.finish(), &X86_64FixtureAbi)
        .map_err(|error| format!("compile: {error:?}"))?;
    let untag_count = compiled
        .code
        .windows(4)
        .filter(|bytes| matches!(bytes, [0x49, 0x83, _, 0xf8]))
        .count();
    if untag_count != 3 {
        return Err(format!(
            "heap constant indirections must be untagged: found {untag_count}"
        ));
    }
    Ok(())
}
