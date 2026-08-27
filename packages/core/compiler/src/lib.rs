//! Compiler data structures and bytecode generation for NCL forms.

use ncl_syntax::{
    Form, FormKind, LambdaListAuxiliaryParameter, LambdaListErrorKind, LambdaListKeywordParameter,
    LambdaListOptionalParameter, OrdinaryLambdaList, Span, SymbolTokenKind,
    parse_ordinary_lambda_list, parse_symbol_token,
};
use std::collections::HashSet;

mod compiler_error;

pub use compiler_error::{CompileError, CompileErrorKind};

/// A literal value embedded directly in bytecode.
#[derive(Clone, Debug, PartialEq)]
pub enum Constant {
    /// The NCL empty-list value.
    Nil,
    /// A boolean literal.
    Boolean(bool),
    /// A signed integer literal.
    Integer(i64),
    /// A rational literal in normalized numerator/denominator form.
    Rational {
        /// Normalized numerator.
        numerator: i64,
        /// Positive denominator.
        denominator: i64,
    },
    /// A floating-point literal.
    Float(f64),
    /// A string literal.
    String(String),
    /// A character literal.
    Character(char),
    /// A package-resolved symbol name.
    Symbol(String),
    /// An escaped symbol name.
    SymbolExact(String),
    /// A package-resolved keyword name.
    Keyword(String),
    /// An escaped keyword name.
    KeywordExact(String),
}

/// An index into [`Program::functions`].
pub type FunctionId = usize;

/// Metadata for one compiled `&OPTIONAL` parameter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OptionalParameter {
    /// Parameter binding name.
    pub name: String,
    /// Whether the name was escaped in source.
    pub name_escaped: bool,
    /// Function containing the init-form.
    pub default_function: FunctionId,
    /// Optional supplied-p binding name.
    pub supplied_p: Option<String>,
    /// Whether the supplied-p name was escaped.
    pub supplied_p_escaped: Option<bool>,
}

/// Metadata for one compiled `&KEY` parameter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeywordParameter {
    /// External keyword name.
    pub keyword_name: String,
    /// Whether the keyword name was escaped.
    pub keyword_name_escaped: bool,
    /// Local parameter binding name.
    pub name: String,
    /// Whether the local name was escaped.
    pub name_escaped: bool,
    /// Function containing the init-form.
    pub default_function: FunctionId,
    /// Optional supplied-p binding name.
    pub supplied_p: Option<String>,
    /// Whether the supplied-p name was escaped.
    pub supplied_p_escaped: Option<bool>,
}

/// Metadata for one compiled `&AUX` parameter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuxiliaryParameter {
    /// Auxiliary binding name.
    pub name: String,
    /// Whether the name was escaped in source.
    pub name_escaped: bool,
    /// Function containing the init-form.
    pub default_function: FunctionId,
}

/// One compiled `HANDLER-CASE` clause.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HandlerCaseClause {
    /// Condition type name.
    pub condition: String,
    /// Optional handler variable name.
    pub variable: Option<String>,
    /// Function containing the handler body.
    pub function: FunctionId,
}

/// One compiled `HANDLER-BIND` clause.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HandlerBindClause {
    /// Condition type name.
    pub condition: String,
    /// Function containing the handler body.
    pub function: FunctionId,
}

/// One compiled `RESTART-BIND` clause.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RestartBindClause {
    /// Restart name.
    pub name: String,
    /// Function containing the restart body.
    pub function: FunctionId,
}

/// One compiled `RESTART-CASE` clause.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RestartCaseClause {
    /// Restart name.
    pub name: String,
    /// Function containing the restart body.
    pub function: FunctionId,
}

/// One stack-bytecode operation.
///
/// `Constant`, `Quote`, `QuasiQuote`, `Load`, `FunctionLoad`, `IsBound`, and
/// `MakeClosure` push one value. `Define`, `Set`, and `Setf` install the
/// primary value at the top of the stack and leave that value on the stack.
/// `DefineFunction` consumes a closure and stores it in the lexical function
/// namespace. `DefineValues` preserves a multiple-value carrier. `Psetq`
/// consumes all RHS values and leaves `NIL`; `MultipleValueSetq` consumes one
/// carrier and leaves its primary value. `JumpIfFalse` consumes its condition.
/// Scope operations do not alter the value stack, and call-like instructions
/// replace their callee and arguments with a result. `Values` creates one
/// carrier stack entry, `MultipleValueList` converts one carrier to a list,
/// while `BindValues` and `Destructure` consume one carrier without pushing a
/// result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DestructurePattern {
    /// A single symbol binding.
    Name(String),
    /// A nested proper list pattern.
    List(Vec<Self>),
    /// A dotted list pattern.
    Dotted {
        /// Proper-list prefix patterns.
        items: Vec<Self>,
        /// Pattern for the dotted tail.
        tail: Box<Self>,
    },
}

