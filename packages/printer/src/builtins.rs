//! Registration of the printer's owned symbols and its dispatch table.

use ncl_object::{
    Builtin, BuiltinArgs, BuiltinConvention, BuiltinFunctionCaller, BuiltinIdentifier,
    BuiltinImplementation, BuiltinName, BuiltinPackage, FunctionArguments, FunctionCaller,
    FunctionDesignator, FunctionObject, LambdaList, MultipleValues, ObjectError, ObjectRef,
    Package, Parameter, ParameterType, Runtime, ThreadContext, Word, car, cdr, classify_object,
    make_cons, pop_root, push_root, set_symbol_special, set_symbol_value, simple_vector_ref,
    structure_layout, symbol_plist, symbol_value, with_root, with_roots,
};

use crate::{PrintError, PrintOptions, write_to_string};

/// The `(package, name)` functions `ncl-printer` owns.
const FUNCTIONS: [(&str, &str); 20] = [
    ("COMMON-LISP", "COPY-PPRINT-DISPATCH"),
    ("COMMON-LISP", "PPRINT"),
    ("COMMON-LISP", "PPRINT-DISPATCH"),
    ("COMMON-LISP", "PPRINT-FILL"),
    ("COMMON-LISP", "PPRINT-INDENT"),
    ("COMMON-LISP", "PPRINT-LINEAR"),
    ("COMMON-LISP", "PPRINT-NEWLINE"),
    ("COMMON-LISP", "PPRINT-TAB"),
    ("COMMON-LISP", "PPRINT-TABULAR"),
    ("COMMON-LISP", "PRIN1"),
    ("COMMON-LISP", "PRIN1-TO-STRING"),
    ("COMMON-LISP", "PRINC"),
    ("COMMON-LISP", "PRINC-TO-STRING"),
    ("COMMON-LISP", "PRINT"),
    ("COMMON-LISP", "PRINT-NOT-READABLE-OBJECT"),
    ("COMMON-LISP", "PRINT-OBJECT"),
    ("COMMON-LISP", "SET-PPRINT-DISPATCH"),
    ("COMMON-LISP", "WRITE-TO-STRING"),
    ("NCL-EXT", "PRINT-SYMBOL-WITH-PREFIX"),
    ("NCL-EXT", "PRINT-UNREADABLY"),
];

const OBJECT_PARAMETER: Parameter = Parameter {
    name: BuiltinName::new("OBJECT"),
    ty: ParameterType::Any,
};
const STREAM_PARAMETER: Parameter = Parameter {
    name: BuiltinName::new("OUTPUT-STREAM"),
    ty: ParameterType::Any,
};

/// The `(package, name)` special variables `ncl-printer` owns.
const VARIABLES: [(&str, &str); 9] = [
    ("COMMON-LISP", "*PRINT-PPRINT-DISPATCH*"),
    ("COMMON-LISP", "*PRINT-PRETTY*"),
    ("COMMON-LISP", "*PRINT-RIGHT-MARGIN*"),
    ("COMMON-LISP", "*PRINT-MISER-WIDTH*"),
    ("COMMON-LISP", "*PRINT-LINES*"),
    ("COMMON-LISP", "*PRINT-CIRCLE*"),
    ("COMMON-LISP", "*PRINT-READABLY*"),
    ("NCL-EXT", "*PRINT-CIRCLE-NOT-SHARED*"),
    ("NCL-EXT", "*PRINT-VECTOR-LENGTH*"),
];

