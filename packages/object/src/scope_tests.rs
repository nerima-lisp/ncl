#[cfg(test)]
mod included_tests {
use super::super::{HandleVec, Local, Scope};
use super::super::super::{
    ObjectRef, Runtime, ThreadContext, Word, classify_object, make_string, string_length,
    string_ref,
};

#[test]
fn handles_read_the_forwarded_value_after_collection() {
    let runtime = Runtime::new().expect("runtime");
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).expect("register");
    let word = make_string(&mut ctx, &runtime, &['x'; 64]).expect("string");
    let mut scope = Scope::new(&mut ctx);
    let handle = scope.root::<crate::StringObject>(Local::from_word(word));
    scope.collect(true).expect("collection");
    let live = scope.get(handle).as_word();
    assert!(matches!(
        classify_object(scope.context(), live),
        ObjectRef::String(_)
    ));
    assert_eq!(string_length(scope.context(), live), Ok(64));
    assert!(
        (0..64).all(|index| string_ref(scope.context(), live, index) == Ok('x'))
    );
}

#[test]
fn vectors_keep_order_and_can_be_updated() {
    let runtime = Runtime::new().expect("runtime");
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).expect("register");
    let mut scope = Scope::new(&mut ctx);
    let handles = scope.root_many::<Word>(&[
        Local::from_word(Word::fixnum(1)),
        Local::from_word(Word::fixnum(2)),
    ]);
    assert_eq!(
        scope
            .get_many(&handles)
            .into_iter()
            .map(Local::as_word)
            .collect::<Vec<_>>(),
        vec![Word::fixnum(1), Word::fixnum(2)]
    );
    let second = handles.iter().nth(1).copied().expect("second handle");
    scope.set(second, Local::from_word(Word::fixnum(3)));
    assert_eq!(scope.get(second).as_word(), Word::fixnum(3));
}

#[test]
fn handle_vec_accumulates_under_gc_stress_and_strict_forwarding() {
    let runtime = Runtime::new().expect("runtime");
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).expect("register");
    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);
    let mut scope = Scope::new(&mut ctx);
    let mut values: HandleVec<'_, Word> = HandleVec {
        handles: Vec::new(),
    };
    for _ in 0..40 {
        let word = crate::make_string(scope.ctx, &runtime, &['x'; 8]).expect("string");
        values.push(&mut scope, Local::from_word(word));
    }
    scope.collect(true).expect("collection");
    assert_eq!(values.len(), 40);
    assert!(values.iter().all(|handle| {
        let word = scope.get(*handle).as_word();
        matches!(
            crate::classify_object(scope.ctx, word),
            crate::ObjectRef::String(_)
        )
    }));
}

#[test]
fn list_construction_keeps_handle_arguments_rooted() {
    let runtime = Runtime::new().expect("runtime");
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).expect("register");
    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);
    let mut scope = Scope::new(&mut ctx);
    let car = scope.root(Local::from_word(Word::fixnum(1)));
    let cdr = scope.root(Local::from_word(Word::NIL));
    let cons = scope.make_cons(&runtime, car, cdr).expect("cons");
    scope.collect(true).expect("collection");
    let cons_word = cons.get(&scope).as_word();
    assert_eq!(crate::car(scope.ctx, cons_word), Ok(Word::fixnum(1)));
}

#[test]
fn list_to_handle_vec_roots_elements_and_rejects_improper_lists() {
    let runtime = Runtime::new().expect("runtime");
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).expect("register");
    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);
    let mut scope = Scope::new(&mut ctx);
    let values = scope.root_many(&[
        Local::from_word(Word::fixnum(1)),
        Local::from_word(Word::fixnum(2)),
    ]);
    let list = scope.make_list(&runtime, &values).expect("list");
    let list_word = scope.get(list).as_word();
    let elements = scope
        .list_to_handle_vec(Local::from_word(list_word))
        .expect("proper list");
    assert_eq!(
        elements
            .iter()
            .map(|handle| scope.get(*handle).as_word())
            .collect::<Vec<_>>(),
        vec![Word::fixnum(1), Word::fixnum(2)]
    );
    assert_eq!(
        scope.list_to_handle_vec(Local::from_word(Word::fixnum(1))),
        Err(crate::ObjectError::TypeError)
    );
}

#[test]
fn make_list_preserves_handle_order_under_gc_stress() {
    let runtime = Runtime::new().expect("runtime");
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).expect("register");
    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);
    let mut scope = Scope::new(&mut ctx);
    let values = scope.root_many(&[
        Local::from_word(Word::fixnum(3)),
        Local::from_word(Word::fixnum(4)),
        Local::from_word(Word::fixnum(5)),
    ]);
    let list = scope.make_list(&runtime, &values).expect("list");
    let elements = scope
        .list_to_handle_vec(Local::from_word(scope.get(list).as_word()))
        .expect("proper list");
    assert_eq!(
        elements
            .iter()
            .map(|handle| scope.get(*handle).as_word())
            .collect::<Vec<_>>(),
        vec![Word::fixnum(3), Word::fixnum(4), Word::fixnum(5)]
    );
}

#[test]
fn intern_returns_a_rooted_symbol() {
    let runtime = Runtime::new().expect("runtime");
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).expect("register");
    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);
    let mut scope = Scope::new(&mut ctx);
    let first = scope
        .intern(&runtime, "COMMON-LISP", "SCOPE-INTERN")
        .expect("intern");
    let second = scope
        .intern(&runtime, "COMMON-LISP", "SCOPE-INTERN")
        .expect("intern");
    scope.collect(true).expect("collection");
    assert_eq!(scope.get(first).as_word(), scope.get(second).as_word());
    assert!(matches!(
        crate::classify_object(scope.context(), scope.get(first).as_word()),
        crate::ObjectRef::Symbol(_)
    ));
}
}
