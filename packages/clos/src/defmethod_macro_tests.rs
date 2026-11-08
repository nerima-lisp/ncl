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
fn method_body_rewriting_preserves_quoted_forms_and_local_functions() -> Result<(), ObjectError> {
    let (runtime, mut context) = setup()?;
    let mut scope = Scope::new(&mut context);
    let call_next = scope.intern(&runtime, COMMON_LISP, "CALL-NEXT-METHOD")?;
    let next_method_p = scope.intern(&runtime, COMMON_LISP, "NEXT-METHOD-P")?;
    let quote = scope.intern(&runtime, COMMON_LISP, "QUOTE")?;
    let lambda = scope.intern(&runtime, COMMON_LISP, "LAMBDA")?;
    let defun = scope.intern(&runtime, COMMON_LISP, "DEFUN")?;
    let value = scope.intern(&runtime, "NCL", "VALUE")?;
    let local_helper = scope.intern(&runtime, "NCL", "LOCAL-HELPER")?;
    let next_methods = scope.intern(&runtime, "NCL", "NEXT-METHODS")?;
    let current_arguments = scope.intern(&runtime, "NCL", "CURRENT-ARGUMENTS")?;
    let call_next_word = scope.get(call_next).as_word();
    let next_method_p_word = scope.get(next_method_p).as_word();
    let quote_word = scope.get(quote).as_word();
    let lambda_word = scope.get(lambda).as_word();
    let defun_word = scope.get(defun).as_word();
    let value_word = scope.get(value).as_word();
    let local_helper_word = scope.get(local_helper).as_word();

    let call_without_arguments = make_list(&mut scope, &runtime, &[call_next_word])?;
    let call_with_argument = make_list(&mut scope, &runtime, &[call_next_word, value_word])?;
    let next_method_p_form = make_list(&mut scope, &runtime, &[next_method_p_word])?;
    let next_method_p_form_word = scope.get(next_method_p_form).as_word();
    let call_without_arguments_word = scope.get(call_without_arguments).as_word();
    let quoted_form = make_list(
        &mut scope,
        &runtime,
        &[quote_word, call_without_arguments_word],
    )?;
    let quoted_form_word = scope.get(quoted_form).as_word();
    let lambda_form = make_list(
        &mut scope,
        &runtime,
        &[lambda_word, Word::NIL, next_method_p_form_word],
    )?;
    let lambda_form_word = scope.get(lambda_form).as_word();
    let defun_form = make_list(
        &mut scope,
        &runtime,
        &[
            defun_word,
            local_helper_word,
            Word::NIL,
            call_without_arguments_word,
        ],
    )?;
    let defun_form_word = scope.get(defun_form).as_word();
    let call_with_argument_word = scope.get(call_with_argument).as_word();
    let source_forms = make_list(
        &mut scope,
        &runtime,
        &[
            call_without_arguments_word,
            call_with_argument_word,
            next_method_p_form_word,
            quoted_form_word,
            lambda_form_word,
            defun_form_word,
            Word::fixnum(19),
        ],
    )?;
    let source_forms =
        scope.list_to_handle_vec(Local::from_word(scope.get(source_forms).as_word()))?;
    let rewritten = rewrite_method_body(
        &mut scope,
        &runtime,
        &source_forms,
        next_methods,
        current_arguments,
    )?;
    let rewritten_word = scope.get(rewritten).as_word();
    let forms = list_words(&mut scope, rewritten_word)?;
    assert_eq!(symbol_name_string(scope.context(), forms[0])?, "PROGN");

    let no_argument_call = list_words(&mut scope, forms[1])?;
    assert_eq!(
        symbol_name_string(scope.context(), no_argument_call[0])?,
        "%CLOS-CALL-NEXT-METHOD"
    );
    assert_eq!(no_argument_call[1], scope.get(next_methods).as_word());
    assert_eq!(no_argument_call[2], scope.get(current_arguments).as_word());

    let explicit_argument_call = list_words(&mut scope, forms[2])?;
    assert_eq!(
        symbol_name_string(scope.context(), explicit_argument_call[0])?,
        "%CLOS-CALL-NEXT-METHOD"
    );
    let supplied_arguments = list_words(&mut scope, explicit_argument_call[2])?;
    assert_eq!(
        symbol_name_string(scope.context(), supplied_arguments[0])?,
        "LIST"
    );
    assert_eq!(supplied_arguments[1], scope.get(value).as_word());

    let next_method_p_call = list_words(&mut scope, forms[3])?;
    assert_eq!(
        symbol_name_string(scope.context(), next_method_p_call[0])?,
        "%CLOS-NEXT-METHOD-P"
    );
    assert_eq!(next_method_p_call[1], scope.get(next_methods).as_word());
    assert_eq!(forms[4], scope.get(quoted_form).as_word());
    assert_eq!(forms[5], scope.get(lambda_form).as_word());
    assert_eq!(forms[6], scope.get(defun_form).as_word());
    assert_eq!(forms[7], Word::fixnum(19));
    Ok(())
}

