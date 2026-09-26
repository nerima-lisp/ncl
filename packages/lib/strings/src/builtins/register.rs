const CHARACTER: Parameter = Parameter {
    name: BuiltinName::new("CHARACTER"),
    ty: ParameterType::Any,
};
const OBJECT: Parameter = Parameter {
    name: BuiltinName::new("OBJECT"),
    ty: ParameterType::Any,
};
const CODE: Parameter = Parameter {
    name: BuiltinName::new("CODE"),
    ty: ParameterType::Fixnum,
};
const INDEX: Parameter = Parameter {
    name: BuiltinName::new("INDEX"),
    ty: ParameterType::Fixnum,
};
const DIGIT: Parameter = Parameter {
    name: BuiltinName::new("DIGIT"),
    ty: ParameterType::Fixnum,
};
const RADIX: Parameter = Parameter {
    name: BuiltinName::new("RADIX"),
    ty: ParameterType::Fixnum,
};
const STRING: Parameter = Parameter {
    name: BuiltinName::new("STRING"),
    ty: ParameterType::StringDesignator,
};
const STRINGS: &[Parameter] = &[STRING, STRING];
const REST_CHARACTER: Parameter = Parameter {
    name: BuiltinName::new("CHARACTERS"),
    ty: ParameterType::Any,
};
const STRING_ARGS: &[Parameter] = &[STRING];
const TRIM_ARGS: &[Parameter] = &[
    Parameter {
        name: BuiltinName::new("CHAR-BAG"),
        ty: ParameterType::StringDesignator,
    },
    STRING,
];
const SIZE: Parameter = Parameter {
    name: BuiltinName::new("SIZE"),
    ty: ParameterType::Fixnum,
};
const INITIAL_ELEMENT: Parameter = Parameter {
    name: BuiltinName::new("INITIAL-ELEMENT"),
    ty: ParameterType::Any,
};

#[allow(clippy::missing_const_for_fn, clippy::cast_possible_truncation)]
fn descriptor(lambda_list: LambdaList) -> Builtin {
    Builtin {
        convention: if lambda_list.is_direct() {
            BuiltinConvention::Direct(Arity::exact(lambda_list.required.len() as u8))
        } else {
            BuiltinConvention::Adapted
        },
        lambda_list,
    }
}

fn identity_adapter(args: &BuiltinArgs<'_>) -> Result<Vec<Word>, ObjectError> {
    let mut values = Vec::with_capacity(args.len());
    for index in 0..args.len() {
        let Some(value) = args.get(index) else {
            return Err(ObjectError::TypeError);
        };
        values.push(value);
    }
    Ok(values)
}

