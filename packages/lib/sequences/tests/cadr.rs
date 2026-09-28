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
) -> Word {
    fn build(
        runtime: &Runtime,
        ctx: &mut ThreadContext,
        functions: &HashMap<&str, FunctionObject>,
        depth: usize,
        next_leaf: &mut i64,
    ) -> Word {
        if depth == 0 {
            let value = Word::fixnum(*next_leaf);
            *next_leaf += 1;
            return value;
        }
        let mut left = build(runtime, ctx, functions, depth - 1, next_leaf);
        let left_root = ncl_object::push_root(ctx, &mut left);
        let right = build(runtime, ctx, functions, depth - 1, next_leaf);
        let value = call(runtime, ctx, functions, "LIST", &[left, right]);
        assert!(ncl_object::pop_root(ctx, left_root));
        value
    }

    let mut next_leaf = 0;
    build(runtime, ctx, functions, 4, &mut next_leaf)
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
                    .unwrap_or_else(|error| panic!("{name} is not a function: {error:?}")),
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
        let argument = nested_argument(&runtime, &mut ctx, &functions);
        ncl_object::with_roots(&mut ctx, &[argument], |ctx, roots| {
            let mut expected = *roots[0];
            for accessor in name[1..name.len() - 1].chars().rev() {
                expected = match accessor {
                    'A' => ncl_object::car(ctx, expected)
                        .unwrap_or_else(|error| panic!("{name} CAR failed: {error:?}")),
                    'D' => ncl_object::cdr(ctx, expected)
                        .unwrap_or_else(|error| panic!("{name} CDR failed: {error:?}")),
                    _ => panic!("unexpected accessor {accessor}"),
                };
            }
            let actual = call(&runtime, ctx, &functions, name, &[*roots[0]]);
            assert_eq!(actual, expected, "{name} returned the wrong value");
            Ok::<(), ncl_object::ObjectError>(())
        })
        .unwrap_or_else(|error| panic!("{name} root failed: {error:?}"));
    }
}
