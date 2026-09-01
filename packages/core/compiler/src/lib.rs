//! Compiler data structures and bytecode generation for NCL forms.

pub(crate) use ncl_syntax::normalize_name;
use ncl_syntax::{
    parse_float_literal, parse_ordinary_lambda_list, parse_radix_integer_literal,
    parse_radix_integer_literal_text, parse_symbol_token, Form, FormKind,
    LambdaListAuxiliaryParameter, LambdaListErrorKind, LambdaListKeywordParameter,
    LambdaListOptionalParameter, OrdinaryLambdaList, Span, SymbolTokenKind,
};
use std::collections::HashSet;

mod compiler_error;
pub use compiler_error::{CompileError, CompileErrorKind};

mod constant;
pub use constant::Constant;

/// An index into [`Program::functions`].
pub type FunctionId = usize;

mod parameter_metadata;
pub use parameter_metadata::{AuxiliaryParameter, KeywordParameter, OptionalParameter};

mod handler_clauses;
pub use handler_clauses::{
    HandlerBindClause, HandlerCaseClause, RestartBindClause, RestartCaseClause,
};

mod destructure_types;
use destructure_types::DestructureLambdaListSection;
pub use destructure_types::{
    DestructureAuxiliaryParameter, DestructureKeywordParameter, DestructureLambdaList,
    DestructureOptionalParameter, DestructurePattern, DestructureSpec,
};

mod instruction;
pub use instruction::{Instruction, PsetfPlace, RotateShiftPlace};

mod function_code;
pub use function_code::{FunctionCode, Program};

/// Stateless compiler entry points for syntax forms.
#[derive(Clone, Copy, Debug, Default)]
pub struct Compiler;

impl Compiler {
    /// Compile a sequence of forms into an entry function.
    ///
    /// # Errors
    ///
    /// Returns [`CompileError`] when a form is malformed or unsupported.
    pub fn compile_forms(forms: &[Form]) -> Result<Program, CompileError> {
        let mut state = CompileState::default();
        state.collect_names(forms);
        let entry = state.reserve_function(None, Vec::new());
        state.compile_sequence(entry, forms)?;
        state.emit(entry, Instruction::Return, Span::new(0, 0))?;
        Ok(Program {
            functions: state.functions,
            entry,
        })
    }

    /// Compile one form as a complete program.
    ///
    /// # Errors
    ///
    /// Returns [`CompileError`] when the form is malformed or unsupported.
    pub fn compile_form(form: &Form) -> Result<Program, CompileError> {
        Self::compile_forms(std::slice::from_ref(form))
    }
}

mod state;
#[allow(clippy::wildcard_imports)]
use state::*;
mod branching;
mod compilation;
mod control_forms;
mod logical_forms;
mod parameters;
mod runtime_definitions;
mod validation;

mod destructuring;

mod flet_binding;
mod let_binding;

mod forms;
mod helpers;
#[allow(clippy::wildcard_imports)]
pub(crate) use helpers::*;
