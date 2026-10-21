//! Logical pathname translations are stored in each runtime.

#[path = "logical_namestring.rs"]
mod logical_namestring;

use ncl_object::hash_table::{HashTable, HashTest, Weakness};
use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, Instance, LambdaList, LispError, MultipleValues, ObjectError,
    ObjectRef, ObjectType, Package, Parameter, ParameterType, PlaceExpander, Runtime, Scope,
    SetfExpansion, ThreadContext, Word, car, cdr, classify_object, instance_class,
    make_bignum_from_limbs, make_cons, set_symbol_value, simple_vector_ref, string_length,
    string_ref, symbol_value, with_root, with_roots,
};

const REGISTRY_SYMBOL: &str = "*LOGICAL-PATHNAME-TRANSLATIONS*";
const PATHSPEC: Parameter = Parameter {
    name: BuiltinName::new("PATHSPEC"),
    ty: ParameterType::Any,
};
const HOST: Parameter = Parameter {
    name: BuiltinName::new("HOST"),
    ty: ParameterType::Any,
};
const HOST_AND_TRANSLATIONS: &[Parameter] = &[
    HOST,
    Parameter {
        name: BuiltinName::new("TRANSLATIONS"),
        ty: ParameterType::Any,
    },
];

pub fn register(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<(), ObjectError> {
    register_logical_pathname_class(ctx, runtime)?;
    initialize_registry(ctx, runtime)?;
    register_builtin(
        ctx,
        runtime,
        BuiltinPackage::CommonLisp,
        "LOGICAL-PATHNAME",
        1,
        std::slice::from_ref(&PATHSPEC),
        logical_pathname_builtin,
    )?;
    register_builtin(
        ctx,
        runtime,
        BuiltinPackage::CommonLisp,
        "LOGICAL-PATHNAME-TRANSLATIONS",
        1,
        std::slice::from_ref(&HOST),
        logical_pathname_translations_builtin,
    )?;
    register_builtin(
        ctx,
        runtime,
        BuiltinPackage::NclExt,
        "LOGICAL-PATHNAME-TRANSLATIONS-SET",
        2,
        HOST_AND_TRANSLATIONS,
        logical_pathname_translations_set_builtin,
    )?;
    register_place_expander(ctx, runtime)
}

pub fn is_logical_pathname(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    word: Word,
) -> Result<bool, ObjectError> {
    if !matches!(classify_object(ctx, word), ObjectRef::Instance(_)) {
        return Ok(false);
    }
    let class = runtime
        .class(ctx, "LOGICAL-PATHNAME")
        .ok_or(ObjectError::Layout)?;
    class_is_subclass(ctx, instance_class(ctx, Instance::from_word(word))?, class)
}

fn class_is_subclass(
    ctx: &ThreadContext,
    actual: Word,
    expected: Word,
) -> Result<bool, ObjectError> {
    if actual == expected {
        return Ok(true);
    }
    let superclasses = simple_vector_ref(ctx, actual, ncl_clos::mop::CLASS_DIRECT_SUPERCLASS)?;
    if superclasses == Word::NIL {
        return Ok(false);
    }
    if !superclasses.is_cons() {
        return class_is_subclass(ctx, superclasses, expected);
    }

    let mut cursor = superclasses;
    while cursor != Word::NIL {
        if class_is_subclass(ctx, car(ctx, cursor)?, expected)? {
            return Ok(true);
        }
        cursor = cdr(ctx, cursor)?;
    }
    Ok(false)
}

fn register_logical_pathname_class(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
) -> Result<(), ObjectError> {
    let mut scope = Scope::new(ctx);
    let name = scope.make_string(runtime, &"LOGICAL-PATHNAME".chars().collect::<Vec<_>>())?;
    let superclass = runtime
        .class(scope.context_mut(), "PATHNAME")
        .ok_or(ObjectError::Layout)?;
    let superclass = scope.root::<Word>(ncl_object::Local::from_word(superclass));
    let name = scope.get(name).as_word();
    let superclass = scope.get(superclass).as_word();
    let class = ncl_clos::make_class(
        scope.context_mut(),
        runtime,
        name,
        superclass,
        Word::NIL,
        Word::fixnum(0),
    )?;
    let class = scope.root::<Word>(ncl_object::Local::from_word(class));
    let class_word = scope.get(class).as_word();
    runtime.define_class(scope.context_mut(), "LOGICAL-PATHNAME", class_word)
}

fn logical_pathname_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let pathspec = args.required(0)?;
    if is_logical_pathname(ctx, runtime, pathspec)? {
        return Ok(pathspec);
    }
    if !matches!(classify_object(ctx, pathspec), ObjectRef::String(_)) {
        let expected = if matches!(classify_object(ctx, pathspec), ObjectRef::Stream(_)) {
            ObjectType::Instance
        } else {
            ObjectType::String
        };
        return Err(type_error(ctx, pathspec, expected));
    }

    logical_pathname_from_namestring(ctx, runtime, pathspec)
}

