#![allow(missing_docs)]
#![allow(
    clippy::unwrap_used,
    reason = "tests assert on public structure builtins"
)]

use ncl_object::{FunctionObject, ObjectError, Package, Runtime, ThreadContext, Word};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_clos::register(&runtime).unwrap();
    (runtime, ctx)
}

fn function(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> FunctionObject {
    FunctionObject::try_from(runtime.function(ctx, "COMMON-LISP", name).unwrap()).unwrap()
}

fn intern(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> Word {
    let package = runtime.find_package(ctx, "COMMON-LISP-USER").unwrap();
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap()
        .0
}

fn list(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Word {
    let mut scope = ncl_object::Scope::new(ctx);
    let roots = scope.root_many(
        &values
            .iter()
            .copied()
            .map(ncl_object::Local::from_word)
            .collect::<Vec<_>>(),
    );
    let result = scope.make_list(runtime, &roots).unwrap();
    scope.get(result).as_word()
}

fn contains(ctx: &ThreadContext, value: Word, target: Word) -> bool {
    value == target
        || (value.is_cons()
            && (contains(ctx, ncl_object::car(ctx, value).unwrap(), target)
                || contains(ctx, ncl_object::cdr(ctx, value).unwrap(), target)))
}

#[test]
fn structure_builtins_copy_and_check_layout_and_slot_boundaries() {
    let (runtime, mut ctx) = setup();
    let make = function(&runtime, &mut ctx, "%STRUCTURE-MAKE");
    let reference = function(&runtime, &mut ctx, "%STRUCTURE-REF");
    let set = function(&runtime, &mut ctx, "%STRUCTURE-SET");
    let predicate = function(&runtime, &mut ctx, "%STRUCTURE-P");
    let copy = function(&runtime, &mut ctx, "%STRUCTURE-COPY");
    let layout = runtime.register_structure_layout(2).unwrap();
    let layout_word = Word::fixnum(i64::from(layout.as_u32()));
    let source = runtime
        .call_builtin(
            &mut ctx,
            make,
            &[layout_word, Word::fixnum(10), Word::fixnum(20)],
        )
        .unwrap();

    assert_eq!(
        runtime.call_builtin(&mut ctx, reference, &[source, Word::fixnum(0)]),
        Ok(Word::fixnum(10))
    );
    assert!(
        runtime
            .call_builtin(&mut ctx, reference, &[source, Word::fixnum(2)])
            .is_err()
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, set, &[source, Word::fixnum(1), Word::fixnum(21)]),
        Ok(Word::fixnum(21))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, reference, &[source, Word::fixnum(1)]),
        Ok(Word::fixnum(21))
    );

    let copied = runtime.call_builtin(&mut ctx, copy, &[source]).unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, predicate, &[source, layout_word]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, predicate, &[copied, layout_word]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, set, &[source, Word::fixnum(0), Word::fixnum(99)]),
        Ok(Word::fixnum(99))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, reference, &[copied, Word::fixnum(0)]),
        Ok(Word::fixnum(10))
    );

    let other_layout = runtime.register_structure_layout(2).unwrap();
    let other_layout_word = Word::fixnum(i64::from(other_layout.as_u32()));
    assert_eq!(
        runtime.call_builtin(&mut ctx, predicate, &[source, other_layout_word]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, make, &[layout_word, Word::fixnum(1)]),
        Err(ObjectError::Layout)
    );
    assert!(
        runtime
            .call_builtin(&mut ctx, reference, &[Word::NIL, Word::fixnum(0)])
            .is_err()
    );
}

#[test]
fn defstruct_public_macro_records_layout_and_accessor_contracts() {
    let (runtime, mut ctx) = setup();
    let defstruct = function(&runtime, &mut ctx, "DEFSTRUCT");
    let record = intern(&runtime, &mut ctx, "STRUCTURE-BOUNDARY");
    let constructor = intern(&runtime, &mut ctx, ":CONSTRUCTOR");
    let conc_name = intern(&runtime, &mut ctx, ":CONC-NAME");
    let value = intern(&runtime, &mut ctx, "VALUE");
    let defstruct_name = intern(&runtime, &mut ctx, "DEFSTRUCT");
    let constructor_option = list(&mut ctx, &runtime, &[constructor, Word::NIL]);
    let name_form = list(&mut ctx, &runtime, &[record, constructor_option]);
    let form = list(
        &mut ctx,
        &runtime,
        &[defstruct_name, name_form, value, conc_name, Word::NIL],
    );

    let expansion = runtime.call_builtin(&mut ctx, defstruct, &[form]).unwrap();
    let layout = runtime.structure_layout_for_symbol(&ctx, record).unwrap();
    assert_eq!(runtime.structure_layout_size(layout), Some(1));
    assert!(contains(&ctx, expansion, value));
    let accessor = intern(&runtime, &mut ctx, "VALUE");
    assert!(contains(&ctx, expansion, accessor));
    assert!(runtime.structure_layout_is_a(layout, layout));
}
