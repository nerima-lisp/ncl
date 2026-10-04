#![allow(clippy::all, clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::similar_names, clippy::indexing_slicing)]
    use super::*;
    use ncl_object::{make_cons, make_simple_vector, Package, Runtime};

    fn setup() -> (Runtime, ThreadContext) {
        let runtime = Runtime::new().expect("runtime");
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime).expect("context");
        super::register(&runtime).expect("clos registration");
        (runtime, ctx)
    }

    #[test]
    fn make_class_merges_inherited_slots_and_replaces_same_name() {
        let (runtime, mut ctx) = setup();
        let inherited_name = Word::fixnum(10);
        let replaced_name = Word::fixnum(11);
        let inherited = make_simple_vector(&mut ctx, &runtime, &[inherited_name]).expect("slot");
        let parent_slots = make_simple_vector(&mut ctx, &runtime, &[inherited]).expect("slots");
        let parent = make_class(&mut ctx, &runtime, Word::fixnum(1), Word::NIL, parent_slots, Word::NIL)
            .expect("parent");
        let replacement = make_simple_vector(&mut ctx, &runtime, &[replaced_name]).expect("slot");
        let own = make_simple_vector(&mut ctx, &runtime, &[replacement]).expect("slots");
        let child = make_class(&mut ctx, &runtime, Word::fixnum(2), parent, own, Word::NIL)
            .expect("child");
        let effective = class_effective_slots(&ctx, child).expect("effective slots");
        assert_eq!(effective.len(), 2);
        assert_eq!(slot_key(&ctx, effective[0]), Ok(inherited_name));
        assert_eq!(slot_key(&ctx, effective[1]), Ok(replaced_name));
    }

    #[test]
    fn direct_superclasses_decodes_nil_single_and_cons_lists() {
        let (runtime, mut ctx) = setup();
        let first = Word::fixnum(20);
        let second = Word::fixnum(21);
        let list = make_cons(&mut ctx, &runtime, first, Word::NIL).expect("list");
        let list = make_cons(&mut ctx, &runtime, second, list).expect("list");
        assert!(direct_superclasses(&ctx, Word::NIL).expect("nil").is_empty());
        assert_eq!(direct_superclasses(&ctx, first).expect("single"), vec![first]);
        assert_eq!(direct_superclasses(&ctx, list).expect("multiple"), vec![second, first]);
    }

    #[test]
    fn class_of_selects_nil_and_fixnum_classes_and_slot_index_by_name() {
        let (runtime, mut ctx) = setup();
        let nil_class = class_of(&mut ctx, &runtime, Word::NIL).expect("nil class");
        let integer_class = class_of(&mut ctx, &runtime, Word::fixnum(7)).expect("integer class");
        assert_eq!(nil_class, runtime.class(&mut ctx, "NULL").expect("NULL"));
        assert_eq!(integer_class, runtime.class(&mut ctx, "INTEGER").expect("INTEGER"));

        let package = runtime.find_package(&ctx, "COMMON-LISP-USER").expect("package");
        let name = Package::from_word(package)
            .intern(&mut ctx, &runtime, "CORE-TEST-SLOT")
            .expect("slot name")
            .0;
        let descriptor = make_simple_vector(&mut ctx, &runtime, &[name]).expect("descriptor");
        let slots = make_simple_vector(&mut ctx, &runtime, &[descriptor]).expect("slots");
        let class = make_class(&mut ctx, &runtime, Word::fixnum(31), Word::NIL, slots, Word::NIL)
            .expect("class");
        let instance = ncl_object::make_instance(&mut ctx, &runtime, class, &[Word::UNBOUND])
            .expect("instance");
        assert_eq!(slot_index(&ctx, instance, name), Ok(0));
        let unknown = Package::from_word(package)
            .intern(&mut ctx, &runtime, "CORE-TEST-UNKNOWN-SLOT")
            .expect("unknown slot name")
            .0;
        assert_eq!(slot_index(&ctx, instance, unknown), Err(ObjectError::TypeError));
    }
