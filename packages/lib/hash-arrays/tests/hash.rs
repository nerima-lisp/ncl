#![allow(missing_docs)]

use ncl_lib_hash_arrays::hash::{
    clrhash, gethash, hash_table_count, make_hash_table, remhash, set_hash_value, sxhash_value,
    HashTableOptions, LispValue,
};
use ncl_object::hash_table::HashTable;
use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, LambdaList, Parameter, ParameterType, Runtime, ThreadContext,
    Word,
};

fn runtime_and_context() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().expect("create runtime");
    let mut context = ThreadContext::new();
    context.register(&runtime).expect("register runtime");
    ncl_lib_hash_arrays::register(&mut context, &runtime).expect("register hash builtins");
    (runtime, context)
}

fn keyword(ctx: &mut ThreadContext, runtime: &Runtime, name: &str) -> Word {
    ncl_object::Package::from_word(
        runtime
            .ensure_package(ctx, "KEYWORD")
            .expect("keyword package"),
    )
    .intern(ctx, runtime, name)
    .expect("intern keyword")
    .0
}

fn callback(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ncl_object::ObjectError> {
    args.required(0)
}

const CALLBACK_PARAMETERS: &[Parameter] = &[
    Parameter {
        name: BuiltinName::new("KEY"),
        ty: ParameterType::Any,
    },
    Parameter {
        name: BuiltinName::new("VALUE"),
        ty: ParameterType::Any,
    },
];

#[test]
fn typed_hash_domain_round_trip_and_clear() {
    let (runtime, mut context) = runtime_and_context();
    let table = make_hash_table(&mut context, &runtime, HashTableOptions::default())
        .expect("make hash table");
    let key = LispValue::from_word(Word::fixnum(7));
    let value = LispValue::from_word(Word::fixnum(42));
    let default = LispValue::from_word(Word::NIL);

    set_hash_value(&mut context, &runtime, table, key, value).expect("insert");
    let found = gethash(&mut context, table, key, default).expect("gethash");
    assert_eq!(found.value, value);
    assert!(found.present);
    assert_eq!(hash_table_count(&context, table).expect("count"), 1);
    assert!(remhash(&mut context, &runtime, table, key).expect("remhash"));
    assert_eq!(clrhash(&mut context, &runtime, table).expect("clrhash"), 0);
}

#[test]
fn hash_builtins_make_with_keywords_and_access_weakness() {
    let (runtime, mut context) = runtime_and_context();
    let make = ncl_object::FunctionObject::try_from(
        runtime
            .function(&mut context, "COMMON-LISP", "MAKE-HASH-TABLE")
            .expect("make-hash-table function"),
    )
    .expect("function object");
    let weakness_keyword = keyword(&mut context, &runtime, "WEAKNESS");
    let key_keyword = keyword(&mut context, &runtime, "KEY");
    let table = runtime
        .call_builtin(&mut context, make, &[weakness_keyword, key_keyword])
        .expect("make hash table");
    let weakness = ncl_object::FunctionObject::try_from(
        runtime
            .function(&mut context, "COMMON-LISP", "HASH-TABLE-WEAKNESS")
            .expect("weakness function"),
    )
    .expect("function object");
    assert_eq!(
        runtime.call_builtin(&mut context, weakness, &[table]),
        Ok(Word::fixnum(1))
    );
}

#[test]
fn maphash_calls_registered_function_and_rejects_bad_arguments() {
    let (runtime, mut context) = runtime_and_context();
    let callback = runtime
        .register_builtin(
            &mut context,
            BuiltinIdentifier::new(
                BuiltinPackage::NclTest,
                BuiltinName::new("MAPHASH-CALLBACK"),
            ),
            BuiltinImplementation::direct(
                Builtin {
                    lambda_list: LambdaList::fixed(CALLBACK_PARAMETERS),
                    convention: BuiltinConvention::Direct(Arity::exact(2)),
                },
                callback,
            ),
        )
        .expect("register callback");
    let table = make_hash_table(&mut context, &runtime, HashTableOptions::default())
        .expect("make hash table");
    set_hash_value(
        &mut context,
        &runtime,
        table,
        LispValue::from_word(Word::fixnum(1)),
        LispValue::from_word(Word::fixnum(2)),
    )
    .expect("insert");
    let maphash = ncl_object::FunctionObject::try_from(
        runtime
            .function(&mut context, "COMMON-LISP", "MAPHASH")
            .expect("maphash function"),
    )
    .expect("function object");
    assert_eq!(
        runtime.call_builtin(
            &mut context,
            maphash,
            &[callback.as_word(), table.as_word()]
        ),
        Ok(table.as_word())
    );
    assert_eq!(
        runtime.call_builtin(&mut context, maphash, &[Word::NIL, table.as_word()]),
        Err(ncl_object::ObjectError::TypeError)
    );
}

#[test]
fn make_hash_table_rejects_odd_or_unknown_keywords() {
    let (runtime, mut context) = runtime_and_context();
    let make = ncl_object::FunctionObject::try_from(
        runtime
            .function(&mut context, "COMMON-LISP", "MAKE-HASH-TABLE")
            .expect("make-hash-table function"),
    )
    .expect("function object");
    let test_keyword = keyword(&mut context, &runtime, "TEST");
    let unknown_keyword = keyword(&mut context, &runtime, "NO-SUCH-KEY");
    assert_eq!(
        runtime.call_builtin(&mut context, make, &[test_keyword]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut context, make, &[unknown_keyword, Word::NIL]),
        Err(ncl_object::ObjectError::TypeError)
    );
}

#[test]
fn make_hash_table_accepts_all_standard_keywords() {
    let (runtime, mut context) = runtime_and_context();
    let make = ncl_object::FunctionObject::try_from(
        runtime
            .function(&mut context, "COMMON-LISP", "MAKE-HASH-TABLE")
            .expect("make-hash-table function"),
    )
    .expect("function object");
    let arguments = [
        keyword(&mut context, &runtime, "TEST"),
        keyword(&mut context, &runtime, "EQUALP"),
        keyword(&mut context, &runtime, "REHASH-SIZE"),
        Word::fixnum(2),
        keyword(&mut context, &runtime, "REHASH-THRESHOLD"),
        Word::fixnum(1),
        keyword(&mut context, &runtime, "WEAKNESS"),
        keyword(&mut context, &runtime, "KEY-OR-VALUE"),
    ];
    let table = runtime
        .call_builtin(&mut context, make, &arguments)
        .expect("make hash table with all keywords");
    let table = HashTable::from_word(table);
    assert_eq!(
        table.test(&context).expect("test"),
        ncl_object::hash_table::HashTest::Equalp
    );
    assert_eq!(
        table.weakness(&context).expect("weakness"),
        ncl_object::hash_table::Weakness::KeyOrValue
    );
}

#[test]
fn sxhash_builtin_exposes_the_typed_hash() {
    let (runtime, mut context) = runtime_and_context();
    let sxhash = ncl_object::FunctionObject::try_from(
        runtime
            .function(&mut context, "COMMON-LISP", "SXHASH")
            .expect("sxhash function"),
    )
    .expect("function object");
    let value = Word::fixnum(42);
    let expected = (sxhash_value(LispValue::from_word(value)) & (i64::MAX as u64)) as i64;
    assert_eq!(
        runtime.call_builtin(&mut context, sxhash, &[value]),
        Ok(Word::fixnum(expected))
    );
}
