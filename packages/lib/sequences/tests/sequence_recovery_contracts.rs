#![allow(missing_docs, clippy::too_many_lines, clippy::unwrap_used)]

use std::collections::HashMap;

use ncl_object::{FunctionObject, ObjectError, Runtime, ThreadContext, Word};

fn setup() -> (Runtime, ThreadContext, HashMap<String, FunctionObject>) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_lib_sequences::register(&runtime).unwrap();
    let names = [
        "LIST",
        "CAR",
        "CDR",
        "CONS",
        "COPY-TREE",
        "EQUAL",
        "EQUALP",
        "FIND-IF",
        "FIND-IF-NOT",
        "POSITION-IF",
        "COUNT-IF",
        "REMOVE-IF",
        "SUBSTITUTE-IF",
        "MAPLIST",
        "MAPL",
        "MAPCAN",
        "MAPCON",
        "INTERSECTION",
        "SET-DIFFERENCE",
        "SET-EXCLUSIVE-OR",
        "SUBSETP",
        "ADJOIN",
        "ASSOC",
        "RASSOC",
        "MEMBER",
    ];
    let functions = names
        .into_iter()
        .map(|name| {
            let word = runtime.function(&mut ctx, "COMMON-LISP", name).unwrap();
            (name.to_owned(), FunctionObject::try_from(word).unwrap())
        })
        .collect();
    (runtime, ctx, functions)
}

fn call(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    functions: &HashMap<String, FunctionObject>,
    name: &str,
    args: &[Word],
) -> Result<Word, ObjectError> {
    let function = functions[name];
    ncl_object::with_roots(ctx, args, |ctx, roots| {
        runtime.call_builtin(
            ctx,
            function,
            &roots.iter().map(|root| **root).collect::<Vec<_>>(),
        )
    })
}

fn list(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    functions: &HashMap<String, FunctionObject>,
    values: &[Word],
) -> Word {
    call(runtime, ctx, functions, "LIST", values).unwrap()
}

fn list_values(ctx: &ThreadContext, mut value: Word) -> Vec<Word> {
    let mut result = Vec::new();
    while value != Word::NIL {
        result.push(ncl_object::car(ctx, value).unwrap());
        value = ncl_object::cdr(ctx, value).unwrap();
    }
    result
}

#[test]
fn copy_tree_and_equality_cover_nested_and_array_values() {
    let (runtime, mut ctx, functions) = setup();
    let nested = list(
        &runtime,
        &mut ctx,
        &functions,
        &[Word::fixnum(2), Word::fixnum(3)],
    );
    let source = list(&runtime, &mut ctx, &functions, &[Word::fixnum(1), nested]);
    let copied = call(&runtime, &mut ctx, &functions, "COPY-TREE", &[source]).unwrap();
    assert_ne!(copied, source);
    let copied_tail = ncl_object::cdr(&ctx, copied).unwrap();
    assert_eq!(ncl_object::car(&ctx, copied).unwrap(), Word::fixnum(1));
    assert!(copied_tail.is_cons());
    let copied_nested = ncl_object::car(&ctx, ncl_object::cdr(&ctx, copied).unwrap()).unwrap();
    assert_ne!(copied_nested, nested);
    assert_eq!(
        list_values(&ctx, copied_nested),
        vec![Word::fixnum(2), Word::fixnum(3)]
    );
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "EQUAL", &[source, copied]).unwrap(),
        Word::TRUE
    );
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "EQUALP", &[source, copied]).unwrap(),
        Word::TRUE
    );
    let left = ncl_object::make_string(&mut ctx, &runtime, &['a', 'b']).unwrap();
    let right = ncl_object::make_string(&mut ctx, &runtime, &['A', 'B']).unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "EQUAL", &[left, right]).unwrap(),
        Word::NIL
    );
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "EQUALP", &[left, right]).unwrap(),
        Word::TRUE
    );
    let vector = ncl_object::make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1)]).unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "EQUALP", &[vector, vector]).unwrap(),
        Word::TRUE
    );
}