/// Register every symbol `ncl-printer` owns with `runtime`.
///
/// Each function symbol is interned in its package and registered with an
/// unbound placeholder until the runtime supplies a callable function object.
/// The owned variables are interned, marked special, and initialised:
/// `*PRINT-PPRINT-DISPATCH*` to an empty dispatch table, the rest to `NIL`.
///
/// # Errors
///
/// Returns an [`ObjectError`] when a package cannot be created, an allocation
/// fails, or a symbol flag cannot be written.
pub fn register(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<(), ObjectError> {
    for package in ["COMMON-LISP", "NCL-EXT"] {
        runtime.ensure_package(ctx, package)?;
    }
    for (package, name) in FUNCTIONS {
        let package_word = runtime.ensure_package(ctx, package)?;
        Package::from_word(package_word).intern(ctx, runtime, name)?;
        if package == "COMMON-LISP" && matches!(name, "PRINC" | "PRIN1" | "PRINT") {
            register_print_builtin(ctx, runtime, name)?;
        } else if package == "COMMON-LISP"
            && matches!(
                name,
                "COPY-PPRINT-DISPATCH"
                    | "PPRINT"
                    | "PPRINT-DISPATCH"
                    | "PPRINT-FILL"
                    | "PPRINT-INDENT"
                    | "PPRINT-LINEAR"
                    | "PPRINT-NEWLINE"
                    | "PPRINT-TAB"
                    | "PPRINT-TABULAR"
                    | "SET-PPRINT-DISPATCH"
            )
        {
            register_pprint_builtin(ctx, runtime, name)?;
        } else {
            runtime.define_function(ctx, package, name, Word::UNBOUND)?; // check-added-lines: allow(unbound) placeholder for unimplemented printer surface
        }
    }
    for (package, name) in VARIABLES {
        let package = runtime.ensure_package(ctx, package)?;
        let (mut symbol, _status) = Package::from_word(package).intern(ctx, runtime, name)?;
        let token = push_root(ctx, &mut symbol);
        let result = initialise_variable(ctx, runtime, name, symbol);
        let _ = pop_root(ctx, token);
        result?;
    }
    Ok(())
}

const TYPE_SPECIFIER_PARAMETER: Parameter = Parameter {
    name: BuiltinName::new("TYPE-SPECIFIER"),
    ty: ParameterType::Any,
};
const FUNCTION_PARAMETER: Parameter = Parameter {
    name: BuiltinName::new("FUNCTION"),
    ty: ParameterType::Any,
};
const PRIORITY_PARAMETER: Parameter = Parameter {
    name: BuiltinName::new("PRIORITY"),
    ty: ParameterType::Any,
};
const TABLE_PARAMETER: Parameter = Parameter {
    name: BuiltinName::new("TABLE"),
    ty: ParameterType::Any,
};

fn register_pprint_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
) -> Result<(), ObjectError> {
    let (descriptor, function): (Builtin, ncl_object::RustBuiltin) = match name {
        "PPRINT" => (
            Builtin {
                lambda_list: LambdaList::with_optional(&[OBJECT_PARAMETER], &[STREAM_PARAMETER]),
                convention: BuiltinConvention::Adapted,
            },
            pprint,
        ),
        "PPRINT-DISPATCH" => (
            Builtin {
                lambda_list: LambdaList::with_optional(&[OBJECT_PARAMETER], &[TABLE_PARAMETER]),
                convention: BuiltinConvention::Adapted,
            },
            pprint_dispatch_builtin,
        ),
        "SET-PPRINT-DISPATCH" => (
            Builtin {
                lambda_list: LambdaList::with_optional(
                    &[TYPE_SPECIFIER_PARAMETER, FUNCTION_PARAMETER],
                    &[PRIORITY_PARAMETER, TABLE_PARAMETER],
                ),
                convention: BuiltinConvention::Adapted,
            },
            set_pprint_dispatch_builtin,
        ),
        "COPY-PPRINT-DISPATCH" => (
            Builtin {
                lambda_list: LambdaList::with_optional(&[], &[TABLE_PARAMETER]),
                convention: BuiltinConvention::Adapted,
            },
            copy_pprint_dispatch_builtin,
        ),
        "PPRINT-NEWLINE" | "PPRINT-INDENT" | "PPRINT-TAB" | "PPRINT-FILL" | "PPRINT-LINEAR"
        | "PPRINT-TABULAR" => (
            Builtin {
                lambda_list: LambdaList::with_rest(&[], OBJECT_PARAMETER),
                convention: BuiltinConvention::Adapted,
            },
            pprint_primitive,
        ),
        _ => return Err(ObjectError::Layout),
    };
    let builtin_name = match name {
        "PPRINT" => BuiltinName::new("PPRINT"),
        "PPRINT-DISPATCH" => BuiltinName::new("PPRINT-DISPATCH"),
        "SET-PPRINT-DISPATCH" => BuiltinName::new("SET-PPRINT-DISPATCH"),
        "COPY-PPRINT-DISPATCH" => BuiltinName::new("COPY-PPRINT-DISPATCH"),
        "PPRINT-NEWLINE" => BuiltinName::new("PPRINT-NEWLINE"),
        "PPRINT-INDENT" => BuiltinName::new("PPRINT-INDENT"),
        "PPRINT-TAB" => BuiltinName::new("PPRINT-TAB"),
        "PPRINT-FILL" => BuiltinName::new("PPRINT-FILL"),
        "PPRINT-LINEAR" => BuiltinName::new("PPRINT-LINEAR"),
        "PPRINT-TABULAR" => BuiltinName::new("PPRINT-TABULAR"),
        _ => return Err(ObjectError::Layout),
    };
    runtime.register_builtin(
        ctx,
        BuiltinIdentifier::new(BuiltinPackage::CommonLisp, builtin_name),
        BuiltinImplementation::adapted(descriptor, function, print_arguments),
    )?;
    Ok(())
}