#[test]
fn initialization_method_expansion_installs_the_base_generic() -> Result<(), ObjectError> {
    let (runtime, mut context) = setup()?;
    let mut scope = Scope::new(&mut context);
    let defmethod = scope.intern(&runtime, COMMON_LISP, "DEFMETHOD")?;
    let initialize_instance = scope.intern(&runtime, COMMON_LISP, "INITIALIZE-INSTANCE")?;
    let instance = scope.intern(&runtime, "NCL", "INSTANCE")?;
    let t = scope.intern(&runtime, COMMON_LISP, "T")?;
    let defmethod_word = scope.get(defmethod).as_word();
    let initialize_instance_word = scope.get(initialize_instance).as_word();
    let specializer_words = [scope.get(instance).as_word(), scope.get(t).as_word()];
    let specializer = make_list(&mut scope, &runtime, &specializer_words)?;
    let specializer_word = scope.get(specializer).as_word();
    let lambda_list = make_list(&mut scope, &runtime, &[specializer_word])?;
    let lambda_list_word = scope.get(lambda_list).as_word();
    let form = make_list(
        &mut scope,
        &runtime,
        &[
            defmethod_word,
            initialize_instance_word,
            lambda_list_word,
            Word::NIL,
        ],
    )?;
    let expansion = defmethod_macro_expansion(&mut scope, &runtime, form, false)?;
    let expansion_word = scope.get(expansion).as_word();
    let forms = list_words(&mut scope, expansion_word)?;
    let base_registration = list_words(&mut scope, forms[2])?;
    assert_eq!(
        symbol_name_string(scope.context(), base_registration[0])?,
        "%CLOS-ENSURE-INITIALIZATION-BASE"
    );
    Ok(())
}