/// One compiled `DESTRUCTURING-BIND` `&OPTIONAL` parameter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DestructureOptionalParameter {
    /// Binding pattern.
    pub pattern: DestructurePattern,
    /// Function containing the init-form.
    pub default_function: FunctionId,
    /// Optional supplied-p binding name.
    pub supplied_p: Option<String>,
}

/// One compiled `DESTRUCTURING-BIND` `&KEY` parameter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DestructureKeywordParameter {
    /// External keyword name.
    pub keyword_name: String,
    /// Binding pattern.
    pub pattern: DestructurePattern,
    /// Function containing the init-form.
    pub default_function: FunctionId,
    /// Optional supplied-p binding name.
    pub supplied_p: Option<String>,
}

/// One compiled `DESTRUCTURING-BIND` `&AUX` parameter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DestructureAuxiliaryParameter {
    /// Auxiliary binding name.
    pub name: String,
    /// Function containing the init-form.
    pub default_function: FunctionId,
}

/// A compiled destructuring lambda list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DestructureLambdaList {
    /// Optional `&WHOLE` binding name.
    pub whole: Option<String>,
    /// Required binding patterns.
    pub required: Vec<DestructurePattern>,
    /// Optional parameters.
    pub optional: Vec<DestructureOptionalParameter>,
    /// Keyword parameters.
    pub keywords: Vec<DestructureKeywordParameter>,
    /// Whether a `&KEY` section was present.
    pub has_keyword_section: bool,
    /// Whether unknown keywords are accepted.
    pub allow_other_keys: bool,
    /// Optional rest binding name.
    pub rest: Option<String>,
    /// Auxiliary parameters.
    pub auxiliary: Vec<DestructureAuxiliaryParameter>,
}

/// The two forms accepted by the `DESTRUCTURING-BIND` bytecode operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DestructureSpec {
    /// A single recursive pattern.
    Pattern(DestructurePattern),
    /// A complete destructuring lambda list.
    LambdaList(DestructureLambdaList),
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum DestructureLambdaListSection {
    Required,
    Optional,
    Rest,
    Keyword,
    Auxiliary,
}

