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
fn header_options_cover_named_and_disabled_defstruct_features() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = setup();
    let defstruct = intern(&runtime, &mut ctx, "DEFSTRUCT");
    let record = intern(&runtime, &mut ctx, "HEADER-OPTIONS-RECORD");
    let include = intern(&runtime, &mut ctx, ":INCLUDE");
    let predicate = intern(&runtime, &mut ctx, ":PREDICATE");
    let predicate_name = intern(&runtime, &mut ctx, "HEADER-OPTIONS-P");
    let copier = intern(&runtime, &mut ctx, ":COPIER");
    let copier_name = intern(&runtime, &mut ctx, "COPY-HEADER-OPTIONS");
    let conc_name = intern(&runtime, &mut ctx, ":CONC-NAME");
    let prefix = intern(&runtime, &mut ctx, "HEADER-");
    let constructor = intern(&runtime, &mut ctx, ":CONSTRUCTOR");
    let print_function = intern(&runtime, &mut ctx, ":PRINT-FUNCTION");
    let printer = intern(&runtime, &mut ctx, "HEADER-OPTIONS-PRINTER");
    let slot = intern(&runtime, &mut ctx, "VALUE");
    let accessor = intern(&runtime, &mut ctx, "HEADER-VALUE");

    let include_option = list(&mut ctx, &runtime, &[include, Word::NIL]);
    let predicate_option = list(&mut ctx, &runtime, &[predicate, predicate_name]);
    let copier_option = list(&mut ctx, &runtime, &[copier, copier_name]);
    let conc_name_option = list(&mut ctx, &runtime, &[conc_name, prefix]);
    let constructor_option = list(&mut ctx, &runtime, &[constructor, Word::NIL]);
    let print_function_option = list(&mut ctx, &runtime, &[print_function, printer]);
    let header = list(
        &mut ctx,
        &runtime,
        &[
            record,
            include_option,
            predicate_option,
            copier_option,
            conc_name_option,
            constructor_option,
            print_function_option,
        ],
    );
    let form = list(&mut ctx, &runtime, &[defstruct, header, slot]);
    let expansion = expand(&mut ctx, &runtime, form)?;

    assert!(contains(&ctx, expansion, predicate_name)?);
    assert!(contains(&ctx, expansion, copier_name)?);
    assert!(contains(&ctx, expansion, accessor)?);
    let plist = ncl_object::symbol_plist(&ctx, record)?;
    assert!(contains(&ctx, plist, printer)?);
    assert!(!runtime.structure_layout_for_symbol(&ctx, record).is_none());
    Ok(())
}

#[test]
fn trailing_include_symbol_inherits_the_parent_layout() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = setup();
    let defstruct = intern(&runtime, &mut ctx, "DEFSTRUCT");
    let base = intern(&runtime, &mut ctx, "TRAILING-INCLUDE-BASE");
    let child = intern(&runtime, &mut ctx, "TRAILING-INCLUDE-CHILD");
    let include = intern(&runtime, &mut ctx, ":INCLUDE");
    let base_slot = intern(&runtime, &mut ctx, "BASE-VALUE");
    let child_slot = intern(&runtime, &mut ctx, "CHILD-VALUE");
    let base_form = list(&mut ctx, &runtime, &[defstruct, base, base_slot]);
    expand(&mut ctx, &runtime, base_form)?;

    let child_form = list(
        &mut ctx,
        &runtime,
        &[defstruct, child, child_slot, include, base],
    );
    let expansion = expand(&mut ctx, &runtime, child_form)?;

    assert!(contains(&ctx, expansion, base_slot)?);
    assert!(contains(&ctx, expansion, child_slot)?);
    assert!(runtime.structure_layout_for_symbol(&ctx, child).is_some());
    Ok(())
}

#[test]
fn trailing_options_cover_named_constructor_and_include_paths() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = setup();
    let defstruct = intern(&runtime, &mut ctx, "DEFSTRUCT");
    let base = intern(&runtime, &mut ctx, "TRAILING-OPTIONS-BASE");
    let child = intern(&runtime, &mut ctx, "TRAILING-OPTIONS-CHILD");
    let base_slot = intern(&runtime, &mut ctx, "BASE-VALUE");
    let child_slot = intern(&runtime, &mut ctx, "CHILD-VALUE");
    let include = intern(&runtime, &mut ctx, ":INCLUDE");
    let conc_name = intern(&runtime, &mut ctx, ":CONC-NAME");
    let prefix = intern(&runtime, &mut ctx, "TRAILING-");
    let predicate = intern(&runtime, &mut ctx, ":PREDICATE");
    let predicate_name = intern(&runtime, &mut ctx, "TRAILING-OPTIONS-P");
    let copier = intern(&runtime, &mut ctx, ":COPIER");
    let copier_name = intern(&runtime, &mut ctx, "COPY-TRAILING-OPTIONS");
    let constructor = intern(&runtime, &mut ctx, ":CONSTRUCTOR");
    let constructor_name = intern(&runtime, &mut ctx, "MAKE-TRAILING-OPTIONS");
    let type_option = intern(&runtime, &mut ctx, ":TYPE");
    let structure = intern(&runtime, &mut ctx, "STRUCTURE");
    let override_slot = list(&mut ctx, &runtime, &[base_slot, Word::fixnum(9)]);
    let include_value = list(&mut ctx, &runtime, &[base, override_slot]);
    let lambda = list(&mut ctx, &runtime, &[child_slot]);
    let base_form = list(&mut ctx, &runtime, &[defstruct, base, base_slot]);
    expand(&mut ctx, &runtime, base_form)?;
    let form = list(
        &mut ctx,
        &runtime,
        &[
            defstruct,
            child,
            child_slot,
            conc_name,
            prefix,
            predicate,
            predicate_name,
            copier,
            copier_name,
            constructor,
            constructor_name,
            lambda,
            include,
            include_value,
            type_option,
            structure,
        ],
    );
    let expansion = expand(&mut ctx, &runtime, form)?;

    assert!(contains(&ctx, expansion, constructor_name)?);
    assert!(contains(&ctx, expansion, predicate_name)?);
    assert!(contains(&ctx, expansion, copier_name)?);
    assert!(contains(&ctx, expansion, Word::fixnum(9))?);
    assert!(contains(&ctx, expansion, child_slot)?);
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

