#![allow(missing_docs)]

use ncl_object::array::{vector_pop, vector_push, vector_push_extend};
use ncl_object::hash_table::{HashTable, HashTest, Weakness};
use ncl_object::typed::{FunctionDesignator, List};
use ncl_object::{
    ArrayElementType, ArrayOptions, BuiltinArgs, BuiltinFunctionCaller, FromLispArg,
    FunctionArguments, FunctionCaller, LispError, Local, MultipleValues, ObjectError, ObjectType,
    Runtime, Scope, ThreadContext, Word, WordView, make_array, make_bignum_from_i128, make_double,
    make_ratio, make_simple_vector, make_string,
};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    (runtime, ctx)
}

#[test]
fn hash_table_options_accept_numeric_forms_and_reject_invalid_values() {
    let (runtime, mut ctx) = setup();
    let positive = make_bignum_from_i128(&mut ctx, &runtime, 17)
        .unwrap_or_else(|error| panic!("bignum: {error:?}"));
    let ratio = make_ratio(&mut ctx, &runtime, Word::fixnum(1), Word::fixnum(2))
        .unwrap_or_else(|error| panic!("ratio: {error:?}"));
    let factor =
        make_double(&mut ctx, &runtime, 2.0).unwrap_or_else(|error| panic!("double: {error:?}"));

    let table = HashTable::new_with_options(
        &mut ctx,
        &runtime,
        HashTest::Equalp,
        Weakness::Value,
        positive.into(),
        Some(factor.into()),
        Some(ratio.into()),
    )
    .unwrap_or_else(|error| panic!("valid options: {error:?}"));
    assert_eq!(table.test(&ctx), Ok(HashTest::Equalp));
    assert_eq!(table.weakness(&ctx), Ok(Weakness::Value));
    assert_eq!(table.capacity(&ctx), Ok(32));
    assert_eq!(table.rehash_size(&ctx), Ok(factor.into()));
    assert_eq!(table.rehash_threshold(&ctx), Ok(ratio.into()));

    for size in [Word::NIL, Word::fixnum(0), Word::fixnum(-1)] {
        assert_eq!(
            HashTable::new_with_options(
                &mut ctx,
                &runtime,
                HashTest::Eq,
                Weakness::None,
                size,
                None,
                None,
            ),
            Err(ObjectError::TypeError)
        );
    }
    let negative = make_bignum_from_i128(&mut ctx, &runtime, -2)
        .unwrap_or_else(|error| panic!("negative bignum: {error:?}"));
    assert_eq!(
        HashTable::new_with_options(
            &mut ctx,
            &runtime,
            HashTest::Eq,
            Weakness::None,
            negative.into(),
            None,
            None,
        ),
        Err(ObjectError::TypeError)
    );
    for rehash_size in [Word::fixnum(0), Word::TRUE] {
        assert_eq!(
            HashTable::new_with_options(
                &mut ctx,
                &runtime,
                HashTest::Eq,
                Weakness::None,
                Word::fixnum(8),
                Some(rehash_size),
                None,
            ),
            Err(ObjectError::TypeError)
        );
    }
    for threshold in [Word::fixnum(0), Word::fixnum(2), Word::TRUE] {
        assert_eq!(
            HashTable::new_with_options(
                &mut ctx,
                &runtime,
                HashTest::Eq,
                Weakness::None,
                Word::fixnum(8),
                None,
                Some(threshold),
            ),
            Err(ObjectError::TypeError)
        );
    }
}

#[test]
fn vectors_report_push_extend_and_pop_state_under_forwarding() {
    let (runtime, mut ctx) = setup();
    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);
    let mut vector = make_array(
        &mut ctx,
        &runtime,
        &[1],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::NIL,
            adjustable: true,
            fill_pointer: Some(0),
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )
    .unwrap_or_else(|error| panic!("vector: {error:?}"));
    let token = ncl_object::push_root(&mut ctx, &mut vector);
    assert_eq!(vector_push(&mut ctx, vector, Word::fixnum(7)), Ok(Some(0)));
    assert_eq!(vector_push(&mut ctx, vector, Word::fixnum(8)), Ok(None));
    let value =
        make_string(&mut ctx, &runtime, &['x']).unwrap_or_else(|error| panic!("value: {error:?}"));
    let (index, extended) = vector_push_extend(&mut ctx, &runtime, vector, value, 2)
        .unwrap_or_else(|error| panic!("extend: {error:?}"));
    assert_eq!(index, 1);
    assert_eq!(vector_pop(&mut ctx, extended), Ok(value));
    assert_eq!(vector_pop(&mut ctx, extended), Ok(Word::fixnum(7)));
    assert_eq!(vector_pop(&mut ctx, extended), Err(ObjectError::TypeError));
    assert_eq!(
        vector_push_extend(&mut ctx, &runtime, extended, Word::NIL, 0),
        Err(ObjectError::TypeError)
    );
    assert!(ncl_object::pop_root(&mut ctx, token));
}

