use super::*;

#[cfg(test)]
#[path = "defgeneric_macro_tests.rs"]
mod defgeneric_macro_tests;

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

#[test]
fn defmethod_macro_preserves_non_symbol_and_multiple_qualifiers() -> Result<(), ObjectError> {
    let (runtime, mut context) = setup()?;
    let mut scope = Scope::new(&mut context);
    let defmethod = scope.intern(&runtime, COMMON_LISP, "DEFMETHOD")?;
    let name = scope.intern(&runtime, "NCL", "RAW-QUALIFIER-METHOD")?;
    let value = scope.intern(&runtime, "NCL", "VALUE")?;
    let integer = scope.intern(&runtime, COMMON_LISP, "INTEGER")?;
    let specializer_fields = [scope.get(value).as_word(), scope.get(integer).as_word()];
    let specializer = make_list(&mut scope, &runtime, &specializer_fields)?;
    let lambda_fields = [scope.get(specializer).as_word()];
    let lambda_list = make_list(&mut scope, &runtime, &lambda_fields)?;
    let form_fields = [
        scope.get(defmethod).as_word(),
        scope.get(name).as_word(),
        Word::fixnum(41),
        Word::fixnum(42),
        scope.get(lambda_list).as_word(),
        Word::fixnum(47),
    ];
    let form = make_list(&mut scope, &runtime, &form_fields)?;
    let expansion = defmethod_macro_expansion(&mut scope, &runtime, form, false)?;
    let expansion_word = scope.get(expansion).as_word();
    let forms = list_words(&mut scope, expansion_word)?;
    let registration = list_words(&mut scope, *forms.last().ok_or(ObjectError::Layout)?)?;
    assert_eq!(
        symbol_name_string(scope.context(), registration[0])?,
        "%CLOS-ADD-METHOD"
    );
    let quoted_definition = list_words(&mut scope, registration[2])?;
    let definition = list_words(&mut scope, quoted_definition[1])?;
    assert_eq!(definition[1], Word::fixnum(41));
    assert_eq!(
        list_words(&mut scope, definition[2])?,
        [Word::fixnum(41), Word::fixnum(42)]
    );
    Ok(())
}

#[test]
fn defgeneric_macroexpansion_records_most_specific_last() -> Result<(), ObjectError> {
    let (runtime, mut context) = setup()?;
    let mut scope = Scope::new(&mut context);
    let defgeneric = scope.intern(&runtime, COMMON_LISP, "DEFGENERIC")?;
    let name = scope.intern(&runtime, "NCL", "ORDERED-GENERIC")?;
    let value = scope.intern(&runtime, "NCL", "VALUE")?;
    let lambda_fields = [scope.get(value).as_word()];
    let lambda_list = make_list(&mut scope, &runtime, &lambda_fields)?;
    let method_combination = scope.intern(&runtime, "KEYWORD", "METHOD-COMBINATION")?;
    let and = scope.intern(&runtime, COMMON_LISP, "AND")?;
    let most_specific_last = scope.intern(&runtime, "KEYWORD", "MOST-SPECIFIC-LAST")?;
    let option_fields = [
        scope.get(method_combination).as_word(),
        scope.get(and).as_word(),
        scope.get(most_specific_last).as_word(),
    ];
    let option = make_list(&mut scope, &runtime, &option_fields)?;
    let form_fields = [
        scope.get(defgeneric).as_word(),
        scope.get(name).as_word(),
        scope.get(lambda_list).as_word(),
        scope.get(option).as_word(),
    ];
    let form = make_list(&mut scope, &runtime, &form_fields)?;
    let mut form_word = scope.get(form).as_word();
    let _form_root = ncl_object::push_heap_root(&runtime, &mut form_word);
    drop(scope);

    let input = [form_word];
    let args = BuiltinArgs::new(&input);
    let mut values = MultipleValues::new();
    let mut expansion_word = defgeneric_macro_builtin(&mut context, &runtime, &args, &mut values)?;
    let _expansion_root = ncl_object::push_heap_root(&runtime, &mut expansion_word);
    let mut scope = Scope::new(&mut context);
    let forms = list_words(&mut scope, expansion_word)?;
    assert_eq!(symbol_name_string(scope.context(), forms[0])?, "PROGN");
    let set_combination = list_words(&mut scope, forms[2])?;
    assert_eq!(
        symbol_name_string(scope.context(), set_combination[0])?,
        "SETF"
    );
    let quoted_descriptor = list_words(&mut scope, set_combination[2])?;
    let descriptor = list_words(&mut scope, quoted_descriptor[1])?;
    assert_eq!(symbol_name_string(scope.context(), descriptor[0])?, "AND");
    assert_eq!(
        symbol_name_string(scope.context(), descriptor[1])?,
        "MOST-SPECIFIC-LAST"
    );
    Ok(())
}

#[test]
fn defgeneric_macroexpansion_rejects_explicit_most_specific_first() -> Result<(), ObjectError> {
    let (runtime, mut context) = setup()?;
    let mut scope = Scope::new(&mut context);
    let defgeneric = scope.intern(&runtime, COMMON_LISP, "DEFGENERIC")?;
    let name = scope.intern(&runtime, "NCL", "ORDERED-GENERIC")?;
    let value = scope.intern(&runtime, "NCL", "VALUE")?;
    let lambda_fields = [scope.get(value).as_word()];
    let lambda_list = make_list(&mut scope, &runtime, &lambda_fields)?;
    let method_combination = scope.intern(&runtime, "KEYWORD", "METHOD-COMBINATION")?;
    let and = scope.intern(&runtime, COMMON_LISP, "AND")?;
    let most_specific_first = scope.intern(&runtime, "KEYWORD", "MOST-SPECIFIC-FIRST")?;
    let option_fields = [
        scope.get(method_combination).as_word(),
        scope.get(and).as_word(),
        scope.get(most_specific_first).as_word(),
    ];
    let option = make_list(&mut scope, &runtime, &option_fields)?;
    let form_fields = [
        scope.get(defgeneric).as_word(),
        scope.get(name).as_word(),
        scope.get(lambda_list).as_word(),
        scope.get(option).as_word(),
    ];
    let form = make_list(&mut scope, &runtime, &form_fields)?;
    let mut form_word = scope.get(form).as_word();
    let _form_root = ncl_object::push_heap_root(&runtime, &mut form_word);
    drop(scope);

    let input = [form_word];
    let args = BuiltinArgs::new(&input);
    let mut values = MultipleValues::new();
    let mut expansion_word = defgeneric_macro_builtin(&mut context, &runtime, &args, &mut values)?;
    let _expansion_root = ncl_object::push_heap_root(&runtime, &mut expansion_word);
    let mut scope = Scope::new(&mut context);
    let expansion = list_words(&mut scope, expansion_word)?;
    assert_eq!(expansion.len(), 2);
    assert_eq!(
        symbol_name_string(scope.context(), expansion[0])?,
        "%CLOS-DEFGENERIC-ERROR"
    );
    let quoted_name = list_words(&mut scope, expansion[1])?;
    assert_eq!(quoted_name.len(), 2);
    assert_eq!(
        symbol_name_string(scope.context(), quoted_name[0])?,
        "QUOTE"
    );
    assert_eq!(
        symbol_name_string(scope.context(), quoted_name[1])?,
        "ORDERED-GENERIC"
    );
    Ok(())
}
