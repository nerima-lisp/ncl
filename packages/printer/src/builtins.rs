//! Registration of the printer's owned symbols and its dispatch table.

use ncl_object::{
    Builtin, BuiltinArgs, BuiltinConvention, BuiltinFunctionCaller, BuiltinIdentifier,
    BuiltinImplementation, BuiltinName, BuiltinPackage, FunctionArguments, FunctionCaller,
    FunctionDesignator, FunctionObject, LambdaList, MultipleValues, ObjectError, ObjectRef,
    Package, Parameter, ParameterType, Runtime, ThreadContext, Word, car, cdr, classify_object,
    make_cons, pop_root, push_root, rplacd, set_symbol_special, set_symbol_value, symbol_value,
    with_root, with_roots,
};

use crate::{PrintError, PrintOptions, write_to_string};

#[path = "builtins/structure.rs"]
mod structure;
use structure::invoke_structure_print_function;

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
const PPRINT_DISPATCH_TABLE_PARAMETER: Parameter = Parameter {
    name: BuiltinName::new("TABLE"),
    ty: ParameterType::Any,
};

/// The `(package, name)` special variables `ncl-printer` owns.
const VARIABLES: [(&str, &str); 4] = [
    ("COMMON-LISP", "*PRINT-PPRINT-DISPATCH*"),
    ("COMMON-LISP", "*PRINT-READABLY*"),
    ("NCL-EXT", "*PRINT-CIRCLE-NOT-SHARED*"),
    ("NCL-EXT", "*PRINT-VECTOR-LENGTH*"),
];

/// Register every symbol `ncl-printer` owns with `runtime`.
///
/// Implemented functions are registered as callable builtins; the remaining
/// function symbols keep unbound placeholders. The owned variables are
/// interned, marked special, and initialised:
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
        if package == "COMMON-LISP" && name == "COPY-PPRINT-DISPATCH" {
            register_copy_pprint_dispatch_builtin(ctx, runtime)?;
        } else if package == "COMMON-LISP" && matches!(name, "PRINC" | "PRIN1" | "PRINT") {
            register_print_builtin(ctx, runtime, name)?;
        } else if package == "COMMON-LISP"
            && matches!(
                name,
                "PRINC-TO-STRING" | "PRIN1-TO-STRING" | "WRITE-TO-STRING"
            )
        {
            crate::to_string::register(ctx, runtime, name)?;
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

fn register_copy_pprint_dispatch_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
) -> Result<(), ObjectError> {
    runtime.register_builtin(
        ctx,
        BuiltinIdentifier::new(
            BuiltinPackage::CommonLisp,
            BuiltinName::new("COPY-PPRINT-DISPATCH"),
        ),
        BuiltinImplementation::adapted(
            Builtin {
                lambda_list: LambdaList::with_optional(&[], &[PPRINT_DISPATCH_TABLE_PARAMETER]),
                convention: BuiltinConvention::Adapted,
            },
            copy_pprint_dispatch_builtin,
            print_arguments,
        ),
    )?;
    Ok(())
}

fn copy_pprint_dispatch_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    let mut table = match args.get(0) {
        Some(Word::NIL) => default_table(ctx, runtime)?,
        Some(table) => table,
        None => current_pprint_dispatch_table(ctx, runtime)?,
    };
    with_root(ctx, &mut table, |ctx, table| {
        copy_pprint_dispatch(ctx, runtime, table.get())
    })
}

fn current_pprint_dispatch_table(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
) -> Result<Word, ObjectError> {
    let package = runtime.ensure_package(ctx, "COMMON-LISP")?;
    with_root(ctx, &mut package.clone(), |ctx, package| {
        let (mut symbol, _) =
            Package::from_word(package.get()).intern(ctx, runtime, "*PRINT-PPRINT-DISPATCH*")?;
        with_root(ctx, &mut symbol, |ctx, symbol| {
            symbol_value(ctx, symbol.get())
        })
    })
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
    let mut object = args.required(0)?;
    with_root(ctx, &mut object, |ctx, object| {
        let mut stream = output_stream(ctx, runtime, args.get(1))?;
        with_root(ctx, &mut stream, |ctx, stream| {
            if matches!(classify_object(ctx, object.get()), ObjectRef::Structure(_))
                && (invoke_structure_print_function(ctx, runtime, object.get(), stream.get())?
                    || invoke_print_object_method(ctx, runtime, object.get(), stream.get())?)
            {
                return Ok(object.get());
            }
            let rendered = write_to_string(ctx, runtime, object.get(), &options)
                .map_err(|error| print_error(&error))?;
            with_root(ctx, &mut rendered.clone(), |ctx, rendered| {
                if surrounding_newlines {
                    call_builtin(
                        ctx,
                        runtime,
                        "WRITE-CHAR",
                        &[Word::character(u32::from('\n')), stream.get()],
                    )?;
                }
                call_builtin(
                    ctx,
                    runtime,
                    "WRITE-STRING",
                    &[rendered.get(), stream.get()],
                )?;
                if surrounding_newlines {
                    call_builtin(
                        ctx,
                        runtime,
                        "WRITE-CHAR",
                        &[Word::character(u32::from('\n')), stream.get()],
                    )?;
                }
                Ok(object.get())
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
        let Some(function_word) = runtime.function(ctx, "COMMON-LISP", "PRINT-OBJECT") else {
            return Ok(false);
        };
        let Ok(function) = FunctionObject::try_from(function_word) else {
            return Ok(false);
        };
        let mut caller = BuiltinFunctionCaller;
        let mut values = MultipleValues::new();
        let designator = FunctionDesignator::Function(function);
        let words = [
            roots.first().ok_or(ObjectError::Layout)?.get(),
            roots.get(1).ok_or(ObjectError::Layout)?.get(),
        ];
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
        let (mut symbol, _) = Package::from_word(package.get()).intern(ctx, runtime, name)?;
        with_root(ctx, &mut symbol, |ctx, symbol| {
            symbol_value(ctx, symbol.get())
        })
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
/// Returns an [`ObjectError`] when `table` is not a proper list or an entry
/// cannot be allocated.
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
    let result = copy_entries(ctx, runtime, &mut source, &mut copied)
        .and_then(|()| reverse_copied_entries(ctx, &mut copied));
    let _ = pop_root(ctx, copied_token);
    let _ = pop_root(ctx, source_token);
    result?;
    Ok(copied)
}

fn reverse_copied_entries(ctx: &mut ThreadContext, copied: &mut Word) -> Result<(), ObjectError> {
    let mut cursor = *copied;
    let mut reversed = Word::NIL;
    while cursor != Word::NIL {
        let next = cdr(ctx, cursor)?;
        rplacd(ctx, cursor, reversed)?;
        reversed = cursor;
        cursor = next;
    }
    *copied = reversed;
    Ok(())
}

include!("pprint_dispatch.rs");
