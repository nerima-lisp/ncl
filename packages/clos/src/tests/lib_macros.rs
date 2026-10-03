use super::*;

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().expect("runtime");
    let mut context = ThreadContext::new();
    context.register(&runtime).expect("context");
    (runtime, context)
}

fn call_macro(
    context: &mut ThreadContext,
    runtime: &Runtime,
    callback: for<'a> fn(
        &mut ThreadContext,
        &Runtime,
        &BuiltinArgs<'a>,
        &mut MultipleValues,
    ) -> Result<Word, ObjectError>,
) -> Result<Word, ObjectError> {
    let values = [Word::fixnum(1)];
    let args = BuiltinArgs::new(&values);
    let mut values = MultipleValues::new();
    callback(context, runtime, &args, &mut values)
}

#[test]
fn malformed_macro_forms_report_type_errors() {
    let (runtime, mut context) = setup();
    assert_eq!(
        call_macro(&mut context, &runtime, defclass_macro_builtin),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call_macro(&mut context, &runtime, defgeneric_macro_builtin),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call_macro(&mut context, &runtime, defmethod_macro_builtin),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn method_qualifier_accepts_supported_names_and_ignores_other_values() {
    let (runtime, mut context) = setup();
    let package = runtime.ensure_package(&mut context, COMMON_LISP).unwrap();
    let before = Package::from_word(package)
        .intern(&mut context, &runtime, ":BEFORE")
        .unwrap()
        .0;
    let after = Package::from_word(package)
        .intern(&mut context, &runtime, "AFTER")
        .unwrap()
        .0;
    assert_eq!(method_qualifier(&context, before), Ok(Some(Word::fixnum(1))));
    assert_eq!(method_qualifier(&context, after), Ok(Some(Word::fixnum(2))));
    assert_eq!(method_qualifier(&context, Word::fixnum(0)), Ok(None));
}

#[test]
fn macro_rewrite_helpers_cover_initialization_and_next_method_forms() -> Result<(), ObjectError> {
    let (runtime, mut context) = setup();
    let mut scope = Scope::new(&mut context);
    let initialize = scope.intern(&runtime, COMMON_LISP, "INITIALIZE-INSTANCE")?;
    let ordinary = scope.intern(&runtime, COMMON_LISP, "ORDINARY")?;
    let quoted_initialize = quoted(&mut scope, &runtime, initialize)?;
    assert!(initialization_base(&mut scope, &runtime, initialize, quoted_initialize)?.is_some());
    assert!(initialization_base(&mut scope, &runtime, ordinary, quoted_initialize)?.is_none());

    let first = gensym(&mut scope, &runtime)?;
    let second = gensym(&mut scope, &runtime)?;
    assert_ne!(scope.get(first).as_word(), scope.get(second).as_word());

    let next_methods = scope.root(Local::from_word(Word::NIL));
    let argument = scope.intern(&runtime, COMMON_LISP, "ARG")?;
    let user_args = scope.root_many(&[Local::from_word(scope.get(argument).as_word())]);
    let lambda = method_lambda_list(&mut scope, &runtime, next_methods, &user_args)?;
    assert_eq!(form_elements(scope.context(), scope.get(lambda).as_word())?.len(), 2);

    let call_next = scope.intern(&runtime, COMMON_LISP, "CALL-NEXT-METHOD")?;
    let call_next_form = make_form(&mut scope, &runtime, &[call_next])?;
    let forms = scope.root_many(&[Local::from_word(scope.get(call_next_form).as_word())]);
    let rewritten = rewrite_method_body(&mut scope, &runtime, &forms, next_methods, &user_args)?;
    assert_eq!(form_elements(scope.context(), scope.get(rewritten).as_word())?.len(), 2);
    Ok(())
}