#[derive(Clone, Debug, PartialEq)]
/// A stack-bytecode instruction emitted by the compiler.
pub enum Instruction {
    /// Push a literal constant.
    Constant(Constant),
    /// Push a quoted form.
    Quote(Form),
    /// Push a quasiquoted form.
    QuasiQuote(Form),
    /// Load a symbol by normal name resolution.
    Load(String),
    /// Load an escaped symbol.
    LoadExact(String),
    /// Load a function by normal name resolution.
    FunctionLoad(String),
    /// Load an escaped function name.
    FunctionLoadExact(String),
    /// Test whether a variable is bound.
    IsBound(String),
    /// Test whether an escaped variable is bound.
    IsBoundExact(String),
    /// Define a variable.
    Define(String),
    /// Define an escaped variable.
    DefineExact(String),
    /// Define a function.
    DefineFunction(String),
    /// Define an escaped function name.
    DefineFunctionExact(String),
    /// Define a special variable.
    DefineSpecial {
        /// Variable name.
        name: String,
        /// Whether to force special binding semantics.
        force: bool,
    },
    /// Define an escaped special variable.
    DefineSpecialExact {
        /// Escaped variable name.
        name: String,
        /// Whether to force special binding semantics.
        force: bool,
    },
    /// Define multiple values.
    DefineValues(String),
    /// Define multiple values using escaped names.
    DefineValuesExact(String),
    /// Set a variable.
    Set(String),
    /// Set an escaped variable.
    SetExact(String),
    /// Perform a `SETF` update.
    Setf(Form),
    /// Perform a place update with `MAP-INTO` semantics.
    MapIntoSetf(Form),
    /// Perform parallel assignment.
    Psetq(Vec<String>),
    /// Perform escaped parallel assignment.
    PsetqExact(Vec<(String, bool)>),
    /// Bind multiple-value assignment targets.
    MultipleValueSetq(Vec<String>),
    /// Bind escaped multiple-value assignment targets.
    MultipleValueSetqExact(Vec<(String, bool)>),
    /// Enter a lexical scope.
    EnterScope,
    /// Exit a lexical scope.
    ExitScope,
    /// Discard the top stack value.
    Pop,
    /// Duplicate the top stack value.
    Dup,
    /// Replace the stack with the primary value.
    Primary,
    /// Construct a multiple-value carrier.
    Values(usize),
    /// Convert a multiple-value carrier to a list.
    MultipleValueList,
    /// Bind multiple values to names.
    BindValues(Vec<String>),
    /// Bind multiple values to escaped names.
    BindValuesExact(Vec<(String, bool)>),
    /// Destructure a value.
    Destructure(DestructureSpec),
    /// Branch when the top value is false.
    JumpIfFalse(usize),
    /// Unconditional branch.
    Jump(usize),
    /// Create a closure for a nested function.
    MakeClosure(FunctionId),
    /// Evaluate a function while ignoring conditions.
    IgnoreErrors(FunctionId),
    /// Run a body with condition handlers selected by type.
    HandlerCase {
        /// Protected function.
        protected: FunctionId,
        /// Handler clauses.
        clauses: Vec<HandlerCaseClause>,
    },
    /// Install dynamically scoped handlers around a body.
    HandlerBind {
        /// Body function.
        body: FunctionId,
        /// Handler clauses.
        handlers: Vec<HandlerBindClause>,
    },
    /// Install dynamically scoped restarts around a body.
    RestartBind {
        /// Body function.
        body: FunctionId,
        /// Restart bindings.
        bindings: Vec<RestartBindClause>,
    },
    /// Catch a matching tag from a body.
    Catch {
        /// Tag-producing function.
        tag: FunctionId,
        /// Body function.
        body: FunctionId,
    },
    /// Establish a simple restart around a body.
    WithSimpleRestart {
        /// Restart name.
        name: String,
        /// Body function.
        body: FunctionId,
    },
    /// Establish restarts associated with a condition.
    WithConditionRestarts {
        /// Condition function.
        condition: FunctionId,
        /// Restart list function.
        restarts: FunctionId,
        /// Body function.
        body: FunctionId,
    },
    /// Run a body with restart-case clauses.
    RestartCase {
        /// Protected function.
        protected: FunctionId,
        /// Restart clauses.
        clauses: Vec<RestartCaseClause>,
    },
    /// Bind a dynamic set of special variables around a body.
    Progv {
        /// Symbols function.
        symbols: FunctionId,
        /// Values function.
        values: FunctionId,
        /// Body function.
        body: FunctionId,
    },
    /// Throw the current tag and values.
    Throw,
    /// Establish a named non-local return target.
    Block {
        /// Body function.
        function: FunctionId,
        /// Block name.
        name: String,
    },
    /// Establish a tagbody control-flow region.
    TagBody {
        /// Body function.
        function: FunctionId,
        /// Tag-to-offset mapping.
        tags: Vec<(String, usize)>,
    },
    /// Run cleanup even when protected evaluation exits non-locally.
    UnwindProtect {
        /// Protected function.
        protected: FunctionId,
        /// Cleanup function.
        cleanup: FunctionId,
    },
    /// Return from a named block.
    ReturnFrom {
        /// Block name.
        name: String,
    },
    /// Transfer control to a tagbody tag.
    Go {
        /// Tag name.
        tag: String,
    },
    /// Evaluate a compiled source span.
    Eval(Span),
    /// Call a function with positional arguments.
    Call(usize),
    /// Apply a final list of arguments.
    Apply(usize),
    /// Map a function over one or more sequences.
    MapCar(usize),
    /// Call a function with multiple-value arguments.
    MultipleValueCall(usize),
    /// Return from the current function.
    Return,
}