#[test]
fn predicate_selection_variants_return_values_and_honor_options() {
    let (runtime, mut ctx, functions) = setup();
    let values = list(
        &runtime,
        &mut ctx,
        &functions,
        &[Word::fixnum(1), Word::fixnum(2), Word::fixnum(3)],
    );
    let atom = runtime.function(&mut ctx, "COMMON-LISP", "ATOM").unwrap();
    let predicate = FunctionObject::try_from(atom).unwrap().as_word();
    let args = [predicate, values];
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "FIND-IF", &args).unwrap(),
        Word::fixnum(1)
    );
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "FIND-IF-NOT", &args).unwrap(),
        Word::NIL
    );
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "POSITION-IF", &args).unwrap(),
        Word::fixnum(0)
    );
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "COUNT-IF", &args).unwrap(),
        Word::fixnum(3)
    );
    let removed = call(&runtime, &mut ctx, &functions, "REMOVE-IF", &args).unwrap();
    assert_eq!(list_values(&ctx, removed), Vec::<Word>::new());
    let substitute = call(
        &runtime,
        &mut ctx,
        &functions,
        "SUBSTITUTE-IF",
        &[Word::fixnum(9), predicate, values],
    )
    .unwrap();
    assert_eq!(list_values(&ctx, substitute), vec![Word::fixnum(9); 3]);
}

#[test]
fn mapping_and_ordered_set_variants_preserve_common_lisp_results() {
    let (runtime, mut ctx, functions) = setup();
    let values = list(
        &runtime,
        &mut ctx,
        &functions,
        &[Word::fixnum(1), Word::fixnum(2)],
    );
    let car = runtime.function(&mut ctx, "COMMON-LISP", "CAR").unwrap();
    let car = FunctionObject::try_from(car).unwrap().as_word();
    let mapped = call(&runtime, &mut ctx, &functions, "MAPLIST", &[car, values]).unwrap();
    assert_eq!(list_values(&ctx, mapped).len(), 2);
    let left = list(
        &runtime,
        &mut ctx,
        &functions,
        &[Word::fixnum(1), Word::fixnum(2)],
    );
    let right = list(
        &runtime,
        &mut ctx,
        &functions,
        &[Word::fixnum(2), Word::fixnum(3)],
    );
    let union = call(
        &runtime,
        &mut ctx,
        &functions,
        "INTERSECTION",
        &[left, right],
    )
    .unwrap();
    assert_eq!(list_values(&ctx, union), vec![Word::fixnum(2)]);
    let difference = call(
        &runtime,
        &mut ctx,
        &functions,
        "SET-DIFFERENCE",
        &[left, right],
    )
    .unwrap();
    assert_eq!(list_values(&ctx, difference), vec![Word::fixnum(1)]);
    let exclusive = call(
        &runtime,
        &mut ctx,
        &functions,
        "SET-EXCLUSIVE-OR",
        &[left, right],
    )
    .unwrap();
    assert_eq!(
        list_values(&ctx, exclusive),
        vec![Word::fixnum(1), Word::fixnum(3)]
    );
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "SUBSETP", &[left, right]).unwrap(),
        Word::NIL
    );
    let adjoined = call(
        &runtime,
        &mut ctx,
        &functions,
        "ADJOIN",
        &[Word::fixnum(3), left],
    )
    .unwrap();
    assert_eq!(
        list_values(&ctx, adjoined),
        vec![Word::fixnum(3), Word::fixnum(1), Word::fixnum(2)]
    );
    let pair1 = list(
        &runtime,
        &mut ctx,
        &functions,
        &[Word::fixnum(1), Word::fixnum(10)],
    );
    let pair2 = list(
        &runtime,
        &mut ctx,
        &functions,
        &[Word::fixnum(2), Word::fixnum(20)],
    );
    let alist = list(&runtime, &mut ctx, &functions, &[pair1, pair2]);
    let found = call(
        &runtime,
        &mut ctx,
        &functions,
        "ASSOC",
        &[Word::fixnum(2), alist],
    )
    .unwrap();
    assert_eq!(
        list_values(&ctx, found),
        vec![Word::fixnum(2), Word::fixnum(20)]
    );
    let member = call(
        &runtime,
        &mut ctx,
        &functions,
        "MEMBER",
        &[Word::fixnum(2), left],
    )
    .unwrap();
    assert_eq!(list_values(&ctx, member), vec![Word::fixnum(2)]);
}