#[test]
fn typed_views_cover_conversions_and_expected_type_names() {
    let (runtime, mut ctx) = setup();
    assert_eq!(
        <Word as FromLispArg>::from_lisp_arg(&ctx, Word::fixnum(4)),
        Ok(Word::fixnum(4))
    );
    assert_eq!(
        ncl_object::Fixnum::from_lisp_arg(&ctx, Word::fixnum(4)).map(ncl_object::Fixnum::value),
        Ok(4)
    );
    assert_eq!(List::from_lisp_arg(&ctx, Word::NIL), Ok(List::Nil));
    assert_eq!(
        List::from_lisp_arg(&ctx, Word::TRUE),
        Err(LispError::TypeError {
            datum: Word::TRUE,
            expected: ObjectType::Cons
        })
    );

    let string =
        make_string(&mut ctx, &runtime, &['s']).unwrap_or_else(|error| panic!("string: {error:?}"));
    let vector = make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1)])
        .unwrap_or_else(|error| panic!("vector: {error:?}"));
    for (word, view) in [
        (
            string,
            WordView::String(ncl_object::StringObject::from_word(string)),
        ),
        (
            vector,
            WordView::SimpleVector(ncl_object::SimpleVector::from_word(vector)),
        ),
    ] {
        assert_eq!(
            WordView::from(ncl_object::classify_object(&ctx, word)),
            view
        );
        assert_eq!(view.as_word(), word);
    }
    for (object_type, name) in [
        (ObjectType::String, "string"),
        (ObjectType::SimpleVector, "simple-vector"),
        (ObjectType::SpecializedArray, "specialized-array"),
        (ObjectType::Structure, "structure-object"),
        (ObjectType::Code, "code"),
    ] {
        assert_eq!(object_type.name(), name);
    }
    assert_eq!(
        WordView::try_from_word(string, ObjectType::String),
        Err(ncl_object::TypeError {
            datum: string,
            expected: ObjectType::String
        })
    );
}

#[derive(Debug)]
struct EchoCaller;

impl FunctionCaller for EchoCaller {
    fn call_function(
        &mut self,
        _ctx: &mut ThreadContext,
        _runtime: &Runtime,
        _designator: FunctionDesignator,
        args: FunctionArguments<'_>,
        values: &mut MultipleValues,
    ) -> Result<Word, ObjectError> {
        let value = args.get(0).ok_or(ObjectError::Layout)?;
        values.set(&[value]);
        Ok(value)
    }
}

#[test]
fn scope_roots_values_and_calls_through_the_function_boundary() {
    let (runtime, mut ctx) = setup();
    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);
    let mut scope = Scope::new(&mut ctx);
    let values = scope.root_many(&[
        Local::from_word(Word::fixnum(10)),
        Local::from_word(Word::fixnum(20)),
    ]);
    assert_eq!(values.len(), 2);
    assert!(!values.is_empty());
    assert_eq!(values.as_slice()[0].index(), 0);
    assert_eq!(values.iter().count(), 2);
    let vector = scope
        .make_simple_vector(&runtime, &values)
        .unwrap_or_else(|error| panic!("scope vector: {error:?}"));
    let string = scope
        .make_string(&runtime, &['a', 'b'])
        .unwrap_or_else(|error| panic!("scope string: {error:?}"));
    let cons = scope
        .make_cons(&runtime, values.as_slice()[0], vector)
        .unwrap_or_else(|error| panic!("scope cons: {error:?}"));
    assert_eq!(
        scope
            .car(cons)
            .unwrap_or_else(|error| panic!("car: {error:?}"))
            .as_word(),
        Word::fixnum(10)
    );
    assert_eq!(
        scope
            .cdr(cons)
            .unwrap_or_else(|error| panic!("cdr: {error:?}"))
            .as_word(),
        scope.get(vector).as_word()
    );
    let list = scope
        .make_list(&runtime, &values)
        .unwrap_or_else(|error| panic!("scope list: {error:?}"));
    let elements = scope
        .list_to_handle_vec(Local::from_word(scope.get(list).as_word()))
        .unwrap_or_else(|error| panic!("list handles: {error:?}"));
    assert_eq!(
        scope
            .get_many(&elements)
            .into_iter()
            .map(Local::as_word)
            .collect::<Vec<_>>(),
        vec![Word::fixnum(10), Word::fixnum(20)]
    );
    let symbol = scope
        .intern(&runtime, "COMMON-LISP", "SCOPE-HOTSPOT")
        .unwrap_or_else(|error| panic!("intern: {error:?}"));
    let mut caller = EchoCaller;
    let mut multiple = MultipleValues::new();
    let result = scope
        .call_function(&runtime, symbol, &values, &mut caller, &mut multiple)
        .unwrap_or_else(|error| panic!("scope call: {error:?}"));
    assert_eq!(scope.get(result).as_word(), Word::fixnum(10));
    assert_eq!(multiple.as_slice(), &[Word::fixnum(10)]);
    assert_ne!(scope.get(symbol).as_word(), Word::NIL);
    assert!(scope.collect(true).is_ok());
    assert_eq!(
        ncl_object::string_length(scope.context(), scope.get(string).as_word()),
        Ok(2)
    );
}

#[test]
fn function_arguments_and_runtime_load_paths_report_contracts() {
    let (runtime, mut ctx) = setup();
    let words = [Word::fixnum(1), Word::fixnum(2)];
    let args = FunctionArguments::new(&words);
    assert_eq!(args.len(), 2);
    assert!(!args.is_empty());
    assert_eq!(args.get(1), Some(Word::fixnum(2)));
    assert_eq!(args.get(2), None);
    assert_eq!(args.as_slice(), &words);
    let mut caller = BuiltinFunctionCaller;
    let mut values = MultipleValues::new();
    let designator = FunctionDesignator::Symbol(ncl_object::typed::Symbol::from_word(Word::NIL));
    assert_eq!(
        caller.call_function(&mut ctx, &runtime, designator, args, &mut values),
        Err(ObjectError::UndefinedFunction)
    );
    assert_eq!(
        runtime.load_port(&mut ctx, &BuiltinArgs::new(&[]), &mut values),
        Err(ObjectError::Layout)
    );
}