/// The bytecode and metadata for one callable function.
#[derive(Clone, Debug, PartialEq)]
pub struct FunctionCode {
    /// Optional function name.
    pub name: Option<String>,
    /// Required parameter names.
    pub parameters: Vec<String>,
    /// Escaping flags for required parameters.
    pub required_escaped: Vec<bool>,
    /// Optional parameters.
    pub optional: Vec<OptionalParameter>,
    /// Keyword parameters.
    pub keywords: Vec<KeywordParameter>,
    /// Whether a keyword section was present.
    pub has_keyword_section: bool,
    /// Whether unknown keywords are accepted.
    pub allow_other_keys: bool,
    /// Optional rest parameter.
    pub rest: Option<String>,
    /// Whether the rest name was escaped.
    pub rest_escaped: bool,
    /// Auxiliary parameters.
    pub auxiliary: Vec<AuxiliaryParameter>,
    /// Stack bytecode instructions.
    pub instructions: Vec<Instruction>,
}

/// A compiled entry function and its nested function bodies.
#[derive(Clone, Debug, PartialEq)]
pub struct Program {
    /// All function bodies, including nested functions.
    pub functions: Vec<FunctionCode>,
    /// Index of the entry function in [`Self::functions`].
    pub entry: FunctionId,
}

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

#[path = "compiler_state.rs"]
mod state;
#[allow(clippy::wildcard_imports)]
use state::*;
#[path = "compiler_branching.rs"]
mod branching;
#[path = "compiler_compilation.rs"]
mod compilation;
#[path = "compiler_control_forms.rs"]
mod control_forms;
#[path = "compiler_logical_forms.rs"]
mod logical_forms;
#[path = "compiler_parameters.rs"]
mod parameters;
#[path = "compiler_runtime_definitions.rs"]
mod runtime_definitions;
#[path = "compiler_validation.rs"]
mod validation;

#[path = "compiler_destructuring.rs"]
mod destructuring;