fn pprint(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let options = PrintOptions::from_specials(ctx, runtime).with_pretty(true);
    let _ = print_object(ctx, runtime, args, options, false)?;
    Ok(Word::NIL)
}

fn dispatch_table(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    supplied: Option<Word>,
) -> Result<Word, ObjectError> {
    supplied.filter(|table| *table != Word::NIL).map_or_else(
        || {
            let package = runtime.ensure_package(ctx, "COMMON-LISP")?;
            with_root(ctx, &mut package.clone(), |ctx, package| {
                let (mut symbol, _) =
                    Package::from_word(*package).intern(ctx, runtime, "*PRINT-PPRINT-DISPATCH*")?;
                with_root(ctx, &mut symbol, |ctx, symbol| symbol_value(ctx, *symbol))
            })
        },
        Ok,
    )
}

fn pprint_dispatch_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let object = args.required(0)?;
    let table = dispatch_table(ctx, runtime, args.get(1))?;
    pprint_dispatch(ctx, object, table)
}

fn set_pprint_dispatch_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let type_specifier = args.required(0)?;
    let function = args.required(1)?;
    let table = dispatch_table(ctx, runtime, args.get(3))?;
    let _priority = args.get(2);
    let updated = set_pprint_dispatch(ctx, runtime, type_specifier, function, table)?;
    if args.get(3).is_none() {
        let package = runtime.ensure_package(ctx, "COMMON-LISP")?;
        with_root(ctx, &mut package.clone(), |ctx, package| {
            let (mut symbol, _) =
                Package::from_word(*package).intern(ctx, runtime, "*PRINT-PPRINT-DISPATCH*")?;
            with_root(ctx, &mut symbol, |ctx, symbol| {
                set_symbol_value(ctx, *symbol, updated)
            })
        })?;
    }
    Ok(Word::NIL)
}

fn copy_pprint_dispatch_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let table = dispatch_table(ctx, runtime, args.get(0))?;
    copy_pprint_dispatch(ctx, runtime, table)
}

#[allow(
    clippy::missing_const_for_fn,
    clippy::unnecessary_wraps,
    reason = "The shared builtin entry ABI requires a fallible, non-const function."
)]
fn pprint_primitive(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let _ = args;
    Ok(Word::NIL)
}

fn register_print_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
) -> Result<(), ObjectError> {
    let descriptor = Builtin {
        lambda_list: LambdaList::with_optional(&[OBJECT_PARAMETER], &[STREAM_PARAMETER]),
        convention: BuiltinConvention::Adapted,
    };
    let (identifier, function): (BuiltinIdentifier, ncl_object::RustBuiltin) = if name == "PRINC" {
        (
            BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new("PRINC")),
            princ,
        )
    } else if name == "PRIN1" {
        (
            BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new("PRIN1")),
            prin1,
        )
    } else if name == "PRINT" {
        (
            BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new("PRINT")),
            print,
        )
    } else {
        return Err(ObjectError::Layout);
    };
    runtime.register_builtin(
        ctx,
        identifier,
        BuiltinImplementation::adapted(descriptor, function, print_arguments),
    )?;
    Ok(())
}

