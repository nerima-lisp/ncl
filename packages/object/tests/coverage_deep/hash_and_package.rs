use super::*;

#[test]
fn hash_modes_cover_numeric_string_cons_and_iteration_contracts() {
    let (runtime, mut context) = setup();
    let negative = make_bignum_from_i128(&mut context, &runtime, -0x1_0000_0001)
        .unwrap_or_else(|error| panic!("bignum: {error:?}"));
    let same_negative = make_bignum_from_i128(&mut context, &runtime, -0x1_0000_0001)
        .unwrap_or_else(|error| panic!("bignum: {error:?}"));
    let double = make_double(&mut context, &runtime, 3.5)
        .unwrap_or_else(|error| panic!("double: {error:?}"));
    let same_double = make_double(&mut context, &runtime, 3.5)
        .unwrap_or_else(|error| panic!("double: {error:?}"));
    let ratio = make_ratio(&mut context, &runtime, negative.into(), double.into())
        .unwrap_or_else(|error| panic!("ratio: {error:?}"));
    let same_ratio = make_ratio(
        &mut context,
        &runtime,
        same_negative.into(),
        same_double.into(),
    )
    .unwrap_or_else(|error| panic!("ratio: {error:?}"));
    let complex = make_complex(&mut context, &runtime, ratio.into(), negative.into())
        .unwrap_or_else(|error| panic!("complex: {error:?}"));
    let same_complex = make_complex(
        &mut context,
        &runtime,
        same_ratio.into(),
        same_negative.into(),
    )
    .unwrap_or_else(|error| panic!("complex: {error:?}"));
    let lower = make_string(&mut context, &runtime, &['a', 'b']).unwrap_or(Word::NIL);
    let upper = make_string(&mut context, &runtime, &['A', 'B']).unwrap_or(Word::NIL);
    let left = make_cons(&mut context, &runtime, lower, Word::fixnum(4)).unwrap_or(Word::NIL);
    let right = make_cons(&mut context, &runtime, upper, Word::fixnum(4)).unwrap_or(Word::NIL);

    for (test, key, equal_key) in [
        (HashTest::Eq, Word::fixnum(1), Word::fixnum(1)),
        (HashTest::Eql, negative.into(), same_negative.into()),
        (HashTest::Equal, ratio.into(), same_ratio.into()),
        (HashTest::Equalp, left, right),
    ] {
        let table = HashTable::new(&mut context, &runtime, test, Weakness::None)
            .unwrap_or_else(|error| panic!("table: {error:?}"));
        assert_eq!(table.test(&context), Ok(test));
        assert_eq!(table.weakness(&context), Ok(Weakness::None));
        assert_eq!(table.count(&context), Ok(0));
        assert!(table.capacity(&context).unwrap_or(0) >= 8);
        assert!(
            table
                .insert(&mut context, &runtime, key, Word::fixnum(9))
                .is_ok()
        );
        assert_eq!(
            table.get(&mut context, equal_key),
            Ok(Some(Word::fixnum(9)))
        );
        let mut entries = Vec::new();
        assert!(
            table
                .for_each_entry(&context, |stored_key, value| entries
                    .push((stored_key, value)))
                .is_ok()
        );
        assert_eq!(entries.len(), 1);
        assert_eq!(
            table.remove(&mut context, &runtime, equal_key),
            Ok(Some(Word::fixnum(9)))
        );
        assert_eq!(table.remove(&mut context, &runtime, equal_key), Ok(None));
    }
    assert_eq!(bignum_sign(&context, negative), Ok(true));
    assert_eq!(bignum_limbs(&context, negative), Ok(vec![1, 1]));
    assert_eq!(double_value(&context, double), Ok(3.5));
    assert_eq!(ratio_numerator(&context, ratio), Ok(negative.into()));
    assert_eq!(ratio_denominator(&context, ratio), Ok(double.into()));
    assert_eq!(complex_real(&context, complex), Ok(ratio.into()));
    assert_eq!(complex_imag(&context, complex), Ok(negative.into()));
    assert_eq!(
        WordView::from(classify_object(&context, same_complex.into())).as_word(),
        same_complex.into()
    );
}

