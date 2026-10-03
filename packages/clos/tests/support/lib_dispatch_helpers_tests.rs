#![allow(clippy::expect_used)]

use super::*;

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().expect("runtime");
    let mut context = ThreadContext::new();
    context.register(&runtime).expect("context");
    crate::register(&runtime).expect("clos registration");
    (runtime, context)
}

#[test]
fn dispatch_list_and_tail_return_exact_words() {
    let (runtime, mut context) = setup();
    let mut scope = Scope::new(&mut context);
    let roots = scope.root_many(
        &[Word::fixnum(11), Word::fixnum(22), Word::fixnum(33)]
            .iter()
            .copied()
            .map(Local::from_word)
            .collect::<Vec<_>>(),
    );
    let list = scope.make_list(&runtime, &roots).expect("list");
    let values = dispatch_list_to_handles(&mut scope, list).expect("dispatch list");
    assert_eq!(
        values
            .iter()
            .map(|handle| scope.get(*handle).as_word())
            .collect::<Vec<_>>(),
        vec![Word::fixnum(11), Word::fixnum(22), Word::fixnum(33)]
    );
    let tail = tail_handles(&mut scope, &values);
    assert_eq!(
        tail.iter()
            .map(|handle| scope.get(*handle).as_word())
            .collect::<Vec<_>>(),
        vec![Word::fixnum(22), Word::fixnum(33)]
    );
    let empty = scope.root_many(&[]);
    assert!(tail_handles(&mut scope, &empty).is_empty());
}

#[test]
fn dispatch_list_rejects_dotted_list_with_type_error_message() {
    let (runtime, mut context) = setup();
    let dotted = ncl_object::make_cons(&mut context, &runtime, Word::fixnum(11), Word::fixnum(22))
        .expect("dotted list");
    let mut scope = Scope::new(&mut context);
    let dotted = scope.root(Local::from_word(dotted));
    let error = dispatch_list_to_handles(&mut scope, dotted).expect_err("dotted list");
    assert_eq!(error, ObjectError::TypeError);
    assert_eq!(error.to_string(), "TypeError");
}
