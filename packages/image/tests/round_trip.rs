#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "tests assert on image failures"
)]

//! Acceptance tests: an object graph round-trips into a fresh runtime and
//! survives a collection there.

use std::collections::HashSet;

use ncl_image::{load, save};
use ncl_object::hash_table::{HashTable, HashTest, Weakness};
use ncl_object::{
    CodeObject, Function, ObjectRef, Package, Runtime, ThreadContext, Word, car, cdr,
    classify_object, code_constants, code_debug, code_entry, code_size, code_stack_map,
    function_code, function_entry, function_lambda_list, function_name, make_code_object,
    make_cons, make_simple_fun, make_simple_vector, make_string, push_root, set_symbol_value,
    simple_vector_length, simple_vector_ref, string_length, string_ref, symbol_flags,
    symbol_function, symbol_name, symbol_package, symbol_plist, symbol_value,
};
use ncl_sys::LowTag;

/// One built graph plus the roots handed to the image.
struct Graph {
    roots: Vec<Word>,
}

/// Build the acceptance object graph and return its rooted roots.
fn build_graph(runtime: &Runtime, ctx: &mut ThreadContext) -> Graph {
    let mut string = make_string(ctx, runtime, &['h', 'i']).unwrap();
    let string_token = push_root(ctx, &mut string);

    let package = runtime.find_package(ctx, "NCL").unwrap();
    let (mut symbol, _) = Package::from_word(package)
        .intern(ctx, runtime, "FOO")
        .unwrap();
    set_symbol_value(ctx, symbol, Word::fixnum(5)).unwrap();
    let symbol_token = push_root(ctx, &mut symbol);

    let mut vector = make_simple_vector(ctx, runtime, &[string, symbol, Word::fixnum(7)]).unwrap();
    let vector_token = push_root(ctx, &mut vector);

    let mut cons = make_cons(ctx, runtime, vector, Word::NIL).unwrap();
    let cons_token = push_root(ctx, &mut cons);

    let table = HashTable::new(ctx, runtime, HashTest::Eql, Weakness::None).unwrap();
    table.insert(ctx, runtime, Word::fixnum(1), string).unwrap();
    let mut table_word = table.as_word();
    let table_token = push_root(ctx, &mut table_word);

    let mut code = make_code_object(ctx, runtime, 0, 0, Word::NIL, Word::NIL, Word::NIL)
        .unwrap()
        .as_word();
    let code_token = push_root(ctx, &mut code);

    let mut name = make_string(ctx, runtime, &['f', 'n']).unwrap();
    let name_token = push_root(ctx, &mut name);

    let mut function = make_simple_fun(
        ctx,
        runtime,
        0,
        name,
        Word::NIL,
        CodeObject::from_word(code),
    )
    .unwrap()
    .as_word();
    let function_token = push_root(ctx, &mut function);

    let roots = vec![cons, table_word, function];
    for token in [
        function_token,
        name_token,
        code_token,
        table_token,
        cons_token,
        vector_token,
        symbol_token,
        string_token,
    ] {
        let _ = ncl_object::pop_root(ctx, token);
    }
    Graph { roots }
}

/// Save a graph from its owning runtime and load it into a fresh runtime.
fn round_trip(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    roots: &[Word],
) -> (Runtime, ThreadContext, Vec<Word>) {
    let image = save(runtime, ctx, roots, &[]).unwrap();

    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let loaded = load(&image, &runtime, &mut ctx).unwrap();
    (runtime, ctx, loaded.roots)
}

#[test]
fn object_graph_round_trips_into_a_fresh_runtime() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let graph = build_graph(&runtime, &mut ctx);

    let (_runtime2, mut ctx2, roots2) = round_trip(&runtime, &mut ctx, &graph.roots);
    assert_eq!(graph.roots.len(), roots2.len());
    for (index, (original, loaded)) in graph.roots.iter().zip(roots2.iter()).enumerate() {
        assert!(
            isomorphic(&mut ctx, *original, &mut ctx2, *loaded),
            "root {index} is not isomorphic after a load"
        );
    }
}