fn logical_pathname_from_namestring(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    namestring: Word,
) -> Result<Word, ObjectError> {
    let mut scope = Scope::new(ctx);
    let namestring = scope.root::<Word>(ncl_object::Local::from_word(namestring));
    let text = word_to_string(scope.context(), scope.get(namestring).as_word())?;
    let Some(parsed) = logical_namestring::parse(&text) else {
        let datum = scope.get(namestring).as_word();
        return Err(type_error(scope.context_mut(), datum, ObjectType::String));
    };
    if !logical_host_registered(scope.context_mut(), runtime, &parsed.host)? {
        let datum = scope.get(namestring).as_word();
        return Err(type_error(scope.context_mut(), datum, ObjectType::String));
    }

    let canonical_namestring =
        scope.make_string(runtime, &parsed.namestring.chars().collect::<Vec<_>>())?;
    let version = logical_version_word(scope.context_mut(), runtime, parsed.version)?;
    let version = scope.root::<Word>(ncl_object::Local::from_word(version));
    let class = runtime
        .class(scope.context_mut(), "LOGICAL-PATHNAME")
        .ok_or(ObjectError::Layout)?;
    let class = scope.root::<Word>(ncl_object::Local::from_word(class));
    let class_word = scope.get(class).as_word();
    let namestring_word = scope.get(canonical_namestring).as_word();
    let version_word = scope.get(version).as_word();
    let slots = [namestring_word, version_word];
    ncl_clos::make_instance(scope.context_mut(), runtime, class_word, &slots)
}

fn logical_version_word(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    version: logical_namestring::LogicalVersion,
) -> Result<Word, ObjectError> {
    match version {
        logical_namestring::LogicalVersion::Newest => keyword_version(ctx, runtime, "NEWEST"),
        logical_namestring::LogicalVersion::Wild => keyword_version(ctx, runtime, "WILD"),
        logical_namestring::LogicalVersion::Integer(limbs) => {
            let magnitude = if limbs.len() <= 2 {
                let low = u64::from(*limbs.first().ok_or(ObjectError::Layout)?);
                let high = u64::from(limbs.get(1).copied().unwrap_or(0));
                Some(low | (high << 32))
            } else {
                None
            };
            if let Some(value) = magnitude.filter(|value| *value < (1_u64 << 62)) {
                return Ok(Word::fixnum(
                    i64::try_from(value).map_err(|_| ObjectError::Layout)?,
                ));
            }
            Ok(make_bignum_from_limbs(ctx, runtime, false, &limbs)?.as_word())
        }
    }
}

fn keyword_version(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
) -> Result<Word, ObjectError> {
    let package = runtime.ensure_package(ctx, "KEYWORD")?;
    Ok(Package::from_word(package).intern(ctx, runtime, name)?.0)
}

fn logical_host_registered(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    host: &str,
) -> Result<bool, ObjectError> {
    let mut scope = Scope::new(ctx);
    let host = scope.make_string(runtime, &host.chars().collect::<Vec<_>>())?;
    let host_word = scope.get(host).as_word();
    let normalized = normalized_host(scope.context_mut(), host_word)?;
    let key = scope.make_string(runtime, &normalized.chars().collect::<Vec<_>>())?;
    let table = registry_table(scope.context_mut(), runtime)?.as_word();
    let table = scope.root::<Word>(ncl_object::Local::from_word(table));
    let table_word = scope.get(table).as_word();
    let key_word = scope.get(key).as_word();
    Ok(HashTable::from_word(table_word)
        .get(scope.context_mut(), key_word)?
        .is_some())
}

