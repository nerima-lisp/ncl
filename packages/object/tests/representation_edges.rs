#![allow(missing_docs, clippy::unwrap_used)]

use ncl_object::hash_table::{HashTable, HashTest, Weakness};
use ncl_object::{
    ArrayElementType, ArrayOptions, Character, CodeObject, ObjectRef, ObjectType, Package, Runtime,
    ThreadContext, Word, WordView, allocate, classify, classify_object, make_array,
    make_bignum_from_i128, make_closure, make_code_object, make_complex, make_cons, make_double,
    make_instance, make_ratio, make_readtable, make_simple_fun, make_simple_vector,
    make_specialized_array, make_stream, make_string, make_structure,
};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    (runtime, ctx)
}

#[test]
fn immediate_views_keep_the_tagged_word_contract() {
    let ctx = ThreadContext::new();

    for (word, expected) in [
        (Word::fixnum(-7), WordView::Fixnum(-7)),
        (Word::character(0x03bb), WordView::Character(0x03bb)),
        (Word::TRUE, WordView::Immediate(Word::TRUE)),
        (Word::UNBOUND, WordView::Immediate(Word::UNBOUND)),
    ] {
        assert_eq!(WordView::from(classify(word)), expected);
        assert_eq!(expected.as_word(), word);
    }
    assert_eq!(classify(Word::fixnum(-7)), ObjectRef::Fixnum(-7));
    assert_eq!(
        classify(Word::character(0x03bb)),
        ObjectRef::Character(0x03bb)
    );
    assert_eq!(
        classify(Word::TRUE),
        ObjectRef::Other {
            word: Word::TRUE,
            widetag: 0,
        }
    );
    assert_eq!(classify(Word::UNBOUND), ObjectRef::Immediate(Word::UNBOUND));

    let character = Character::try_from_word(Word::character(65)).unwrap();
    assert_eq!(character.value(), 65);
    assert_eq!(character.as_word(), Word::character(65));
    assert_eq!(
        Character::try_from_word(Word::fixnum(65)).unwrap_err(),
        ncl_object::TypeError {
            datum: Word::fixnum(65),
            expected: ObjectType::Character,
        }
    );
    assert_eq!(
        WordView::try_from_word(Word::TRUE, ObjectType::Fixnum).unwrap_err(),
        ncl_object::TypeError {
            datum: Word::TRUE,
            expected: ObjectType::Fixnum,
        }
    );
    assert_eq!(
        <ncl_object::List as ncl_object::FromLispArg>::from_lisp_arg(&ctx, Word::NIL),
        Ok(ncl_object::List::Nil)
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the test covers every supported heap representation"
)]
fn heap_classification_round_trips_each_supported_representation() {
    let (runtime, mut ctx) = setup();
    let package_word = runtime.find_package(&ctx, "COMMON-LISP").unwrap();
    assert!(matches!(
        classify_object(&ctx, package_word),
        ObjectRef::Package(_)
    ));
    assert_eq!(
        WordView::from(classify_object(&ctx, package_word)).as_word(),
        package_word
    );
    let package = Package::from_word(package_word);
    let (symbol, _) = package
        .intern(&mut ctx, &runtime, "REPRESENTATION-EDGE")
        .unwrap();
    assert!(matches!(
        classify_object(&ctx, symbol),
        ObjectRef::Symbol(_)
    ));
    assert_eq!(
        WordView::from(classify_object(&ctx, symbol)).as_word(),
        symbol
    );

    let string = make_string(&mut ctx, &runtime, &['s']).unwrap();
    assert!(matches!(
        classify_object(&ctx, string),
        ObjectRef::String(_)
    ));
    assert_eq!(
        WordView::from(classify_object(&ctx, string)).as_word(),
        string
    );

    let vector = make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1)]).unwrap();
    assert!(matches!(
        classify_object(&ctx, vector),
        ObjectRef::SimpleVector(_)
    ));
    assert_eq!(
        WordView::from(classify_object(&ctx, vector)).as_word(),
        vector
    );

    let cons = make_cons(&mut ctx, &runtime, Word::NIL, Word::NIL).unwrap();
    assert!(matches!(classify_object(&ctx, cons), ObjectRef::Cons(_)));
    assert_eq!(WordView::from(classify_object(&ctx, cons)).as_word(), cons);

    let specialized = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Fixnum,
        &[Word::fixnum(1)],
    )
    .unwrap();
    assert!(matches!(
        classify_object(&ctx, specialized),
        ObjectRef::SpecializedArray(_)
    ));
    assert_eq!(
        WordView::from(classify_object(&ctx, specialized)).as_word(),
        specialized
    );

    let array = make_array(
        &mut ctx,
        &runtime,
        &[1],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::NIL,
            adjustable: false,
            fill_pointer: None,
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )
    .unwrap();
    assert!(matches!(classify_object(&ctx, array), ObjectRef::Array(_)));
    assert_eq!(
        WordView::from(classify_object(&ctx, array)).as_word(),
        array
    );

    let hash = HashTable::new(&mut ctx, &runtime, HashTest::Eq, Weakness::None)
        .unwrap()
        .as_word();
    assert!(matches!(
        classify_object(&ctx, hash),
        ObjectRef::HashTable(_)
    ));
    assert_eq!(WordView::from(classify_object(&ctx, hash)).as_word(), hash);

    let code = make_code_object(&mut ctx, &runtime, 0, 0, Word::NIL, Word::NIL, Word::NIL).unwrap();
    assert!(matches!(
        classify_object(&ctx, code.as_word()),
        ObjectRef::Code(_)
    ));
    assert_eq!(
        WordView::from(classify_object(&ctx, code.as_word())).as_word(),
        code.as_word()
    );
    let mut code_word = code.as_word();
    let code_token = ncl_object::push_root(&mut ctx, &mut code_word);
    let function = make_simple_fun(
        &mut ctx,
        &runtime,
        0,
        Word::NIL,
        Word::NIL,
        CodeObject::from_word(code_word),
    )
    .unwrap();
    assert!(matches!(
        classify_object(&ctx, function.as_word()),
        ObjectRef::Function(_)
    ));
    assert_eq!(
        WordView::from(classify_object(&ctx, function.as_word())).as_word(),
        function.as_word()
    );
    let closure = make_closure(
        &mut ctx,
        &runtime,
        0,
        Word::NIL,
        Word::NIL,
        CodeObject::from_word(code_word),
        &[Word::fixnum(2)],
    )
    .unwrap();
    assert!(matches!(
        classify_object(&ctx, closure.as_word()),
        ObjectRef::Closure(_)
    ));
    assert_eq!(
        WordView::from(classify_object(&ctx, closure.as_word())).as_word(),
        closure.as_word()
    );
    assert!(ncl_object::pop_root(&mut ctx, code_token));

    let instance = make_instance(&mut ctx, &runtime, Word::NIL, &[Word::fixnum(3)])
        .unwrap()
        .as_word();
    assert!(matches!(
        classify_object(&ctx, instance),
        ObjectRef::Instance(_)
    ));
    assert_eq!(
        WordView::from(classify_object(&ctx, instance)).as_word(),
        instance
    );

    let layout = runtime.register_structure_layout(1).unwrap();
    let structure = make_structure(&mut ctx, &runtime, layout, &[Word::fixnum(4)]).unwrap();
    assert!(matches!(
        classify_object(&ctx, structure),
        ObjectRef::Structure(_)
    ));
    assert_eq!(
        WordView::from(classify_object(&ctx, structure)).as_word(),
        structure
    );

    let bignum = make_bignum_from_i128(&mut ctx, &runtime, 1_i128 << 70)
        .unwrap()
        .as_word();
    assert!(matches!(
        classify_object(&ctx, bignum),
        ObjectRef::Bignum(_)
    ));
    assert_eq!(
        WordView::from(classify_object(&ctx, bignum)).as_word(),
        bignum
    );

    let ratio = make_ratio(&mut ctx, &runtime, Word::fixnum(1), Word::fixnum(2))
        .unwrap()
        .as_word();
    assert!(matches!(classify_object(&ctx, ratio), ObjectRef::Ratio(_)));
    assert_eq!(
        WordView::from(classify_object(&ctx, ratio)).as_word(),
        ratio
    );

    let double = make_double(&mut ctx, &runtime, 1.5).unwrap().as_word();
    assert!(matches!(
        classify_object(&ctx, double),
        ObjectRef::DoubleFloat(_)
    ));
    assert_eq!(
        WordView::from(classify_object(&ctx, double)).as_word(),
        double
    );

    let complex = make_complex(&mut ctx, &runtime, Word::fixnum(1), Word::fixnum(2))
        .unwrap()
        .as_word();
    assert!(matches!(
        classify_object(&ctx, complex),
        ObjectRef::Complex(_)
    ));
    assert_eq!(
        WordView::from(classify_object(&ctx, complex)).as_word(),
        complex
    );

    let readtable = make_readtable(&mut ctx, &runtime, Word::NIL, Word::NIL, Word::NIL)
        .unwrap()
        .as_word();
    assert!(matches!(
        classify_object(&ctx, readtable),
        ObjectRef::Readtable(_)
    ));
    assert_eq!(
        WordView::from(classify_object(&ctx, readtable)).as_word(),
        readtable
    );

    let stream = make_stream(
        &mut ctx,
        &runtime,
        Word::NIL,
        Word::NIL,
        Word::NIL,
        Word::NIL,
        Word::NIL,
    )
    .unwrap()
    .as_word();
    assert!(matches!(
        classify_object(&ctx, stream),
        ObjectRef::Stream(_)
    ));
    assert_eq!(
        WordView::from(classify_object(&ctx, stream)).as_word(),
        stream
    );
}

#[test]
fn unknown_heap_widetag_preserves_other_representation() {
    let (runtime, mut ctx) = setup();
    let object = allocate(&mut ctx, &runtime, 0x7f, 1).unwrap();

    assert_eq!(runtime.widetag(object), Some(0x7f));
    assert_eq!(
        classify_object(&ctx, object),
        ObjectRef::Other {
            word: object,
            widetag: 0x7f,
        }
    );
    assert_eq!(
        WordView::from(classify_object(&ctx, object)),
        WordView::Other {
            word: object,
            widetag: 0x7f,
        }
    );
    assert_eq!(
        WordView::from(classify_object(&ctx, object)).as_word(),
        object
    );
}
