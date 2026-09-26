use super::{
    Arity, Builtin, BuiltinIdentifier, BuiltinImplementation, BuiltinName, BuiltinPackage,
    LambdaList, ObjectError, Package, Parameter, ParameterType, Runtime, ThreadContext, Word,
    boundp, copy_symbol, fboundp, fmakunbound, gensym, gentemp, get, make_symbol, makunbound,
    predicates, remprop, set, set_symbol_special, set_symbol_value, symbol_function_builtin,
    symbol_name_builtin, symbol_package_builtin, symbol_plist_builtin, symbol_value,
    symbol_value_builtin,
};

const SYMBOL: Parameter = Parameter {
    name: BuiltinName::new("SYMBOL"),
    ty: ParameterType::Any,
};
const VALUE: Parameter = Parameter {
    name: BuiltinName::new("VALUE"),
    ty: ParameterType::Any,
};
const OBJECT: Parameter = Parameter {
    name: BuiltinName::new("OBJECT"),
    ty: ParameterType::Any,
};
const INDICATOR: Parameter = Parameter {
    name: BuiltinName::new("INDICATOR"),
    ty: ParameterType::Any,
};
const DEFAULT: Parameter = Parameter {
    name: BuiltinName::new("DEFAULT"),
    ty: ParameterType::Any,
};
const PREFIX: Parameter = Parameter {
    name: BuiltinName::new("PREFIX"),
    ty: ParameterType::Any,
};
const COPY_PROPERTIES: Parameter = Parameter {
    name: BuiltinName::new("COPY-PROPERTIES"),
    ty: ParameterType::Any,
};
const PACKAGE_PARAM: Parameter = Parameter {
    name: BuiltinName::new("PACKAGE"),
    ty: ParameterType::Any,
};
const COPY_REQUIRED: &[Parameter] = &[SYMBOL];
const COPY_OPTIONAL: &[Parameter] = &[COPY_PROPERTIES];
const SYMBOL_INDICATOR: &[Parameter] = &[SYMBOL, INDICATOR];
const SYMBOL_INDICATOR_DEFAULT: &[Parameter] = &[DEFAULT];
const SYMBOL_OBJECT: &[Parameter] = &[OBJECT];
const PREFIX_OPTIONAL: &[Parameter] = &[PREFIX];
const GENTEMP_OPTIONAL: &[Parameter] = &[PREFIX, PACKAGE_PARAM];

const fn descriptor(required: &'static [Parameter], optional: &'static [Parameter]) -> Builtin {
    Builtin {
        lambda_list: LambdaList::with_optional(required, optional),
        convention: ncl_object::BuiltinConvention::Adapted,
    }
}

pub(super) fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    let mut ctx = ThreadContext::new();
    ctx.register(runtime)?;
    let common = Package::from_word(
        runtime
            .find_package(&ctx, "COMMON-LISP")
            .ok_or(ObjectError::Layout)?,
    );
    let (gensym_counter, _) = common.intern(&mut ctx, runtime, "*GENSYM-COUNTER*")?;
    set_symbol_special(&mut ctx, gensym_counter, true)?;
    if symbol_value(&ctx, gensym_counter)? == Word::UNBOUND {
        set_symbol_value(&mut ctx, gensym_counter, Word::fixnum(0))?;
    }
    let (current_package, _) = common.intern(&mut ctx, runtime, "*PACKAGE*")?;
    set_symbol_special(&mut ctx, current_package, true)?;
    if symbol_value(&ctx, current_package)? == Word::UNBOUND {
        let user = runtime
            .find_package(&ctx, "COMMON-LISP-USER")
            .ok_or(ObjectError::Layout)?;
        set_symbol_value(&mut ctx, current_package, user)?;
    }
    let one = LambdaList::fixed(&[SYMBOL]);
    let two = LambdaList::fixed(&[SYMBOL, VALUE]);
    let entries: &[(&str, Builtin, ncl_object::RustBuiltin)] = &[
        ("BOUNDP", direct(one, 1), boundp),
        ("FBOUNDP", direct(one, 1), fboundp),
        (
            "COPY-SYMBOL",
            descriptor(COPY_REQUIRED, COPY_OPTIONAL),
            copy_symbol,
        ),
        ("FMAKUNBOUND", direct(one, 1), fmakunbound),
        ("MAKUNBOUND", direct(one, 1), makunbound),
        ("SET", direct(two, 2), set),
        ("SYMBOL-FUNCTION", direct(one, 1), symbol_function_builtin),
        ("SYMBOL-NAME", direct(one, 1), symbol_name_builtin),
        ("SYMBOL-PACKAGE", direct(one, 1), symbol_package_builtin),
        ("SYMBOL-PLIST", direct(one, 1), symbol_plist_builtin),
        ("SYMBOL-VALUE", direct(one, 1), symbol_value_builtin),
        (
            "SYMBOLP",
            direct(LambdaList::fixed(SYMBOL_OBJECT), 1),
            predicates::symbolp,
        ),
        (
            "GET",
            descriptor(SYMBOL_INDICATOR, SYMBOL_INDICATOR_DEFAULT),
            get,
        ),
        (
            "REMPROP",
            direct(LambdaList::fixed(SYMBOL_INDICATOR), 2),
            remprop,
        ),
        ("GENSYM", descriptor(&[], PREFIX_OPTIONAL), gensym),
        ("GENTEMP", descriptor(&[], GENTEMP_OPTIONAL), gentemp),
    ];
    for (name, descriptor, function) in entries {
        runtime.register_builtin(
            &mut ctx,
            BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new(name)),
            BuiltinImplementation::direct(*descriptor, *function),
        )?;
    }
    runtime.register_builtin(
        &mut ctx,
        BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new("MAKE-SYMBOL")),
        BuiltinImplementation::direct(
            direct(LambdaList::fixed(&[make_symbol::PARAMETER]), 1),
            make_symbol::builtin,
        ),
    )?;
    Ok(())
}

const fn direct(lambda_list: LambdaList, arity: u8) -> Builtin {
    Builtin {
        lambda_list,
        convention: ncl_object::BuiltinConvention::Direct(Arity::exact(arity)),
    }
}
