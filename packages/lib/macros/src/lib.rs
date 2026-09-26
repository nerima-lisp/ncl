//! Builtin registration and small, structural macro expanders.
#![allow(clippy::missing_errors_doc)]

mod control;
mod defining;
mod form;
mod functions;
mod place;
mod setf;

pub use form::{elements, fresh_symbol, list, symbol};
pub use place::{PlaceExpander, PlaceRegistry, SetfExpansion, register_place};
pub use setf::{
    expand_decf, expand_get_setf_expansion, expand_incf, expand_pop, expand_psetf, expand_push,
    expand_remf, expand_rotatef, expand_setf, expand_shiftf,
};

use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, LambdaList, ObjectError, Package, Parameter, ParameterType,
    Runtime, ThreadContext, Word, set_symbol_macro, set_symbol_special,
};

const CL: &str = "COMMON-LISP";
const FORM: Parameter = Parameter {
    name: BuiltinName::new("FORM"),
    ty: ParameterType::Any,
};
const ENVIRONMENT: Parameter = Parameter {
    name: BuiltinName::new("ENVIRONMENT"),
    ty: ParameterType::Any,
};
const MACRO_LAMBDA_LIST: LambdaList = LambdaList::with_rest(&[FORM], ENVIRONMENT);
const PLACE: Parameter = Parameter {
    name: BuiltinName::new("PLACE"),
    ty: ParameterType::Any,
};
fn expansion_arg(args: &[Word]) -> Result<Word, ObjectError> {
    args.first().copied().ok_or(ObjectError::TypeError)
}

type LegacyBuiltin = fn(
    &Runtime,
    &mut ThreadContext,
    args: &[Word],
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError>;

fn call_legacy(
    callback: LegacyBuiltin,
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    let words = (0..args.len())
        .filter_map(|index| args.get(index))
        .collect::<Vec<_>>();
    callback(runtime, ctx, &words, values)
}

macro_rules! adapters {
    ($($adapter:ident => $callback:path),+ $(,)?) => {
        $(fn $adapter(
            ctx: &mut ThreadContext,
            runtime: &Runtime,
            args: &BuiltinArgs<'_>,
            values: &mut ncl_object::MultipleValues,
        ) -> Result<Word, ObjectError> {
            call_legacy($callback, ctx, runtime, args, values)
        })+
    };
}

fn macro_arguments(ctx: &mut ThreadContext, form: Word) -> Result<Vec<Word>, ObjectError> {
    let mut parts = elements(ctx, form)?;
    if parts.is_empty() {
        return Err(ObjectError::TypeError);
    }
    parts.remove(0);
    Ok(parts)
}

fn setf_callback(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    let form = expansion_arg(args)?;
    let arguments = macro_arguments(ctx, form)?;
    expand_setf(ctx, runtime, &PlaceRegistry::new(runtime), &arguments)
}

fn psetf_callback(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    let form = expansion_arg(args)?;
    let arguments = macro_arguments(ctx, form)?;
    expand_psetf(ctx, runtime, &PlaceRegistry::new(runtime), &arguments)
}

fn call_macro(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _values: &mut ncl_object::MultipleValues,
    expand: fn(&mut ThreadContext, &Runtime, &PlaceRegistry, &[Word]) -> Result<Word, ObjectError>,
) -> Result<Word, ObjectError> {
    let form = expansion_arg(args)?;
    let arguments = macro_arguments(ctx, form)?;
    expand(ctx, runtime, &PlaceRegistry::new(runtime), &arguments)
}

fn incf_callback(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    call_macro(runtime, ctx, args, values, expand_incf)
}
fn decf_callback(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    call_macro(runtime, ctx, args, values, expand_decf)
}
fn push_callback(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    let form = expansion_arg(args)?;
    let arguments = macro_arguments(ctx, form)?;
    expand_push(ctx, runtime, &PlaceRegistry::new(runtime), &arguments, false)
}
fn pushnew_callback(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    let form = expansion_arg(args)?;
    let arguments = macro_arguments(ctx, form)?;
    expand_push(ctx, runtime, &PlaceRegistry::new(runtime), &arguments, true)
}
fn pop_callback(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    call_macro(runtime, ctx, args, values, expand_pop)
}
fn remf_callback(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    call_macro(runtime, ctx, args, values, expand_remf)
}
fn shiftf_callback(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    call_macro(runtime, ctx, args, values, expand_shiftf)
}
fn rotatef_callback(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    call_macro(runtime, ctx, args, values, expand_rotatef)
}
fn get_setf_expansion_callback(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    let place_word = expansion_arg(args)?;
    let expansion = expand_get_setf_expansion(ctx, runtime, &PlaceRegistry::new(runtime), place_word)?;
    values.set(&expansion);
    Ok(expansion[4])
}

fn get_setf_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    let words = (0..args.len())
        .filter_map(|index| args.get(index))
        .collect::<Vec<_>>();
    get_setf_expansion_callback(runtime, ctx, &words, values)
}

