//! Package operations moved from kernel integration coverage.

use ncl_object::layout::widetag;
use ncl_object::package::{FindStatus, INTERNAL, Package};
use ncl_object::{
    ObjectError, Runtime, ThreadContext, Word, get, make_string, make_symbol, simple_vector_set,
    symbol_name,
};
use ncl_packages::{PackageListOperations, PackageOperations};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut context = ThreadContext::new();
    context
        .register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    (runtime, context)
}

#[test]
fn package_lists_remove_non_head_entries_and_reject_conflicts() {
    let (runtime, mut context) = setup();
    let first = Package::new(&mut context, &runtime, "FIRST").unwrap_or_else(|_| panic!("first"));
    let second =
        Package::new(&mut context, &runtime, "SECOND").unwrap_or_else(|_| panic!("second"));
    let third = Package::new(&mut context, &runtime, "THIRD").unwrap_or_else(|_| panic!("third"));
    assert!(
        first
            .use_package(&mut context, &runtime, second.as_word())
            .unwrap_or(false)
    );
    assert!(
        first
            .use_package(&mut context, &runtime, third.as_word())
            .unwrap_or(false)
    );
    assert!(
        first
            .unuse_package(&mut context, third.as_word())
            .unwrap_or(false)
    );
    assert!(
        first
            .unuse_package(&mut context, second.as_word())
            .unwrap_or(false)
    );
    let name = make_string(&mut context, &runtime, &['C']).unwrap_or(Word::NIL);
    let (left, _) = second
        .intern(&mut context, &runtime, "C")
        .unwrap_or_else(|_| panic!("left"));
    let (right, _) = third
        .intern(&mut context, &runtime, "C")
        .unwrap_or_else(|_| panic!("right"));
    assert!(second.export(&mut context, &runtime, name).unwrap_or(false));
    assert!(third.export(&mut context, &runtime, name).unwrap_or(false));
    assert!(first.import(&mut context, &runtime, name, left).is_ok());
    assert_eq!(
        first.import(&mut context, &runtime, name, right),
        Err(ObjectError::PackageConflict)
    );
    assert!(second.shadow(&mut context, &runtime, name).is_ok());
    assert!(
        second
            .unintern(&mut context, &runtime, name)
            .unwrap_or(false)
    );
    assert_eq!(second.find_symbol(&mut context, &runtime, name), Ok(None));
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

    let mut visited = Vec::new();
    producer
        .for_each_symbol(&context, |symbol| visited.push(symbol))
        .unwrap_or_else(|error| panic!("visit package symbols: {error:?}"));
    assert_eq!(visited.len(), 2);

    let malformed = Package::new(&mut context, &runtime, "MALFORMED-SYMBOL-TABLE")
        .unwrap_or_else(|error| panic!("malformed package: {error:?}"));
    let internal = get(&context, malformed.as_word(), widetag::PACKAGE, INTERNAL)
        .unwrap_or_else(|error| panic!("internal table: {error:?}"));
    let index = get(&context, internal, widetag::HASH_TABLE, 12)
        .unwrap_or_else(|error| panic!("hash table index: {error:?}"));
    simple_vector_set(&mut context, index, 0, Word::NIL)
        .unwrap_or_else(|error| panic!("malformed index entry: {error:?}"));
    assert_eq!(
        malformed.for_each_symbol(&context, |_| {}),
        Err(ObjectError::Layout)
    );
}