#[test]
fn loaded_objects_survive_a_full_collection() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let graph = build_graph(&runtime, &mut ctx);

    let (_runtime2, mut ctx2, loaded) = round_trip(&runtime, &mut ctx, &graph.roots);
    let mut roots = loaded;
    let token = ncl_sys::register_root_set(ctx2.thread_mut(), &mut roots);
    ctx2.collect(true).unwrap();

    // The cons still points at the vector, whose string and symbol survived.
    let vector = car(&mut ctx2, roots[0]).unwrap();
    assert_eq!(simple_vector_length(&ctx2, vector).unwrap(), 3);
    let string = simple_vector_ref(&ctx2, vector, 0).unwrap();
    assert_eq!(read_string(&ctx2, string), "hi");
    let symbol = simple_vector_ref(&ctx2, vector, 1).unwrap();
    assert_eq!(
        read_string(&ctx2, symbol_name(&ctx2, symbol).unwrap()),
        "FOO"
    );
    assert_eq!(symbol_value(&ctx2, symbol).unwrap().as_fixnum(), Some(5));

    // The hash table still maps 1 to the string.
    let table = HashTable::from_word(roots[1]);
    let found = table.get(&mut ctx2, Word::fixnum(1)).unwrap().unwrap();
    assert_eq!(read_string(&ctx2, found), "hi");

    // The function object still names its code object.
    let function = Function::from_word(roots[2]);
    assert_eq!(function_entry(&ctx2, function).unwrap(), 0);
    let code = function_code(&ctx2, function).unwrap();
    assert_eq!(code_entry(&ctx2, code).unwrap().as_fixnum(), Some(0));
    let _ = ncl_sys::pop_root(ctx2.thread_mut(), token);
}

/// Report whether two object graphs have the same shape and contents.
fn isomorphic(a: &mut ThreadContext, wa: Word, b: &mut ThreadContext, wb: Word) -> bool {
    let mut seen = HashSet::new();
    iso(a, wa, b, wb, &mut seen)
}