/// Register the Common Lisp character and string builtins owned by this crate.
///
/// # Errors
/// Returns an object error if a package, class, or builtin cannot be registered.
#[allow(clippy::too_many_lines)]
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    let mut ctx = ThreadContext::new();
    ctx.register(runtime)?;

    // These three names are owned by the character/string subsystem.  The
    // class registry may also be populated by CLOS later; defining the
    // placeholder here keeps this crate's ownership table complete when it is
    // registered in isolation.
    for name in ["CHARACTER", "STRING"] {
        runtime.define_class(&mut ctx, name, Word::fixnum(1))?;
    }
    let common_lisp = runtime
        .find_package(&ctx, "COMMON-LISP")
        .ok_or(ObjectError::PackageConflict)?;
    let (char_code_limit, _) =
        Package::from_word(common_lisp).intern(&mut ctx, runtime, "CHAR-CODE-LIMIT")?;
    set_symbol_value(&mut ctx, char_code_limit, Word::fixnum(0x11_0000))?;
    set_symbol_constant(&mut ctx, char_code_limit, true)?;
    macro_rules! register {
        ($name:literal, $params:expr, $function:ident) => {
            let descriptor = descriptor($params);
            let implementation = if descriptor.lambda_list.is_direct() {
                BuiltinImplementation::direct(descriptor, $function)
            } else {
                BuiltinImplementation::adapted(descriptor, $function, identity_adapter)
            };
            runtime.register_builtin(
                &mut ctx,
                BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new($name)),
                implementation,
            )?;
        };
    }
    register!(
        "CHARACTERP",
        LambdaList::fixed(&[OBJECT]),
        characterp_builtin
    );
    register!("CHARACTER", LambdaList::fixed(&[OBJECT]), character_builtin);
    register!("CHAR", LambdaList::fixed(&[STRING, INDEX]), char_builtin);
    register!("SCHAR", LambdaList::fixed(&[STRING, INDEX]), schar_builtin);
    register!(
        "CHAR-CODE",
        LambdaList::fixed(&[CHARACTER]),
        char_code_typed_entry
    );
    register!(
        "CHAR-NAME",
        LambdaList::fixed(&[CHARACTER]),
        char_name_builtin
    );
    register!("CODE-CHAR", LambdaList::fixed(&[CODE]), code_char_builtin);
    register!("NAME-CHAR", LambdaList::fixed(&[STRING]), name_char_builtin);
    register!(
        "DIGIT-CHAR",
        LambdaList::with_optional(&[DIGIT], &[RADIX]),
        digit_char_builtin
    );
    register!(
        "ALPHA-CHAR-P",
        LambdaList::fixed(&[CHARACTER]),
        alpha_char_p_builtin
    );
    register!(
        "ALPHANUMERICP",
        LambdaList::fixed(&[CHARACTER]),
        alphanumericp_builtin
    );
    register!(
        "UPPER-CASE-P",
        LambdaList::fixed(&[CHARACTER]),
        upper_case_p_builtin
    );
    register!(
        "LOWER-CASE-P",
        LambdaList::fixed(&[CHARACTER]),
        lower_case_p_builtin
    );
    register!(
        "BOTH-CASE-P",
        LambdaList::fixed(&[CHARACTER]),
        both_case_p_builtin
    );
    register!(
        "DIGIT-CHAR-P",
        LambdaList::fixed(&[CHARACTER]),
        digit_char_p_builtin
    );
    register!(
        "GRAPHIC-CHAR-P",
        LambdaList::fixed(&[CHARACTER]),
        graphic_char_p_builtin
    );
    register!(
        "STANDARD-CHAR-P",
        LambdaList::fixed(&[CHARACTER]),
        standard_char_p_builtin
    );
    register!(
        "CHAR-UPCASE",
        LambdaList::fixed(&[CHARACTER]),
        char_upcase_builtin
    );
    register!(
        "CHAR-DOWNCASE",
        LambdaList::fixed(&[CHARACTER]),
        char_downcase_builtin
    );
    register!(
        "CHAR-INT",
        LambdaList::fixed(&[CHARACTER]),
        char_int_builtin
    );
    register!(
        "CHAR=",
        LambdaList::with_rest(&[CHARACTER], REST_CHARACTER),
        char_equal_builtin
    );
    register!(
        "CHAR/=",
        LambdaList::with_rest(&[CHARACTER], REST_CHARACTER),
        char_not_equal_builtin
    );
    register!(
        "CHAR<",
        LambdaList::with_rest(&[CHARACTER], REST_CHARACTER),
        char_less_builtin
    );
    register!(
        "CHAR>",
        LambdaList::with_rest(&[CHARACTER], REST_CHARACTER),
        char_greater_builtin
    );
    register!(
        "CHAR<=",
        LambdaList::with_rest(&[CHARACTER], REST_CHARACTER),
        char_not_greater_builtin
    );
    register!(
        "CHAR>=",
        LambdaList::with_rest(&[CHARACTER], REST_CHARACTER),
        char_not_less_builtin
    );
    register!(
        "CHAR-EQUAL",
        LambdaList::with_rest(&[CHARACTER], REST_CHARACTER),
        char_equal_ci_builtin
    );
    register!(
        "CHAR-NOT-EQUAL",
        LambdaList::with_rest(&[CHARACTER], REST_CHARACTER),
        char_not_equal_ci_builtin
    );
    register!(
        "CHAR-LESSP",
        LambdaList::with_rest(&[CHARACTER], REST_CHARACTER),
        char_less_ci_builtin
    );
    register!(
        "CHAR-GREATERP",
        LambdaList::with_rest(&[CHARACTER], REST_CHARACTER),
        char_greater_ci_builtin
    );
    register!(
        "CHAR-NOT-GREATERP",
        LambdaList::with_rest(&[CHARACTER], REST_CHARACTER),
        char_not_greater_ci_builtin
    );
    register!(
        "CHAR-NOT-LESSP",
        LambdaList::with_rest(&[CHARACTER], REST_CHARACTER),
        char_not_less_ci_builtin
    );
    register!("STRINGP", LambdaList::fixed(&[OBJECT]), stringp_builtin);
    register!("STRING", LambdaList::fixed(&[OBJECT]), string_builtin);
    register!("STRING=", LambdaList::fixed(STRINGS), string_equal_builtin);
    register!(
        "STRING/=",
        LambdaList::fixed(STRINGS),
        string_not_equal_builtin
    );
    register!("STRING<", LambdaList::fixed(STRINGS), string_less_builtin);
    register!(
        "STRING>",
        LambdaList::fixed(STRINGS),
        string_greater_builtin
    );
    register!(
        "STRING<=",
        LambdaList::fixed(STRINGS),
        string_not_greater_builtin
    );
    register!(
        "STRING>=",
        LambdaList::fixed(STRINGS),
        string_not_less_builtin
    );
    register!(
        "STRING-EQUAL",
        LambdaList::fixed(STRINGS),
        string_equal_ci_builtin
    );
    register!(
        "STRING-NOT-EQUAL",
        LambdaList::fixed(STRINGS),
        string_not_equal_ci_builtin
    );
    register!(
        "STRING-LESSP",
        LambdaList::fixed(STRINGS),
        string_less_ci_builtin
    );
    register!(
        "STRING-GREATERP",
        LambdaList::fixed(STRINGS),
        string_greater_ci_builtin
    );
    register!(
        "STRING-NOT-GREATERP",
        LambdaList::fixed(STRINGS),
        string_not_greater_ci_builtin
    );
    register!(
        "STRING-NOT-LESSP",
        LambdaList::fixed(STRINGS),
        string_not_less_ci_builtin
    );
    register!(
        "STRING-UPCASE",
        LambdaList::fixed(STRING_ARGS),
        string_upcase_builtin
    );
    register!(
        "STRING-DOWNCASE",
        LambdaList::fixed(STRING_ARGS),
        string_downcase_builtin
    );
    register!(
        "STRING-CAPITALIZE",
        LambdaList::fixed(STRING_ARGS),
        string_capitalize_builtin
    );
    register!(
        "NSTRING-UPCASE",
        LambdaList::fixed(&[STRING]),
        nstring_upcase_builtin
    );
    register!(
        "NSTRING-DOWNCASE",
        LambdaList::fixed(&[STRING]),
        nstring_downcase_builtin
    );
    register!(
        "NSTRING-CAPITALIZE",
        LambdaList::fixed(&[STRING]),
        nstring_capitalize_builtin
    );
    register!(
        "SIMPLE-STRING-P",
        LambdaList::fixed(&[OBJECT]),
        simple_string_p_builtin
    );
    register!(
        "MAKE-STRING",
        LambdaList::with_optional(&[SIZE], &[INITIAL_ELEMENT]),
        make_string_builtin
    );
    register!(
        "STRING-TRIM",
        LambdaList::fixed(TRIM_ARGS),
        string_trim_builtin
    );
    register!(
        "STRING-LEFT-TRIM",
        LambdaList::fixed(TRIM_ARGS),
        string_left_trim_builtin
    );
    register!(
        "STRING-RIGHT-TRIM",
        LambdaList::fixed(TRIM_ARGS),
        string_right_trim_builtin
    );
    macro_rules! register_unicode {
        ($name:literal, $function:ident) => {
            let descriptor = descriptor(LambdaList::fixed(&[STRING]));
            runtime.register_builtin(
                &mut ctx,
                BuiltinIdentifier::new(BuiltinPackage::NclUnicode, BuiltinName::new($name)),
                BuiltinImplementation::direct(descriptor, $function),
            )?;
        };
    }
    register_unicode!("NORMALIZE-NFC", normalize_nfc_builtin);
    register_unicode!("NORMALIZE-NFD", normalize_nfd_builtin);
    register_unicode!("NORMALIZE-NFKC", normalize_nfkc_builtin);
    register_unicode!("NORMALIZE-NFKD", normalize_nfkd_builtin);
    register_unicode!("FULL-UPCASE", full_upcase_builtin);
    register_unicode!("FULL-DOWNCASE", full_downcase_builtin);
    register_unicode!("FULL-TITLECASE", full_titlecase_builtin);
    register_unicode!("STRING-TO-UTF8", string_to_utf8_builtin);
    register_unicode!("UTF8-TO-STRING", utf8_to_string_builtin);
    register_unicode!("GRAPHEME-BOUNDARIES", grapheme_boundaries_builtin);
    runtime.register_builtin(
        &mut ctx,
        BuiltinIdentifier::new(
            BuiltinPackage::NclUnicode,
            BuiltinName::new("GENERAL-CATEGORY"),
        ),
        BuiltinImplementation::direct(
            descriptor(LambdaList::fixed(&[CHARACTER])),
            general_category_builtin,
        ),
    )?;
    Ok(())
}