fn print_arguments(args: &BuiltinArgs<'_>) -> Result<Vec<Word>, ObjectError> {
    (0..args.len())
        .map(|index| args.get(index).ok_or(ObjectError::TypeError))
        .collect()
}

fn princ(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    let options = PrintOptions::from_specials(ctx, runtime).with_escape(false);
    print_object(ctx, runtime, args, options, false)
}

fn prin1(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    let options = PrintOptions::from_specials(ctx, runtime).with_escape(true);
    print_object(ctx, runtime, args, options, false)
}

fn print(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    let options = PrintOptions::from_specials(ctx, runtime).with_escape(true);
    print_object(ctx, runtime, args, options, true)
}

fn print_object(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    options: PrintOptions,
    surrounding_newlines: bool,
) -> Result<Word, ObjectError> {
    let object = args.required(0)?;
    let stream = output_stream(ctx, runtime, args.get(1))?;
    if matches!(classify_object(ctx, object), ObjectRef::Structure(_))
        && (invoke_structure_print_function(ctx, runtime, object, stream)?
            || invoke_print_object_method(ctx, runtime, object, stream)?)
    {
        return Ok(object);
    }
    with_roots(ctx, &[object, stream], |ctx, roots| {
        let object = **roots.first().ok_or(ObjectError::Layout)?;
        let rendered =
            write_to_string(ctx, runtime, object, &options).map_err(|error| print_error(&error))?;
        let result = with_root(ctx, &mut rendered.clone(), |ctx, rendered| {
            if surrounding_newlines {
                let stream = **roots.get(1).ok_or(ObjectError::Layout)?;
                call_builtin(
                    ctx,
                    runtime,
                    "WRITE-CHAR",
                    &[Word::character(u32::from('\n')), stream],
                )?;
            }
            let stream = **roots.get(1).ok_or(ObjectError::Layout)?;
            call_builtin(ctx, runtime, "WRITE-STRING", &[*rendered, stream])?;
            if surrounding_newlines {
                let stream = **roots.get(1).ok_or(ObjectError::Layout)?;
                call_builtin(
                    ctx,
                    runtime,
                    "WRITE-CHAR",
                    &[Word::character(u32::from('\n')), stream],
                )?;
            }
            Ok(())
        });
        result?;
        roots.first().map(|root| **root).ok_or(ObjectError::Layout)
    })
}

