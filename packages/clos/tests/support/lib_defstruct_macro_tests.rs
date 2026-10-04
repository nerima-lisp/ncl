use super::*;

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("context registration: {error:?}"));
    register(&runtime).unwrap_or_else(|error| panic!("CLOS registration: {error:?}"));
    (runtime, ctx)
}

fn intern(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> Word {
    let package = runtime
        .find_package(ctx, "COMMON-LISP-USER")
        .unwrap_or_else(|| panic!("COMMON-LISP-USER package"));
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap_or_else(|error| panic!("intern: {error:?}"))
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
    let result = scope
        .make_list(runtime, &roots)
        .unwrap_or_else(|error| panic!("proper list: {error:?}"));
    scope.get(result).as_word()
}

fn expand(ctx: &mut ThreadContext, runtime: &Runtime, form: Word) -> Result<Word, ObjectError> {
    defstruct_macro_builtin(
        ctx,
        runtime,
        &ncl_object::BuiltinArgs::new(&[form]),
        &mut MultipleValues::default(),
    )
}

fn contains(ctx: &ThreadContext, value: Word, target: Word) -> Result<bool, ObjectError> {
    if value == target || !value.is_cons() {
        return Ok(value == target);
    }
    Ok(contains(ctx, ncl_object::car(ctx, value)?, target)?
        || contains(ctx, ncl_object::cdr(ctx, value)?, target)?)
}

#[test]
fn header_include_override_expands_to_the_exact_override_value() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = setup();
    let defstruct = intern(&runtime, &mut ctx, "DEFSTRUCT");
    let base = intern(&runtime, &mut ctx, "HEADER-BASE");
    let child = intern(&runtime, &mut ctx, "HEADER-CHILD");
    let base_slot = intern(&runtime, &mut ctx, "BASE-VALUE");
    let child_slot = intern(&runtime, &mut ctx, "CHILD-VALUE");
    let include = intern(&runtime, &mut ctx, ":INCLUDE");
    let base_slot_form = list(&mut ctx, &runtime, &[base_slot, Word::fixnum(1)]);
    let base_form = list(&mut ctx, &runtime, &[defstruct, base, base_slot_form]);
    expand(&mut ctx, &runtime, base_form)?;

    let override_slot = list(&mut ctx, &runtime, &[base_slot, Word::fixnum(9)]);
    let include_value = list(&mut ctx, &runtime, &[base, override_slot]);
    let include_option = list(&mut ctx, &runtime, &[include, include_value]);
    let child_header = list(&mut ctx, &runtime, &[child, include_option]);
    let child_form = list(&mut ctx, &runtime, &[defstruct, child_header, child_slot]);
    let expansion = expand(&mut ctx, &runtime, child_form)?;

    assert!(contains(&ctx, expansion, base_slot)?);
    assert!(contains(&ctx, expansion, child_slot)?);
    assert!(contains(&ctx, expansion, Word::fixnum(9))?);
    assert!(runtime.structure_layout_for_symbol(&ctx, child).is_some());
    Ok(())
}

#[test]
fn malformed_defstruct_forms_return_type_error_with_no_partial_value() {
    let (runtime, mut ctx) = setup();
    let defstruct = intern(&runtime, &mut ctx, "DEFSTRUCT");
    let record = intern(&runtime, &mut ctx, "MALFORMED-RECORD");
    let predicate = intern(&runtime, &mut ctx, ":PREDICATE");
    let constructor = intern(&runtime, &mut ctx, ":CONSTRUCTOR");

    let missing_name = list(&mut ctx, &runtime, &[defstruct]);
    assert_eq!(
        expand(&mut ctx, &runtime, missing_name),
        Err(ObjectError::TypeError)
    );

    let short_header = list(&mut ctx, &runtime, &[record, predicate]);
    let short_header_form = list(&mut ctx, &runtime, &[defstruct, short_header]);
    assert_eq!(
        expand(&mut ctx, &runtime, short_header_form),
        Err(ObjectError::TypeError)
    );

    let trailing_option = list(&mut ctx, &runtime, &[defstruct, record, constructor]);
    assert_eq!(
        expand(&mut ctx, &runtime, trailing_option),
        Err(ObjectError::TypeError)
    );
}