#[test]
fn option_error_and_disabled_paths_are_explicitly_exercised() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = setup();
    let defstruct = intern(&runtime, &mut ctx, "DEFSTRUCT");
    let header_record = intern(&runtime, &mut ctx, "OPTION-BRANCH-HEADER");
    let header_slot = intern(&runtime, &mut ctx, "HEADER-SLOT");
    let include = intern(&runtime, &mut ctx, ":INCLUDE");
    let predicate = intern(&runtime, &mut ctx, ":PREDICATE");
    let copier = intern(&runtime, &mut ctx, ":COPIER");
    let print_function = intern(&runtime, &mut ctx, ":PRINT-FUNCTION");
    let printer = intern(&runtime, &mut ctx, "OPTION-BRANCH-PRINTER");
    let unknown = intern(&runtime, &mut ctx, ":OPTION-BRANCH-UNKNOWN");
    let include_override = list(&mut ctx, &runtime, &[header_slot, Word::fixnum(7)]);
    let include_form = list(&mut ctx, &runtime, &[include, Word::NIL, include_override]);
    let predicate_form = list(&mut ctx, &runtime, &[predicate, Word::NIL]);
    let copier_form = list(&mut ctx, &runtime, &[copier, Word::NIL]);
    let print_function_form = list(&mut ctx, &runtime, &[print_function, printer]);
    let header = list(
        &mut ctx,
        &runtime,
        &[
            header_record,
            include_form,
            predicate_form,
            copier_form,
            print_function_form,
        ],
    );
    let form = list(&mut ctx, &runtime, &[defstruct, header, header_slot]);
    let expansion = expand(&mut ctx, &runtime, form)?;
    assert!(contains(&ctx, expansion, Word::NIL)?);
    let plist = ncl_object::symbol_plist(&ctx, header_record)?;
    assert!(contains(&ctx, plist, printer)?);

    let unknown_form = list(&mut ctx, &runtime, &[unknown, Word::NIL]);
    let unknown_header_options = list(&mut ctx, &runtime, &[header_record, unknown_form]);
    let unknown_header = list(&mut ctx, &runtime, &[defstruct, unknown_header_options]);
    assert_eq!(
        expand(&mut ctx, &runtime, unknown_header),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

#[test]
fn trailing_option_error_and_print_paths_are_explicitly_exercised() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = setup();
    let defstruct = intern(&runtime, &mut ctx, "DEFSTRUCT");
    let record = intern(&runtime, &mut ctx, "OPTION-BRANCH-TRAILING");
    let slot = intern(&runtime, &mut ctx, "TRAILING-SLOT");
    let print_function = intern(&runtime, &mut ctx, ":PRINT-FUNCTION");
    let printer = intern(&runtime, &mut ctx, "OPTION-BRANCH-TRAILING-PRINTER");
    let predicate = intern(&runtime, &mut ctx, ":PREDICATE");
    let predicate_name = intern(&runtime, &mut ctx, "OPTION-BRANCH-TRAILING-P");
    let copier = intern(&runtime, &mut ctx, ":COPIER");
    let copier_name = intern(&runtime, &mut ctx, "COPY-OPTION-BRANCH-TRAILING");
    let type_option = intern(&runtime, &mut ctx, ":TYPE");
    let wrong_type = intern(&runtime, &mut ctx, "VECTOR");
    let structure = intern(&runtime, &mut ctx, "STRUCTURE");
    let wrong_type_record = intern(&runtime, &mut ctx, "OPTION-BRANCH-WRONG-TYPE");
    let form = list(
        &mut ctx,
        &runtime,
        &[
            defstruct,
            record,
            slot,
            print_function,
            printer,
            predicate,
            predicate_name,
            copier,
            copier_name,
            type_option,
            structure,
        ],
    );
    let expansion = expand(&mut ctx, &runtime, form)?;
    let plist = ncl_object::symbol_plist(&ctx, record)?;
    assert!(contains(&ctx, plist, printer)?);
    assert!(contains(&ctx, expansion, predicate_name)?);
    assert!(contains(&ctx, expansion, copier_name)?);

    let wrong_type_form = list(
        &mut ctx,
        &runtime,
        &[defstruct, wrong_type_record, type_option, wrong_type],
    );
    assert_eq!(
        expand(&mut ctx, &runtime, wrong_type_form),
        Err(ObjectError::TypeError)
    );
    Ok(())
}
