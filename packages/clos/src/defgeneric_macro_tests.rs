use super::*;

type SymbolSpec = (&'static str, &'static str);
type OptionSpec = (SymbolSpec, &'static [SymbolSpec]);

fn assert_defgeneric_error_expansion(
    runtime: &Runtime,
    context: &mut ThreadContext,
    form: Word,
    expected_name: &str,
) -> Result<(), ObjectError> {
    let mut form_word = form;
    let _form_root = ncl_object::push_heap_root(runtime, &mut form_word);
    let input = [form_word];
    let args = BuiltinArgs::new(&input);
    let mut values = MultipleValues::new();
    let mut expansion_word = defgeneric_macro_builtin(context, runtime, &args, &mut values)?;
    let _expansion_root = ncl_object::push_heap_root(runtime, &mut expansion_word);
    let mut scope = Scope::new(context);
    let expansion = list_words(&mut scope, expansion_word)?;
    assert_eq!(expansion.len(), 2, "{expected_name} error expansion shape");
    assert_eq!(
        symbol_name_string(scope.context(), expansion[0])?,
        "%CLOS-DEFGENERIC-ERROR",
        "{expected_name} must expand to the defgeneric error form"
    );
    let quoted_name = list_words(&mut scope, expansion[1])?;
    assert_eq!(quoted_name.len(), 2, "{expected_name} quoted name shape");
    assert_eq!(
        symbol_name_string(scope.context(), quoted_name[0])?,
        "QUOTE",
        "{expected_name} error must quote the generic name"
    );
    assert_eq!(
        symbol_name_string(scope.context(), quoted_name[1])?,
        expected_name
    );
    Ok(())
}

fn assert_defgeneric_error_for_options(
    runtime: &Runtime,
    context: &mut ThreadContext,
    generic_name: &str,
    options: &[OptionSpec],
) -> Result<(), ObjectError> {
    let mut scope = Scope::new(context);
    let defgeneric = scope.intern(runtime, COMMON_LISP, "DEFGENERIC")?;
    let name = scope.intern(runtime, "NCL", generic_name)?;
    let value = scope.intern(runtime, "NCL", "VALUE")?;
    let value_word = scope.get(value).as_word();
    let lambda_list = make_list(&mut scope, runtime, &[value_word])?;
    let mut option_words = Vec::with_capacity(options.len());
    for ((package, key), values) in options {
        let key = scope.intern(runtime, package, key)?;
        let mut fields = vec![scope.get(key).as_word()];
        for (package, name) in *values {
            let value = scope.intern(runtime, package, name)?;
            fields.push(scope.get(value).as_word());
        }
        let option = make_list(&mut scope, runtime, &fields)?;
        option_words.push(scope.get(option).as_word());
    }
    let mut form_fields = vec![
        scope.get(defgeneric).as_word(),
        scope.get(name).as_word(),
        scope.get(lambda_list).as_word(),
    ];
    form_fields.extend(option_words);
    let form = make_list(&mut scope, runtime, &form_fields)?;
    let mut form_word = scope.get(form).as_word();
    let _form_root = ncl_object::push_heap_root(runtime, &mut form_word);
    drop(scope);
    assert_defgeneric_error_expansion(runtime, context, form_word, generic_name)
}

#[test]
fn defgeneric_macroexpansion_rejects_invalid_method_combination_options() -> Result<(), ObjectError>
{
    let (runtime, mut context) = setup()?;
    let method_combination = ("KEYWORD", "METHOD-COMBINATION");
    let invalid_cases: &[(&str, &[OptionSpec])] = &[
        (
            "INVALID-UNKNOWN-COMBINATION",
            &[(
                method_combination,
                &[("COMMON-LISP", "NO-SUCH-COMBINATION")],
            )],
        ),
        (
            "INVALID-NONCANONICAL-COMBINATION",
            &[(method_combination, &[("NCL", "AND")])],
        ),
        (
            "INVALID-SHORT-COMBINATION-ORDER",
            &[(
                method_combination,
                &[("COMMON-LISP", "AND"), ("KEYWORD", "MOST-SPECIFIC-FIRST")],
            )],
        ),
        (
            "INVALID-STANDARD-COMBINATION-ORDER",
            &[(
                method_combination,
                &[
                    ("COMMON-LISP", "STANDARD"),
                    ("KEYWORD", "MOST-SPECIFIC-LAST"),
                ],
            )],
        ),
        (
            "INVALID-DUPLICATE-COMBINATION",
            &[
                (method_combination, &[("COMMON-LISP", "AND")]),
                (method_combination, &[("COMMON-LISP", "OR")]),
            ],
        ),
        (
            "INVALID-EXTRA-COMBINATION-ARGUMENT",
            &[(
                method_combination,
                &[
                    ("COMMON-LISP", "AND"),
                    ("KEYWORD", "MOST-SPECIFIC-LAST"),
                    ("KEYWORD", "UNEXPECTED"),
                ],
            )],
        ),
        (
            "INVALID-MISSING-COMBINATION-NAME",
            &[(method_combination, &[])],
        ),
        (
            "INVALID-UNKNOWN-DEFGENERIC-OPTION",
            &[(("KEYWORD", "UNSUPPORTED-OPTION"), &[("COMMON-LISP", "T")])],
        ),
    ];

    for (generic_name, options) in invalid_cases {
        assert_defgeneric_error_for_options(&runtime, &mut context, generic_name, options)?;
    }
    Ok(())
}

#[test]
fn defgeneric_macroexpansion_rejects_malformed_inline_method() -> Result<(), ObjectError> {
    let (runtime, mut context) = setup()?;
    let mut scope = Scope::new(&mut context);
    let defgeneric = scope.intern(&runtime, COMMON_LISP, "DEFGENERIC")?;
    let name = scope.intern(&runtime, "NCL", "INVALID-INLINE-METHOD")?;
    let value = scope.intern(&runtime, "NCL", "VALUE")?;
    let value_word = scope.get(value).as_word();
    let lambda_list = make_list(&mut scope, &runtime, &[value_word])?;
    let method_keyword = scope.intern(&runtime, "KEYWORD", "METHOD")?;
    let empty_method_fields = [scope.get(method_keyword).as_word()];
    let empty_method = make_list(&mut scope, &runtime, &empty_method_fields)?;
    let form_fields = [
        scope.get(defgeneric).as_word(),
        scope.get(name).as_word(),
        scope.get(lambda_list).as_word(),
        scope.get(empty_method).as_word(),
    ];
    let form = make_list(&mut scope, &runtime, &form_fields)?;
    let mut form_word = scope.get(form).as_word();
    let _form_root = ncl_object::push_heap_root(&runtime, &mut form_word);
    drop(scope);

    assert_defgeneric_error_expansion(&runtime, &mut context, form_word, "INVALID-INLINE-METHOD")
}

#[test]
fn defgeneric_macroexpansion_inlines_valid_method_definition() -> Result<(), ObjectError> {
    let (runtime, mut context) = setup()?;
    let mut scope = Scope::new(&mut context);
    let defgeneric = scope.intern(&runtime, COMMON_LISP, "DEFGENERIC")?;
    let name = scope.intern(&runtime, "NCL", "INLINE-GENERIC")?;
    let value = scope.intern(&runtime, "NCL", "VALUE")?;
    let integer = scope.intern(&runtime, COMMON_LISP, "INTEGER")?;
    let method_keyword = scope.intern(&runtime, "KEYWORD", "METHOD")?;
    let before = scope.intern(&runtime, "KEYWORD", "BEFORE")?;
    let value_word = scope.get(value).as_word();
    let generic_lambda_list = make_list(&mut scope, &runtime, &[value_word])?;
    let specializer_fields = [value_word, scope.get(integer).as_word()];
    let specializer = make_list(&mut scope, &runtime, &specializer_fields)?;
    let specializer_word = scope.get(specializer).as_word();
    let method_lambda_list = make_list(&mut scope, &runtime, &[specializer_word])?;
    let method_fields = [
        scope.get(method_keyword).as_word(),
        scope.get(before).as_word(),
        scope.get(method_lambda_list).as_word(),
        Word::fixnum(73),
    ];
    let method_option = make_list(&mut scope, &runtime, &method_fields)?;
    let form_fields = [
        scope.get(defgeneric).as_word(),
        scope.get(name).as_word(),
        scope.get(generic_lambda_list).as_word(),
        scope.get(method_option).as_word(),
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
    assert_valid_inline_method_expansion(&mut scope, &forms)
}

fn assert_valid_inline_method_expansion(
    scope: &mut Scope<'_>,
    forms: &[Word],
) -> Result<(), ObjectError> {
    assert_eq!(forms.len(), 6);
    assert_eq!(symbol_name_string(scope.context(), forms[0])?, "PROGN");

    let method_definition = list_words(scope, forms[3])?;
    assert_eq!(
        symbol_name_string(scope.context(), method_definition[0])?,
        "DEFUN"
    );
    let method_lambda = list_words(scope, method_definition[2])?;
    assert_eq!(method_lambda.len(), 3);
    assert_eq!(
        symbol_name_string(scope.context(), method_lambda[2])?,
        "VALUE"
    );
    let body = list_words(scope, method_definition[3])?;
    assert_eq!(symbol_name_string(scope.context(), body[0])?, "PROGN");
    assert_eq!(body[1], Word::fixnum(73));

    let registration = list_words(scope, forms[4])?;
    assert_eq!(
        symbol_name_string(scope.context(), registration[0])?,
        "%CLOS-ADD-METHOD"
    );
    let quoted_name = list_words(scope, registration[1])?;
    assert_eq!(
        symbol_name_string(scope.context(), quoted_name[0])?,
        "QUOTE"
    );
    assert_eq!(
        symbol_name_string(scope.context(), quoted_name[1])?,
        "INLINE-GENERIC"
    );
    let quoted_definition = list_words(scope, registration[2])?;
    assert_eq!(
        symbol_name_string(scope.context(), quoted_definition[0])?,
        "QUOTE"
    );
    let definition = list_words(scope, quoted_definition[1])?;
    assert_eq!(
        symbol_name_string(scope.context(), definition[1])?,
        "BEFORE"
    );
    let qualifiers = list_words(scope, definition[2])?;
    assert_eq!(qualifiers.len(), 1);
    assert_eq!(
        symbol_name_string(scope.context(), qualifiers[0])?,
        "BEFORE"
    );
    let specializers = list_words(scope, definition[3])?;
    let specializer = list_words(scope, specializers[0])?;
    assert_eq!(
        symbol_name_string(scope.context(), specializer[0])?,
        "VALUE"
    );
    assert_eq!(
        symbol_name_string(scope.context(), specializer[1])?,
        "INTEGER"
    );
    assert_eq!(definition[4], Word::fixnum(1));
    let final_name = list_words(scope, forms[5])?;
    assert_eq!(symbol_name_string(scope.context(), final_name[0])?, "QUOTE");
    assert_eq!(
        symbol_name_string(scope.context(), final_name[1])?,
        "INLINE-GENERIC"
    );
    Ok(())
}
