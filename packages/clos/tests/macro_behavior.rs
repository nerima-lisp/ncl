#![allow(missing_docs)]
#![allow(
    clippy::unwrap_used,
    reason = "tests assert on CLOS macro expansion contracts"
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

fn elements(ctx: &ThreadContext, mut value: Word) -> Result<Vec<Word>, ObjectError> {
    let mut result = Vec::new();
    while value != Word::NIL {
        result.push(ncl_object::car(ctx, value)?);
        value = ncl_object::cdr(ctx, value)?;
    }
    Ok(result)
}

fn name(ctx: &ThreadContext, symbol: Word) -> String {
    let string = ncl_object::symbol_name(ctx, symbol).unwrap();
    (0..ncl_object::string_length(ctx, string).unwrap())
        .map(|index| ncl_object::string_ref(ctx, string, index).unwrap())
        .collect()
}

fn call_macro(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    macro_name: &str,
    form: Word,
) -> Result<Word, ObjectError> {
    let function = FunctionObject::try_from(
        runtime
            .function(ctx, "COMMON-LISP", macro_name)
            .ok_or(ObjectError::UndefinedFunction)?,
    )
    .map_err(|_| ObjectError::TypeError)?;
    runtime.call_builtin(ctx, function, &[form])
}

#[test]
fn defclass_registers_slot_metadata_and_emits_accessor_definition() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = setup();
    let defclass = intern(&runtime, &mut ctx, "DEFCLASS");
    let profile = intern(&runtime, &mut ctx, "MACRO-PROFILE");
    let superclass = intern(&runtime, &mut ctx, "STANDARD-OBJECT");
    let display_name = intern(&runtime, &mut ctx, "DISPLAY-NAME");
    let initarg = intern(&runtime, &mut ctx, ":INITARG");
    let display_name_arg = intern(&runtime, &mut ctx, ":DISPLAY-NAME");
    let initform = intern(&runtime, &mut ctx, ":INITFORM");
    let accessor = intern(&runtime, &mut ctx, ":ACCESSOR");
    let reader = intern(&runtime, &mut ctx, "MACRO-PROFILE-DISPLAY-NAME");
    let slot = list(
        &mut ctx,
        &runtime,
        &[
            display_name,
            initarg,
            display_name_arg,
            initform,
            Word::fixnum(7),
            accessor,
            reader,
        ],
    );
    let superclass_list = list(&mut ctx, &runtime, &[superclass]);
    let slot_list = list(&mut ctx, &runtime, &[slot]);
    let form = list(
        &mut ctx,
        &runtime,
        &[defclass, profile, superclass_list, slot_list],
    );

    let expansion = call_macro(&runtime, &mut ctx, "DEFCLASS", form)?;
    let expansion_elements = elements(&ctx, expansion)?;
    assert_eq!(name(&ctx, expansion_elements[0]), "PROGN");
    let accessor_definition = elements(&ctx, expansion_elements[1])?;
    assert_eq!(name(&ctx, accessor_definition[0]), "DEFUN");
    assert_eq!(accessor_definition[1], reader);
    assert_eq!(elements(&ctx, accessor_definition[2])?.len(), 1);
    let body = elements(&ctx, accessor_definition[3])?;
    assert_eq!(name(&ctx, body[0]), "SLOT-VALUE");
    assert_eq!(body[1], intern_in(&runtime, &mut ctx, "NCL", "INSTANCE"));
    let quoted_slot = elements(&ctx, body[2])?;
    assert_eq!(name(&ctx, quoted_slot[0]), "QUOTE");
    assert_eq!(quoted_slot[1], display_name);

    let class = runtime.class(&mut ctx, "MACRO-PROFILE").unwrap();
    let slots = ncl_object::simple_vector_ref(&ctx, class, 2)?;
    let descriptor = ncl_object::simple_vector_ref(&ctx, slots, 0)?;
    assert_eq!(
        ncl_object::simple_vector_ref(&ctx, descriptor, 0),
        Ok(display_name)
    );
    assert_eq!(
        ncl_object::simple_vector_ref(&ctx, descriptor, 1),
        Ok(display_name_arg)
    );
    assert_eq!(
        ncl_object::simple_vector_ref(&ctx, descriptor, 2),
        Ok(Word::fixnum(7))
    );
    Ok(())
}

#[test]
fn defclass_without_accessors_returns_nil_after_registering_class() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = setup();
    let defclass = intern(&runtime, &mut ctx, "DEFCLASS");
    let class_name = intern(&runtime, &mut ctx, "MACRO-NO-ACCESSOR");
    let superclass = intern(&runtime, &mut ctx, "STANDARD-OBJECT");
    let superclass_list = list(&mut ctx, &runtime, &[superclass]);
    let form = list(&mut ctx, &runtime, &[defclass, class_name, superclass_list]);

    assert_eq!(
        call_macro(&runtime, &mut ctx, "DEFCLASS", form),
        Ok(Word::NIL)
    );
    let class = runtime
        .class(&mut ctx, "MACRO-NO-ACCESSOR")
        .ok_or(ObjectError::Layout)?;
    assert_eq!(
        ncl_object::simple_vector_ref(&ctx, class, 0),
        Ok(class_name)
    );
    Ok(())
}

#[test]
fn defgeneric_expansion_returns_quoted_generic_name() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = setup();
    let generic_name = intern(&runtime, &mut ctx, "MACRO-GENERIC");
    let defgeneric = intern(&runtime, &mut ctx, "DEFGENERIC");
    let form = list(&mut ctx, &runtime, &[defgeneric, generic_name]);

    let expansion = call_macro(&runtime, &mut ctx, "DEFGENERIC", form)?;
    let values = elements(&ctx, expansion)?;
    assert_eq!(values.len(), 4);
    assert_eq!(name(&ctx, values[0]), "PROGN");
    let clear = elements(&ctx, values[1])?;
    assert_eq!(name(&ctx, clear[0]), "%CLOS-DEFINE-GENERIC");
    let clear_name = elements(&ctx, clear[1])?;
    assert_eq!(name(&ctx, clear_name[0]), "QUOTE");
    assert_eq!(clear_name[1], generic_name);
    let function = elements(&ctx, values[2])?;
    assert_eq!(name(&ctx, function[0]), "DEFUN");
    assert_eq!(function[1], generic_name);
    let returned_name = elements(&ctx, values[3])?;
    assert_eq!(name(&ctx, returned_name[0]), "QUOTE");
    assert_eq!(returned_name[1], generic_name);
    Ok(())
}