fn assert_package_registry(runtime: &Runtime, context: &mut ThreadContext, producer: Package) {
    let nickname = make_string(context, runtime, &['P', 'R', 'O']).unwrap_or(Word::NIL);
    assert!(producer.add_nickname(context, runtime, nickname).is_ok());
    let second_nickname = make_string(context, runtime, &['A', 'L', 'T']).unwrap_or(Word::NIL);
    assert!(
        producer
            .add_nickname(context, runtime, second_nickname)
            .unwrap_or(false)
    );
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
        producer.find_symbol(context, runtime, name),
        Ok(Some((symbol, FindStatus::Internal)))
    );
    assert!(producer.export(context, runtime, name).unwrap_or(false));
    assert_eq!(
        producer.find_symbol(context, runtime, name),
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
        consumer.find_symbol(context, runtime, inherited_name),
        Ok(Some((inherited, FindStatus::Inherited)))
    );
    assert!(
        consumer
            .import(context, runtime, inherited_name, inherited)
            .is_ok()
    );
    assert_eq!(
        consumer.find_symbol(context, runtime, inherited_name),
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

#[test]
fn package_visibility_and_list_removal_cover_internal_external_and_inherited() {
    let (runtime, mut context) = setup();
    let owner = Package::new(&mut context, &runtime, "OWNER").unwrap_or_else(|_| panic!("owner"));
    let consumer =
        Package::new(&mut context, &runtime, "CONSUMER").unwrap_or_else(|_| panic!("consumer"));
    let name_a = make_string(&mut context, &runtime, &['A']).unwrap_or(Word::NIL);
    let name_b = make_string(&mut context, &runtime, &['B']).unwrap_or(Word::NIL);
    assert_eq!(
        owner
            .name(&context)
            .and_then(|word| ncl_object::string_ref(&context, word, 0)),
        Ok('O')
    );
    let (symbol_a, status) = owner
        .intern(&mut context, &runtime, "A")
        .unwrap_or_else(|_| panic!("intern"));
    assert_eq!(status, FindStatus::Internal);
    assert_eq!(owner.export(&mut context, &runtime, name_b), Ok(false));
    assert_eq!(owner.unexport(&mut context, &runtime, name_a), Ok(false));
    assert_eq!(owner.export(&mut context, &runtime, name_a), Ok(true));
    assert_eq!(owner.export(&mut context, &runtime, name_a), Ok(true));
    assert_eq!(owner.unexport(&mut context, &runtime, name_a), Ok(true));
    assert_eq!(owner.unexport(&mut context, &runtime, name_a), Ok(false));
    assert!(
        owner
            .export(&mut context, &runtime, name_a)
            .unwrap_or(false)
    );
    assert!(
        consumer
            .use_package(&mut context, &runtime, owner.as_word())
            .unwrap_or(false)
    );
    assert_eq!(
        consumer.find_symbol(&mut context, &runtime, name_a),
        Ok(Some((symbol_a, FindStatus::Inherited)))
    );
    assert!(
        consumer
            .import(&mut context, &runtime, name_a, symbol_a)
            .is_ok()
    );
    assert_eq!(
        consumer.import(&mut context, &runtime, name_a, symbol_a),
        Ok(())
    );
    assert!(
        consumer
            .unuse_package(&mut context, owner.as_word())
            .unwrap_or(false)
    );
    assert_eq!(
        consumer.find_symbol(&mut context, &runtime, name_a),
        Ok(Some((symbol_a, FindStatus::Internal)))
    );
    assert!(consumer.shadow(&mut context, &runtime, name_b).is_ok());
    assert!(consumer.shadow(&mut context, &runtime, name_a).is_ok());
    assert!(
        consumer
            .unintern(&mut context, &runtime, name_b)
            .unwrap_or(false)
    );
    assert!(
        consumer
            .unintern(&mut context, &runtime, name_a)
            .unwrap_or(false)
    );
    assert_eq!(
        consumer.find_symbol(&mut context, &runtime, name_b),
        Ok(None)
    );
}

#[test]
fn package_export_and_use_list_return_inherited_symbol() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("Runtime::new failed: {error:?}"));
    let mut ctx = ThreadContext::new();
    assert!(ctx.register(&runtime).is_ok());
    let base = Package::new(&mut ctx, &runtime, "BASE")
        .unwrap_or_else(|error| panic!("Package allocation failed: {error:?}"));
    let user = Package::new(&mut ctx, &runtime, "USER")
        .unwrap_or_else(|error| panic!("Package allocation failed: {error:?}"));
    let mut base_word = base.as_word();
    let mut user_word = user.as_word();
    let base_token = ncl_object::push_root(&mut ctx, &mut base_word);
    let user_token = ncl_object::push_root(&mut ctx, &mut user_word);
    let (symbol, _) = Package::from_word(base_word)
        .intern(&mut ctx, &runtime, "NAME")
        .unwrap_or((Word::NIL, FindStatus::Internal));
    let name = make_string(&mut ctx, &runtime, &['N', 'A', 'M', 'E']).unwrap_or(Word::NIL);
    let exported = Package::from_word(base_word).export(&mut ctx, &runtime, name);
    assert!(exported.unwrap_or(false));
    assert!(
        Package::from_word(user_word)
            .use_package(&mut ctx, &runtime, base_word)
            .unwrap_or(false)
    );
    assert_eq!(
        Package::from_word(user_word).intern(&mut ctx, &runtime, "NAME"),
        Ok((symbol, FindStatus::Inherited))
    );
    assert!(ncl_object::pop_root(&mut ctx, user_token));
    assert!(ncl_object::pop_root(&mut ctx, base_token));
}

