//! Macroexpansion, declarations, compiler macros, and IR lowering.
//!
//! `ncl-compiler-front` turns reader output (`ncl-object` values) into the
//! internal AST in [`ast`], tracks lexical and declaration environments, expands
//! macros through [`MacroCaller`], and, in the `lower` module owned by the
//! back-half lane, lowers the AST into `ncl-ir`.
//!
//! # Frozen contract
//!
//! [`ast`], [`literal`], [`symbols`], [`types`], [`lambda_list`], and
//! [`declaration`] are the frozen boundary that the lowering lane consumes.
//! Adding a field or a variant to any of them is a contract change.
//! [`MacroCaller`] and [`MacroRegistry`] are the other frozen entry points.
//!
//! # Lane scope
//!
//! The front-half lane owns `env`, `expand`, `special`, the declaration and
//! lambda-list parsers, [`MacroCaller`], and [`MacroRegistry`]. The back-half
//! lane owns `lower` (AST to `ncl-ir`) and the remaining owned symbols.

pub mod ast;
pub mod compiler_macro;
pub mod declaration;
pub mod error;
pub mod form;
pub mod lambda_list;
pub mod literal;
pub mod macro_caller;
pub mod symbols;
pub mod types;

pub use ast::{
    EvalSituation, Expr, FunctionDesignator, LambdaExpr, LetBinding, LocalFunction, LocalMacro,
    SymbolMacro, TagbodyItem,
};
pub use compiler_macro::{ArityPattern, CompilerMacro, MacroExpander, MacroRegistry};
pub use declaration::{Declaration, OptimizeQuality, Quality, parse_declare_form};
pub use error::FrontError;
pub use lambda_list::{AuxParam, KeyParam, LambdaList, OptionalParam, ParamName};
pub use literal::{Literal, NumberLiteral};
pub use macro_caller::MacroCaller;
pub use symbols::SymbolRef;
pub use types::TypeSpecifier;
