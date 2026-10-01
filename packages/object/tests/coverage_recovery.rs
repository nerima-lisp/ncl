#![allow(missing_docs)]

use ncl_object::array::{
    adjust_array, adjustable_array_p, array_displacement, array_element_type,
    array_has_fill_pointer_p, fill_pointer, set_fill_pointer, vector_pop, vector_push,
    vector_push_extend,
};
use ncl_object::typed::{FunctionDesignator, List, PackageDesignator, Sequence, StringDesignator};
use ncl_object::{
    ArrayElementType, ArrayOptions, BuiltinArgs, BuiltinIdentifier, BuiltinName, BuiltinPackage,
    FromLispArg, FunctionObject, MultipleValues, ObjectError, ObjectType, Package, Runtime,
    ThreadContext, Word, WordView, array_dimensions, array_row_major_ref, array_row_major_set,
    classify_object, make_array, make_cons, make_string, pop_root, push_root, with_root,
    with_rooted_slice,
};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut context = ThreadContext::new();
    context
        .register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    context.set_strict_forwarding(true);
    (runtime, context)
}

#[test]
fn adjustable_fill_pointer_vector_reports_all_state_transitions() {
    let (runtime, mut context) = setup();
    context.set_gc_stress(true);
    let mut vector = make_array(
        &mut context,
        &runtime,
        &[2],
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
    let token = push_root(&mut context, &mut vector);

    assert_eq!(array_dimensions(&context, vector), Ok(vec![2]));
    assert_eq!(
        array_dimensions(&context, Word::NIL),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        array_element_type(&context, vector),
        Ok(ArrayElementType::T)
    );
    assert_eq!(array_displacement(&context, vector), Ok((Word::NIL, 0)));
    assert_eq!(adjustable_array_p(&context, vector), Ok(true));
    assert_eq!(array_has_fill_pointer_p(&context, vector), Ok(true));
    assert_eq!(fill_pointer(&context, vector), Ok(0));
    assert_eq!(
        vector_pop(&mut context, vector),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        vector_push(&mut context, Word::NIL, Word::TRUE),
        Err(ObjectError::TypeError)
    );

    assert_eq!(
        vector_push(&mut context, vector, Word::fixnum(7)),
        Ok(Some(0))
    );
    assert_eq!(fill_pointer(&context, vector), Ok(1));
    assert_eq!(vector_pop(&mut context, vector), Ok(Word::fixnum(7)));
    assert_eq!(
        set_fill_pointer(&mut context, vector, 5),
        Err(ObjectError::TypeError)
    );
    assert_eq!(set_fill_pointer(&mut context, vector, 2), Ok(()));
    assert_eq!(fill_pointer(&context, vector), Ok(2));

    let value = make_string(&mut context, &runtime, &['x', 'y'])
        .unwrap_or_else(|error| panic!("value: {error:?}"));
    let (index, extended) = vector_push_extend(&mut context, &runtime, vector, value, 2)
        .unwrap_or_else(|error| panic!("extend: {error:?}"));
    assert_eq!(index, 2);
    assert_eq!(array_row_major_ref(&context, extended, 2), Ok(value));
    assert_eq!(vector_pop(&mut context, extended), Ok(value));
    assert!(pop_root(&mut context, token));
}

#[test]
fn arrays_reject_wrong_types_and_adjust_copy_preserves_values() {
    let (runtime, mut context) = setup();
    let source = make_array(
        &mut context,
        &runtime,
        &[2],
        ArrayOptions {
            element_type: ArrayElementType::Fixnum,
            initial_element: Word::fixnum(4),
            adjustable: true,
            fill_pointer: None,
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )
    .unwrap_or_else(|error| panic!("source: {error:?}"));
    assert_eq!(
        array_row_major_set(&mut context, source, 0, Word::TRUE),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        array_row_major_ref(&context, source, 2),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        adjust_array(&mut context, &runtime, Word::NIL, &[2], Word::NIL),
        Err(ObjectError::TypeError)
    );

    let adjusted = adjust_array(&mut context, &runtime, source, &[3], Word::fixnum(9))
        .unwrap_or_else(|error| panic!("adjust: {error:?}"));
    assert_eq!(array_dimensions(&context, adjusted), Ok(vec![3]));
    assert_eq!(
        array_row_major_ref(&context, adjusted, 0),
        Ok(Word::fixnum(4))
    );
    assert_eq!(
        array_row_major_ref(&context, adjusted, 2),
        Ok(Word::fixnum(9))
    );
}

#[test]
fn runtime_structure_registry_and_function_tables_are_observable() {
    let (runtime, mut context) = setup();
    let package = Package::new(&mut context, &runtime, "STRUCT-TEST")
        .unwrap_or_else(|error| panic!("package: {error:?}"));
    let (name, _) = package
        .intern(&mut context, &runtime, "CHILD")
        .unwrap_or_else(|error| panic!("intern: {error:?}"));
    let parent = runtime
        .register_structure_layout(1)
        .unwrap_or_else(|error| panic!("parent layout: {error:?}"));
    let child = runtime
        .register_structure_layout(2)
        .unwrap_or_else(|error| panic!("child layout: {error:?}"));

    runtime
        .register_structure_class_with_parent(&context, parent, None, name)
        .unwrap_or_else(|error| panic!("parent: {error:?}"));
    runtime
        .register_structure_class_with_parent(&context, child, Some(parent), name)
        .unwrap_or_else(|error| panic!("child: {error:?}"));
    assert_eq!(
        runtime.structure_class_name(&context, name),
        Ok("STRUCT-TEST::CHILD".into())
    );
    assert_eq!(
        runtime.structure_layout_for_symbol(&context, name),
        Some(child)
    );
    assert!(runtime.structure_layout_is_a(child, parent));
    assert!(runtime.structure_layout_is_a(child, child));
    assert!(!runtime.structure_layout_is_a(parent, child));
    assert_eq!(
        runtime.structure_layout_for_symbol(&context, Word::NIL),
        None
    );
    assert_eq!(
        runtime.structure_class_name(&context, Word::NIL),
        Err(ObjectError::TypeError)
    );

    let id = BuiltinIdentifier::new(BuiltinPackage::NclTest, BuiltinName::new("RECOVERY"));
    assert_eq!(runtime.builtin_address(id), None);
    runtime.register_builtin_address(id, 0x1234);
    assert_eq!(runtime.builtin_address(id), Some(0x1234));
    runtime
        .install_generic_builtin_entry(&mut context, 0x2345)
        .unwrap_or_else(|error| panic!("generic entry: {error:?}"));
    runtime
        .define_function(&mut context, "STRUCT-TEST", "F", Word::TRUE)
        .unwrap_or_else(|error| panic!("define function: {error:?}"));
    assert_eq!(
        runtime.function(&mut context, "STRUCT-TEST", "F"),
        Some(Word::TRUE)
    );
    assert_eq!(
        runtime.function(&mut context, "STRUCT-TEST", "MISSING"),
        None
    );
}

#[test]
fn root_helpers_and_builtin_argument_views_preserve_results() {
    let (runtime, mut context) = setup();
    let cons = make_cons(&mut context, &runtime, Word::fixnum(1), Word::NIL)
        .unwrap_or_else(|error| panic!("cons: {error:?}"));
    let mut rooted = cons;
    let result = with_root(&mut context, &mut rooted, |ctx, slot| {
        ctx.collect(true)?;
        Ok(*slot)
    })
    .unwrap_or_else(|error| panic!("root: {error:?}"));
    assert_eq!(
        classify_object(&context, result),
        classify_object(&context, rooted)
    );

    let values = [Word::fixnum(1), Word::fixnum(2)];
    let sum = with_rooted_slice(&mut context, &values, |ctx, rooted| {
        rooted[0] = Word::fixnum(9);
        ctx.collect(true)?;
        Ok(rooted
            .iter()
            .map(|word| {
                word.as_fixnum()
                    .unwrap_or_else(|| panic!("fixnum: {word:?}"))
            })
            .sum::<i64>())
    })
    .unwrap_or_else(|error| panic!("rooted slice: {error:?}"));
    assert_eq!(sum, 11);
    assert_eq!(values, [Word::fixnum(1), Word::fixnum(2)]);
    let mut error_value = Word::NIL;
    assert_eq!(
        with_root(&mut context, &mut error_value, |_ctx, _slot| {
            Err::<Word, ObjectError>(ObjectError::Unsupported)
        }),
        Err(ObjectError::Unsupported)
    );
    assert_eq!(
        with_rooted_slice(&mut context, &[Word::NIL], |_ctx, _rooted| {
            Err::<i32, ObjectError>(ObjectError::Unsupported)
        }),
        Err(ObjectError::Unsupported)
    );

    let argument_words = [Word::fixnum(3)];
    let args = BuiltinArgs::new(&argument_words);
    assert_eq!(args.len(), 1);
    assert!(!args.is_empty());
    assert_eq!(args.as_slice(), &[Word::fixnum(3)]);
    assert_eq!(args.get(1), None);
    assert_eq!(args.required(1), Err(ObjectError::TypeError));
    let mut multiple = MultipleValues::new();
    multiple.push(Word::TRUE);
    multiple.set(&[Word::fixnum(8), Word::NIL]);
    assert_eq!(multiple.as_slice(), &[Word::fixnum(8), Word::NIL]);
    assert_eq!(multiple.len(), 2);
    multiple.clear();
    assert!(multiple.is_empty());
}

#[test]
fn typed_views_cover_designators_and_expected_type_names() {
    let (runtime, mut context) = setup();
    let package = Package::new(&mut context, &runtime, "TYPED-TEST")
        .unwrap_or_else(|error| panic!("package: {error:?}"));
    let (symbol, _) = package
        .intern(&mut context, &runtime, "S")
        .unwrap_or_else(|error| panic!("symbol: {error:?}"));
    let string = make_string(&mut context, &runtime, &['s'])
        .unwrap_or_else(|error| panic!("string: {error:?}"));
    let cons = make_cons(&mut context, &runtime, Word::NIL, Word::NIL)
        .unwrap_or_else(|error| panic!("cons: {error:?}"));

    assert_eq!(
        FunctionDesignator::try_from_word(&context, symbol),
        Ok(FunctionDesignator::Symbol(
            ncl_object::typed::Symbol::from_word(symbol)
        ))
    );
    assert_eq!(
        FunctionDesignator::try_from_word(&context, Word::fixnum(1)),
        Err(ObjectError::TypeError)
    );
    assert_eq!(List::from_lisp_arg(&context, Word::NIL), Ok(List::Nil));
    assert!(matches!(
        List::from_lisp_arg(&context, cons),
        Ok(List::Cons(_))
    ));
    assert_eq!(
        List::from_lisp_arg(&context, Word::TRUE),
        Err(ncl_object::LispError::TypeError {
            datum: Word::TRUE,
            expected: ObjectType::Cons
        })
    );
    assert_eq!(
        WordView::try_from_word(Word::fixnum(2), ObjectType::Fixnum),
        Ok(WordView::Fixnum(2))
    );
    assert_eq!(
        WordView::try_from_word(Word::TRUE, ObjectType::Fixnum),
        Err(ncl_object::TypeError {
            datum: Word::TRUE,
            expected: ObjectType::Fixnum
        })
    );
    assert_eq!(ObjectType::Structure.name(), "structure-object");
    assert!(matches!(
        StringDesignator::Character(65),
        StringDesignator::Character(65)
    ));
    assert!(matches!(
        Sequence::List(List::Nil),
        Sequence::List(List::Nil)
    ));
    let _ = (
        string,
        PackageDesignator::Package(package),
        FunctionObject::try_from(Word::TRUE),
    );
}