#[test]
fn registry_package_operations_survive_gc_stress() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);
    let package = runtime
        .ensure_package(&mut ctx, "STRESS-A")
        .unwrap_or_else(|error| panic!("package: {error:?}"));
    let mut package = package;
    let package_token = ncl_object::push_root(&mut ctx, &mut package);
    let used = runtime
        .ensure_package(&mut ctx, "STRESS-B")
        .unwrap_or_else(|error| panic!("package: {error:?}"));
    let mut used = used;
    let used_token = ncl_object::push_root(&mut ctx, &mut used);
    let mut import_name = make_string(&mut ctx, &runtime, &['I', 'M', 'P', 'O', 'R', 'T'])
        .unwrap_or_else(|error| panic!("allocation: {error:?}"));
    let import_name_token = ncl_object::push_root(&mut ctx, &mut import_name);
    let mut imported = make_symbol(&mut ctx, &runtime, Word::NIL)
        .unwrap_or_else(|error| panic!("symbol: {error:?}"));
    let imported_token = ncl_object::push_root(&mut ctx, &mut imported);
    Package::from_word(used)
        .import(&mut ctx, &runtime, import_name, imported)
        .unwrap_or_else(|error| panic!("import: {error:?}"));
    assert_eq!(
        Package::from_word(used).find_symbol(&mut ctx, &runtime, import_name),
        Ok(Some((imported, FindStatus::Internal)))
    );
    let (symbol, _) = Package::from_word(used)
        .intern(&mut ctx, &runtime, "INHERITED")
        .unwrap_or_else(|error| panic!("intern: {error:?}"));
    let mut symbol = symbol;
    let symbol_token = ncl_object::push_root(&mut ctx, &mut symbol);
    let inherited_name = make_string(
        &mut ctx,
        &runtime,
        &['I', 'N', 'H', 'E', 'R', 'I', 'T', 'E', 'D'],
    )
    .unwrap_or_else(|error| panic!("allocation: {error:?}"));
    Package::from_word(used)
        .export(&mut ctx, &runtime, inherited_name)
        .unwrap_or_else(|error| panic!("export: {error:?}"));
    Package::from_word(package)
        .use_package(&mut ctx, &runtime, used)
        .unwrap_or_else(|error| panic!("use: {error:?}"));
    let lookup = make_string(
        &mut ctx,
        &runtime,
        &['I', 'N', 'H', 'E', 'R', 'I', 'T', 'E', 'D'],
    )
    .unwrap_or_else(|error| panic!("allocation: {error:?}"));
    assert_eq!(
        Package::from_word(package).find_symbol(&mut ctx, &runtime, lookup),
        Ok(Some((symbol, FindStatus::Inherited)))
    );
    assert_eq!(runtime.ensure_package(&mut ctx, "STRESS-A"), Ok(package));
    assert!(ncl_object::pop_root(&mut ctx, symbol_token));
    assert!(ncl_object::pop_root(&mut ctx, imported_token));
    assert!(ncl_object::pop_root(&mut ctx, import_name_token));
    assert!(ncl_object::pop_root(&mut ctx, used_token));
    assert!(ncl_object::pop_root(&mut ctx, package_token));
}
