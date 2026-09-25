#![allow(missing_docs)]

use ncl_object::hash_table::{HashTable, HashTest, Weakness};
use ncl_object::{
    ArrayElementType, ArrayOptions, FindStatus, ObjectError, ObjectRef, Package, Runtime,
    ThreadContext, WordView, array_dimensions, array_row_major_ref, array_row_major_set,
    classify, classify_object, make_array, make_cons, make_simple_vector, make_string,
    make_structure, structure_layout, structure_ref, structure_set,
};
use ncl_sys::Word;

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut context = ThreadContext::new();
    context
        .register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    (runtime, context)
}

#[test]
fn nested_displacement_and_structure_metadata_have_observable_results() {
    let (runtime, mut context) = setup();
    let base = make_array(
        &mut context,
        &runtime,
        &[2],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::fixnum(3),
            adjustable: false,
            fill_pointer: None,
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )
    .unwrap_or(Word::NIL);
    let outer = make_array(
        &mut context,
        &runtime,
        &[1],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::NIL,
            adjustable: false,
            fill_pointer: None,
            displaced_to: Some(base),
            displaced_index_offset: 1,
        },
    )
    .unwrap_or(Word::NIL);
    let nested = make_array(
        &mut context,
        &runtime,
        &[1],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::NIL,
            adjustable: false,
            fill_pointer: None,
            displaced_to: Some(outer),
            displaced_index_offset: 0,
        },
    )
    .unwrap_or(Word::NIL);
    assert_eq!(array_dimensions(&context, nested), Ok(vec![1]));
    assert_eq!(array_row_major_ref(&context, nested, 0), Ok(Word::fixnum(3)));
    assert!(array_row_major_set(&mut context, nested, 0, Word::fixnum(8)).is_ok());
    assert_eq!(array_row_major_ref(&context, base, 1), Ok(Word::fixnum(8)));

    let layout = runtime
        .register_structure_layout(2)
        .unwrap_or_else(|error| panic!("layout: {error:?}"));
    assert_eq!(layout.as_u32(), 1);
    assert_eq!(runtime.structure_layout_size(layout), Some(2));
    assert_eq!(runtime.structure_layout_size(99.into()), None);
    assert_eq!(
        make_structure(&mut context, &runtime, layout, &[Word::NIL]),
        Err(ObjectError::Layout)
    );
    let structure = make_structure(
        &mut context,
        &runtime,
        layout,
        &[Word::fixnum(1), Word::fixnum(2)],
    )
    .unwrap_or(Word::NIL);
    assert_eq!(structure_layout(&context, structure), Ok(layout));
    assert_eq!(structure_ref(&context, structure, 0), Ok(Word::fixnum(1)));
    assert_eq!(structure_ref(&context, structure, 2), Err(ObjectError::Layout));
    assert!(structure_set(&mut context, structure, 1, Word::fixnum(9)).is_ok());
    assert_eq!(structure_ref(&context, structure, 1), Ok(Word::fixnum(9)));
    assert_eq!(structure_set(&mut context, Word::NIL, 0, Word::NIL), Err(ObjectError::TypeError));
}

#[test]
fn package_visibility_and_list_removal_cover_internal_external_and_inherited() {
    let (runtime, mut context) = setup();
    let owner = Package::new(&mut context, &runtime, "OWNER").unwrap_or_else(|_| panic!("owner"));
    let consumer = Package::new(&mut context, &runtime, "CONSUMER")
        .unwrap_or_else(|_| panic!("consumer"));
    let name_a = make_string(&mut context, &runtime, &['A']).unwrap_or(Word::NIL);
    let name_b = make_string(&mut context, &runtime, &['B']).unwrap_or(Word::NIL);
    assert_eq!(owner.name(&context).and_then(|word| ncl_object::string_ref(&context, word, 0)), Ok('O'));
    let (symbol_a, status) = owner.intern(&mut context, &runtime, "A").unwrap_or_else(|_| panic!("intern"));
    assert_eq!(status, FindStatus::Internal);
    assert_eq!(owner.export(&mut context, &runtime, name_b), Ok(false));
    assert_eq!(owner.unexport(&mut context, &runtime, name_a), Ok(false));
    assert_eq!(owner.export(&mut context, &runtime, name_a), Ok(true));
    assert_eq!(owner.export(&mut context, &runtime, name_a), Ok(true));
    assert_eq!(owner.unexport(&mut context, &runtime, name_a), Ok(true));
    assert_eq!(owner.unexport(&mut context, &runtime, name_a), Ok(false));
    assert!(owner.export(&mut context, &runtime, name_a).unwrap_or(false));
    assert!(consumer.use_package(&mut context, &runtime, owner.as_word()).unwrap_or(false));
    assert_eq!(consumer.find_symbol(&mut context, name_a), Ok(Some((symbol_a, FindStatus::Inherited))));
    assert!(consumer.import(&mut context, &runtime, name_a, symbol_a).is_ok());
    assert_eq!(consumer.import(&mut context, &runtime, name_a, symbol_a), Ok(()));
    assert!(consumer.unuse_package(&mut context, owner.as_word()).unwrap_or(false));
    assert_eq!(consumer.find_symbol(&mut context, name_a), Ok(Some((symbol_a, FindStatus::Internal))));
    assert!(consumer.shadow(&mut context, &runtime, name_b).is_ok());
    assert!(consumer.shadow(&mut context, &runtime, name_a).is_ok());
    assert!(consumer.unintern(&mut context, &runtime, name_b).unwrap_or(false));
    assert!(consumer.unintern(&mut context, &runtime, name_a).unwrap_or(false));
    assert_eq!(consumer.find_symbol(&mut context, name_b), Ok(None));
}

