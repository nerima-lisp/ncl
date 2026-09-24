//! Lowering the frozen AST into `ncl-ir`.
//!
//! [`lower_toplevel`] turns one expanded form into an entry `ncl_ir::Function`
//! plus one `ncl_ir::Function` per lambda the form contains. Every generated
//! function satisfies [`ncl_ir::verify`]; the lowering tests assert this.
//!
//! # Calling convention of the generated IR
//!
//! A generated function takes a leading `argc` parameter of type `Word`
//! followed by its captured variables and its declared parameters, and returns
//! one `Word`. `argc` is the number of actual arguments the caller supplied,
//! excluding the captures and the `argc` slot itself, so an `&optional`
//! parameter can compare it against its position. The entry function produced by
//! [`lower_toplevel`] takes no parameters.
//!
//! # Lowering rules
//!
//! These implement "front end の lowering 規則" of `compiler-pipeline.md`:
//!
//! - A lexical variable that a lambda captures and that some form assigns is
//!   boxed in a one-word cell (`Alloc { words: 1 }`) the closures capture by
//!   reference; a read-only capture is passed by value.
//! - A `block` or `tagbody` whose `return-from`/`go` stay inside one function is
//!   lowered to `Jump` between basic blocks.
//! - `multiple-value-prog1` preserves its first form's value across the rest and
//!   records it with `SetMultipleValues`; `multiple-value-call` goes through the
//!   variadic adapter builtin.
//! - `&optional` parameters lower to `LoadArg`, an `argc` comparison, and a
//!   default block.
//!
//! # Known gaps reported for `ncl-ir`
//!
//! `ncl-ir` has no constant or op naming a function entry, so a lambda's callee
//! is a `Constant::Symbol` placeholder and its captured values are passed as
//! leading arguments rather than through a closure object. `multiple-value-call`
//! and the special-variable builtins use named `Builtin`s. `catch`,
//! `unwind-protect`, and `progv` need the handler-region and builtin lowering
//! owned by the runtime lane and lower to [`LowerError::Unsupported`].

mod capture;
mod control;
mod env;
mod error;
mod expr;
mod function;
mod lambda;
mod literal;

pub use error::LowerError;

use ncl_ir::Function;

use crate::ast::Expr;

/// The functions produced by lowering one form.
#[derive(Clone, Debug)]
pub struct Lowered {
    /// The function for the lowered form itself.
    pub entry: Function,
    /// One function per lambda expression in the form, in lowering order.
    pub nested: Vec<Function>,
}

/// Lower one expanded form into an entry function and its nested functions.
///
/// The entry function takes no parameters and returns one `Word`. The nested
/// functions are the lambdas the form contains; `ncl-ir` cannot yet name a
/// function entry, so they are returned alongside the entry for the runtime to
/// register and link.
///
/// # Errors
///
/// Returns [`LowerError`] when a form has no `ncl-ir` representation.
pub fn lower_toplevel(expr: &Expr) -> Result<Lowered, LowerError> {
    let mut module = function::Lowerer::new();
    let mut entry = function::FunctionLowerer::entry("toplevel");
    let value = expr::lower_expr(&mut entry, &mut module, expr)?;
    entry.return_value(value)?;
    Ok(Lowered {
        entry: entry.into_function(),
        nested: module.into_functions(),
    })
}