adapters!
    (setf_adapter => setf_callback,
     psetf_adapter => psetf_callback,
     incf_adapter => incf_callback,
     decf_adapter => decf_callback,
     push_adapter => push_callback,
     pushnew_adapter => pushnew_callback,
     pop_adapter => pop_callback,
     remf_adapter => remf_callback,
     shiftf_adapter => shiftf_callback,
     rotatef_adapter => rotatef_callback);

fn callback_for(name: &str) -> Option<ncl_object::RustBuiltin> {
    match name {
        "SETF" => Some(setf_adapter),
        "PSETF" => Some(psetf_adapter),
        "INCF" => Some(incf_adapter),
        "DECF" => Some(decf_adapter),
        "PUSH" => Some(push_adapter),
        "PUSHNEW" => Some(pushnew_adapter),
        "POP" => Some(pop_adapter),
        "REMF" => Some(remf_adapter),
        "SHIFTF" => Some(shiftf_adapter),
        "ROTATEF" => Some(rotatef_adapter),
        "DEFUN" => Some(defining::defun_adapter),
        "DEFMACRO" => Some(defining::defmacro_adapter),
        "DEFVAR" => Some(defining::defvar_adapter),
        "DEFPARAMETER" => Some(defining::defparameter_adapter),
        "DEFCONSTANT" => Some(defining::defconstant_adapter),
        "DEFINE-SYMBOL-MACRO" => Some(defining::define_symbol_macro_adapter),
        "DEFINE-COMPILER-MACRO" => Some(defining::define_compiler_macro_adapter),
        "DEFSETF" => Some(defining::defsetf_adapter),
        "DEFINE-SETF-EXPANDER" => Some(defining::define_setf_expander_adapter),
        "WHEN" => Some(control::expand_when_adapter),
        "UNLESS" => Some(control::expand_unless_adapter),
        "AND" => Some(control::expand_and_adapter),
        "OR" => Some(control::expand_or_adapter),
        "COND" => Some(control::expand_cond_adapter),
        "CASE" => Some(control::expand_case_adapter),
        "ECASE" => Some(control::expand_ecase_adapter),
        "CCASE" => Some(control::expand_ccase_adapter),
        "TYPECASE" => Some(control::expand_typecase_adapter),
        "ETYPECASE" => Some(control::expand_etypecase_adapter),
        "CTYPECASE" => Some(control::expand_ctypecase_adapter),
        "PROG" => Some(control::expand_prog_adapter),
        "PROG*" => Some(control::expand_prog_star_adapter),
        "PROG1" => Some(control::expand_prog1_adapter),
        "PROG2" => Some(control::expand_prog2_adapter),
        "RETURN" => Some(control::expand_return_adapter),
        "NTH-VALUE" => Some(control::expand_nth_value_adapter),
        "DO" => Some(control::expand_do_adapter),
        "DO*" => Some(control::expand_do_star_adapter),
        _ => None,
    }
}

/// Register the symbols owned by this crate.
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    let mut ctx = ThreadContext::new();
    ctx.register(runtime)?;
    for &name in OWNED_MACROS {
        let symbol = Package::from_word(runtime.ensure_package(&mut ctx, CL)?)
            .intern(&mut ctx, runtime, name)?
            .0;
        set_symbol_macro(&mut ctx, symbol, true)?;
    }
    for &name in MACROS {
        let Some(callback) = callback_for(name) else {
            continue;
        };
        let implementation = BuiltinImplementation::adapted(
            Builtin {
                lambda_list: MACRO_LAMBDA_LIST,
                convention: BuiltinConvention::Adapted,
            },
            callback,
            |args| {
                Ok((0..args.len())
                    .filter_map(|index| args.get(index))
                    .collect())
            },
        );
        let function = runtime.register_builtin(
            &mut ctx,
            BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new(name)),
            implementation,
        )?;
        let _ = function;
    }
    runtime.register_builtin(
        &mut ctx,
        BuiltinIdentifier::new(
            BuiltinPackage::CommonLisp,
            BuiltinName::new("GET-SETF-EXPANSION"),
        ),
        BuiltinImplementation::direct(
            Builtin {
                lambda_list: LambdaList::fixed(&[PLACE]),
                convention: BuiltinConvention::Direct(Arity::exact(1)),
            },
            get_setf_adapter,
        ),
    )?;
    functions::register(runtime, &mut ctx)?;
    let variable = Package::from_word(runtime.ensure_package(&mut ctx, CL)?)
        .intern(&mut ctx, runtime, "*MACROEXPAND-HOOK*")?
        .0;
    set_symbol_special(&mut ctx, variable, true)?;
    ncl_object::set_symbol_value(&mut ctx, variable, Word::NIL)?;
    Ok(())
}