#[test]
fn runtime_context_roots_and_classification_report_values() {
    let (runtime, mut context) = setup();
    assert_eq!(runtime.widetag(Word::NIL), None);
    assert!(runtime
        .define_function(&mut context, "COVERAGE", "FUNCTION", Word::TRUE)
        .is_ok());
    assert_eq!(
        runtime.function(&mut context, "COVERAGE", "FUNCTION"),
        Some(Word::TRUE)
    );
    assert_eq!(runtime.function(&mut context, "COVERAGE", "MISSING"), None);
    context.bind(7, Word::fixnum(4));
    context.bind(7, Word::fixnum(5));
    assert_eq!(context.unbind(7), Ok(Word::fixnum(5)));
    assert_eq!(context.unbind(7), Ok(Word::fixnum(4)));
    assert_eq!(context.unbind(7), Err(ObjectError::Unbound));
    context.set_values(&[Word::fixnum(1), Word::TRUE]);
    assert_eq!(context.values(), &[Word::fixnum(1), Word::TRUE]);
    context.set_pending(ObjectError::Layout);
    assert_eq!(context.take_pending(), Some(ObjectError::Layout));
    assert_eq!(context.take_pending(), None);
    context.set_strict_forwarding(true);
    context.set_strict_forwarding(false);
    context.set_gc_stress(true);
    assert!(ncl_object::make_cons(&mut context, &runtime, Word::NIL, Word::TRUE).is_ok());
    context.set_gc_stress(false);
    assert_eq!(ncl_object::car(&mut context, Word::NIL), Ok(Word::NIL));
    assert_eq!(ncl_object::cdr(&mut context, Word::NIL), Ok(Word::NIL));
    assert_eq!(ncl_object::car(&mut context, Word::TRUE), Err(ObjectError::TypeError));
    assert_eq!(ncl_object::cdr(&mut context, Word::TRUE), Err(ObjectError::TypeError));
    assert_eq!(classify(Word::character(65)), ObjectRef::Character(65));
    assert_eq!(classify(Word::fixnum(12)), ObjectRef::Fixnum(12));
    assert_eq!(classify_object(&context, Word::NIL), ObjectRef::Symbol(Word::NIL));
    assert_eq!(WordView::from(classify(Word::TRUE)).as_word(), Word::TRUE);
}

#[test]
fn hash_tables_resize_replace_and_iterate_meaningful_entries() {
    let (runtime, mut context) = setup();
    let table = HashTable::new(&mut context, &runtime, HashTest::Equal, Weakness::None)
        .unwrap_or_else(|_| panic!("table"));
    let initial_capacity = table.capacity(&context).unwrap_or(0);
    for index in 0..16 {
        let key = make_string(&mut context, &runtime, &[char::from(b'a' + index)]).unwrap_or(Word::NIL);
        assert!(table.insert(&mut context, &runtime, key, Word::fixnum(index.into())).is_ok());
    }
    assert!(table.capacity(&context).unwrap_or(0) > initial_capacity);
    let key = make_string(&mut context, &runtime, &['a']).unwrap_or(Word::NIL);
    assert!(table.insert(&mut context, &runtime, key, Word::fixnum(99)).is_ok());
    assert_eq!(table.get(&mut context, key), Ok(Some(Word::fixnum(99))));
    assert_eq!(table.remove(&mut context, &runtime, key), Ok(Some(Word::fixnum(99))));
    assert_eq!(table.get(&mut context, key), Ok(None));
    let mut count = 0;
    assert!(table.for_each_entry(&context, |_, value| {
        assert!(value.as_fixnum().is_some());
        count += 1;
    }).is_ok());
    assert_eq!(count, 15);
}