#[test]
fn package_registry_and_visibility_operations_cover_all_statuses() {
    let (runtime, mut context) = setup();
    let producer = Package::from_word(
        runtime
            .ensure_package(&mut context, "PRODUCER")
            .unwrap_or_else(|error| panic!("producer: {error:?}")),
    );
    let consumer = Package::new(&mut context, &runtime, "CONSUMER")
        .unwrap_or_else(|error| panic!("consumer: {error:?}"));
    assert_package_registry(&runtime, &mut context, producer);
    assert_symbol_visibility(&runtime, &mut context, producer, consumer);
    assert_runtime_registry(&runtime, &mut context, producer);
}

fn assert_package_registry(runtime: &Runtime, context: &mut ThreadContext, producer: Package) {
    let nickname = make_string(context, runtime, &['P', 'R', 'O']).unwrap_or(Word::NIL);
    assert!(producer.add_nickname(context, runtime, nickname).is_ok());
    assert!(
        !producer
            .add_nickname(context, runtime, nickname)
            .unwrap_or(true)
    );
    assert_eq!(
        runtime.find_package(context, "PRO"),
        Some(producer.as_word())
    );
    assert_eq!(runtime.find_package(context, "MISSING"), None);
}

fn assert_symbol_visibility(
    runtime: &Runtime,
    context: &mut ThreadContext,
    producer: Package,
    consumer: Package,
) {
    let name = make_string(context, runtime, &['X']).unwrap_or(Word::NIL);
    let (symbol, status) = producer
        .intern(context, runtime, "X")
        .unwrap_or_else(|error| panic!("intern: {error:?}"));
    assert_eq!(status, FindStatus::Internal);
    assert_eq!(
        producer.find_symbol(context, name),
        Ok(Some((symbol, FindStatus::Internal)))
    );
    assert!(producer.export(context, runtime, name).unwrap_or(false));
    assert_eq!(
        producer.find_symbol(context, name),
        Ok(Some((symbol, FindStatus::External)))
    );
    assert!(producer.export(context, runtime, name).unwrap_or(false));
    assert!(producer.unexport(context, runtime, name).unwrap_or(false));
    assert!(!producer.unexport(context, runtime, name).unwrap_or(true));
    assert!(
        producer
            .use_package(context, runtime, producer.as_word())
            .is_ok()
    );
    assert!(
        !producer
            .use_package(context, runtime, producer.as_word())
            .unwrap_or(true)
    );
    let inherited_name = make_string(context, runtime, &['Y']).unwrap_or(Word::NIL);
    let (inherited, _) = producer
        .intern(context, runtime, "Y")
        .unwrap_or_else(|error| panic!("intern: {error:?}"));
    assert!(
        producer
            .export(context, runtime, inherited_name)
            .unwrap_or(false)
    );
    assert!(
        consumer
            .use_package(context, runtime, producer.as_word())
            .unwrap_or(false)
    );
    assert_eq!(
        consumer.find_symbol(context, inherited_name),
        Ok(Some((inherited, FindStatus::Inherited)))
    );
    assert!(
        consumer
            .import(context, runtime, inherited_name, inherited)
            .is_ok()
    );
    assert_eq!(
        consumer.find_symbol(context, inherited_name),
        Ok(Some((inherited, FindStatus::Internal)))
    );
    assert!(
        consumer
            .unuse_package(context, producer.as_word())
            .unwrap_or(false)
    );
    assert!(
        !consumer
            .unuse_package(context, producer.as_word())
            .unwrap_or(true)
    );
    assert!(consumer.shadow(context, runtime, name).is_ok());
    assert!(consumer.shadow(context, runtime, name).is_ok());
    assert_ne!(
        consumer.shadowing_symbols(context).unwrap_or(Word::NIL),
        Word::NIL
    );
    let generated = consumer.gensym(context, runtime).unwrap_or(Word::NIL);
    assert!(symbol_name(context, generated).is_ok());
}

fn assert_runtime_registry(runtime: &Runtime, context: &mut ThreadContext, producer: Package) {
    assert_eq!(
        runtime.ensure_package(context, "PRODUCER"),
        Ok(producer.as_word())
    );
    assert!(runtime.gc_config().dynamic_space_size > 0);
    runtime.add_feature("NCL-COVERAGE");
    runtime.add_feature("NCL-COVERAGE");
    assert_eq!(runtime.features(), vec![String::from("NCL-COVERAGE")]);
    assert!(
        runtime
            .define_class(context, "COVERAGE", Word::TRUE)
            .is_ok()
    );
    assert_eq!(runtime.class(context, "COVERAGE"), Some(Word::TRUE));
    assert_eq!(runtime.class(context, "UNKNOWN"), None);
}
