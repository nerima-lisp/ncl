//! Typed Common Lisp pathname builtin boundary.

use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, FromLispArg, Instance, LambdaList, MultipleValues, ObjectError,
    ObjectRef, Parameter, ParameterType, Pathname as PathnameView, Runtime, ThreadContext, Word,
    classify_object, instance_class, make_instance, make_string, slot_ref, string_length,
    string_ref,
};

use crate::{Pathname, parse_namestring};

const OBJECT: Parameter = Parameter {
    name: BuiltinName::new("OBJECT"),
    ty: ParameterType::Any,
};
const STRING: Parameter = Parameter {
    name: BuiltinName::new("STRING"),
    ty: ParameterType::StringDesignator,
};

const fn descriptor(parameters: &'static [Parameter]) -> Builtin {
    Builtin {
        lambda_list: LambdaList::fixed(parameters),
        convention: BuiltinConvention::Direct(Arity::exact(match parameters.len() {
            0 => 0,
            1 => 1,
            2 => 2,
            3 => 3,
            4 => 4,
            _ => 0,
        })),
    }
}

fn string_from_word(ctx: &ThreadContext, word: Word) -> Result<String, ObjectError> {
    if !matches!(classify_object(ctx, word), ObjectRef::String(_)) {
        return Err(ObjectError::TypeError);
    }
    let length = string_length(ctx, word)?;
    let mut result = String::new();
    for index in 0..length {
        result.push(string_ref(ctx, word, index)?);
    }
    Ok(result)
}

fn make_heap_pathname(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    value: &Pathname,
) -> Result<Word, ObjectError> {
    let class = runtime.class(ctx, "PATHNAME").ok_or(ObjectError::Layout)?;
    let text: Vec<char> = value.namestring().chars().collect();
    let namestring = make_string(ctx, runtime, &text)?;
    Ok(make_instance(ctx, runtime, class, &[namestring])?.as_word())
}

fn pathname_from_word(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    word: Word,
) -> Result<Pathname, ObjectError> {
    if matches!(classify_object(ctx, word), ObjectRef::String(_)) {
        return parse_namestring(&string_from_word(ctx, word)?).map_err(|_| ObjectError::TypeError);
    }
    let class = runtime.class(ctx, "PATHNAME").ok_or(ObjectError::Layout)?;
    let instance = match classify_object(ctx, word) {
        ObjectRef::Instance(_) => Instance::from_word(word),
        _ => return Err(ObjectError::TypeError),
    };
    if instance_class(ctx, instance)? != class {
        return Err(ObjectError::TypeError);
    }
    let namestring = slot_ref(ctx, instance, 0)?;
    parse_namestring(&string_from_word(ctx, namestring)?).map_err(|_| ObjectError::TypeError)
}

fn pathname_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let value = pathname_from_word(ctx, runtime, args.required(0)?)?;
    make_heap_pathname(ctx, runtime, &value)
}

fn pathnamep_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let class = runtime.class(ctx, "PATHNAME").ok_or(ObjectError::Layout)?;
    let word = args.required(0)?;
    let result = match classify_object(ctx, word) {
        ObjectRef::Instance(_) => instance_class(ctx, Instance::from_word(word))? == class,
        _ => false,
    };
    Ok(if result { Word::TRUE } else { Word::NIL })
}

fn parse_namestring_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    pathname_builtin(ctx, runtime, args, values)
}

fn namestring_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let pathname = args.required(0)?;
    let instance =
        PathnameView::from_lisp_arg(ctx, pathname).map_err(|_| ObjectError::TypeError)?;
    let text = slot_ref(ctx, Instance::from_word(instance.as_word()), 0)?;
    let _ = runtime;
    Ok(text)
}

/// Register the executable pathname builtins currently supported by the heap representation.
///
/// # Errors
///
/// Returns an object error when the pathname class or a builtin cannot be installed.
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    let mut ctx = ThreadContext::new();
    ctx.register(runtime)?;
    runtime.define_class(&mut ctx, "PATHNAME", Word::fixnum(1))?;
    macro_rules! install {
        ($name:literal, $params:expr, $function:ident) => {
            runtime.register_builtin(
                &mut ctx,
                BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new($name)),
                BuiltinImplementation::direct(descriptor($params), $function),
            )?;
        };
    }
    install!("PATHNAME", &[OBJECT], pathname_builtin);
    install!("PATHNAMEP", &[OBJECT], pathnamep_builtin);
    install!("PARSE-NAMESTRING", &[STRING], parse_namestring_builtin);
    install!("NAMESTRING", &[OBJECT], namestring_builtin);
    Ok(())
}
