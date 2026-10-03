#![allow(missing_docs, clippy::expect_used)]

use ncl_asm_x86_64::{Assembler, Imm, Inst, Mem, Reg};
use ncl_codegen::{X86_64Abi, compile_function_x86_64};
use ncl_ir::{Constant, FunctionBuilder, OpKind, Terminator, Ty};
#[cfg(target_arch = "x86_64")]
use ncl_sys::{Thread, alloc_code, invoke_entry, publish_code, write_code};
use ncl_sys::{Word, thread_layout};

fn encoded(instructions: impl IntoIterator<Item = Inst>) -> Vec<u8> {
    let mut assembler = Assembler::new();
    for instruction in instructions {
        assembler
            .emit(&instruction)
            .expect("expected instruction encoding");
    }
    assembler.bytes().to_vec()
}

#[test]
fn x86_64_set_multiple_values_writes_count_and_first_value_exactly() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(79),
        "set-multiple-values",
        Vec::new(),
        vec![Ty::Word],
    );
    let first_constant = builder.add_constant(Constant::Fixnum(11));
    let first = builder
        .push_op(
            OpKind::Const {
                result: first_constant,
            },
            &[Ty::Word],
        )
        .expect("first constant")[0];
    let second_constant = builder.add_constant(Constant::Fixnum(22));
    let second = builder
        .push_op(
            OpKind::Const {
                result: second_constant,
            },
            &[Ty::Word],
        )
        .expect("second constant")[0];
    let result = builder
        .push_op(
            OpKind::SetMultipleValues {
                values: vec![first, second],
            },
            &[Ty::Word],
        )
        .expect("multiple values")[0];
    builder
        .terminate(Terminator::Return {
            values: vec![result],
        })
        .expect("return");

    let compiled = compile_function_x86_64(&builder.finish(), &X86_64Abi).expect("compile");
    let count_store = encoded([
        Inst::MovRI(Reg::Rdx, Imm::I32(2)),
        Inst::MovMR(
            Mem::base(
                Reg::R15,
                i32::try_from(thread_layout().mv_count).expect("multiple value count offset"),
            ),
            Reg::Rdx,
        ),
    ]);
    assert!(
        compiled
            .code
            .windows(count_store.len())
            .any(|window| window == count_store)
    );

    let first_value_load = encoded([Inst::MovRI(
        Reg::R10,
        Imm::I32(i32::try_from(Word::fixnum(11).bits()).expect("first value immediate")),
    )]);
    assert!(
        compiled
            .code
            .windows(first_value_load.len())
            .any(|window| window == first_value_load)
    );

    let first_value_store = encoded([Inst::MovMR(
        Mem::base(
            Reg::R15,
            i32::try_from(thread_layout().mv).expect("multiple value area offset"),
        ),
        Reg::R10,
    )]);
    assert!(
        compiled
            .code
            .windows(first_value_store.len())
            .any(|window| window == first_value_store)
    );

    #[cfg(target_arch = "x86_64")]
    {
        let mut code = alloc_code(compiled.code.len()).expect("code allocation");
        write_code(&mut code, 0, &compiled.code).expect("code write");
        publish_code(&mut code).expect("code publication");
        let mut thread = Thread::new();
        let (value, count) = invoke_entry(
            &code,
            compiled.entry_offset as usize,
            &raw mut thread,
            0,
            [0; 4],
            0,
        );
        assert_eq!(value, Word::fixnum(11).bits());
        assert_eq!(count, 1);
    }
}