const MACROS: &[&str] = &[
    "AND",
    "CASE",
    "CCASE",
    "COND",
    "CTYPECASE",
    "DECF",
    "DEFCONSTANT",
    "DEFINE-COMPILER-MACRO",
    "DEFINE-SETF-EXPANDER",
    "DEFINE-SYMBOL-MACRO",
    "DEFMACRO",
    "DEFUN",
    "DEFPARAMETER",
    "DEFSETF",
    "DEFVAR",
    "DO",
    "DO*",
    "ECASE",
    "ETYPECASE",
    "INCF",
    "NTH-VALUE",
    "OR",
    "POP",
    "PROG",
    "PROG*",
    "PROG1",
    "PROG2",
    "PSETF",
    "PUSH",
    "PUSHNEW",
    "REMF",
    "RETURN",
    "SETF",
    "TYPECASE",
    "UNLESS",
    "WHEN",
];

const OWNED_MACROS: &[&str] = &[
    "AND",
    "ASSERT",
    "CALL-METHOD",
    "CASE",
    "CCASE",
    "CHECK-TYPE",
    "COND",
    "CTYPECASE",
    "DECF",
    "DEFCONSTANT",
    "DEFINE-COMPILER-MACRO",
    "DEFINE-MODIFY-MACRO",
    "DEFINE-SETF-EXPANDER",
    "DEFINE-SYMBOL-MACRO",
    "DEFMACRO",
    "DEFUN",
    "DEFPACKAGE",
    "DEFPARAMETER",
    "DEFSETF",
    "DEFVAR",
    "DESTRUCTURING-BIND",
    "DO",
    "DO*",
    "DO-ALL-SYMBOLS",
    "DO-EXTERNAL-SYMBOLS",
    "DO-SYMBOLS",
    "DOLIST",
    "DOTIMES",
    "ECASE",
    "ETYPECASE",
    "FORMATTER",
    "HANDLER-BIND",
    "HANDLER-CASE",
    "IGNORE-ERRORS",
    "IN-PACKAGE",
    "INCF",
    "LAMBDA",
    "LOOP",
    "LOOP-FINISH",
    "NTH-VALUE",
    "OR",
    "POP",
    "PPRINT-EXIT-IF-LIST-EXHAUSTED",
    "PPRINT-LOGICAL-BLOCK",
    "PPRINT-POP",
    "PRINT-UNREADABLE-OBJECT",
    "PROG",
    "PROG*",
    "PROG1",
    "PROG2",
    "PSETF",
    "PUSH",
    "PUSHNEW",
    "REMF",
    "RESTART-BIND",
    "RESTART-CASE",
    "RETURN",
    "SETF",
    "STEP",
    "TIME",
    "TRACE",
    "TYPECASE",
    "UNLESS",
    "UNTRACE",
    "WHEN",
    "WITH-ACCESSORS",
    "WITH-COMPILATION-UNIT",
    "WITH-CONDITION-RESTARTS",
    "WITH-HASH-TABLE-ITERATOR",
    "WITH-INPUT-FROM-STRING",
    "WITH-OPEN-FILE",
    "WITH-OPEN-STREAM",
    "WITH-OUTPUT-TO-STRING",
    "WITH-PACKAGE-ITERATOR",
    "WITH-SIMPLE-RESTART",
    "WITH-SLOTS",
];

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn registration_marks_owned_macros_and_installs_function_cells() {
        let runtime = Runtime::new().expect("runtime");
        register(&runtime).expect("macro registration");
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime).expect("context registration");
        let package = runtime.find_package(&ctx, CL).expect("COMMON-LISP");
        let symbol = Package::from_word(package)
            .intern(&mut ctx, &runtime, "WHEN")
            .expect("WHEN")
            .0;
        assert!(ncl_object::symbol_is_macro(&ctx, symbol).expect("macro flag"));
        assert_ne!(
            ncl_object::symbol_function(&ctx, symbol).expect("function cell"),
            Word::UNBOUND
        );
        for name in MACROS {
            let symbol = Package::from_word(package)
                .intern(&mut ctx, &runtime, name)
                .expect(name)
                .0;
            assert_ne!(
                ncl_object::symbol_function(&ctx, symbol).expect(name),
                Word::UNBOUND,
                "registered macro {name} has an unbound function cell"
            );
        }

        for name in ["DEFUN", "DEFMACRO", "DEFVAR", "DEFPARAMETER", "DEFCONSTANT"] {
            let symbol = Package::from_word(package)
                .intern(&mut ctx, &runtime, name)
                .expect(name)
                .0;
            assert!(ncl_object::symbol_is_macro(&ctx, symbol).expect(name));
        }
        let hook = Package::from_word(package)
            .intern(&mut ctx, &runtime, "*MACROEXPAND-HOOK*")
            .expect("*MACROEXPAND-HOOK*")
            .0;
        assert!(ncl_object::symbol_is_special(&ctx, hook).expect("special flag"));
        assert!(
            runtime
                .function(&mut ctx, CL, "GET-SETF-EXPANSION")
                .is_some()
        );
    }
}