fn invoke_structure_print_function(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    object: Word,
    stream: Word,
) -> Result<bool, ObjectError> {
    with_roots(ctx, &[object, stream], |ctx, roots| {
        let object = **roots.first().ok_or(ObjectError::Layout)?;
        let stream = **roots.get(1).ok_or(ObjectError::Layout)?;
        let layout = structure_layout(ctx, object)?;
        let Some(class) = runtime.structure_class(ctx, layout) else {
            return Ok(false);
        };
        let mut class = class;
        with_root(ctx, &mut class, |ctx, class| {
            let mut name = simple_vector_ref(ctx, *class, 0)?;
            with_root(ctx, &mut name, |ctx, name| {
                let mut key = Package::from_word(runtime.ensure_package(ctx, "NCL")?)
                    .intern(ctx, runtime, "%STRUCTURE-PRINT-FUNCTION")?
                    .0;
                with_root(ctx, &mut key, |ctx, key| {
                    let mut plist = symbol_plist(ctx, *name)?;
                    with_root(ctx, &mut plist, |ctx, plist| {
                        let mut plist_word = *plist;
                        while plist_word != Word::NIL {
                            let (found, next) = with_root(ctx, &mut plist_word, |ctx, plist| {
                                let mut property = car(ctx, *plist)?;
                                let found = with_root(ctx, &mut property, |ctx, property| {
                                    if car(ctx, *property)? == *key {
                                        let mut function = cdr(ctx, *property)?;
                                        with_root(ctx, &mut function, |ctx, function| {
                                            let mut function_word = *function;
                                            if (*function).is_cons() {
                                                let mut operator = car(ctx, *function)?;
                                                with_root(ctx, &mut operator, |ctx, operator| {
                                                    let common_lisp = runtime
                                                        .ensure_package(ctx, "COMMON-LISP")?;
                                                    let function_operator =
                                                        Package::from_word(common_lisp)
                                                            .intern(ctx, runtime, "FUNCTION")?
                                                            .0;
                                                    if *operator == function_operator {
                                                        function_word =
                                                            car(ctx, cdr(ctx, *function)?)?;
                                                    }
                                                    Ok(())
                                                })?;
                                            }
                                            let designator = FunctionDesignator::try_from_word(
                                                ctx,
                                                function_word,
                                            )?;
                                            let words = [object, stream, Word::fixnum(0)];
                                            let mut caller = BuiltinFunctionCaller;
                                            let mut values = MultipleValues::new();
                                            caller.call_function(
                                                ctx,
                                                runtime,
                                                designator,
                                                FunctionArguments::new(&words),
                                                &mut values,
                                            )?;
                                            Ok(true)
                                        })
                                    } else {
                                        Ok(false)
                                    }
                                })?;
                                Ok((found, cdr(ctx, *plist)?))
                            })?;
                            if found {
                                return Ok(true);
                            }
                            plist_word = next;
                        }
                        Ok(false)
                    })
                })
            })
        })
    })
}

/// Invoke a user-defined `PRINT-OBJECT` method when the CLOS generic exists.
/// The generic wrapper reports `UndefinedFunction` until its method registry
/// has a primary method; that is the signal to use the built-in structure
/// printer below.
fn invoke_print_object_method(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    object: Word,
    stream: Word,
) -> Result<bool, ObjectError> {
    with_roots(ctx, &[object, stream], |ctx, roots| {
        let object = **roots.first().ok_or(ObjectError::Layout)?;
        let stream = **roots.get(1).ok_or(ObjectError::Layout)?;
        let Some(function_word) = runtime.function(ctx, "COMMON-LISP", "PRINT-OBJECT") else {
            return Ok(false);
        };
        let Ok(function) = FunctionObject::try_from(function_word) else {
            return Ok(false);
        };
        let mut caller = BuiltinFunctionCaller;
        let mut values = MultipleValues::new();
        let designator = FunctionDesignator::Function(function);
        let words = [object, stream];
        match caller.call_function(
            ctx,
            runtime,
            designator,
            FunctionArguments::new(&words),
            &mut values,
        ) {
            Ok(_) => Ok(true),
            Err(ObjectError::UndefinedFunction) => Ok(false),
            Err(error) => Err(error),
        }
    })
}

fn output_stream(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    supplied: Option<Word>,
) -> Result<Word, ObjectError> {
    if supplied == Some(Word::TRUE) {
        return output_stream_variable(ctx, runtime, "*TERMINAL-IO*");
    }
    if let Some(stream) = supplied.filter(|stream| *stream != Word::NIL) {
        return Ok(stream);
    }
    output_stream_variable(ctx, runtime, "*STANDARD-OUTPUT*")
}

fn output_stream_variable(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
) -> Result<Word, ObjectError> {
    let package = runtime.ensure_package(ctx, "COMMON-LISP")?;
    with_root(ctx, &mut package.clone(), |ctx, package| {
        let (mut symbol, _) = Package::from_word(*package).intern(ctx, runtime, name)?;
        with_root(ctx, &mut symbol, |ctx, symbol| symbol_value(ctx, *symbol))
    })
}

fn builtin_function(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
) -> Result<FunctionObject, ObjectError> {
    let word = runtime
        .function(ctx, "COMMON-LISP", name)
        .ok_or(ObjectError::UndefinedFunction)?;
    FunctionObject::try_from(word).map_err(|_| ObjectError::UndefinedFunction)
}

