#![allow(
    clippy::similar_names,
    clippy::too_many_lines,
    missing_docs,
    clippy::unwrap_used
)]

use std::collections::HashMap;

use ncl_object::{FunctionObject, ObjectError, Package, Runtime, ThreadContext, Word};

fn setup() -> (Runtime, ThreadContext, HashMap<String, FunctionObject>) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_lib_sequences::register(&runtime).unwrap();
    let names = [
        "APPEND",
        "ASSOC",
        "ATOM",
        "CAR",
        "CONSP",
        "CONCATENATE",
        "COPY-LIST",
        "ELT",
        "ENDP",
        "EVERY",
        "FIND",
        "INTERSECTION",
        "LIST",
        "LISTP",
        "LIST*",
        "MAP",
        "MAP-INTO",
        "MAPC",
        "MAPCAN",
        "MAPCAR",
        "MAPCON",
        "MAPL",
        "MAPLIST",
        "MEMBER",
        "NCONC",
        "NREVERSE",
        "NOTANY",
        "NOTEVERY",
        "RASSOC",
        "SOME",
        "SUBSEQ",
        "UNION",
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

fn keyword(ctx: &mut ThreadContext, runtime: &Runtime, name: &str) -> Word {
    let package = runtime.find_package(ctx, "KEYWORD").unwrap();
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap()
        .0
}

fn common_lisp_symbol(ctx: &mut ThreadContext, runtime: &Runtime, name: &str) -> Word {
    let package = runtime.find_package(ctx, "COMMON-LISP").unwrap();
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap()
        .0
}

#[test]
fn sequence_construction_and_access_report_exact_results_and_errors() {
    let (runtime, mut ctx, functions) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let three = Word::fixnum(3);
    let source = list(&runtime, &mut ctx, &functions, &[one, two, three]);
    let vector = ncl_object::make_simple_vector(&mut ctx, &runtime, &[three, two]).unwrap();
    let text = ncl_object::make_string(&mut ctx, &runtime, &['a', 'b']).unwrap();
    let list_type = common_lisp_symbol(&mut ctx, &runtime, "LIST");
    let vector_type = common_lisp_symbol(&mut ctx, &runtime, "VECTOR");
    let string_type = common_lisp_symbol(&mut ctx, &runtime, "STRING");

    let concatenated = call(
        &runtime,
        &mut ctx,
        &functions,
        "CONCATENATE",
        &[list_type, source, vector],
    )
    .unwrap();
    assert_eq!(
        list_values(&ctx, concatenated),
        vec![one, two, three, three, two]
    );
    let concatenated_vector = call(
        &runtime,
        &mut ctx,
        &functions,
        "CONCATENATE",
        &[vector_type, source, vector],
    )
    .unwrap();
    assert_eq!(
        (0..5)
            .map(|index| ncl_object::simple_vector_ref(&ctx, concatenated_vector, index).unwrap())
            .collect::<Vec<_>>(),
        vec![one, two, three, three, two]
    );
    let concatenated_string = call(
        &runtime,
        &mut ctx,
        &functions,
        "CONCATENATE",
        &[string_type, text],
    )
    .unwrap();
    assert_eq!(
        (0..2)
            .map(|index| ncl_object::string_ref(&ctx, concatenated_string, index).unwrap())
            .collect::<String>(),
        "ab"
    );
    let concatenated_chars = call(
        &runtime,
        &mut ctx,
        &functions,
        "CONCATENATE",
        &[list_type, text],
    )
    .unwrap();
    assert_eq!(
        list_values(&ctx, concatenated_chars),
        vec![
            Word::character(u32::from('a')),
            Word::character(u32::from('b'))
        ]
    );

    let cons = ncl_object::make_cons(&mut ctx, &runtime, one, Word::NIL).unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "ATOM", &[cons]),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "LISTP", &[cons]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "LIST*", &[]),
        Err(ObjectError::TypeError)
    );
    let dotted = ncl_object::make_cons(&mut ctx, &runtime, one, two).unwrap();
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "APPEND",
            &[dotted, Word::NIL]
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "LISTP", &[dotted]),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "APPEND", &[]),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "NCONC", &[]),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "CONCATENATE",
            &[Word::fixnum(9), source],
        ),
        Err(ObjectError::TypeError)
    );
    let unknown_type = common_lisp_symbol(&mut ctx, &runtime, "UNKNOWN");
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "CONCATENATE",
            &[unknown_type, source],
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "ELT",
            &[source, Word::fixnum(-1)],
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "ELT",
            &[source, Word::fixnum(3)],
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "SUBSEQ",
            &[source, Word::fixnum(2), Word::fixnum(1)],
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "SUBSEQ",
            &[source, Word::fixnum(0), Word::fixnum(4)],
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "NREVERSE", &[vector]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "ENDP", &[one]),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn higher_order_variants_preserve_tail_semantics_and_short_circuit_results() {
    let (runtime, mut ctx, functions) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let three = Word::fixnum(3);
    let source = list(&runtime, &mut ctx, &functions, &[one, two, three]);
    let car = functions["CAR"].as_word();
    let list_function = functions["LIST"].as_word();
    let atom = functions["ATOM"].as_word();
    let consp = functions["CONSP"].as_word();
    let nested = list(&runtime, &mut ctx, &functions, &[two]);
    let mixed = list(&runtime, &mut ctx, &functions, &[one, nested]);

    let maplist = call(&runtime, &mut ctx, &functions, "MAPLIST", &[car, source]).unwrap();
    assert_eq!(list_values(&ctx, maplist), vec![one, two, three]);
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "MAPL", &[car, source]),
        Ok(source)
    );
    let mapcan = call(
        &runtime,
        &mut ctx,
        &functions,
        "MAPCAN",
        &[list_function, source],
    )
    .unwrap();
    assert_eq!(list_values(&ctx, mapcan), vec![one, two, three]);
    let mapcon = call(
        &runtime,
        &mut ctx,
        &functions,
        "MAPCON",
        &[list_function, source],
    )
    .unwrap();
    let tail_two = ncl_object::cdr(&ctx, source).unwrap();
    let tail_three = ncl_object::cdr(&ctx, tail_two).unwrap();
    assert_eq!(
        list_values(&ctx, mapcon),
        vec![source, tail_two, tail_three]
    );

    let every = call(&runtime, &mut ctx, &functions, "EVERY", &[atom, source]);
    assert_eq!(every, Ok(Word::TRUE));
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "SOME", &[consp, mixed]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "NOTANY", &[consp, mixed]),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "NOTEVERY", &[consp, mixed]),
        Ok(Word::TRUE)
    );

    let mapped = call(
        &runtime,
        &mut ctx,
        &functions,
        "MAP",
        &[source, atom, source],
    )
    .unwrap();
    assert_eq!(list_values(&ctx, mapped), vec![Word::TRUE; 3]);
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "MAP",
            &[Word::NIL, atom, source],
        ),
        Ok(Word::NIL)
    );
    let destination = list(&runtime, &mut ctx, &functions, &[Word::NIL, Word::NIL]);
    let consp_results = call(
        &runtime,
        &mut ctx,
        &functions,
        "MAP-INTO",
        &[destination, consp, source],
    );
    assert_eq!(consp_results, Ok(destination));
    assert_eq!(list_values(&ctx, destination), vec![Word::NIL, Word::NIL]);

    let destination = list(&runtime, &mut ctx, &functions, &[Word::NIL, Word::NIL]);
    let first_pair = list(&runtime, &mut ctx, &functions, &[one]);
    let second_pair = list(&runtime, &mut ctx, &functions, &[two]);
    let source_pairs = list(&runtime, &mut ctx, &functions, &[first_pair, second_pair]);
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "MAP-INTO",
            &[destination, functions["CAR"].as_word(), source_pairs],
        ),
        Ok(destination)
    );
    assert_eq!(list_values(&ctx, destination), vec![one, two]);

    let text = ncl_object::make_string(&mut ctx, &runtime, &['a', 'b']).unwrap();
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "MAP-INTO",
            &[text, consp, text],
        ),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn ordered_set_and_association_options_match_exactly_and_reject_dotted_lists() {
    let (runtime, mut ctx, functions) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let three = Word::fixnum(3);
    let left = list(&runtime, &mut ctx, &functions, &[one, two, two]);
    let right = list(&runtime, &mut ctx, &functions, &[two, three]);
    let test = keyword(&mut ctx, &runtime, "TEST");
    let equal = runtime.function(&mut ctx, "COMMON-LISP", "EQUAL").unwrap();
    let equal = FunctionObject::try_from(equal).unwrap().as_word();

    let union = call(
        &runtime,
        &mut ctx,
        &functions,
        "UNION",
        &[left, right, test, equal],
    )
    .unwrap();
    assert_eq!(list_values(&ctx, union), vec![one, two, three]);
    let intersection = call(
        &runtime,
        &mut ctx,
        &functions,
        "INTERSECTION",
        &[left, right, test, equal],
    )
    .unwrap();
    assert_eq!(list_values(&ctx, intersection), vec![two]);

    let pair_one = list(&runtime, &mut ctx, &functions, &[one, Word::fixnum(10)]);
    let pair_two = list(&runtime, &mut ctx, &functions, &[two, Word::fixnum(20)]);
    let alist = list(&runtime, &mut ctx, &functions, &[pair_one, pair_two]);
    let dotted_pair_two = ncl_object::make_cons(&mut ctx, &runtime, two, Word::fixnum(20)).unwrap();
    let dotted_alist = list(&runtime, &mut ctx, &functions, &[dotted_pair_two]);
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "ASSOC",
            &[two, alist, test, equal],
        )
        .map(|value| list_values(&ctx, value)),
        Ok(vec![two, Word::fixnum(20)])
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "RASSOC",
            &[Word::fixnum(20), dotted_alist, test, equal],
        ),
        Ok(dotted_pair_two)
    );
    let member = call(
        &runtime,
        &mut ctx,
        &functions,
        "MEMBER",
        &[two, left, test, equal],
    )
    .unwrap();
    assert_eq!(list_values(&ctx, member), vec![two, two]);

    let dotted = ncl_object::make_cons(&mut ctx, &runtime, one, two).unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "MEMBER", &[three, dotted]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "ASSOC", &[one, dotted]),
        Err(ObjectError::TypeError)
    );
}
