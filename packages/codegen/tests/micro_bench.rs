#![allow(clippy::expect_used, missing_docs)]

use ncl_codegen::{
    AbiError, ContextField, FLAG_ALLOCATION_SLOW, RuntimeAbi, RuntimeFunction,
    compile_function_aarch64,
};
use ncl_disasm::{Architecture, DecodedInstruction, decode};
use ncl_ir::{BlockId, Constant, FunctionBuilder, OpKind, Prim, Terminator, Ty, ValueId};

struct MicroBenchAbi;

impl RuntimeAbi for MicroBenchAbi {
    fn builtin_address(&self, identifier: ncl_object::BuiltinIdentifier) -> Result<u64, AbiError> {
        Err(AbiError::MissingBuiltin(identifier))
    }

    fn field_offset(&self, field: ContextField) -> Result<i32, AbiError> {
        let layout = ncl_sys::thread_layout();
        let offset = match field {
            ContextField::TlabBump => layout.tlab_bump,
            ContextField::TlabLimit => layout.tlab_limit,
            ContextField::SafepointRequest => layout.safepoint_request,
            ContextField::Pending => layout.pending,
            ContextField::MultipleValueCount => layout.mv_count,
            ContextField::MultipleValueArea => layout.mv,
            ContextField::Handler => layout.handler,
            ContextField::Cleanup => layout.cleanup,
            ContextField::Catch => layout.catch,
            ContextField::RuntimeErrorFlags => layout.runtime_error_flags,
        };
        i32::try_from(offset).map_err(|_| AbiError::UnsupportedContextField(field))
    }

    fn runtime_address(&self, _function: RuntimeFunction) -> Result<u64, AbiError> {
        Ok(0x1000)
    }
}

fn fixnum_i64(builder: &mut FunctionBuilder, value: i64) -> ValueId {
    let constant = builder.add_constant(Constant::Fixnum(value));
    builder
        .push_op(OpKind::Const { result: constant }, &[Ty::I64])
        .expect("fixnum constant")[0]
}

fn fixnum_loop() -> ncl_ir::Function {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(110),
        "micro-fixnum-loop",
        Vec::new(),
        vec![Ty::I64],
    );
    let index = builder.fresh_value();
    let sum = builder.fresh_value();
    let header = builder.create_block(vec![(Ty::I64, index), (Ty::I64, sum)]);
    let body = builder.create_block(Vec::new());
    let exit_sum = builder.fresh_value();
    let exit = builder.create_block(vec![(Ty::I64, exit_sum)]);

    builder.position_at(BlockId(0)).expect("entry block");
    let zero = fixnum_i64(&mut builder, 0);
    let one = fixnum_i64(&mut builder, 1);
    let limit = fixnum_i64(&mut builder, 100);
    builder
        .terminate(Terminator::Jump {
            target: header,
            args: vec![zero, zero],
        })
        .expect("enter loop");

    builder.position_at(header).expect("loop header");
    let condition = builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumLt,
                args: vec![index, limit],
                condition: None,
            },
            &[Ty::Bool],
        )
        .expect("loop comparison")[0];
    builder
        .terminate(Terminator::Branch {
            condition,
            then_target: body,
            then_args: Vec::new(),
            else_target: exit,
            else_args: vec![sum],
        })
        .expect("loop branch");

    builder.position_at(body).expect("loop body");
    let next_index = builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumAdd,
                args: vec![index, one],
                condition: None,
            },
            &[Ty::I64],
        )
        .expect("increment loop index")[0];
    let next_sum = builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumAdd,
                args: vec![sum, index],
                condition: None,
            },
            &[Ty::I64],
        )
        .expect("accumulate loop index")[0];
    builder
        .terminate(Terminator::Jump {
            target: header,
            args: vec![next_index, next_sum],
        })
        .expect("loop backedge");

    builder.position_at(exit).expect("loop exit");
    builder
        .terminate(Terminator::Return {
            values: vec![exit_sum],
        })
        .expect("return loop sum");
    builder.finish()
}

fn cons_allocation() -> ncl_ir::Function {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(111),
        "micro-cons-allocation",
        Vec::new(),
        vec![Ty::Address],
    );
    let cons = builder
        .push_op(OpKind::Alloc { words: 2 }, &[Ty::Address])
        .expect("allocate two-word cons")[0];
    builder
        .terminate(Terminator::Return { values: vec![cons] })
        .expect("return cons");
    builder.finish()
}

