use ncl_syntax::{
    Form, FormKind, LambdaListAuxiliaryParameter, LambdaListErrorKind, LambdaListKeywordParameter,
    LambdaListOptionalParameter, OrdinaryLambdaList, Span, SymbolTokenKind,
    parse_ordinary_lambda_list, parse_symbol_token,
};
use std::collections::HashSet;

mod data;
mod error;
mod helpers;
mod state;

pub use data::{
    AuxiliaryParameter, Constant, DestructureAuxiliaryParameter, DestructureKeywordParameter,
    DestructureLambdaList, DestructureOptionalParameter, DestructurePattern, DestructureSpec,
    FunctionCode, FunctionId, HandlerBindClause, HandlerCaseClause, Instruction, KeywordParameter,
    OptionalParameter, Program, RestartBindClause, RestartCaseClause,
};
pub use error::{CompileError, CompileErrorKind};

use self::helpers::{
    case_default_clause, compile_eval_when_executes, literal_constant, normalize_name,
    operator_span, special_operator_name, symbol_reference, tag_name,
};
use data::CompileState;
use data::DestructureLambdaListSection;

/// Stateless compiler entry points for syntax forms.
#[derive(Clone, Copy, Debug, Default)]
pub struct Compiler;

impl Compiler {
    /// Compile a sequence of forms into an entry function.
    #[must_use = "the compiled program or error must be handled"]
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
    #[must_use = "the compiled program or error must be handled"]
    pub fn compile_form(form: &Form) -> Result<Program, CompileError> {
        Self::compile_forms(std::slice::from_ref(form))
    }
}