fn call_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
    args: &[Word],
) -> Result<Word, ObjectError> {
    let function = builtin_function(ctx, runtime, name)?;
    runtime.call_builtin(ctx, function, args)
}

const fn print_error(error: &PrintError) -> ObjectError {
    match error {
        PrintError::Object(error) => *error,
        PrintError::Sink(_) | PrintError::NotReadable | PrintError::Circularity => {
            ObjectError::Layout
        }
    }
}

/// Mark one owned variable special and give it its initial value.
fn initialise_variable(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
    symbol: Word,
) -> Result<(), ObjectError> {
    set_symbol_special(ctx, symbol, true)?;
    let value = if name == "*PRINT-PPRINT-DISPATCH*" {
        default_table(ctx, runtime)?
    } else if name == "*PRINT-RIGHT-MARGIN*" {
        Word::fixnum(80)
    } else {
        Word::NIL
    };
    set_symbol_value(ctx, symbol, value)
}

/// Build a dispatch table whose default entry has no function.
///
/// The table is a list of `(type-specifier . function)` entries. The default
/// entry's specifier is `T`, which matches every object.
///
/// # Errors
///
/// Returns an [`ObjectError`] when an entry cannot be allocated.
pub fn default_table(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<Word, ObjectError> {
    let mut entry = make_cons(ctx, runtime, Word::TRUE, Word::NIL)?;
    let token = push_root(ctx, &mut entry);
    let result = make_cons(ctx, runtime, entry, Word::NIL);
    let _ = pop_root(ctx, token);
    result
}

/// Return the function `table` associates with `object`, or `NIL`.
///
/// A `T` entry matches every object; any other specifier matches by identity.
/// Type-specifier matching needs `ncl-types`, which is not on `main` yet, so
/// non-`T` entries compare with `eq`.
///
/// # Errors
///
/// Returns an [`ObjectError`] when a list accessor fails.
#[must_use = "the dispatch function is the result"]
pub fn pprint_dispatch(
    ctx: &mut ThreadContext,
    object: Word,
    table: Word,
) -> Result<Word, ObjectError> {
    let mut cursor = table;
    while cursor != Word::NIL {
        let entry = car(ctx, cursor)?;
        if entry.is_cons() {
            let specifier = car(ctx, entry)?;
            let function = cdr(ctx, entry)?;
            if specifier == Word::TRUE || specifier == object {
                return Ok(function);
            }
        }
        cursor = cdr(ctx, cursor)?;
    }
    Ok(Word::NIL)
}

/// Return `table` with `(type_specifier . function)` prepended.
///
/// The new entry shadows any earlier entry with the same specifier, because
/// lookup returns the first match.
///
/// # Errors
///
/// Returns an [`ObjectError`] when an entry cannot be allocated.
#[must_use = "the updated table is the result"]
pub fn set_pprint_dispatch(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    type_specifier: Word,
    function: Word,
    table: Word,
) -> Result<Word, ObjectError> {
    let mut entry = make_cons(ctx, runtime, type_specifier, function)?;
    let token = push_root(ctx, &mut entry);
    let result = make_cons(ctx, runtime, entry, table);
    let _ = pop_root(ctx, token);
    result
}

/// Return a shallow copy of a dispatch table.
///
/// # Errors
///
/// Returns an [`ObjectError`] when an entry cannot be allocated.
#[must_use = "the copied table is the result"]
pub fn copy_pprint_dispatch(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    table: Word,
) -> Result<Word, ObjectError> {
    let mut source = table;
    let mut copied = Word::NIL;
    let source_token = push_root(ctx, &mut source);
    let copied_token = push_root(ctx, &mut copied);
    let result = copy_entries(ctx, runtime, &mut source, &mut copied);
    let _ = pop_root(ctx, copied_token);
    let _ = pop_root(ctx, source_token);
    result?;
    Ok(copied)
}

include!("pprint_dispatch.rs");