fn word_to_string(ctx: &ThreadContext, word: Word) -> Result<String, ObjectError> {
    (0..string_length(ctx, word)?)
        .map(|index| string_ref(ctx, word, index))
        .collect()
}

const fn type_error(ctx: &mut ThreadContext, datum: Word, expected: ObjectType) -> ObjectError {
    ctx.set_pending_lisp_error(LispError::TypeError { datum, expected });
    ObjectError::TypeError
}

fn initialize_registry(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<(), ObjectError> {
    let package = runtime.ensure_package(ctx, "NCL-EXT")?;
    let mut package = package;
    with_root(ctx, &mut package, |ctx, package| {
        let mut symbol = Package::from_word(package.get())
            .intern(ctx, runtime, REGISTRY_SYMBOL)?
            .0;
        with_root(ctx, &mut symbol, |ctx, symbol| {
            let mut table =
                HashTable::new(ctx, runtime, HashTest::Equal, Weakness::None)?.as_word();
            with_root(ctx, &mut table, |ctx, table| {
                set_symbol_value(ctx, symbol.get(), table.get())
            })
        })
    })
}

fn register_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    package: BuiltinPackage,
    name: &'static str,
    arity: u8,
    parameters: &'static [Parameter],
    callback: ncl_object::RustBuiltin,
) -> Result<(), ObjectError> {
    runtime
        .register_builtin(
            ctx,
            BuiltinIdentifier::new(package, BuiltinName::new(name)),
            BuiltinImplementation::direct(
                Builtin {
                    lambda_list: LambdaList::fixed(parameters),
                    convention: BuiltinConvention::Direct(Arity::exact(arity)),
                },
                callback,
            ),
        )
        .map(|_| ())
}

fn register_place_expander(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<(), ObjectError> {
    let package = runtime.ensure_package(ctx, "COMMON-LISP")?;
    let operator = Package::from_word(package)
        .intern(ctx, runtime, "LOGICAL-PATHNAME-TRANSLATIONS")?
        .0;
    runtime.register_place_expander(
        ctx,
        operator,
        logical_pathname_translations_place as PlaceExpander,
    )
}

fn logical_pathname_translations_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let host_word = args.required(0)?;
    let host = normalized_host(ctx, host_word)?;
    let mut scope = Scope::new(ctx);
    let key = scope.make_string(runtime, &host.chars().collect::<Vec<_>>())?;
    let table = registry_table(scope.context_mut(), runtime)?.as_word();
    let table = scope.root::<Word>(ncl_object::Local::from_word(table));
    let key = scope.get(key).as_word();
    let table = HashTable::from_word(scope.get(table).as_word());
    table.get(scope.context_mut(), key)?.map_or_else(
        || {
            scope
                .context_mut()
                .set_pending_lisp_error(LispError::TypeError {
                    datum: host_word,
                    expected: ObjectType::String,
                });
            Err(ObjectError::TypeError)
        },
        Ok,
    )
}

fn logical_pathname_translations_set_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let mut scope = Scope::new(ctx);
    let host = scope.root::<Word>(ncl_object::Local::from_word(args.required(0)?));
    let translations = scope.root::<Word>(ncl_object::Local::from_word(args.required(1)?));
    let host_word = scope.get(host).as_word();
    let host = normalized_host(scope.context_mut(), host_word)?;
    let key = scope.make_string(runtime, &host.chars().collect::<Vec<_>>())?;
    let table = registry_table(scope.context_mut(), runtime)?.as_word();
    let table = scope.root::<Word>(ncl_object::Local::from_word(table));
    let key = scope.get(key).as_word();
    let table_word = scope.get(table).as_word();
    let value = scope.get(translations).as_word();
    HashTable::from_word(table_word).insert(scope.context_mut(), runtime, key, value)?;
    Ok(value)
}