#[allow(clippy::too_many_lines, reason = "flat per-kind comparison")]
fn iso(
    a: &mut ThreadContext,
    wa: Word,
    b: &mut ThreadContext,
    wb: Word,
    seen: &mut HashSet<(usize, usize)>,
) -> bool {
    if !is_heap(wa) || !is_heap(wb) {
        return wa.bits() == wb.bits();
    }
    let key = (
        usize::try_from(wa.bits()).unwrap_or(0),
        usize::try_from(wb.bits()).unwrap_or(0),
    );
    if !seen.insert(key) {
        return true;
    }
    // Cons cells have no header word, so they must be classified by lowtag.
    if wa.lowtag() == LowTag::List as u8 && wb.lowtag() == LowTag::List as u8 {
        let (xa, xb) = (car(a, wa).unwrap(), cdr(a, wa).unwrap());
        let (ya, yb) = (car(b, wb).unwrap(), cdr(b, wb).unwrap());
        return iso(a, xa, b, ya, seen) && iso(a, xb, b, yb, seen);
    }
    match (classify_object(a, wa), classify_object(b, wb)) {
        (ObjectRef::Cons(x), ObjectRef::Cons(y)) => {
            let (xa, xb) = (car(a, x).unwrap(), cdr(a, x).unwrap());
            let (ya, yb) = (car(b, y).unwrap(), cdr(b, y).unwrap());
            iso(a, xa, b, ya, seen) && iso(a, xb, b, yb, seen)
        }
        (ObjectRef::Symbol(x), ObjectRef::Symbol(y)) => {
            let (name_a, name_b) = (symbol_name(a, x).unwrap(), symbol_name(b, y).unwrap());
            let (value_a, value_b) = (symbol_value(a, x).unwrap(), symbol_value(b, y).unwrap());
            let (fun_a, fun_b) = (
                symbol_function(a, x).unwrap(),
                symbol_function(b, y).unwrap(),
            );
            let (plist_a, plist_b) = (symbol_plist(a, x).unwrap(), symbol_plist(b, y).unwrap());
            read_string(a, name_a) == read_string(b, name_b)
                && package_name(a, x) == package_name(b, y)
                && symbol_flags(a, x).unwrap() == symbol_flags(b, y).unwrap()
                && iso(a, value_a, b, value_b, seen)
                && iso(a, fun_a, b, fun_b, seen)
                && iso(a, plist_a, b, plist_b, seen)
        }
        (ObjectRef::String(x), ObjectRef::String(y)) => read_string(a, x) == read_string(b, y),
        (ObjectRef::SimpleVector(x), ObjectRef::SimpleVector(y)) => {
            let (na, nb) = (
                simple_vector_length(a, x).unwrap(),
                simple_vector_length(b, y).unwrap(),
            );
            if na != nb {
                return false;
            }
            for index in 0..na {
                let (ea, eb) = (
                    simple_vector_ref(a, x, index).unwrap(),
                    simple_vector_ref(b, y, index).unwrap(),
                );
                if !iso(a, ea, b, eb, seen) {
                    return false;
                }
            }
            true
        }
        (ObjectRef::HashTable(x), ObjectRef::HashTable(y)) => {
            let (ta, tb) = (HashTable::from_word(x), HashTable::from_word(y));
            if ta.test(a).unwrap() != tb.test(b).unwrap()
                || ta.weakness(a).unwrap() != tb.weakness(b).unwrap()
            {
                return false;
            }
            let mut left = Vec::new();
            ta.for_each_entry(a, |key, value| left.push((key, value)))
                .unwrap();
            let mut right = Vec::new();
            tb.for_each_entry(b, |key, value| right.push((key, value)))
                .unwrap();
            if left.len() != right.len() {
                return false;
            }
            for (key, value) in left {
                let mut matched = false;
                for (other_key, other_value) in &right {
                    if iso(a, key, b, *other_key, seen) && iso(a, value, b, *other_value, seen) {
                        matched = true;
                        break;
                    }
                }
                if !matched {
                    return false;
                }
            }
            true
        }
        (ObjectRef::Function(x), ObjectRef::Function(y)) => {
            let (fa, fb) = (Function::from_word(x), Function::from_word(y));
            if function_entry(a, fa).unwrap() != function_entry(b, fb).unwrap() {
                return false;
            }
            let (na, nb) = (function_name(a, fa).unwrap(), function_name(b, fb).unwrap());
            let (la, lb) = (
                function_lambda_list(a, fa).unwrap(),
                function_lambda_list(b, fb).unwrap(),
            );
            let (ca, cb) = (
                function_code(a, fa).unwrap().as_word(),
                function_code(b, fb).unwrap().as_word(),
            );
            iso(a, na, b, nb, seen) && iso(a, la, b, lb, seen) && iso(a, ca, b, cb, seen)
        }
        (ObjectRef::Code(x), ObjectRef::Code(y)) => {
            let (ca, cb) = (CodeObject::from_word(x), CodeObject::from_word(y));
            if code_entry(a, ca).unwrap() != code_entry(b, cb).unwrap()
                || code_size(a, ca).unwrap() != code_size(b, cb).unwrap()
            {
                return false;
            }
            let (ka, kb) = (
                code_constants(a, ca).unwrap(),
                code_constants(b, cb).unwrap(),
            );
            let (sa, sb) = (
                code_stack_map(a, ca).unwrap(),
                code_stack_map(b, cb).unwrap(),
            );
            let (da, db) = (code_debug(a, ca).unwrap(), code_debug(b, cb).unwrap());
            iso(a, ka, b, kb, seen) && iso(a, sa, b, sb, seen) && iso(a, da, b, db, seen)
        }
        _ => false,
    }
}

/// Report whether a word addresses a heap object.
fn is_heap(word: Word) -> bool {
    if word.is_fixnum() || word.is_character() || word.is_unbound() {
        return false;
    }
    let tag = word.lowtag();
    if tag == LowTag::List as u8 {
        return word != Word::NIL;
    }
    tag == LowTag::Instance as u8 || tag == LowTag::OtherPointer as u8
}

/// Read a string object into a Rust string.
fn read_string(ctx: &ThreadContext, word: Word) -> String {
    let length = string_length(ctx, word).unwrap();
    (0..length)
        .map(|index| string_ref(ctx, word, index).unwrap())
        .collect()
}

/// Read a symbol's home package name.
fn package_name(ctx: &ThreadContext, symbol: Word) -> String {
    let package = symbol_package(ctx, symbol).unwrap();
    if package == Word::NIL {
        String::new()
    } else {
        read_string(ctx, Package::from_word(package).name(ctx).unwrap())
    }
}
