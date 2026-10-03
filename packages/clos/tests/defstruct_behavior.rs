#![allow(missing_docs)]
#![allow(
    clippy::unwrap_used,
    reason = "tests assert on DEFSTRUCT expansion contracts"
)]

use ncl_object::{FunctionObject, ObjectError, Package, Runtime, ThreadContext, Word};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_clos::register(&runtime).unwrap();
    (runtime, ctx)
}

fn intern(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> Word {
    intern_in(runtime, ctx, "COMMON-LISP-USER", name)
}

fn intern_in(runtime: &Runtime, ctx: &mut ThreadContext, package_name: &str, name: &str) -> Word {
    let package = runtime.find_package(ctx, package_name).unwrap();
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

fn defstruct(runtime: &Runtime, ctx: &mut ThreadContext, form: Word) -> Result<Word, ObjectError> {
    let function =
        FunctionObject::try_from(runtime.function(ctx, "COMMON-LISP", "DEFSTRUCT").unwrap())
            .unwrap();
    runtime.call_builtin(ctx, function, &[form])
}

#[test]
fn include_override_inherits_slots_and_replaces_the_parent_default() {
    let (runtime, mut ctx) = setup();
    let defstruct_name = intern(&runtime, &mut ctx, "DEFSTRUCT");
    let base = intern(&runtime, &mut ctx, "COVERAGE-BASE");
    let base_slot = intern(&runtime, &mut ctx, "BASE-VALUE");
    let child = intern(&runtime, &mut ctx, "COVERAGE-CHILD");
    let child_slot = intern(&runtime, &mut ctx, "CHILD-VALUE");
    let include = intern(&runtime, &mut ctx, ":INCLUDE");

    let base_slot_form = list(&mut ctx, &runtime, &[base_slot, Word::fixnum(1)]);
    let base_form = list(&mut ctx, &runtime, &[defstruct_name, base, base_slot_form]);
    let override_slot = list(&mut ctx, &runtime, &[base_slot, Word::fixnum(9)]);
    let include_base = list(&mut ctx, &runtime, &[base, override_slot]);
    let include_form = list(&mut ctx, &runtime, &[include, include_base]);
    defstruct(&runtime, &mut ctx, base_form).unwrap();
    let child_form = list(
        &mut ctx,
        &runtime,
        &[defstruct_name, child, include_form, child_slot],
    );
    let expansion = defstruct(&runtime, &mut ctx, child_form).unwrap();

    assert!(contains(&ctx, expansion, base_slot));
    assert!(contains(&ctx, expansion, child_slot));
    assert!(contains(&ctx, expansion, Word::fixnum(9)));
    assert!(runtime.structure_layout_for_symbol(&ctx, child).is_some());
}

#[test]
fn nil_predicate_and_copier_disable_generated_forms_and_read_only_omits_setter() {
    let (runtime, mut ctx) = setup();
    let defstruct_name = intern(&runtime, &mut ctx, "DEFSTRUCT");
    let record = intern(&runtime, &mut ctx, "COVERAGE-OPTIONS");
    let predicate = intern(&runtime, &mut ctx, ":PREDICATE");
    let copier = intern(&runtime, &mut ctx, ":COPIER");
    let read_only = intern(&runtime, &mut ctx, ":READ-ONLY");
    let value = intern(&runtime, &mut ctx, "OPTION-VALUE");
    let predicate_name = intern(&runtime, &mut ctx, "COVERAGE-OPTIONS-P");
    let copier_name = intern(&runtime, &mut ctx, "COPY-COVERAGE-OPTIONS");
    let setter = intern(&runtime, &mut ctx, "%STRUCTURE-SET");
    let slot = list(
        &mut ctx,
        &runtime,
        &[value, Word::NIL, read_only, Word::TRUE],
    );
    let form = list(
        &mut ctx,
        &runtime,
        &[
            defstruct_name,
            record,
            slot,
            predicate,
            Word::NIL,
            copier,
            Word::NIL,
        ],
    );

    let expansion = defstruct(&runtime, &mut ctx, form).unwrap();
    assert!(!contains(&ctx, expansion, predicate_name));
    assert!(!contains(&ctx, expansion, copier_name));
    assert!(!contains(&ctx, expansion, setter));
    assert!(contains(&ctx, expansion, value));
}

#[test]
fn print_function_is_recorded_and_non_structure_type_is_rejected() {
    let (runtime, mut ctx) = setup();
    let defstruct_name = intern(&runtime, &mut ctx, "DEFSTRUCT");
    let record = intern(&runtime, &mut ctx, "COVERAGE-PRINT");
    let print_option = intern(&runtime, &mut ctx, ":PRINT-FUNCTION");
    let print_function = intern(&runtime, &mut ctx, "COVERAGE-PRINT-FUNCTION");
    let print_form = list(
        &mut ctx,
        &runtime,
        &[defstruct_name, record, print_option, print_function],
    );
    let expansion = defstruct(&runtime, &mut ctx, print_form).unwrap();
    let key = intern_in(&runtime, &mut ctx, "NCL", "%STRUCTURE-PRINT-FUNCTION");
    let plist = ncl_object::symbol_plist(&ctx, record).unwrap();
    assert!(contains(&ctx, plist, key));
    assert!(contains(&ctx, plist, print_function));
    assert!(contains(&ctx, expansion, record));

    let type_option = intern(&runtime, &mut ctx, ":TYPE");
    let vector = intern(&runtime, &mut ctx, "VECTOR");
    let bad_record = intern(&runtime, &mut ctx, "COVERAGE-BAD-TYPE");
    let invalid_form = list(
        &mut ctx,
        &runtime,
        &[defstruct_name, bad_record, type_option, vector],
    );
    let invalid = defstruct(&runtime, &mut ctx, invalid_form);
    assert_eq!(invalid, Err(ObjectError::TypeError));
}