fn normalized_host(ctx: &mut ThreadContext, host: Word) -> Result<String, ObjectError> {
    if !matches!(classify_object(ctx, host), ObjectRef::String(_)) {
        ctx.set_pending_lisp_error(LispError::TypeError {
            datum: host,
            expected: ObjectType::String,
        });
        return Err(ObjectError::TypeError);
    }
    let length = string_length(ctx, host)?;
    let mut normalized = String::new();
    for index in 0..length {
        normalized.extend(string_ref(ctx, host, index)?.to_uppercase());
    }
    Ok(normalized)
}

fn registry_table(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<HashTable, ObjectError> {
    let package = runtime
        .find_package(ctx, "NCL-EXT")
        .ok_or(ObjectError::Layout)?;
    let symbol = Package::from_word(package)
        .intern(ctx, runtime, REGISTRY_SYMBOL)?
        .0;
    let table = symbol_value(ctx, symbol)?;
    match classify_object(ctx, table) {
        ObjectRef::HashTable(table) => Ok(HashTable::from_word(table)),
        _ => Err(ObjectError::Layout), // check-added-lines: allow(wildcard) only the translation registry is accepted
    }
}

fn logical_pathname_translations_place(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
) -> Result<SetfExpansion, ObjectError> {
    if args.len() != 1 {
        return Err(ObjectError::TypeError);
    }
    with_roots(ctx, args, |ctx, roots| {
        let mut scope = Scope::new(ctx);
        let host_word = roots.first().ok_or(ObjectError::Layout)?.get();
        let host = scope.root::<Word>(ncl_object::Local::from_word(host_word));
        let temporary = fresh_symbol(scope.context_mut(), runtime)?;
        let temporary = scope.root::<Word>(ncl_object::Local::from_word(temporary));
        let store = fresh_symbol(scope.context_mut(), runtime)?;
        let store = scope.root::<Word>(ncl_object::Local::from_word(store));

        let temporary_word = scope.get(temporary).as_word();
        let access = call_form(
            scope.context_mut(),
            runtime,
            "LOGICAL-PATHNAME-TRANSLATIONS",
            &[temporary_word],
        )?;
        let access = scope.root::<Word>(ncl_object::Local::from_word(access));
        let temporary_word = scope.get(temporary).as_word();
        let store_word = scope.get(store).as_word();
        let store_form = call_form(
            scope.context_mut(),
            runtime,
            "NCL-EXT::LOGICAL-PATHNAME-TRANSLATIONS-SET",
            &[temporary_word, store_word],
        )?;

        Ok(SetfExpansion {
            temporary_variables: vec![scope.get(temporary).as_word()],
            value_forms: vec![scope.get(host).as_word()],
            store_variables: vec![scope.get(store).as_word()],
            store_form,
            access_form: scope.get(access).as_word(),
        })
    })
}

fn fresh_symbol(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<Word, ObjectError> {
    let package = runtime.ensure_package(ctx, "NCL")?;
    Package::from_word(package).gensym(ctx, runtime)
}

fn call_form(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    operator: &str,
    args: &[Word],
) -> Result<Word, ObjectError> {
    with_roots(ctx, args, |ctx, roots| {
        let (package_name, symbol_name) = operator
            .split_once("::")
            .map_or(("COMMON-LISP", operator), |(package, name)| (package, name));
        let package = runtime.ensure_package(ctx, package_name)?;
        let mut symbol = Package::from_word(package)
            .intern(ctx, runtime, symbol_name)?
            .0;
        with_root(ctx, &mut symbol, |ctx, symbol| {
            let mut values = Vec::with_capacity(roots.len() + 1);
            values.push(symbol.get());
            values.extend(roots.iter().map(|root| root.get()));
            with_roots(ctx, &values, |ctx, roots| {
                let mut form = Word::NIL;
                for value in roots.iter().rev() {
                    form = with_root(ctx, &mut form, |ctx, form| {
                        make_cons(ctx, runtime, value.get(), form.get())
                    })?;
                }
                Ok(form)
            })
        })
    })
}
