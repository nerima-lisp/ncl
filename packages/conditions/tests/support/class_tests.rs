#![allow(clippy::unwrap_used, reason = "coverage tests assert on class helpers")]

use super::*;
use ncl_object::{make_cons, make_string};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    (runtime, ctx)
}

#[test]
fn direct_parents_decodes_nil_single_and_proper_list() {
    let (runtime, mut ctx) = setup();
    let first = make_simple_vector(&mut ctx, &runtime, &[Word::NIL, Word::NIL]).unwrap();
    let second = make_simple_vector(&mut ctx, &runtime, &[Word::NIL, Word::NIL]).unwrap();
    let tail = make_cons(&mut ctx, &runtime, second, Word::NIL).unwrap();
    let list = make_cons(&mut ctx, &runtime, first, tail).unwrap();

    assert_eq!(direct_parents(&ctx, Word::NIL).unwrap(), Vec::<Word>::new());
    assert_eq!(direct_parents(&ctx, first).unwrap(), vec![first]);
    assert_eq!(direct_parents(&ctx, list).unwrap(), vec![first, second]);
}

#[test]
fn string_helpers_distinguish_length_and_content_mismatches() {
    let (runtime, mut ctx) = setup();
    let abc = make_string(&mut ctx, &runtime, &['a', 'b', 'c']).unwrap();
    let abd = make_string(&mut ctx, &runtime, &['a', 'b', 'd']).unwrap();
    let ab = make_string(&mut ctx, &runtime, &['a', 'b']).unwrap();

    assert!(string_equals(&ctx, abc, "abc").unwrap());
    assert!(!string_equals(&ctx, abc, "ab").unwrap());
    assert!(string_words_equal(&ctx, abc, abc).unwrap());
    assert!(!string_words_equal(&ctx, abc, abd).unwrap());
    assert!(!string_words_equal(&ctx, abc, ab).unwrap());
}

#[test]
fn wire_superclasses_handles_empty_single_and_multiple_parents() {
    let (runtime, mut ctx) = setup();
    install_class(&mut ctx, &runtime, "CLASS-TEST").unwrap();
    install_class(&mut ctx, &runtime, "PARENT-A").unwrap();
    install_class(&mut ctx, &runtime, "PARENT-B").unwrap();

    wire_superclasses(&mut ctx, &runtime, "CLASS-TEST", &[]).unwrap();
    let class = runtime.class(&mut ctx, "CLASS-TEST").unwrap();
    let superclass = superclass_of(&ctx, class).unwrap();
    assert_eq!(
        direct_parents(&ctx, superclass).unwrap(),
        Vec::<Word>::new()
    );
    wire_superclasses(&mut ctx, &runtime, "CLASS-TEST", &["PARENT-A"]).unwrap();
    let class = runtime.class(&mut ctx, "CLASS-TEST").unwrap();
    let superclass = superclass_of(&ctx, class).unwrap();
    assert_eq!(direct_parents(&ctx, superclass).unwrap().len(), 1);
    wire_superclasses(&mut ctx, &runtime, "CLASS-TEST", &["PARENT-A", "PARENT-B"]).unwrap();
    let class = runtime.class(&mut ctx, "CLASS-TEST").unwrap();
    let superclass = superclass_of(&ctx, class).unwrap();
    assert_eq!(direct_parents(&ctx, superclass).unwrap().len(), 2);
}
