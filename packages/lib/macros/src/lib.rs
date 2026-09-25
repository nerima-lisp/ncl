//! Builtin registration and small, structural macro expanders.
#![allow(clippy::missing_errors_doc)]

mod form;
mod place;

pub use form::{elements, fresh_symbol, list, symbol};
pub use place::{PlaceExpander, PlaceRegistry, SetfExpansion};

use ncl_object::{
    set_symbol_macro, Builtin, BuiltinImplementation, ObjectError, Package, Runtime, ThreadContext,
    Word,
};

const CL: &str = "COMMON-LISP";

fn expansion_arg(args: &[Word]) -> Result<Word, ObjectError> {
    args.first().copied().ok_or(ObjectError::TypeError)
}

fn identity(
    _ctx: &mut ThreadContext,
    args: &[Word],
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    expansion_arg(args)
}

fn callback_for(name: &str) -> ncl_object::RustBuiltin {
    let _ = name;
    identity
}

/// Register the symbols owned by this crate.
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    let mut ctx = ThreadContext::new();
    ctx.register(runtime)?;
    for name in MACROS {
        let implementation = BuiltinImplementation::adapted(
            Builtin {
                arity: 0,
                direct: false,
                lambda_list: "form &environment environment",
            },
            callback_for(name),
            |args| Ok(args.to_vec()),
        );
        let function = runtime.register_builtin(&mut ctx, CL, name, implementation)?;
        let symbol = Package::from(runtime.ensure_package(&mut ctx, CL)?)
            .intern(&mut ctx, runtime, name)?
            .0;
        set_symbol_macro(&mut ctx, symbol, true)?;
        let _ = function;
    }
    for name in FUNCTIONS {
        runtime.define_function(&mut ctx, CL, name, Word::UNBOUND)?;
    }
    let variable = Package::from(runtime.ensure_package(&mut ctx, CL)?)
        .intern(&mut ctx, runtime, "*MACROEXPAND-HOOK*")?
        .0;
    ncl_object::set_symbol_value(&mut ctx, variable, Word::NIL)?;
    Ok(())
}

const MACROS: &[&str] = &[
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

const FUNCTIONS: &[&str] = &[
    "APPLY",
    "COMPILED-FUNCTION-P",
    "COMPILER-MACRO-FUNCTION",
    "COMPLEMENT",
    "CONSTANTLY",
    "CONSTANTP",
    "FDEFINITION",
    "FUNCALL",
    "FUNCTION-LAMBDA-EXPRESSION",
    "FUNCTIONP",
    "GET-SETF-EXPANSION",
    "IDENTITY",
    "MACRO-FUNCTION",
    "MACROEXPAND",
    "MACROEXPAND-1",
    "NOT",
    "REPLACE",
    "SPECIAL-OPERATOR-P",
    "VALUES",
    "VALUES-LIST",
    "WARN",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registration_marks_owned_macros_and_installs_function_cells() {
        let runtime = Runtime::new().expect("runtime");
        register(&runtime).expect("macro registration");
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime).expect("context registration");
        let package = runtime.find_package(&ctx, CL).expect("COMMON-LISP");
        let symbol = Package::from(package)
            .intern(&mut ctx, &runtime, "WHEN")
            .expect("WHEN")
            .0;
        assert!(ncl_object::symbol_is_macro(&ctx, symbol).expect("macro flag"));
        assert_ne!(
            ncl_object::symbol_function(&ctx, symbol).expect("function cell"),
            Word::UNBOUND
        );
    }
}
