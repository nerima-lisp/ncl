use super::*;

fn make_list<'ctx>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    values: &[Word],
) -> Result<ncl_object::Handle<'ctx, Word>, ObjectError> {
    let roots = scope.root_many(
        &values
            .iter()
            .copied()
            .map(Local::from_word)
            .collect::<Vec<_>>(),
    );
    scope.make_list(runtime, &roots)
}

fn list_words(scope: &mut Scope<'_>, value: Word) -> Result<Vec<Word>, ObjectError> {
    let values = scope.list_to_handle_vec(Local::from_word(value))?;
    Ok(values
        .iter()
        .map(|handle| scope.get(*handle).as_word())
        .collect())
}

fn method_form<'ctx>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
) -> Result<ncl_object::Handle<'ctx, Word>, ObjectError> {
    let defmethod = scope.intern(runtime, COMMON_LISP, "DEFMETHOD")?;
    let name = scope.intern(runtime, "NCL", "R6-B2-IMPLICIT-METHOD")?;
    let value = scope.intern(runtime, "NCL", "VALUE")?;
    let integer = scope.intern(runtime, COMMON_LISP, "INTEGER")?;
    let optional = scope.intern(runtime, COMMON_LISP, "&OPTIONAL")?;
    let fallback = scope.intern(runtime, "NCL", "FALLBACK")?;
    let value_word = scope.get(value).as_word();
    let integer_word = scope.get(integer).as_word();
    let specializer = make_list(scope, runtime, &[value_word, integer_word])?;
    let specializer_word = scope.get(specializer).as_word();
    let optional_word = scope.get(optional).as_word();
    let fallback_word = scope.get(fallback).as_word();
    let lambda_list = make_list(
        scope,
        runtime,
        &[specializer_word, optional_word, fallback_word],
    )?;
    let defmethod_word = scope.get(defmethod).as_word();
    let name_word = scope.get(name).as_word();
    let lambda_list_word = scope.get(lambda_list).as_word();
    make_list(
        scope,
        runtime,
        &[
            defmethod_word,
            name_word,
            lambda_list_word,
            Word::fixnum(47),
        ],
    )
}

fn setup() -> Result<(Runtime, ThreadContext), ObjectError> {
    let runtime = Runtime::new()?;
    let mut context = ThreadContext::new();
    context.register(&runtime)?;
    super::register(&runtime)?;
    Ok((runtime, context))
}

#[test]
fn implicit_generic_expansion_keeps_optional_arguments_in_its_lambda_list()
-> Result<(), ObjectError> {
    let (runtime, mut context) = setup()?;
    let mut scope = Scope::new(&mut context);
    let form = method_form(&mut scope, &runtime)?;
    let expansion = defmethod_macro_expansion(&mut scope, &runtime, form, false)?;
    let expansion_word = scope.get(expansion).as_word();
    let forms = list_words(&mut scope, expansion_word)?;
    assert_eq!(symbol_name_string(scope.context(), forms[0])?, "PROGN");

    let generic_init = list_words(&mut scope, forms[1])?;
    assert_eq!(symbol_name_string(scope.context(), generic_init[0])?, "IF");
    let registration_predicate = list_words(&mut scope, generic_init[1])?;
    assert_eq!(
        symbol_name_string(scope.context(), registration_predicate[0])?,
        "%CLOS-GENERIC-FUNCTION-REGISTERED-P"
    );
    assert_eq!(generic_init[2], Word::NIL);
    let ensure_generic = list_words(&mut scope, generic_init[3])?;
    let new_generic = list_words(&mut scope, ensure_generic[3])?;
    let define_generic = list_words(&mut scope, new_generic[2])?;
    let quoted_lambda_list = list_words(&mut scope, define_generic[2])?;
    let lambda_list = list_words(&mut scope, quoted_lambda_list[1])?;
    let lambda_names = lambda_list
        .iter()
        .map(|word| symbol_name_string(scope.context(), *word))
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(lambda_names, ["VALUE", "&OPTIONAL", "FALLBACK"]);
    Ok(())
}

#[test]
fn inline_defgeneric_method_expansion_omits_standalone_initialization() -> Result<(), ObjectError> {
    let (runtime, mut context) = setup()?;
    let mut scope = Scope::new(&mut context);
    let form = method_form(&mut scope, &runtime)?;
    let expansion = defmethod_macro_expansion(&mut scope, &runtime, form, true)?;
    let expansion_word = scope.get(expansion).as_word();
    let forms = list_words(&mut scope, expansion_word)?;
    assert_eq!(symbol_name_string(scope.context(), forms[0])?, "PROGN");
    let inline_method = list_words(&mut scope, forms[1])?;
    assert_eq!(
        symbol_name_string(scope.context(), inline_method[0])?,
        "DEFUN"
    );
    assert_eq!(forms.len(), 3);
    Ok(())
}