fn decoded(code: &[u8]) -> Vec<DecodedInstruction> {
    const LESS_THAN_SELECT: u32 = 0x9a91_b210;
    let mut instructions = Vec::new();
    let (words, remainder) = code.as_chunks::<4>();
    for (index, bytes) in words.iter().enumerate() {
        let address = u64::try_from(index * 4).expect("instruction address");
        let instruction_word = u32::from_le_bytes(*bytes);
        match decode(Architecture::Aarch64, bytes, address) {
            Ok(mut decoded) => instructions.append(&mut decoded),
            // ncl-disasm does not yet decode CSEL, so retain its exact expected
            // word here and decode every other generated instruction normally.
            Err(_) if instruction_word == LESS_THAN_SELECT => {
                instructions.push(DecodedInstruction {
                    address,
                    size: 4,
                    bytes: bytes.to_vec(),
                    text: "csel x16, x16, x17, lt".into(),
                    branch_target: None,
                });
            }
            Err(error) => panic!("decode instruction at {address}: {error}"),
        }
    }
    assert!(remainder.is_empty());
    instructions
}

fn word(instruction: &DecodedInstruction) -> u32 {
    u32::from_le_bytes(
        instruction
            .bytes
            .as_slice()
            .try_into()
            .expect("AArch64 word"),
    )
}

#[test]
fn fixnum_loop_codegen_compares_selects_adds_and_branches_back() {
    let compiled =
        compile_function_aarch64(&fixnum_loop(), &MicroBenchAbi).expect("compile fixnum loop");
    let instructions = decoded(&compiled.code);
    let compare = instructions
        .iter()
        .position(|instruction| word(instruction) == 0xeb11_021f)
        .expect("signed comparison of x16 and x17");
    let select = instructions
        .iter()
        .position(|instruction| word(instruction) == 0x9a91_b210)
        .expect("less-than selects true or NIL");
    let additions = instructions
        .iter()
        .enumerate()
        .filter_map(|(index, instruction)| (word(instruction) == 0x8b11_0210).then_some(index))
        .collect::<Vec<_>>();
    let backedge = instructions
        .iter()
        .enumerate()
        .find(|(_, instruction)| {
            instruction
                .branch_target
                .is_some_and(|target| target < instruction.address)
        })
        .map(|(index, _)| index)
        .expect("branch back to the loop header");

    assert!(
        compare < select,
        "comparison must feed the Lisp boolean selection"
    );
    assert_eq!(
        additions.len(),
        2,
        "loop has index and accumulator additions"
    );
    assert!(additions.iter().all(|index| *index < backedge));
    assert!(instructions[backedge].text.starts_with("b #"));
}

#[test]
fn cons_codegen_reserves_two_heap_words_on_the_tlab_fast_path() {
    let compiled = compile_function_aarch64(&cons_allocation(), &MicroBenchAbi)
        .expect("compile cons allocation");
    let instructions = decoded(&compiled.code);
    let cons_bytes = 2 * core::mem::size_of::<ncl_sys::Word>();
    let reserve = instructions
        .iter()
        .position(|instruction| instruction.text == format!("add x0, x16, #{cons_bytes}"))
        .expect("advance the TLAB bump pointer by the cons byte size");
    let compare_limit = instructions
        .iter()
        .position(|instruction| word(instruction) == 0xeb11_001f)
        .expect("compare the proposed bump pointer with the TLAB limit");
    let check_limit = instructions
        .iter()
        .position(|instruction| instruction.text.starts_with("b.hi #"))
        .expect("branch to allocation slow path when the TLAB limit is exceeded");
    let bump_offset = ncl_sys::thread_layout().tlab_bump;
    let update_bump = instructions
        .iter()
        .position(|instruction| instruction.text == format!("str x0, [x21, #{bump_offset}]"))
        .expect("store the new TLAB bump pointer on the fast path");
    let map = compiled
        .safepoint_maps
        .first()
        .expect("allocation safepoint map");

    assert_eq!(cons_bytes, 16, "a cons contains two eight-byte words");
    assert!(reserve < compare_limit && compare_limit < check_limit);
    assert!(
        check_limit < update_bump,
        "update the TLAB after the capacity check"
    );
    assert!(
        instructions[check_limit]
            .branch_target
            .is_some_and(|target| target > instructions[check_limit].address)
    );
    assert_ne!(map.map_flags & FLAG_ALLOCATION_SLOW, 0);
}
