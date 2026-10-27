use super::*;

static LAST_ARRAY_BARRIER_SLOT: AtomicUsize = AtomicUsize::new(usize::MAX);
static LAST_ARRAY_BARRIER_OWNER: AtomicU64 = AtomicU64::new(0);

extern "C" fn array_write_barrier(_thread: NonNull<Thread>, slot: usize, owner: Word) {
    LAST_ARRAY_BARRIER_SLOT.store(slot, Ordering::SeqCst);
    LAST_ARRAY_BARRIER_OWNER.store(owner.bits(), Ordering::SeqCst);
}

struct ArrayAbi;

impl RuntimeAbi for ArrayAbi {
    fn builtin_address(
        &self,
        identifier: ncl_object::BuiltinIdentifier,
    ) -> Result<u64, ncl_codegen::AbiError> {
        BuiltinAbi.builtin_address(identifier)
    }

    fn field_offset(&self, field: ContextField) -> Result<i32, ncl_codegen::AbiError> {
        BuiltinAbi.field_offset(field)
    }

    fn runtime_address(&self, function: RuntimeFunction) -> Result<u64, ncl_codegen::AbiError> {
        if function == RuntimeFunction::WriteBarrier {
            Ok(array_write_barrier as *const () as usize as u64)
        } else {
            BuiltinAbi.runtime_address(function)
        }
    }
}

fn reference_function(prim: Prim, id: u32) -> ncl_ir::Function {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(id),
        "simple-vector-reference",
        vec![
            Param {
                name: "vector".into(),
                ty: Ty::Word,
            },
            Param {
                name: "index".into(),
                ty: Ty::Word,
            },
        ],
        vec![Ty::Word],
    );
    let value = builder
        .push_op(
            OpKind::Prim {
                op: prim,
                args: vec![ValueId(0), ValueId(1)],
                condition: None,
            },
            &[Ty::Word],
        )
        .expect("array reference")[0];
    builder
        .terminate(Terminator::Return {
            values: vec![value],
        })
        .expect("return");
    builder.finish()
}

fn store_then_reference_function() -> ncl_ir::Function {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(611),
        "simple-vector-set",
        vec![
            Param {
                name: "vector".into(),
                ty: Ty::Word,
            },
            Param {
                name: "index".into(),
                ty: Ty::Word,
            },
            Param {
                name: "value".into(),
                ty: Ty::Word,
            },
        ],
        vec![Ty::Word],
    );
    builder
        .push_op(
            OpKind::Prim {
                op: Prim::Aset,
                args: vec![ValueId(0), ValueId(1), ValueId(2)],
                condition: None,
            },
            &[Ty::Unit],
        )
        .expect("array set");
    let value = builder
        .push_op(
            OpKind::Prim {
                op: Prim::Aref,
                args: vec![ValueId(0), ValueId(1)],
                condition: None,
            },
            &[Ty::Word],
        )
        .expect("read after set")[0];
    builder
        .terminate(Terminator::Return {
            values: vec![value],
        })
        .expect("return");
    builder.finish()
}

const fn simple_vector() -> [u64; 5] {
    [
        0,
        Word::fixnum(3).bits(),
        Word::fixnum(10).bits(),
        Word::fixnum(20).bits(),
        Word::fixnum(30).bits(),
    ]
}

fn execute(function: &ncl_ir::Function, abi: &dyn RuntimeAbi, arguments: &[Word]) -> Word {
    let compiled = compile_function_x86_64(function, abi).expect("lowering");
    let mut code = alloc_code(compiled.code.len()).expect("code allocation");
    write_code(&mut code, 0, &compiled.code).expect("code write");
    publish_code(&mut code).expect("code publication");
    let mut thread = Thread::new();
    let mut raw_arguments = [0; 4];
    for (destination, argument) in raw_arguments.iter_mut().zip(arguments) {
        *destination = argument.bits();
    }
    let (value, count) = invoke_entry(
        &code,
        compiled.entry_offset as usize,
        &mut thread,
        u64::try_from(arguments.len()).expect("argument count fits u64"),
        raw_arguments,
        0,
    );
    assert_eq!(count, 1);
    Word::from_bits(value)
}

#[test]
fn executes_aref_and_svref_at_a_nonzero_simple_vector_index() {
    for (prim, function_id) in [(Prim::Aref, 610), (Prim::Svref, 612)] {
        let mut storage = simple_vector();
        let vector = Word::pointer(storage.as_mut_ptr() as usize, ncl_sys::LowTag::OtherPointer);
        let value = execute(
            &reference_function(prim, function_id),
            &ArrayAbi,
            &[vector, Word::fixnum(2)],
        );
        assert_eq!(value, Word::fixnum(30));
    }
}

#[test]
fn executes_aset_at_a_nonzero_index_and_barriers_the_payload_slot() {
    LAST_ARRAY_BARRIER_SLOT.store(usize::MAX, Ordering::SeqCst);
    LAST_ARRAY_BARRIER_OWNER.store(0, Ordering::SeqCst);
    let mut storage = simple_vector();
    let vector = Word::pointer(storage.as_mut_ptr() as usize, ncl_sys::LowTag::OtherPointer);
    let value = Word::fixnum(77);
    assert_eq!(
        execute(
            &store_then_reference_function(),
            &ArrayAbi,
            &[vector, Word::fixnum(2), value],
        ),
        value
    );
    assert_eq!(storage[4], value.bits());
    assert_eq!(
        LAST_ARRAY_BARRIER_SLOT.load(Ordering::SeqCst),
        ncl_object::simple_vector_offset::DATA + 2
    );
    assert_eq!(
        LAST_ARRAY_BARRIER_OWNER.load(Ordering::SeqCst),
        vector.bits()
    );
}