impl CompileState {
    #[allow(clippy::too_many_lines)]
    fn compile_let(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        sequential: bool,
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(CompileError::new(
                CompileErrorKind::Arity {
                    operator: if sequential { "LET*" } else { "LET" }.to_string(),
                    expected: "at least one".to_string(),
                    actual: items.len().saturating_sub(1),
                },
                operator_span(items, span),
            ));
        }
        let Some(binding_form) = items.get(1) else {
            return Err(Self::internal_error(
                span,
                "missing let bindings after arity check",
            ));
        };
        let FormKind::List(bindings) = &binding_form.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedList {
                    context: "let bindings".to_string(),
                },
                binding_form.span,
            ));
        };

        let mut parsed = Vec::with_capacity(bindings.len());
        let mut names = HashSet::new();
        for binding in bindings {
            let FormKind::List(binding_items) = &binding.kind else {
                return Err(CompileError::new(
                    CompileErrorKind::ExpectedList {
                        context: "let binding".to_string(),
                    },
                    binding.span,
                ));
            };
            if !(binding_items.len() == 1 || binding_items.len() == 2) {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "let binding needs a name and optional value".to_string(),
                    },
                    binding.span,
                ));
            }
            let Some(name_form) = binding_items.first() else {
                return Err(Self::internal_error(
                    binding.span,
                    "missing let binding name",
                ));
            };
            let name = Self::symbol_name(name_form, "let binding name")?;
            if !sequential && !names.insert(name.clone()) {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "let bindings must have distinct names".to_string(),
                    },
                    name_form.span,
                ));
            }
            parsed.push((name, binding_items.get(1)));
        }

        self.emit(function, Instruction::EnterScope, binding_form.span)?;
        if sequential {
            for (name, value) in &parsed {
                if let Some(value) = value {
                    self.compile_expression(function, value)?;
                } else {
                    self.emit(
                        function,
                        Instruction::Constant(Constant::Nil),
                        binding_form.span,
                    )?;
                }
                self.emit(
                    function,
                    Instruction::Define(name.clone()),
                    binding_form.span,
                )?;
                self.emit(function, Instruction::Pop, binding_form.span)?;
            }
        } else {
            for (_, value) in &parsed {
                if let Some(value) = value {
                    self.compile_expression(function, value)?;
                } else {
                    self.emit(
                        function,
                        Instruction::Constant(Constant::Nil),
                        binding_form.span,
                    )?;
                }
            }
            for (name, _) in parsed.iter().rev() {
                self.emit(
                    function,
                    Instruction::Define(name.clone()),
                    binding_form.span,
                )?;
                self.emit(function, Instruction::Pop, binding_form.span)?;
            }
        }

        let body = items.get(2..).unwrap_or(&[]);
        self.compile_sequence(function, body)?;
        self.emit(function, Instruction::ExitScope, span)?;
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    fn compile_flet(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        recursive: bool,
    ) -> Result<(), CompileError> {
        let operator = if recursive { "LABELS" } else { "FLET" };
        if items.len() < 2 {
            return Err(CompileError::new(
                CompileErrorKind::Arity {
                    operator: operator.to_string(),
                    expected: "at least one".to_string(),
                    actual: items.len().saturating_sub(1),
                },
                operator_span(items, span),
            ));
        }
        let Some(binding_form) = items.get(1) else {
            return Err(Self::internal_error(
                span,
                "missing local function bindings after arity check",
            ));
        };
        let FormKind::List(bindings) = &binding_form.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedList {
                    context: "local function bindings".to_string(),
                },
                binding_form.span,
            ));
        };

        let mut parsed = Vec::with_capacity(bindings.len());
        let mut names = HashSet::new();
        for binding in bindings {
            let FormKind::List(parts) = &binding.kind else {
                return Err(CompileError::new(
                    CompileErrorKind::ExpectedList {
                        context: "local function binding".to_string(),
                    },
                    binding.span,
                ));
            };
            if parts.len() < 3 {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "local function needs a name, parameters, and a body".to_string(),
                    },
                    binding.span,
                ));
            }
            let Some(name_form) = parts.first() else {
                return Err(Self::internal_error(
                    binding.span,
                    "missing local function name after arity check",
                ));
            };
            let (name, name_escaped) = Self::symbol_name_info(name_form, "local function name")?;
            let local_key = Self::local_function_key(&name, name_escaped);
            if !names.insert(local_key) {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "local function names must be unique".to_string(),
                    },
                    name_form.span,
                ));
            }
            let Some(parameter_form) = parts.get(1) else {
                return Err(Self::internal_error(
                    binding.span,
                    "missing local function parameters after arity check",
                ));
            };
            let lambda_list = Self::parameters(parameter_form)?;
            parsed.push((name, name_escaped, lambda_list, parts[2..].to_vec()));
        }

        if recursive {
            self.emit(function, Instruction::EnterScope, binding_form.span)?;
            self.local_function_scopes.push(names.clone());
        }
        for (name, _name_escaped, lambda_list, body) in &parsed {
            let child = self.reserve_function_with_rest(
                Some(name.clone()),
                lambda_list.required.clone(),
                lambda_list.required_escaped.clone(),
                lambda_list.rest.clone(),
                lambda_list.rest_escaped,
            );
            let optional = self.compile_optional_parameters(&lambda_list.optional)?;
            self.functions[child].optional = optional;
            let keywords = self.compile_keyword_parameters(&lambda_list.keywords)?;
            self.functions[child].keywords = keywords;
            self.functions[child].has_keyword_section = lambda_list.has_keyword_section;
            self.functions[child].allow_other_keys = lambda_list.allow_other_keys;
            let auxiliary = self.compile_auxiliary_parameters(&lambda_list.auxiliary)?;
            self.functions[child].auxiliary = auxiliary;
            self.compile_sequence(child, body)?;
            self.emit(child, Instruction::Return, span)?;
            self.emit(function, Instruction::MakeClosure(child), span)?;
        }
        if !recursive {
            self.emit(function, Instruction::EnterScope, binding_form.span)?;
        }
        for (name, name_escaped, _, _) in parsed.iter().rev() {
            let instruction = if *name_escaped {
                Instruction::DefineFunctionExact(name.clone())
            } else {
                Instruction::DefineFunction(name.clone())
            };
            self.emit(function, instruction, binding_form.span)?;
        }

        let body = items.get(2..).unwrap_or(&[]);
        if !recursive {
            self.local_function_scopes.push(names);
        }
        self.compile_sequence(function, body)?;
        self.local_function_scopes.pop();
        self.emit(function, Instruction::ExitScope, span)?;
        Ok(())
    }
}

#[path = "compiler_forms.rs"]
mod forms;
#[path = "compiler_helpers.rs"]
mod helpers;
#[allow(clippy::wildcard_imports)]
use helpers::*;
