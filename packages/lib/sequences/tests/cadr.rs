#![allow(missing_docs)]

use std::collections::HashMap;

use ncl_object::{FunctionObject, Runtime, ThreadContext, Word};

fn call(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    functions: &HashMap<&str, FunctionObject>,
    name: &str,
    args: &[Word],
) -> Word {
    let function = *functions
        .get(name)
        .unwrap_or_else(|| panic!("missing builtin {name}"));
    ncl_object::with_roots(ctx, args, |ctx, roots| {
        let rooted_args = roots.iter().map(|root| **root).collect::<Vec<_>>();
        runtime.call_builtin(ctx, function, &rooted_args)
    })
    .unwrap_or_else(|error| panic!("{name} failed: {error:?}"))
}

fn nested_argument(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    functions: &HashMap<&str, FunctionObject>,
    name: &str,
) -> Word {
    let mut value = Word::fixnum(42);
    let root = ncl_object::push_root(ctx, &mut value);
    for _ in name[1..name.len() - 1].chars() {
        value = call(runtime, ctx, functions, "LIST", &[value, value]);
    }
    assert!(ncl_object::pop_root(ctx, root));
    value
}

#[test]
fn registers_and_composes_all_cadr_family_builtins() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("context: {error:?}"));
    ncl_lib_sequences::register(&runtime).unwrap_or_else(|error| panic!("sequences: {error:?}"));

    let names = [
        "CAAR", "CADR", "CDAR", "CDDR", "CAAAR", "CAADR", "CADAR", "CADDR", "CDAAR", "CDADR",
        "CDDAR", "CDDDR", "CAAAAR", "CAAADR", "CAADAR", "CAADDR", "CADAAR", "CADADR", "CADDAR",
        "CADDDR", "CDAAAR", "CDAADR", "CDADAR", "CDADDR", "CDDAAR", "CDDADR", "CDDDAR", "CDDDDR",
    ];
    let functions = names
        .iter()
        .map(|name| {
            let word = runtime
                .function(&mut ctx, "COMMON-LISP", name)
                .unwrap_or_else(|| panic!("missing builtin {name}"));
            (
                *name,
                FunctionObject::try_from(word)
                    .unwrap_or_else(|_| panic!("{name} is not a function")),
            )
        })
        .collect::<HashMap<_, _>>();
    let list = runtime
        .function(&mut ctx, "COMMON-LISP", "LIST")
        .and_then(|word| FunctionObject::try_from(word).ok())
        .unwrap_or_else(|| panic!("missing builtin LIST"));
    let mut functions = functions;
    functions.insert("LIST", list);

    for name in names {
        let argument = nested_argument(&runtime, &mut ctx, &functions, name);
        ncl_object::with_roots(&mut ctx, &[argument], |ctx, roots| {
            let actual = call(&runtime, ctx, &functions, name, &[*roots[0]]);
            assert_ne!(actual, Word::UNBOUND, "{name} returned an unbound value");
            Ok::<_, ncl_object::ObjectError>(())
        })
        .unwrap_or_else(|error| panic!("{name} root failed: {error:?}"));
    }
}