#[test]
fn long_form_method_combination_expansion_stores_definition_and_builder() -> Result<(), ObjectError>
{
    let (runtime, mut context) = setup()?;
    let mut scope = Scope::new(&mut context);
    let define = scope.intern(&runtime, COMMON_LISP, "DEFINE-METHOD-COMBINATION")?;
    let name = scope.intern(&runtime, "NCL", "WEIGHTED-PROBE")?;
    let group = scope.intern(&runtime, "NCL", "POSITIVE-METHODS")?;
    let qualifier = scope.intern(&runtime, "KEYWORD", "POSITIVE")?;
    let define_word = scope.get(define).as_word();
    let name_word = scope.get(name).as_word();
    let group_word = scope.get(group).as_word();
    let qualifier_word = scope.get(qualifier).as_word();
    let group_pattern = make_list(&mut scope, &runtime, &[qualifier_word])?;
    let group_pattern_word = scope.get(group_pattern).as_word();
    let group_spec = make_list(&mut scope, &runtime, &[group_word, group_pattern_word])?;
    let group_spec_word = scope.get(group_spec).as_word();
    let group_specs = make_list(&mut scope, &runtime, &[group_spec_word])?;
    let group_specs_word = scope.get(group_specs).as_word();
    let form = make_list(
        &mut scope,
        &runtime,
        &[
            define_word,
            name_word,
            Word::NIL,
            group_specs_word,
            Word::fixnum(23),
        ],
    )?;
    let mut form_word = scope.get(form).as_word();
    let _form_root = ncl_object::push_heap_root(&runtime, &mut form_word);
    drop(scope);

    let input = [form_word];
    let args = BuiltinArgs::new(&input);
    let mut values = MultipleValues::new();
    let mut expansion_word =
        define_method_combination_macro_builtin(&mut context, &runtime, &args, &mut values)?;
    let _expansion_root = ncl_object::push_heap_root(&runtime, &mut expansion_word);
    let mut scope = Scope::new(&mut context);
    let expansion = list_words(&mut scope, expansion_word)?;
    assert_eq!(symbol_name_string(scope.context(), expansion[0])?, "PROGN");
    assert_eq!(expansion.len(), 5);

    let helper = list_words(&mut scope, expansion[1])?;
    assert_eq!(symbol_name_string(scope.context(), helper[0])?, "DEFUN");
    assert!(
        symbol_name_string(scope.context(), helper[1])?.starts_with("%CLOS-METHOD-COMBINATION-")
    );
    let helper_arguments = list_words(&mut scope, helper[2])?;
    assert_eq!(
        symbol_name_string(scope.context(), helper_arguments[0])?,
        "GROUPS"
    );
    assert_eq!(
        symbol_name_string(scope.context(), helper_arguments[1])?,
        "OPTIONS"
    );
    let helper_body = list_words(&mut scope, helper[3])?;
    assert_eq!(symbol_name_string(scope.context(), helper_body[0])?, "LET");
    let bindings = list_words(&mut scope, helper_body[1])?;
    let binding = list_words(&mut scope, bindings[0])?;
    let positive_methods = scope.intern(&runtime, "NCL", "POSITIVE-METHODS")?;
    assert_eq!(binding[0], scope.get(positive_methods).as_word());
    let nth_form = list_words(&mut scope, binding[1])?;
    assert_eq!(symbol_name_string(scope.context(), nth_form[0])?, "NTH");
    assert_eq!(nth_form[1], Word::fixnum(0));
    assert_eq!(symbol_name_string(scope.context(), nth_form[2])?, "GROUPS");

    let body = list_words(&mut scope, helper_body[2])?;
    assert_eq!(symbol_name_string(scope.context(), body[0])?, "APPLY");
    let function_form = list_words(&mut scope, body[1])?;
    assert_eq!(
        symbol_name_string(scope.context(), function_form[0])?,
        "FUNCTION"
    );
    let lambda_form = list_words(&mut scope, function_form[1])?;
    assert_eq!(
        symbol_name_string(scope.context(), lambda_form[0])?,
        "LAMBDA"
    );
    assert_eq!(lambda_form[1], Word::NIL);
    assert_eq!(lambda_form[2], Word::fixnum(23));
    assert_eq!(symbol_name_string(scope.context(), body[2])?, "OPTIONS");

    let stored_definition = list_words(&mut scope, expansion[2])?;
    assert_eq!(
        symbol_name_string(scope.context(), stored_definition[0])?,
        "SETF"
    );
    let stored_function = list_words(&mut scope, expansion[3])?;
    assert_eq!(
        symbol_name_string(scope.context(), stored_function[0])?,
        "SETF"
    );
    let result_name = list_words(&mut scope, expansion[4])?;
    assert_eq!(
        symbol_name_string(scope.context(), result_name[0])?,
        "QUOTE"
    );
    assert_eq!(
        symbol_name_string(scope.context(), result_name[1])?,
        "WEIGHTED-PROBE"
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
