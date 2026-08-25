use std::collections::HashSet;
use std::error::Error;
use std::fmt;

mod destructuring;

use ncl_syntax::{
    Form, FormKind, LambdaListAuxiliaryParameter, LambdaListErrorKind, LambdaListKeywordParameter,
    LambdaListOptionalParameter, OrdinaryLambdaList, Span, SymbolTokenKind,
    parse_float_literal, parse_ordinary_lambda_list, parse_radix_integer_literal, parse_symbol_token,
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

/// Metadata for one compiled `&OPTIONAL` parameter.
#[derive(Clone, Debug, PartialEq)]
pub struct OptionalParameter {
    pub name: String,
    pub name_escaped: bool,
    pub default_function: FunctionId,
    pub supplied_p: Option<String>,
    pub supplied_p_escaped: Option<bool>,
}

/// Metadata for one compiled `&KEY` parameter.
#[derive(Clone, Debug, PartialEq)]
pub struct KeywordParameter {
    pub keyword_name: String,
    pub keyword_name_escaped: bool,
    pub name: String,
    pub name_escaped: bool,
    pub default_function: FunctionId,
    pub supplied_p: Option<String>,
    pub supplied_p_escaped: Option<bool>,
}

/// Metadata for one compiled `&AUX` parameter.
#[derive(Clone, Debug, PartialEq)]
pub struct AuxiliaryParameter {
    pub name: String,
    pub name_escaped: bool,
    pub default_function: FunctionId,
}

/// One compiled `HANDLER-CASE` clause.
#[derive(Clone, Debug, PartialEq)]
pub struct HandlerCaseClause {
    pub condition: String,
    pub variable: Option<String>,
    pub function: FunctionId,
    pub no_error: bool,
    pub variable_count: usize,
}

/// One compiled `HANDLER-BIND` clause.
#[derive(Clone, Debug, PartialEq)]
pub struct HandlerBindClause {
    pub condition: String,
    pub function: FunctionId,
}

/// One compiled `RESTART-BIND` clause.
#[derive(Clone, Debug, PartialEq)]
pub struct RestartBindClause {
    pub name: String,
    pub function: FunctionId,
}

/// One compiled `RESTART-CASE` clause.
#[derive(Clone, Debug, PartialEq)]
pub struct RestartCaseClause {
    pub name: String,
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
/// carrier stack entry, `NthValue` selects from one carrier, and
/// `MultipleValueList` converts one carrier to a list, while `BindValues` and
/// `Destructure` consume one carrier without pushing a result.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BindingName {
    pub name: String,
    pub escaped: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DestructurePattern {
    Name(BindingName),
    List(Vec<Self>),
    Dotted { items: Vec<Self>, tail: Box<Self> },
}

/// One compiled `DESTRUCTURING-BIND` `&OPTIONAL` parameter.
#[derive(Clone, Debug, PartialEq)]
pub struct DestructureOptionalParameter {
    pub pattern: DestructurePattern,
    pub default_function: FunctionId,
    pub supplied_p: Option<BindingName>,
}

/// One compiled `DESTRUCTURING-BIND` `&KEY` parameter.
#[derive(Clone, Debug, PartialEq)]
pub struct DestructureKeywordParameter {
    pub keyword_name: String,
    pub keyword_name_escaped: bool,
    pub pattern: DestructurePattern,
    pub default_function: FunctionId,
    pub supplied_p: Option<BindingName>,
}

/// One compiled `DESTRUCTURING-BIND` `&AUX` parameter.
#[derive(Clone, Debug, PartialEq)]
pub struct DestructureAuxiliaryParameter {
    pub name: BindingName,
    pub default_function: FunctionId,
}

/// A compiled destructuring lambda list.
#[derive(Clone, Debug, PartialEq)]
pub struct DestructureLambdaList {
    pub whole: Option<BindingName>,
    pub required: Vec<DestructurePattern>,
    pub optional: Vec<DestructureOptionalParameter>,
    pub keywords: Vec<DestructureKeywordParameter>,
    pub has_keyword_section: bool,
    pub allow_other_keys: bool,
    pub rest: Option<BindingName>,
    pub auxiliary: Vec<DestructureAuxiliaryParameter>,
}

/// The two forms accepted by the `DESTRUCTURING-BIND` bytecode operation.
#[derive(Clone, Debug, PartialEq)]
pub enum DestructureSpec {
    Pattern(DestructurePattern),
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
pub enum Instruction {
    Constant(Constant),
    Quote(Form),
    QuasiQuote(Form),
    Load(String),
    LoadExact(String),
    FunctionLoad(String),
    FunctionLoadExact(String),
    FunctionCallLoad(String),
    FunctionCallLoadExact(String),
    SetfFunctionLoad(String),
    IsBound(String),
    IsBoundExact(String),
    Define(String),
    DefineExact(String),
    DefineFunction(String),
    DefineFunctionExact(String),
    DefineSpecial {
        name: String,
        force: bool,
    },
    DefineSpecialExact {
        name: String,
        force: bool,
    },
    DefineDynamic(String),
    DefineDynamicExact(String),
    DeclareSpecial {
        names: Vec<String>,
        exact_names: Vec<String>,
    },
    DefineValues(String),
    DefineValuesExact(String),
    Set(String),
    SetExact(String),
    Setf(Form),
    MapIntoSetf(Form),
    Psetq(Vec<String>),
    PsetqExact(Vec<(String, bool)>),
    Push(String),
    PushExact(String),
    PopPlace(String),
    PopPlaceExact(String),
    PushNew(String),
    PushNewExact(String),
    Rotatef(Vec<String>),
    RotatefExact(Vec<(String, bool)>),
    Shiftf(Vec<String>),
    ShiftfExact(Vec<(String, bool)>),
    MultipleValueSetq(Vec<String>),
    MultipleValueSetqExact(Vec<(String, bool)>),
    EnterScope,
    EnterMacroletEnvironment(Form),
    ExitScope,
    EnterSpecialScope {
        names: Vec<String>,
        exact_names: Vec<String>,
    },
    ExitSpecialScope,
    Pop,
    Dup,
    Primary,
    Values(usize),
    NthValue(Span),
    MultipleValueList,
    BindValues(Vec<String>),
    BindValuesExact(Vec<(String, bool)>),
    Destructure(DestructureSpec),
    JumpIfFalse(usize),
    Jump(usize),
    MakeClosure(FunctionId),
    IgnoreErrors(FunctionId),
    HandlerCase {
        protected: FunctionId,
        clauses: Vec<HandlerCaseClause>,
    },
    HandlerBind {
        body: FunctionId,
        handlers: Vec<HandlerBindClause>,
    },
    RestartBind {
        body: FunctionId,
        bindings: Vec<RestartBindClause>,
    },
    Catch {
        tag: FunctionId,
        body: FunctionId,
    },
    WithSimpleRestart {
        name: String,
        body: FunctionId,
    },
    WithConditionRestarts {
        condition: FunctionId,
        restarts: FunctionId,
        body: FunctionId,
    },
    RestartCase {
        protected: FunctionId,
        clauses: Vec<RestartCaseClause>,
    },
    Progv {
        symbols: FunctionId,
        values: FunctionId,
        body: FunctionId,
    },
    Throw,
    Block {
        function: FunctionId,
        name: String,
    },
    TagBody {
        function: FunctionId,
        tags: Vec<(String, usize)>,
    },
    UnwindProtect {
        protected: FunctionId,
        cleanup: FunctionId,
    },
    ReturnFrom {
        name: String,
    },
    Go {
        tag: String,
    },
    Eval(Span),
    EvalWithEnvironment(Span),
    Call(usize),
    Apply(usize),
    MapCar(usize),
    MultipleValueCall(usize),
    Return,
}

/// The bytecode and metadata for one callable function.
#[derive(Clone, Debug, PartialEq)]
pub struct FunctionCode {
    pub name: Option<String>,
    pub documentation: Option<String>,
    pub parameters: Vec<String>,
    pub required_escaped: Vec<bool>,
    pub optional: Vec<OptionalParameter>,
    pub keywords: Vec<KeywordParameter>,
    pub has_keyword_section: bool,
    pub allow_other_keys: bool,
    pub rest: Option<String>,
    pub rest_escaped: bool,
    pub auxiliary: Vec<AuxiliaryParameter>,
    pub special_names: Vec<String>,
    pub special_exact_names: Vec<String>,
    pub instructions: Vec<Instruction>,
}

/// A compiled entry function and its nested function bodies.
#[derive(Clone, Debug, PartialEq)]
pub struct Program {
    pub functions: Vec<FunctionCode>,
    pub entry: FunctionId,
}

/// The category of a compile-time error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompileErrorKind {
    Arity {
        operator: String,
        expected: String,
        actual: usize,
    },
    ExpectedList {
        context: String,
    },
    ExpectedSymbol {
        context: String,
    },
    InvalidForm {
        message: String,
    },
    UnsupportedForm {
        message: String,
    },
    Internal {
        message: String,
    },
}

/// A typed compiler error tied to the source span that caused it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileError {
    pub kind: CompileErrorKind,
    pub span: Span,
}

impl CompileError {
    pub fn new(kind: CompileErrorKind, span: Span) -> Self {
        Self { kind, span }
    }
}

impl fmt::Display for CompileErrorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Arity {
                operator,
                expected,
                actual,
            } => write!(
                formatter,
                "{operator} expected {expected} arguments, received {actual}"
            ),
            Self::ExpectedList { context } => write!(formatter, "{context} must be a list"),
            Self::ExpectedSymbol { context } => {
                write!(formatter, "{context} must be a symbol")
            }
            Self::InvalidForm { message }
            | Self::UnsupportedForm { message }
            | Self::Internal { message } => formatter.write_str(message),
        }
    }
}

impl fmt::Display for CompileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} at byte {}..{}",
            self.kind, self.span.start, self.span.end
        )
    }
}

impl Error for CompileError {}

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

#[derive(Default)]
struct CompileState {
    functions: Vec<FunctionCode>,
    local_function_scopes: Vec<HashSet<String>>,
    used_names: HashSet<String>,
    temporary_counter: usize,
}

impl CompileState {
    fn is_local_function(&self, name: &str) -> bool {
        self.local_function_scopes
            .iter()
            .rev()
            .any(|scope| scope.contains(name))
    }

    fn reserve_function(&mut self, name: Option<String>, parameters: Vec<String>) -> FunctionId {
        let required_escaped = vec![false; parameters.len()];
        self.reserve_function_with_rest(name, parameters, required_escaped, None, false)
    }

    fn reserve_function_with_rest(
        &mut self,
        name: Option<String>,
        parameters: Vec<String>,
        required_escaped: Vec<bool>,
        rest: Option<String>,
        rest_escaped: bool,
    ) -> FunctionId {
        let function = self.functions.len();
        self.functions.push(FunctionCode {
            name,
            documentation: None,
            parameters,
            required_escaped,
            optional: Vec::new(),
            keywords: Vec::new(),
            has_keyword_section: false,
            allow_other_keys: false,
            rest,
            rest_escaped,
            auxiliary: Vec::new(),
            special_names: Vec::new(),
            special_exact_names: Vec::new(),
            instructions: Vec::new(),
        });
        function
    }

    fn local_function_key(name: &str, escaped: bool) -> String {
        if escaped {
            format!("\0{name}")
        } else {
            normalize_name(name)
        }
    }

    fn emit(
        &mut self,
        function: FunctionId,
        instruction: Instruction,
        span: Span,
    ) -> Result<usize, CompileError> {
        let Some(code) = self.functions.get_mut(function) else {
            return Err(self.internal_error(span, "invalid function id while emitting bytecode"));
        };
        let position = code.instructions.len();
        code.instructions.push(instruction);
        Ok(position)
    }

    fn instruction_count(&self, function: FunctionId, span: Span) -> Result<usize, CompileError> {
        self.functions
            .get(function)
            .map(|code| code.instructions.len())
            .ok_or_else(|| self.internal_error(span, "invalid function id while reading bytecode"))
    }

    fn patch_jump(
        &mut self,
        function: FunctionId,
        instruction: usize,
        target: usize,
        span: Span,
    ) -> Result<(), CompileError> {
        let Some(code) = self.functions.get_mut(function) else {
            return Err(self.internal_error(span, "invalid function id while patching jump"));
        };
        let Some(operation) = code.instructions.get_mut(instruction) else {
            return Err(self.internal_error(span, "invalid jump instruction position"));
        };
        match operation {
            Instruction::JumpIfFalse(value) | Instruction::Jump(value) => {
                *value = target;
                Ok(())
            }
            _ => Err(self.internal_error(span, "attempted to patch a non-jump instruction")),
        }
    }

    fn collect_names(&mut self, forms: &[Form]) {
        for form in forms {
            self.collect_form_names(form);
        }
    }

    fn collect_form_names(&mut self, form: &Form) {
        match &form.kind {
            FormKind::Atom(name) => {
                self.used_names.insert(normalize_name(name));
            }
            FormKind::List(items) | FormKind::Vector(items) => {
                for item in items {
                    self.collect_form_names(item);
                }
            }
            FormKind::DottedList { items, tail } => {
                for item in items {
                    self.collect_form_names(item);
                }
                self.collect_form_names(tail);
            }
            FormKind::Complex { real, imaginary } => {
                self.collect_form_names(real);
                self.collect_form_names(imaginary);
            }
            FormKind::ReadTimeEval(inner) => self.collect_form_names(inner),
            FormKind::String(_) | FormKind::Character(_) | FormKind::BitVector(_) => {}
        }
    }

    fn fresh_name(&mut self, prefix: &str) -> String {
        loop {
            let candidate = format!("__NCL_{prefix}_{}", self.temporary_counter);
            self.temporary_counter += 1;
            if self.used_names.insert(candidate.clone()) {
                return candidate;
            }
        }
    }

    fn compile_sequence(
        &mut self,
        function: FunctionId,
        forms: &[Form],
    ) -> Result<(), CompileError> {
        if forms.is_empty() {
            self.emit(
                function,
                Instruction::Constant(Constant::Nil),
                Span::new(0, 0),
            )?;
            return Ok(());
        }

        for (index, form) in forms.iter().enumerate() {
            self.compile_expression(function, form)?;
            if index + 1 < forms.len() {
                self.emit(function, Instruction::Pop, form.span)?;
            }
        }
        Ok(())
    }

    fn compile_expression(
        &mut self,
        function: FunctionId,
        form: &Form,
    ) -> Result<(), CompileError> {
        match &form.kind {
            FormKind::Atom(atom) => {
                if let Some(constant) = literal_constant(atom) {
                    self.emit(function, Instruction::Constant(constant), form.span)?;
                } else if let Some((name, escaped)) = symbol_reference(atom) {
                    let instruction = if escaped {
                        Instruction::LoadExact(name)
                    } else {
                        Instruction::Load(name)
                    };
                    self.emit(function, instruction, form.span)?;
                } else {
                    self.emit(function, Instruction::Load(normalize_name(atom)), form.span)?;
                }
            }
            FormKind::String(value) => {
                self.emit(
                    function,
                    Instruction::Constant(Constant::String(value.clone())),
                    form.span,
                )?;
            }
            FormKind::Character(value) => {
                self.emit(
                    function,
                    Instruction::Constant(Constant::Character(*value)),
                    form.span,
                )?;
            }
            FormKind::Complex { .. } | FormKind::Vector(_) | FormKind::BitVector(_) => {
                self.emit(function, Instruction::Quote(form.clone()), form.span)?;
            }
            FormKind::ReadTimeEval(_) => {
                return Err(CompileError::new(
                    CompileErrorKind::UnsupportedForm {
                        message: "read-time evaluation must be resolved before compilation"
                            .to_string(),
                    },
                    form.span,
                ));
            }
            FormKind::DottedList { .. } => {
                return Err(CompileError::new(
                    CompileErrorKind::UnsupportedForm {
                        message: "dotted lists cannot be evaluated".to_string(),
                    },
                    form.span,
                ));
            }
            FormKind::List(items) => self.compile_list(function, form.span, items)?,
        }
        Ok(())
    }

    fn compile_list(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        let Some(operator) = items.first() else {
            self.emit(function, Instruction::Constant(Constant::Nil), span)?;
            return Ok(());
        };

        let operator_name = match &operator.kind {
            FormKind::Atom(name) => special_operator_name(name),
            _ => None,
        };
        if let Some(name) = operator_name.as_deref() {
            match name {
                "QUOTE" => return self.compile_quote(function, span, items),
                "QUASIQUOTE" => return self.compile_quasiquote(function, span, items),
                "DECLARE" => return self.compile_declare(function, span, items, false, false),
                "LOCALLY" => return self.compile_locally(function, span, items),
                "EVAL-WHEN" => return self.compile_eval_when(function, span, items),
                "LOAD-TIME-VALUE" => {
                    return self.compile_runtime_definition(function, span, items);
                }
                "NTH-VALUE" => return self.compile_nth_value(function, span, items),
                "DECLAIM" => return self.compile_declare(function, span, items, true, false),
                "PROCLAIM" => return self.compile_declare(function, span, items, true, true),
                "THE" => return self.compile_the(function, span, items),
                "IF" => return self.compile_if(function, span, items),
                "PROGN" => return self.compile_progn(function, items),
                "PROG1" => return self.compile_prog1(function, span, items),
                "PROG2" => return self.compile_prog2(function, span, items),
                "PROG" => return self.compile_prog(function, span, items, false),
                "PROG*" => return self.compile_prog(function, span, items, true),
                "VALUES" => return self.compile_values(function, span, items),
                "IGNORE-ERRORS" => return self.compile_ignore_errors(function, span, items),
                "HANDLER-CASE" => return self.compile_handler_case(function, span, items),
                "HANDLER-BIND" => return self.compile_handler_bind(function, span, items),
                "RESTART-BIND" => return self.compile_restart_bind(function, span, items),
                "CATCH" => return self.compile_catch(function, span, items),
                "WITH-SIMPLE-RESTART" => {
                    return self.compile_with_simple_restart(function, span, items);
                }
                "WITH-CONDITION-RESTARTS" => {
                    return self.compile_with_condition_restarts(function, span, items);
                }
                "WITH-OPEN-FILE" => {
                    return self.compile_with_open_file(function, span, items);
                }
                "WITH-OPEN-STREAM" => {
                    return self.compile_with_open_stream(function, span, items);
                }
                "WITH-OUTPUT-TO-STRING" => {
                    return self.compile_with_output_to_string(function, span, items);
                }
                "WITH-INPUT-FROM-STRING" => {
                    return self.compile_with_input_from_string(function, span, items);
                }
                "WITH-HASH-TABLE-ITERATOR" => {
                    return self.compile_with_hash_table_iterator(function, span, items);
                }
                "RESTART-CASE" => return self.compile_restart_case(function, span, items),
                "PROGV" => return self.compile_progv(function, span, items),
                "THROW" => return self.compile_throw(function, span, items),
                "UNWIND-PROTECT" => {
                    return self.compile_unwind_protect(function, span, items);
                }
                "BLOCK" => return self.compile_block(function, span, items),
                "RETURN" => return self.compile_return(function, span, items),
                "RETURN-FROM" => return self.compile_return_from(function, span, items),
                "TAGBODY" => return self.compile_tagbody(function, span, items),
                "GO" => return self.compile_go(function, span, items),
                "MULTIPLE-VALUE-BIND" => {
                    return self.compile_multiple_value_bind(function, span, items);
                }
                "MULTIPLE-VALUE-CALL" => {
                    return self.compile_multiple_value_call(function, span, items);
                }
                "MULTIPLE-VALUE-LIST" => {
                    return self.compile_multiple_value_list(function, span, items);
                }
                "MULTIPLE-VALUE-PROG1" => {
                    return self.compile_multiple_value_prog1(function, span, items);
                }
                "AND" => return self.compile_and(function, span, items),
                "OR" => return self.compile_or(function, span, items),
                "WHEN" => return self.compile_when(function, span, items, true),
                "UNLESS" => return self.compile_when(function, span, items, false),
                "COND" => return self.compile_cond(function, span, items),
                "CASE" | "ECASE" => return self.compile_case(function, span, items),
                "CCASE" | "CTYPECASE" => {
                    return self.compile_runtime_definition(function, span, items);
                }
                "TYPECASE" | "ETYPECASE" => {
                    return self.compile_typecase(function, span, items);
                }
                "LAMBDA" => return self.compile_lambda(function, span, items),
                "FUNCTION" => return self.compile_function(function, span, items),
                "DEFINE" => return self.compile_define(function, span, items),
                "DEFINE-SYMBOL-MACRO" => {
                    return self.compile_runtime_definition(function, span, items);
                }
                "MACROEXPAND-1" | "MACROEXPAND" | "NCL-MACRO-ENVIRONMENT" => {
                    return self.compile_runtime_definition(function, span, items);
                }
                "NCL-MACROLET-ENVIRONMENT" => {
                    return self.compile_macrolet_environment(function, span, items);
                }
                "DEFUN" => return self.compile_defun(function, span, items),
                "SETQ" => return self.compile_setq(function, span, items),
                "PSETQ" => return self.compile_psetq(function, span, items),
                "MULTIPLE-VALUE-SETQ" => {
                    return self.compile_multiple_value_setq(function, span, items);
                }
                "SETF" => return self.compile_setf(function, span, items),
                "PSETF" => return self.compile_psetf(function, span, items),
                "PUSH" => return self.compile_push(function, span, items),
                "POP" => return self.compile_pop(function, span, items),
                "PUSHNEW" => return self.compile_pushnew(function, span, items),
                "ROTATEF" => return self.compile_rotatef(function, span, items),
                "SHIFTF" => return self.compile_shiftf(function, span, items),
                "INCF" => {
                    if matches!(
                        items.get(1).map(|place| &place.kind),
                        Some(FormKind::Atom(_))
                    ) {
                        return self.compile_modify_symbol(function, span, items, "INCF", "+");
                    }
                    return self.compile_runtime_definition(function, span, items);
                }
                "DECF" => {
                    if matches!(
                        items.get(1).map(|place| &place.kind),
                        Some(FormKind::Atom(_))
                    ) {
                        return self.compile_modify_symbol(function, span, items, "DECF", "-");
                    }
                    return self.compile_runtime_definition(function, span, items);
                }
                "DEFVAR" => return self.compile_defvar(function, span, items, false),
                "DEFPARAMETER" => return self.compile_defvar(function, span, items, true),
                "DEFCONSTANT" => return self.compile_runtime_definition(function, span, items),
                "DEFSTRUCT" => return self.compile_defstruct(function, span, items),
                "DEFCLASS" | "DEFGENERIC" | "DEFMETHOD" | "DEFINE-CONDITION" => {
                    return self.compile_runtime_definition(function, span, items);
                }
                "DEFSETF"
                | "DEFINE-MODIFY-MACRO"
                | "DEFINE-SETF-EXPANDER"
                | "GET-SETF-EXPANSION" => {
                    return self.compile_runtime_definition(function, span, items);
                }
                "EVAL" => return self.compile_eval(function, span, items),
                "FUNCALL" => return self.compile_funcall(function, span, items),
                "APPLY" => return self.compile_apply(function, span, items),
                "MAP-INTO" => return self.compile_map_into(function, span, items),
                "MAPHASH" => return self.compile_maphash(function, span, items),
                "MAPCAR" => return self.compile_mapcar(function, span, items),
                "DESTRUCTURING-BIND" => {
                    return self.compile_destructuring_bind(function, span, items);
                }
                "LET" => return self.compile_let(function, span, items, false),
                "LET*" => return self.compile_let(function, span, items, true),
                "FLET" => return self.compile_flet(function, span, items, false),
                "LABELS" => return self.compile_flet(function, span, items, true),
                "DOTIMES" => return self.compile_dotimes(function, span, items),
                "DOLIST" => return self.compile_dolist(function, span, items),
                "DO" => return self.compile_do(function, span, items, false),
                "DO*" => return self.compile_do(function, span, items, true),
                _ => {}
            }
        }

        if let FormKind::Atom(name) = &operator.kind {
            let (reference_name, escaped) =
                symbol_reference(name).unwrap_or_else(|| (normalize_name(name), false));
            self.emit(
                function,
                if escaped {
                    Instruction::FunctionCallLoadExact(reference_name)
                } else {
                    Instruction::FunctionCallLoad(reference_name)
                },
                operator.span,
            )?;
            for item in items.iter().skip(1) {
                self.compile_expression(function, item)?;
            }
        } else {
            for item in items {
                self.compile_expression(function, item)?;
            }
        }
        self.emit(
            function,
            Instruction::Call(items.len().saturating_sub(1)),
            span,
        )?;
        Ok(())
    }

    fn compile_quote(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        self.require_arity(items, "QUOTE", "one", 1, span)?;
        let Some(argument) = items.get(1) else {
            return Err(self.internal_error(span, "missing quote argument after arity check"));
        };
        self.emit(
            function,
            Instruction::Quote(argument.clone()),
            argument.span,
        )?;
        Ok(())
    }

    fn compile_quasiquote(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        self.require_arity(items, "QUASIQUOTE", "one", 1, span)?;
        let Some(argument) = items.get(1) else {
            return Err(self.internal_error(span, "missing quasiquote argument after arity check"));
        };
        self.emit(
            function,
            Instruction::QuasiQuote(argument.clone()),
            argument.span,
        )?;
        Ok(())
    }

    fn compile_if(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if !(items.len() == 3 || items.len() == 4) {
            return Err(self.arity_error(items, "IF", "two or three", span));
        }
        let Some(condition) = items.get(1) else {
            return Err(self.internal_error(span, "missing if condition after arity check"));
        };
        let Some(then_branch) = items.get(2) else {
            return Err(self.internal_error(span, "missing if branch after arity check"));
        };

        self.compile_expression(function, condition)?;
        let false_jump = self.emit(
            function,
            Instruction::JumpIfFalse(usize::MAX),
            condition.span,
        )?;
        self.compile_expression(function, then_branch)?;
        let end_jump = self.emit(function, Instruction::Jump(usize::MAX), then_branch.span)?;
        let else_target = self.instruction_count(function, span)?;
        self.patch_jump(function, false_jump, else_target, condition.span)?;

        if let Some(else_branch) = items.get(3) {
            self.compile_expression(function, else_branch)?;
        } else {
            self.emit(function, Instruction::Constant(Constant::Nil), span)?;
        }
        let end_target = self.instruction_count(function, span)?;
        self.patch_jump(function, end_jump, end_target, span)?;
        Ok(())
    }

    fn compile_progn(&mut self, function: FunctionId, items: &[Form]) -> Result<(), CompileError> {
        let forms = items.get(1..).unwrap_or(&[]);
        self.compile_sequence(function, forms)
    }

    fn compile_declare(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        global: bool,
        quoted: bool,
    ) -> Result<(), CompileError> {
        if global {
            let specs = if quoted {
                let Some(argument) = items.get(1) else {
                    return Err(CompileError::new(
                        CompileErrorKind::Arity {
                            operator: "PROCLAIM".to_string(),
                            expected: "at least one".to_string(),
                            actual: 0,
                        },
                        operator_span(items, span),
                    ));
                };
                match &argument.kind {
                    FormKind::List(quoted_form)
                        if quoted_form.len() == 2
                            && quoted_form
                                .first()
                                .and_then(|form| match &form.kind {
                                    FormKind::Atom(name) => special_operator_name(name),
                                    _ => None,
                                })
                                .as_deref()
                                == Some("QUOTE") =>
                    {
                        vec![quoted_form[1].clone()]
                    }
                    _ => vec![argument.clone()],
                }
            } else {
                if items.len() < 2 {
                    return Err(CompileError::new(
                        CompileErrorKind::Arity {
                            operator: "DECLAIM".to_string(),
                            expected: "at least one".to_string(),
                            actual: items.len().saturating_sub(1),
                        },
                        operator_span(items, span),
                    ));
                }
                items[1..].to_vec()
            };
            let declared = self.special_names_from_specs(&specs)?;
            let (names, exact_names) = split_special_names(declared);
            self.emit(
                function,
                Instruction::DeclareSpecial { names, exact_names },
                span,
            )?;
        }
        self.emit(function, Instruction::Constant(Constant::Nil), span)?;
        Ok(())
    }

    fn compile_locally(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        let declared = self.declared_special_names(items.get(1..).unwrap_or(&[]))?;
        let (names, exact_names) = split_special_names(declared);
        let has_specials = !names.is_empty() || !exact_names.is_empty();
        if has_specials {
            self.emit(
                function,
                Instruction::EnterSpecialScope { names, exact_names },
                span,
            )?;
        }
        self.compile_progn(function, items)?;
        if has_specials {
            self.emit(function, Instruction::ExitSpecialScope, span)?;
        }
        Ok(())
    }

    fn special_names_from_specs(
        &self,
        specs: &[Form],
    ) -> Result<HashSet<(String, bool)>, CompileError> {
        let mut names = HashSet::new();
        for spec in specs {
            let FormKind::List(items) = &spec.kind else {
                continue;
            };
            let Some(operator) = items.first().and_then(|form| match &form.kind {
                FormKind::Atom(name) => Some(name.as_str()),
                _ => None,
            }) else {
                continue;
            };
            if special_operator_name(operator).as_deref() != Some("SPECIAL") {
                continue;
            }
            for variable in &items[1..] {
                names.insert(self.symbol_name_info(variable, "special declaration name")?);
            }
        }
        Ok(names)
    }

    fn declared_special_names(
        &self,
        forms: &[Form],
    ) -> Result<HashSet<(String, bool)>, CompileError> {
        let mut names = HashSet::new();
        for form in forms {
            let FormKind::List(items) = &form.kind else {
                break;
            };
            let Some(operator) = items.first().and_then(|form| match &form.kind {
                FormKind::Atom(name) => Some(name.as_str()),
                _ => None,
            }) else {
                break;
            };
            if special_operator_name(operator).as_deref() != Some("DECLARE") {
                break;
            }
            names.extend(self.special_names_from_specs(&items[1..])?);
        }
        Ok(names)
    }

    fn compile_the(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        self.require_arity(items, "THE", "two", 2, span)?;
        let Some(type_form) = items.get(1) else {
            return Err(self.internal_error(span, "missing THE type after arity check"));
        };
        let Some(value_form) = items.get(2) else {
            return Err(self.internal_error(span, "missing THE value after arity check"));
        };
        self.emit(
            function,
            Instruction::FunctionLoad("__NCL_THE_CHECK".to_string()),
            span,
        )?;
        self.compile_expression(function, value_form)?;
        self.emit(
            function,
            Instruction::Quote(type_form.clone()),
            type_form.span,
        )?;
        self.emit(function, Instruction::Call(2), span)?;
        Ok(())
    }

    fn compile_eval_when(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(self.arity_error(items, "EVAL-WHEN", "at least one", span));
        }
        if compile_eval_when_executes(&items[1])? {
            self.compile_sequence(function, items.get(2..).unwrap_or(&[]))
        } else {
            self.emit(function, Instruction::Constant(Constant::Nil), span)?;
            Ok(())
        }
    }

    fn compile_prog1(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(self.arity_error(items, "PROG1", "at least one", span));
        }

        let Some(first) = items.get(1) else {
            return Err(self.internal_error(span, "missing PROG1 form after arity check"));
        };
        let retained = self.fresh_name("PROG1_VALUE");

        self.emit(function, Instruction::EnterScope, first.span)?;
        self.compile_expression(function, first)?;
        self.emit(function, Instruction::Define(retained.clone()), first.span)?;
        self.emit(function, Instruction::Pop, first.span)?;

        let tail = items.get(2..).unwrap_or(&[]);
        if !tail.is_empty() {
            self.compile_sequence(function, tail)?;
            self.emit(function, Instruction::Pop, span)?;
        }

        self.emit(function, Instruction::Load(retained), span)?;
        self.emit(function, Instruction::ExitScope, span)?;
        Ok(())
    }

    fn compile_prog2(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 {
            return Err(self.arity_error(items, "PROG2", "at least two", span));
        }

        let Some(first) = items.get(1) else {
            return Err(self.internal_error(span, "missing first PROG2 form after arity check"));
        };
        let Some(second) = items.get(2) else {
            return Err(self.internal_error(span, "missing second PROG2 form after arity check"));
        };
        let retained = self.fresh_name("PROG2_VALUE");

        self.emit(function, Instruction::EnterScope, first.span)?;
        self.compile_expression(function, first)?;
        self.emit(function, Instruction::Pop, first.span)?;
        self.compile_expression(function, second)?;
        self.emit(function, Instruction::Define(retained.clone()), second.span)?;
        self.emit(function, Instruction::Pop, second.span)?;

        let tail = items.get(3..).unwrap_or(&[]);
        if !tail.is_empty() {
            self.compile_sequence(function, tail)?;
            self.emit(function, Instruction::Pop, span)?;
        }

        self.emit(function, Instruction::Load(retained), span)?;
        self.emit(function, Instruction::ExitScope, span)?;
        Ok(())
    }

    fn compile_prog(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        sequential: bool,
    ) -> Result<(), CompileError> {
        let operator = if sequential { "PROG*" } else { "PROG" };
        if items.len() < 2 {
            return Err(self.arity_error(items, operator, "at least one", span));
        }
        let Some(binding_form) = items.get(1) else {
            return Err(self.internal_error(span, "missing PROG bindings after arity check"));
        };
        let FormKind::List(binding_forms) = &binding_form.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedList {
                    context: "PROG bindings".to_string(),
                },
                binding_form.span,
            ));
        };

        let mut names = HashSet::new();
        let mut parsed = Vec::with_capacity(binding_forms.len());
        for binding in binding_forms {
            let (name_form, init) = match &binding.kind {
                FormKind::Atom(_) => (binding, None),
                FormKind::List(parts) => {
                    if !(1..=2).contains(&parts.len()) {
                        return Err(CompileError::new(
                            CompileErrorKind::InvalidForm {
                                message: "PROG binding needs a name and optional value".to_string(),
                            },
                            binding.span,
                        ));
                    }
                    let Some(name_form) = parts.first() else {
                        return Err(self.internal_error(binding.span, "missing PROG binding name"));
                    };
                    (name_form, parts.get(1).cloned())
                }
                _ => {
                    return Err(CompileError::new(
                        CompileErrorKind::ExpectedSymbol {
                            context: "PROG binding name".to_string(),
                        },
                        binding.span,
                    ));
                }
            };
            let (name, escaped) = self.symbol_name_info(name_form, "PROG binding name")?;
            let key = if escaped {
                format!("\0{name}")
            } else {
                name.clone()
            };
            if !names.insert(key) {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "PROG binding names must be unique".to_string(),
                    },
                    name_form.span,
                ));
            }
            parsed.push((name, escaped, init));
        }

        let prog_function = self.reserve_function(None, Vec::new());
        self.emit(prog_function, Instruction::EnterScope, binding_form.span)?;

        if sequential {
            for (name, escaped, init) in &parsed {
                if let Some(init) = init {
                    self.compile_expression(prog_function, init)?;
                } else {
                    self.emit(
                        prog_function,
                        Instruction::Constant(Constant::Nil),
                        binding_form.span,
                    )?;
                }
                let define = if *escaped {
                    Instruction::DefineExact(name.clone())
                } else {
                    Instruction::Define(name.clone())
                };
                self.emit(prog_function, define, binding_form.span)?;
                self.emit(prog_function, Instruction::Pop, binding_form.span)?;
            }
        } else {
            let mut initial_temporaries = Vec::with_capacity(parsed.len());
            for (_, _, init) in &parsed {
                if let Some(init) = init {
                    self.compile_expression(prog_function, init)?;
                } else {
                    self.emit(
                        prog_function,
                        Instruction::Constant(Constant::Nil),
                        binding_form.span,
                    )?;
                }
                let temporary = self.fresh_name("PROG_INIT");
                self.emit(
                    prog_function,
                    Instruction::Define(temporary.clone()),
                    binding_form.span,
                )?;
                self.emit(prog_function, Instruction::Pop, binding_form.span)?;
                initial_temporaries.push(temporary);
            }
            for ((name, escaped, _), temporary) in parsed.iter().zip(initial_temporaries) {
                self.emit(
                    prog_function,
                    Instruction::Load(temporary),
                    binding_form.span,
                )?;
                let define = if *escaped {
                    Instruction::DefineExact(name.clone())
                } else {
                    Instruction::Define(name.clone())
                };
                self.emit(prog_function, define, binding_form.span)?;
                self.emit(prog_function, Instruction::Pop, binding_form.span)?;
            }
        }

        self.compile_tagbody_forms(prog_function, span, items.get(2..).unwrap_or(&[]))?;
        self.emit(prog_function, Instruction::ExitScope, span)?;
        self.emit(prog_function, Instruction::Return, span)?;
        self.emit(
            function,
            Instruction::Block {
                function: prog_function,
                name: "NIL".to_string(),
            },
            span,
        )?;
        Ok(())
    }

    fn compile_values(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        for item in items.get(1..).unwrap_or(&[]) {
            self.compile_expression(function, item)?;
            self.emit(function, Instruction::Primary, item.span)?;
        }
        self.emit(
            function,
            Instruction::Values(items.len().saturating_sub(1)),
            span,
        )?;
        Ok(())
    }

    fn compile_nth_value(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        self.require_arity(items, "NTH-VALUE", "two", 2, span)?;
        let Some(index_form) = items.get(1) else {
            return Err(self.internal_error(span, "missing NTH-VALUE index form after arity check"));
        };
        let Some(value_form) = items.get(2) else {
            return Err(self.internal_error(span, "missing NTH-VALUE value form after arity check"));
        };

        self.compile_expression(function, index_form)?;
        self.emit(function, Instruction::Primary, index_form.span)?;
        self.compile_expression(function, value_form)?;
        self.emit(function, Instruction::NthValue(index_form.span), span)?;
        Ok(())
    }

    fn compile_multiple_value_list(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        self.require_arity(items, "MULTIPLE-VALUE-LIST", "one", 1, span)?;
        let Some(value_form) = items.get(1) else {
            return Err(self.internal_error(
                span,
                "missing MULTIPLE-VALUE-LIST value form after arity check",
            ));
        };
        self.compile_expression(function, value_form)?;
        self.emit(function, Instruction::MultipleValueList, value_form.span)?;
        Ok(())
    }

    fn compile_ignore_errors(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        let child = self.reserve_function(None, Vec::new());
        self.compile_sequence(child, &items[1..])?;
        self.emit(child, Instruction::Return, span)?;
        self.emit(function, Instruction::IgnoreErrors(child), span)?;
        Ok(())
    }

    fn compile_handler_case(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 {
            return Err(self.arity_error(items, "HANDLER-CASE", "at least two", span));
        }

        let protected = items
            .get(1)
            .ok_or_else(|| self.internal_error(span, "missing HANDLER-CASE protected form"))?;
        let protected_function = self.reserve_function(None, Vec::new());
        self.compile_expression(protected_function, protected)?;
        self.emit(protected_function, Instruction::Return, protected.span)?;

        let mut clauses = Vec::with_capacity(items.len().saturating_sub(2));
        let mut no_error_seen = false;
        for clause in &items[2..] {
            let FormKind::List(clause_items) = &clause.kind else {
                return Err(CompileError::new(
                    CompileErrorKind::ExpectedList {
                        context: "handler-case clause".to_string(),
                    },
                    clause.span,
                ));
            };
            if clause_items.len() < 2 {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "handler-case clause needs a condition and variable list"
                            .to_string(),
                    },
                    clause.span,
                ));
            }
            let no_error = is_no_error_marker(&clause_items[0]);
            if no_error {
                if no_error_seen {
                    return Err(CompileError::new(
                        CompileErrorKind::InvalidForm {
                            message: "handler-case accepts at most one :NO-ERROR clause"
                                .to_string(),
                        },
                        clause.span,
                    ));
                }
                no_error_seen = true;
            }
            let condition = if no_error {
                "NO-ERROR".to_string()
            } else {
                self.condition_name(&clause_items[0], "handler-case condition")?
            };
            let FormKind::List(variable_items) = &clause_items[1].kind else {
                return Err(CompileError::new(
                    CompileErrorKind::ExpectedList {
                        context: "handler-case variable list".to_string(),
                    },
                    clause_items[1].span,
                ));
            };
            if !no_error && variable_items.len() > 1 {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "handler-case variable list accepts at most one variable"
                            .to_string(),
                    },
                    clause_items[1].span,
                ));
            }
            let variable_info = variable_items
                .iter()
                .map(|form| self.symbol_name_info(form, "handler-case variable"))
                .collect::<Result<Vec<_>, _>>()?;
            let variable = if no_error {
                None
            } else {
                variable_info.first().map(|(name, _)| name.clone())
            };
            let parameters = variable_info.iter().map(|(name, _)| name.clone()).collect();
            let required_escaped = variable_info.iter().map(|(_, escaped)| *escaped).collect();
            let clause_function =
                self.reserve_function_with_rest(None, parameters, required_escaped, None, false);
            self.compile_sequence(clause_function, &clause_items[2..])?;
            self.emit(clause_function, Instruction::Return, clause.span)?;
            clauses.push(HandlerCaseClause {
                condition,
                variable,
                function: clause_function,
                no_error,
                variable_count: variable_info.len(),
            });
        }

        self.emit(
            function,
            Instruction::HandlerCase {
                protected: protected_function,
                clauses,
            },
            span,
        )?;
        Ok(())
    }

    fn compile_handler_bind(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(self.arity_error(items, "HANDLER-BIND", "at least one", span));
        }
        let handler_form = items
            .get(1)
            .ok_or_else(|| self.internal_error(span, "missing HANDLER-BIND handler list"))?;
        let FormKind::List(handler_items) = &handler_form.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedList {
                    context: "handler-bind handler list".to_string(),
                },
                handler_form.span,
            ));
        };

        let mut handlers = Vec::with_capacity(handler_items.len());
        for handler in handler_items {
            let FormKind::List(handler_clause) = &handler.kind else {
                return Err(CompileError::new(
                    CompileErrorKind::ExpectedList {
                        context: "handler-bind clause".to_string(),
                    },
                    handler.span,
                ));
            };
            if handler_clause.len() != 2 {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "handler-bind clause needs a condition and handler".to_string(),
                    },
                    handler.span,
                ));
            }
            let condition = self.condition_name(&handler_clause[0], "handler-bind condition")?;
            let condition_variable = self.fresh_name("HANDLER_CONDITION");
            let clause_function = self.reserve_function(None, vec![condition_variable.clone()]);
            self.compile_expression(clause_function, &handler_clause[1])?;
            self.compile_expression(
                clause_function,
                &Form::atom(condition_variable, handler_clause[1].span),
            )?;
            self.emit(
                clause_function,
                Instruction::Call(1),
                handler_clause[1].span,
            )?;
            self.emit(clause_function, Instruction::Return, handler.span)?;
            handlers.push(HandlerBindClause {
                condition,
                function: clause_function,
            });
        }

        let body_function = self.reserve_function(None, Vec::new());
        self.compile_sequence(body_function, items.get(2..).unwrap_or(&[]))?;
        self.emit(body_function, Instruction::Return, span)?;
        self.emit(
            function,
            Instruction::HandlerBind {
                body: body_function,
                handlers,
            },
            span,
        )?;
        Ok(())
    }

    fn compile_restart_bind(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(self.arity_error(items, "RESTART-BIND", "at least one", span));
        }
        let binding_form = items
            .get(1)
            .ok_or_else(|| self.internal_error(span, "missing RESTART-BIND binding list"))?;
        let FormKind::List(binding_items) = &binding_form.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedList {
                    context: "restart-bind binding list".to_string(),
                },
                binding_form.span,
            ));
        };

        let mut bindings = Vec::with_capacity(binding_items.len());
        for binding in binding_items {
            let FormKind::List(binding_clause) = &binding.kind else {
                return Err(CompileError::new(
                    CompileErrorKind::ExpectedList {
                        context: "restart-bind clause".to_string(),
                    },
                    binding.span,
                ));
            };
            if binding_clause.len() != 2 {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "restart-bind clause needs a name and function".to_string(),
                    },
                    binding.span,
                ));
            }
            let name = self.control_name(&binding_clause[0], "RESTART-BIND restart name")?;
            let binding_function = self.reserve_function(None, Vec::new());
            self.compile_expression(binding_function, &binding_clause[1])?;
            self.emit(
                binding_function,
                Instruction::Return,
                binding_clause[1].span,
            )?;
            bindings.push(RestartBindClause {
                name,
                function: binding_function,
            });
        }

        let body_function = self.reserve_function(None, Vec::new());
        self.compile_sequence(body_function, items.get(2..).unwrap_or(&[]))?;
        self.emit(body_function, Instruction::Return, span)?;
        self.emit(
            function,
            Instruction::RestartBind {
                body: body_function,
                bindings,
            },
            span,
        )?;
        Ok(())
    }

    fn compile_catch(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(self.arity_error(items, "CATCH", "at least one", span));
        }

        let tag_function = self.reserve_function(None, Vec::new());
        self.compile_expression(tag_function, &items[1])?;
        self.emit(tag_function, Instruction::Return, items[1].span)?;

        let body_function = self.reserve_function(None, Vec::new());
        self.compile_sequence(body_function, items.get(2..).unwrap_or(&[]))?;
        self.emit(body_function, Instruction::Return, span)?;
        self.emit(
            function,
            Instruction::Catch {
                tag: tag_function,
                body: body_function,
            },
            span,
        )?;
        Ok(())
    }

    fn compile_with_simple_restart(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(self.arity_error(items, "WITH-SIMPLE-RESTART", "at least one", span));
        }

        let clause = &items[1];
        let FormKind::List(parts) = &clause.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedList {
                    context: "WITH-SIMPLE-RESTART restart clause".to_string(),
                },
                clause.span,
            ));
        };
        if parts.len() < 2 {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "WITH-SIMPLE-RESTART restart clause needs a name and report format"
                        .to_string(),
                },
                clause.span,
            ));
        }

        let name = self.control_name(&parts[0], "WITH-SIMPLE-RESTART name")?;
        let body = self.reserve_function(None, Vec::new());
        self.compile_sequence(body, items.get(2..).unwrap_or(&[]))?;
        self.emit(body, Instruction::Return, span)?;
        self.emit(
            function,
            Instruction::WithSimpleRestart { name, body },
            span,
        )?;
        Ok(())
    }

    fn compile_restart_case(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 {
            return Err(self.arity_error(items, "RESTART-CASE", "at least two", span));
        }

        let protected = items
            .get(1)
            .ok_or_else(|| self.internal_error(span, "missing RESTART-CASE protected form"))?;
        let protected_function = self.reserve_function(None, Vec::new());
        self.compile_expression(protected_function, protected)?;
        self.emit(protected_function, Instruction::Return, protected.span)?;

        let mut clauses = Vec::with_capacity(items.len().saturating_sub(2));
        for clause in &items[2..] {
            let FormKind::List(clause_items) = &clause.kind else {
                return Err(CompileError::new(
                    CompileErrorKind::ExpectedList {
                        context: "restart-case clause".to_string(),
                    },
                    clause.span,
                ));
            };
            if clause_items.len() < 2 {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "restart-case clause needs a name, lambda list, and body"
                            .to_string(),
                    },
                    clause.span,
                ));
            }
            let name = self.control_name(&clause_items[0], "RESTART-CASE restart name")?;
            let lambda_list = self.parameters(&clause_items[1])?;
            let clause_function = self.reserve_function_with_rest(
                None,
                lambda_list.required.clone(),
                lambda_list.required_escaped.clone(),
                lambda_list.rest.clone(),
                lambda_list.rest_escaped,
            );
            let optional = self.compile_optional_parameters(&lambda_list.optional)?;
            self.functions[clause_function].optional = optional;
            let keywords = self.compile_keyword_parameters(&lambda_list.keywords)?;
            self.functions[clause_function].keywords = keywords;
            self.functions[clause_function].has_keyword_section = lambda_list.has_keyword_section;
            self.functions[clause_function].allow_other_keys = lambda_list.allow_other_keys;
            let auxiliary = self.compile_auxiliary_parameters(&lambda_list.auxiliary)?;
            self.functions[clause_function].auxiliary = auxiliary;
            self.compile_sequence(clause_function, &clause_items[2..])?;
            self.emit(clause_function, Instruction::Return, clause.span)?;
            clauses.push(RestartCaseClause {
                name,
                function: clause_function,
            });
        }

        self.emit(
            function,
            Instruction::RestartCase {
                protected: protected_function,
                clauses,
            },
            span,
        )?;
        Ok(())
    }

    fn compile_with_condition_restarts(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 4 {
            return Err(self.arity_error(items, "WITH-CONDITION-RESTARTS", "at least three", span));
        }

        let condition = self.reserve_function(None, Vec::new());
        self.compile_expression(condition, &items[1])?;
        self.emit(condition, Instruction::Return, items[1].span)?;

        let restarts = self.reserve_function(None, Vec::new());
        self.compile_expression(restarts, &items[2])?;
        self.emit(restarts, Instruction::Return, items[2].span)?;

        let body = self.reserve_function(None, Vec::new());
        self.compile_sequence(body, &items[3..])?;
        self.emit(body, Instruction::Return, span)?;
        self.emit(
            function,
            Instruction::WithConditionRestarts {
                condition,
                restarts,
                body,
            },
            span,
        )?;
        Ok(())
    }

    fn compile_throw(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() != 3 {
            return Err(self.arity_error(items, "THROW", "two", span));
        }

        self.compile_expression(function, &items[1])?;
        self.compile_expression(function, &items[2])?;
        self.emit(function, Instruction::Throw, span)?;
        Ok(())
    }

    fn compile_progv(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 {
            return Err(self.arity_error(items, "PROGV", "at least two", span));
        }

        let symbols_function = self.reserve_function(None, Vec::new());
        self.compile_expression(symbols_function, &items[1])?;
        self.emit(symbols_function, Instruction::Return, items[1].span)?;

        let values_function = self.reserve_function(None, Vec::new());
        self.compile_expression(values_function, &items[2])?;
        self.emit(values_function, Instruction::Return, items[2].span)?;

        let body_function = self.reserve_function(None, Vec::new());
        self.compile_sequence(body_function, items.get(3..).unwrap_or(&[]))?;
        self.emit(body_function, Instruction::Return, span)?;

        self.emit(
            function,
            Instruction::Progv {
                symbols: symbols_function,
                values: values_function,
                body: body_function,
            },
            span,
        )?;
        Ok(())
    }

    fn compile_unwind_protect(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(self.arity_error(items, "UNWIND-PROTECT", "at least one", span));
        }

        let protected = items.get(1).ok_or_else(|| {
            self.internal_error(
                span,
                "missing UNWIND-PROTECT protected form after arity check",
            )
        })?;
        let protected_function = self.reserve_function(None, Vec::new());
        self.compile_expression(protected_function, protected)?;
        self.emit(protected_function, Instruction::Return, protected.span)?;

        let cleanup_function = self.reserve_function(None, Vec::new());
        self.compile_sequence(cleanup_function, items.get(2..).unwrap_or(&[]))?;
        self.emit(cleanup_function, Instruction::Return, span)?;
        self.emit(
            function,
            Instruction::UnwindProtect {
                protected: protected_function,
                cleanup: cleanup_function,
            },
            span,
        )?;
        Ok(())
    }

    fn compile_with_open_file(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(self.arity_error(items, "WITH-OPEN-FILE", "at least one", span));
        }
        let binding_form = items.get(1).ok_or_else(|| {
            self.internal_error(span, "missing WITH-OPEN-FILE binding after arity check")
        })?;
        let FormKind::List(binding) = &binding_form.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedList {
                    context: "WITH-OPEN-FILE binding".to_string(),
                },
                binding_form.span,
            ));
        };
        if binding.len() < 2 {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "WITH-OPEN-FILE binding needs a stream variable and pathname"
                        .to_string(),
                },
                binding_form.span,
            ));
        }
        self.symbol_name(&binding[0], "WITH-OPEN-FILE stream variable")?;

        let mut open_items = Vec::with_capacity(binding.len());
        open_items.push(Form::atom("OPEN", binding_form.span));
        open_items.extend(binding[1..].iter().cloned());
        let open_form = Form::list(open_items, binding_form.span);
        let generated_binding = Form::list(
            vec![Form::list(
                vec![binding[0].clone(), open_form],
                binding_form.span,
            )],
            binding_form.span,
        );
        let body = if items.len() > 2 {
            let mut body_items = Vec::with_capacity(items.len() - 1);
            body_items.push(Form::atom("PROGN", span));
            body_items.extend(items[2..].iter().cloned());
            Form::list(body_items, span)
        } else {
            Form::atom("NIL", span)
        };
        let close_form = Form::list(vec![Form::atom("CLOSE", span), binding[0].clone()], span);
        let protected_form = Form::list(
            vec![Form::atom("UNWIND-PROTECT", span), body, close_form],
            span,
        );
        let expanded = Form::list(
            vec![Form::atom("LET", span), generated_binding, protected_form],
            span,
        );
        self.compile_expression(function, &expanded)
    }

    fn compile_with_open_stream(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(self.arity_error(items, "WITH-OPEN-STREAM", "at least one", span));
        }
        let binding_form = items.get(1).ok_or_else(|| {
            self.internal_error(span, "missing WITH-OPEN-STREAM binding after arity check")
        })?;
        let FormKind::List(binding) = &binding_form.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedList {
                    context: "WITH-OPEN-STREAM binding".to_string(),
                },
                binding_form.span,
            ));
        };
        if binding.len() != 2 {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "WITH-OPEN-STREAM binding needs a stream variable and stream form"
                        .to_string(),
                },
                binding_form.span,
            ));
        }
        self.symbol_name(&binding[0], "WITH-OPEN-STREAM stream variable")?;

        let generated_binding = Form::list(
            vec![Form::list(
                vec![binding[0].clone(), binding[1].clone()],
                binding_form.span,
            )],
            binding_form.span,
        );
        let body = if items.len() > 2 {
            let mut body_items = Vec::with_capacity(items.len() - 1);
            body_items.push(Form::atom("PROGN", span));
            body_items.extend(items[2..].iter().cloned());
            Form::list(body_items, span)
        } else {
            Form::atom("NIL", span)
        };
        let close_form = Form::list(vec![Form::atom("CLOSE", span), binding[0].clone()], span);
        let protected_form = Form::list(
            vec![Form::atom("UNWIND-PROTECT", span), body, close_form],
            span,
        );
        let expanded = Form::list(
            vec![Form::atom("LET", span), generated_binding, protected_form],
            span,
        );
        self.compile_expression(function, &expanded)
    }

    fn compile_with_output_to_string(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(self.arity_error(items, "WITH-OUTPUT-TO-STRING", "at least one", span));
        }
        let binding_form = items.get(1).ok_or_else(|| {
            self.internal_error(
                span,
                "missing WITH-OUTPUT-TO-STRING binding after arity check",
            )
        })?;
        let FormKind::List(binding) = &binding_form.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedList {
                    context: "WITH-OUTPUT-TO-STRING binding".to_string(),
                },
                binding_form.span,
            ));
        };
        if binding.is_empty() {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "WITH-OUTPUT-TO-STRING binding needs a stream variable".to_string(),
                },
                binding_form.span,
            ));
        }
        let has_literal_nil_string_form = binding.get(1).is_some_and(
            |form| matches!(&form.kind, FormKind::Atom(name) if name.eq_ignore_ascii_case("nil")),
        );
        if binding.len() != 1 && !has_literal_nil_string_form {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "WITH-OUTPUT-TO-STRING currently supports only a variable binding or literal NIL string form with :element-type".to_string(),
                },
                binding_form.span,
            ));
        }
        if has_literal_nil_string_form && !(binding.len() - 2).is_multiple_of(2) {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "WITH-OUTPUT-TO-STRING keyword arguments must be keyword/value pairs".to_string(),
                },
                binding_form.span,
            ));
        }
        self.symbol_name(&binding[0], "WITH-OUTPUT-TO-STRING stream variable")?;

        let mut initializer_items =
            vec![Form::atom("MAKE-STRING-OUTPUT-STREAM", binding_form.span)];
        let mut element_type_form = None;
        if has_literal_nil_string_form {
            for pair in binding[2..].chunks_exact(2) {
                let Some((keyword, escaped)) = macro_keyword_name(&pair[0]) else {
                    return Err(CompileError::new(
                        CompileErrorKind::InvalidForm {
                            message: "WITH-OUTPUT-TO-STRING keyword arguments must use keyword names".to_string(),
                        },
                        pair[0].span,
                    ));
                };
                if escaped || keyword != "ELEMENT-TYPE" {
                    return Err(CompileError::new(
                        CompileErrorKind::InvalidForm {
                            message: "WITH-OUTPUT-TO-STRING currently supports only :element-type".to_string(),
                        },
                        pair[0].span,
                    ));
                }
                if element_type_form.is_some() {
                    return Err(CompileError::new(
                        CompileErrorKind::InvalidForm {
                            message: "WITH-OUTPUT-TO-STRING received duplicate :element-type".to_string(),
                        },
                        pair[0].span,
                    ));
                }
                element_type_form = Some(pair[1].clone());
            }
        }
        if let Some(element_type_form) = element_type_form {
            initializer_items.push(Form::atom(":ELEMENT-TYPE", binding_form.span));
            initializer_items.push(element_type_form);
        }
        let initializer = Form::list(initializer_items, binding_form.span);
        let generated_binding = Form::list(
            vec![Form::list(
                vec![binding[0].clone(), initializer],
                binding_form.span,
            )],
            binding_form.span,
        );
        let result_form = Form::list(
            vec![
                Form::atom("GET-OUTPUT-STREAM-STRING", span),
                binding[0].clone(),
            ],
            span,
        );
        let body = if items.len() > 2 {
            let mut body_items = Vec::with_capacity(items.len() + 1);
            body_items.push(Form::atom("PROGN", span));
            body_items.extend(items[2..].iter().cloned());
            body_items.push(result_form);
            Form::list(body_items, span)
        } else {
            result_form
        };
        let close_form = Form::list(vec![Form::atom("CLOSE", span), binding[0].clone()], span);
        let protected_form = Form::list(
            vec![Form::atom("UNWIND-PROTECT", span), body, close_form],
            span,
        );
        let expanded = Form::list(
            vec![Form::atom("LET", span), generated_binding, protected_form],
            span,
        );
        self.compile_expression(function, &expanded)
    }

    fn compile_with_input_from_string(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(self.arity_error(items, "WITH-INPUT-FROM-STRING", "at least one", span));
        }
        let binding_form = items.get(1).ok_or_else(|| {
            self.internal_error(
                span,
                "missing WITH-INPUT-FROM-STRING binding after arity check",
            )
        })?;
        let FormKind::List(binding) = &binding_form.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedList {
                    context: "WITH-INPUT-FROM-STRING binding".to_string(),
                },
                binding_form.span,
            ));
        };
        if binding.len() < 2 {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "WITH-INPUT-FROM-STRING binding needs a variable and string form"
                        .to_string(),
                },
                binding_form.span,
            ));
        }
        self.symbol_name(&binding[0], "WITH-INPUT-FROM-STRING stream variable")?;

        if !(binding.len() - 2).is_multiple_of(2) {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message:
                        "WITH-INPUT-FROM-STRING requires keyword/value pairs after the string form"
                            .to_string(),
                },
                binding_form.span,
            ));
        }
        let mut index_form: Option<Form> = None;
        let mut start_form: Option<Form> = None;
        let mut end_form: Option<Form> = None;
        for pair in binding[2..].chunks_exact(2) {
            let Some((keyword, _escaped)) = macro_keyword_name(&pair[0]) else {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "WITH-INPUT-FROM-STRING options must use keyword names"
                            .to_string(),
                    },
                    pair[0].span,
                ));
            };
            let slot = match keyword.as_str() {
                "INDEX" => &mut index_form,
                "START" => &mut start_form,
                "END" => &mut end_form,
                _ => {
                    return Err(CompileError::new(
                        CompileErrorKind::InvalidForm {
                            message: format!(
                                "WITH-INPUT-FROM-STRING does not recognize keyword :{keyword}"
                            ),
                        },
                        pair[0].span,
                    ));
                }
            };
            if slot.is_some() {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: format!(
                            "WITH-INPUT-FROM-STRING received duplicate keyword :{keyword}"
                        ),
                    },
                    pair[0].span,
                ));
            }
            *slot = Some(pair[1].clone());
        }

        let mut initializer_items = vec![
            Form::atom("MAKE-STRING-INPUT-STREAM", binding_form.span),
            binding[1].clone(),
        ];
        if let Some(start) = start_form {
            initializer_items.push(start);
        } else if end_form.is_some() {
            initializer_items.push(Form::atom("0", binding_form.span));
        }
        if let Some(end) = end_form {
            initializer_items.push(end);
        }
        let initializer = Form::list(initializer_items, binding_form.span);
        let generated_binding = Form::list(
            vec![Form::list(
                vec![binding[0].clone(), initializer],
                binding_form.span,
            )],
            binding_form.span,
        );
        let body = if items.len() > 2 {
            let mut body_items = Vec::with_capacity(items.len() - 1);
            body_items.push(Form::atom("PROGN", span));
            body_items.extend(items[2..].iter().cloned());
            Form::list(body_items, span)
        } else {
            Form::atom("NIL", span)
        };
        let body = if let Some(index_form) = index_form {
            let position_form = Form::list(
                vec![
                    Form::atom("__NCL-STRING-INPUT-STREAM-POSITION", span),
                    binding[0].clone(),
                ],
                span,
            );
            let update_form = Form::list(
                vec![Form::atom("SETF", span), index_form, position_form],
                span,
            );
            Form::list(
                vec![Form::atom("MULTIPLE-VALUE-PROG1", span), body, update_form],
                span,
            )
        } else {
            body
        };
        let close_form = Form::list(vec![Form::atom("CLOSE", span), binding[0].clone()], span);
        let protected_form = Form::list(
            vec![Form::atom("UNWIND-PROTECT", span), body, close_form],
            span,
        );
        let expanded = Form::list(
            vec![Form::atom("LET", span), generated_binding, protected_form],
            span,
        );
        self.compile_expression(function, &expanded)
    }

    fn compile_with_hash_table_iterator(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(self.arity_error(
                items,
                "WITH-HASH-TABLE-ITERATOR",
                "at least one",
                span,
            ));
        }
        let binding_form = items.get(1).ok_or_else(|| {
            self.internal_error(
                span,
                "missing WITH-HASH-TABLE-ITERATOR binding after arity check",
            )
        })?;
        let FormKind::List(binding) = &binding_form.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedList {
                    context: "WITH-HASH-TABLE-ITERATOR binding".to_string(),
                },
                binding_form.span,
            ));
        };
        if binding.len() != 2 {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "WITH-HASH-TABLE-ITERATOR binding needs an iterator name and a hash table"
                        .to_string(),
                },
                binding_form.span,
            ));
        }
        self.symbol_name(
            &binding[0],
            "WITH-HASH-TABLE-ITERATOR iterator name",
        )?;

        let state = Form::atom(
            self.fresh_name("HASH_TABLE_ITERATOR_STATE"),
            binding_form.span,
        );
        let initializer = Form::list(
            vec![
                Form::atom("__NCL-MAKE-HASH-TABLE-ITERATOR", binding_form.span),
                binding[1].clone(),
            ],
            binding_form.span,
        );
        let local_function = Form::list(
            vec![
                binding[0].clone(),
                Form::list(Vec::new(), binding_form.span),
                Form::list(
                    vec![
                        Form::atom("__NCL-HASH-TABLE-ITERATOR-NEXT", span),
                        state.clone(),
                    ],
                    span,
                ),
            ],
            binding_form.span,
        );
        let local_bindings = Form::list(vec![local_function], binding_form.span);
        let mut flet_items = Vec::with_capacity(items.len());
        flet_items.push(Form::atom("FLET", span));
        flet_items.push(local_bindings);
        flet_items.extend(items[2..].iter().cloned());
        let flet = Form::list(flet_items, span);
        let state_binding = Form::list(vec![state, initializer], binding_form.span);
        let let_bindings = Form::list(vec![state_binding], binding_form.span);
        let expanded = Form::list(
            vec![Form::atom("LET", span), let_bindings, flet],
            span,
        );
        self.compile_expression(function, &expanded)
    }

    fn compile_block(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(self.arity_error(items, "BLOCK", "at least one", span));
        }
        let name = self.control_name(
            items
                .get(1)
                .ok_or_else(|| self.internal_error(span, "missing BLOCK name after arity check"))?,
            "BLOCK name",
        )?;
        let child = self.reserve_function(None, Vec::new());
        self.compile_sequence(child, items.get(2..).unwrap_or(&[]))?;
        self.emit(child, Instruction::Return, span)?;
        self.emit(
            function,
            Instruction::Block {
                function: child,
                name,
            },
            span,
        )?;
        Ok(())
    }

    fn compile_return(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if !(items.len() == 1 || items.len() == 2) {
            return Err(self.arity_error(items, "RETURN", "zero or one", span));
        }
        if let Some(value) = items.get(1) {
            self.compile_expression(function, value)?;
        } else {
            self.emit(function, Instruction::Constant(Constant::Nil), span)?;
        }
        self.emit(
            function,
            Instruction::ReturnFrom {
                name: "NIL".to_string(),
            },
            span,
        )?;
        Ok(())
    }

    fn compile_return_from(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if !(items.len() == 2 || items.len() == 3) {
            return Err(self.arity_error(items, "RETURN-FROM", "one or two", span));
        }
        let name = self.control_name(
            items.get(1).ok_or_else(|| {
                self.internal_error(span, "missing RETURN-FROM name after arity check")
            })?,
            "RETURN-FROM name",
        )?;
        if let Some(value) = items.get(2) {
            self.compile_expression(function, value)?;
        } else {
            self.emit(function, Instruction::Constant(Constant::Nil), span)?;
        }
        self.emit(function, Instruction::ReturnFrom { name }, span)?;
        Ok(())
    }

    fn compile_tagbody(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        self.compile_tagbody_forms(function, span, items.get(1..).unwrap_or(&[]))
    }

    fn compile_tagbody_forms(
        &mut self,
        function: FunctionId,
        span: Span,
        forms: &[Form],
    ) -> Result<(), CompileError> {
        let child = self.reserve_function(None, Vec::new());
        let mut tags = Vec::new();

        for form in forms {
            if let Some(tag) = tag_name(form) {
                if tags.iter().any(|(existing, _)| existing == &tag) {
                    return Err(CompileError::new(
                        CompileErrorKind::InvalidForm {
                            message: format!("duplicate TAGBODY tag {tag}"),
                        },
                        form.span,
                    ));
                }
                let position = self.instruction_count(child, form.span)?;
                tags.push((tag, position));
            } else {
                self.compile_expression(child, form)?;
                self.emit(child, Instruction::Pop, form.span)?;
            }
        }

        self.emit(child, Instruction::Constant(Constant::Nil), span)?;
        self.emit(child, Instruction::Return, span)?;
        self.emit(
            function,
            Instruction::TagBody {
                function: child,
                tags,
            },
            span,
        )?;
        Ok(())
    }

    fn compile_go(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        self.require_arity(items, "GO", "one", 1, span)?;
        let tag = self.control_tag(
            items
                .get(1)
                .ok_or_else(|| self.internal_error(span, "missing GO tag after arity check"))?,
            "GO tag",
        )?;
        self.emit(function, Instruction::Go { tag }, span)?;
        Ok(())
    }

    fn compile_multiple_value_bind(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 {
            return Err(self.arity_error(items, "MULTIPLE-VALUE-BIND", "at least two", span));
        }
        let Some(variable_form) = items.get(1) else {
            return Err(self.internal_error(span, "missing MULTIPLE-VALUE-BIND variables"));
        };
        let FormKind::List(variables) = &variable_form.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedList {
                    context: "MULTIPLE-VALUE-BIND variables".to_string(),
                },
                variable_form.span,
            ));
        };
        let mut names = Vec::with_capacity(variables.len());
        for variable in variables {
            names.push(self.symbol_name_info(variable, "MULTIPLE-VALUE-BIND variable")?);
        }
        let Some(value_form) = items.get(2) else {
            return Err(self.internal_error(span, "missing MULTIPLE-VALUE-BIND value form"));
        };

        self.emit(function, Instruction::EnterScope, variable_form.span)?;
        self.compile_expression(function, value_form)?;
        let has_exact = names.iter().any(|(_, escaped)| *escaped);
        let instruction = if has_exact {
            Instruction::BindValuesExact(names)
        } else {
            Instruction::BindValues(names.into_iter().map(|(name, _)| name).collect())
        };
        self.emit(function, instruction, value_form.span)?;
        self.compile_sequence(function, items.get(3..).unwrap_or(&[]))?;
        self.emit(function, Instruction::ExitScope, span)?;
        Ok(())
    }

    fn compile_multiple_value_call(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(self.arity_error(items, "MULTIPLE-VALUE-CALL", "at least one", span));
        }
        let Some(function_form) = items.get(1) else {
            return Err(self.internal_error(span, "missing MULTIPLE-VALUE-CALL function"));
        };
        self.compile_expression(function, function_form)?;
        self.emit(function, Instruction::Primary, function_form.span)?;
        for value_form in items.get(2..).unwrap_or(&[]) {
            self.compile_expression(function, value_form)?;
        }
        self.emit(
            function,
            Instruction::MultipleValueCall(items.len().saturating_sub(2)),
            span,
        )?;
        Ok(())
    }

    fn compile_multiple_value_prog1(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(self.arity_error(items, "MULTIPLE-VALUE-PROG1", "at least one", span));
        }
        let Some(first) = items.get(1) else {
            return Err(
                self.internal_error(span, "missing MULTIPLE-VALUE-PROG1 form after arity check")
            );
        };
        let retained = self.fresh_name("MULTIPLE_VALUE_PROG1_VALUE");

        self.emit(function, Instruction::EnterScope, first.span)?;
        self.compile_expression(function, first)?;
        self.emit(
            function,
            Instruction::DefineValues(retained.clone()),
            first.span,
        )?;
        self.emit(function, Instruction::Pop, first.span)?;

        let tail = items.get(2..).unwrap_or(&[]);
        if !tail.is_empty() {
            self.compile_sequence(function, tail)?;
            self.emit(function, Instruction::Pop, span)?;
        }

        self.emit(function, Instruction::Load(retained), span)?;
        self.emit(function, Instruction::ExitScope, span)?;
        Ok(())
    }

    fn compile_and(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        let forms = items.get(1..).unwrap_or(&[]);
        let Some((last, prefix)) = forms.split_last() else {
            self.emit(
                function,
                Instruction::Constant(Constant::Boolean(true)),
                span,
            )?;
            return Ok(());
        };

        let mut false_jumps = Vec::with_capacity(prefix.len());
        for form in prefix {
            self.compile_expression(function, form)?;
            self.emit(function, Instruction::Dup, form.span)?;
            let jump = self.emit(function, Instruction::JumpIfFalse(usize::MAX), form.span)?;
            false_jumps.push(jump);
            self.emit(function, Instruction::Pop, form.span)?;
        }
        self.compile_expression(function, last)?;

        let end = self.instruction_count(function, span)?;
        for jump in false_jumps {
            self.patch_jump(function, jump, end, span)?;
        }
        Ok(())
    }

    fn compile_or(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        let forms = items.get(1..).unwrap_or(&[]);
        let Some((last, prefix)) = forms.split_last() else {
            self.emit(function, Instruction::Constant(Constant::Nil), span)?;
            return Ok(());
        };

        let mut end_jumps = Vec::with_capacity(prefix.len());
        for form in prefix {
            self.compile_expression(function, form)?;
            self.emit(function, Instruction::Dup, form.span)?;
            let false_jump =
                self.emit(function, Instruction::JumpIfFalse(usize::MAX), form.span)?;
            let end_jump = self.emit(function, Instruction::Jump(usize::MAX), form.span)?;
            let next = self.instruction_count(function, span)?;
            self.patch_jump(function, false_jump, next, form.span)?;
            self.emit(function, Instruction::Pop, form.span)?;
            end_jumps.push(end_jump);
        }
        self.compile_expression(function, last)?;

        let end = self.instruction_count(function, span)?;
        for jump in end_jumps {
            self.patch_jump(function, jump, end, span)?;
        }
        Ok(())
    }

    fn compile_when(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        positive: bool,
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(CompileError::new(
                CompileErrorKind::Arity {
                    operator: if positive { "WHEN" } else { "UNLESS" }.to_string(),
                    expected: "at least one".to_string(),
                    actual: items.len().saturating_sub(1),
                },
                operator_span(items, span),
            ));
        }
        let condition = items
            .get(1)
            .ok_or_else(|| self.internal_error(span, "missing when condition"))?;
        self.compile_expression(function, condition)?;
        let branch_jump = self.emit(
            function,
            Instruction::JumpIfFalse(usize::MAX),
            condition.span,
        )?;

        if positive {
            self.compile_sequence(function, items.get(2..).unwrap_or(&[]))?;
            let end_jump = self.emit(function, Instruction::Jump(usize::MAX), span)?;
            let false_target = self.instruction_count(function, span)?;
            self.emit(function, Instruction::Constant(Constant::Nil), span)?;
            let end_target = self.instruction_count(function, span)?;
            self.patch_jump(function, branch_jump, false_target, condition.span)?;
            self.patch_jump(function, end_jump, end_target, span)?;
        } else {
            self.emit(function, Instruction::Constant(Constant::Nil), span)?;
            let end_jump = self.emit(function, Instruction::Jump(usize::MAX), span)?;
            let body_target = self.instruction_count(function, span)?;
            self.compile_sequence(function, items.get(2..).unwrap_or(&[]))?;
            let end_target = self.instruction_count(function, span)?;
            self.patch_jump(function, branch_jump, body_target, condition.span)?;
            self.patch_jump(function, end_jump, end_target, span)?;
        }
        Ok(())
    }

    fn compile_cond(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        let clauses = items.get(1..).unwrap_or(&[]);
        let mut end_jumps = Vec::with_capacity(clauses.len());
        for clause in clauses {
            let FormKind::List(clause_items) = &clause.kind else {
                return Err(CompileError::new(
                    CompileErrorKind::ExpectedList {
                        context: "cond clause".to_string(),
                    },
                    clause.span,
                ));
            };
            let Some(condition) = clause_items.first() else {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "cond clause cannot be empty".to_string(),
                    },
                    clause.span,
                ));
            };
            self.compile_expression(function, condition)?;
            if clause_items.len() == 1 {
                self.emit(function, Instruction::Dup, condition.span)?;
                let false_jump = self.emit(
                    function,
                    Instruction::JumpIfFalse(usize::MAX),
                    condition.span,
                )?;
                let end_jump = self.emit(function, Instruction::Jump(usize::MAX), clause.span)?;
                let next_clause = self.instruction_count(function, clause.span)?;
                self.patch_jump(function, false_jump, next_clause, condition.span)?;
                self.emit(function, Instruction::Pop, condition.span)?;
                end_jumps.push(end_jump);
            } else {
                let false_jump = self.emit(
                    function,
                    Instruction::JumpIfFalse(usize::MAX),
                    condition.span,
                )?;
                self.compile_sequence(function, &clause_items[1..])?;
                let end_jump = self.emit(function, Instruction::Jump(usize::MAX), clause.span)?;
                let next_clause = self.instruction_count(function, clause.span)?;
                self.patch_jump(function, false_jump, next_clause, condition.span)?;
                end_jumps.push(end_jump);
            }
        }
        self.emit(function, Instruction::Constant(Constant::Nil), span)?;
        let end = self.instruction_count(function, span)?;
        for jump in end_jumps {
            self.patch_jump(function, jump, end, span)?;
        }
        Ok(())
    }

    fn compile_case(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        let operator = items
            .first()
            .and_then(|form| match &form.kind {
                FormKind::Atom(atom) => Some(normalize_name(atom)),
                _ => None,
            })
            .unwrap_or_else(|| "CASE".to_string());
        if items.len() < 2 {
            return Err(self.arity_error(items, &operator, "at least one", span));
        }

        let mut clauses = Vec::new();
        let mut default_clause: Option<(Vec<Form>, Span)> = None;
        for clause in items.iter().skip(2) {
            let FormKind::List(clause_items) = &clause.kind else {
                return Err(CompileError::new(
                    CompileErrorKind::ExpectedList {
                        context: "case clause".to_string(),
                    },
                    clause.span,
                ));
            };
            let Some(key_spec) = clause_items.first() else {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "case clause cannot be empty".to_string(),
                    },
                    clause.span,
                ));
            };
            if case_default_clause(key_spec) {
                default_clause = Some((clause_items.get(1..).unwrap_or(&[]).to_vec(), clause.span));
                continue;
            }
            let keys = match &key_spec.kind {
                FormKind::List(keys) => keys.to_vec(),
                _ => vec![key_spec.clone()],
            };
            clauses.push((
                keys,
                clause_items.get(1..).unwrap_or(&[]).to_vec(),
                clause.span,
            ));
        }

        let key_name = self.fresh_name("CASE_KEY");
        self.emit(function, Instruction::EnterScope, span)?;
        self.compile_expression(function, &items[1])?;
        self.emit(
            function,
            Instruction::Define(key_name.clone()),
            items[1].span,
        )?;
        self.emit(function, Instruction::Pop, items[1].span)?;

        let mut body_jumps: Vec<(Vec<usize>, Vec<Form>, Span)> = Vec::new();
        for (keys, body, clause_span) in clauses {
            let mut clause_jumps = Vec::new();
            for key in keys {
                self.emit(
                    function,
                    Instruction::FunctionLoad("EQL".to_string()),
                    key.span,
                )?;
                self.emit(function, Instruction::Load(key_name.clone()), items[1].span)?;
                self.emit(function, Instruction::Quote(key.clone()), key.span)?;
                self.emit(function, Instruction::Call(2), key.span)?;
                let false_jump =
                    self.emit(function, Instruction::JumpIfFalse(usize::MAX), key.span)?;
                let body_jump = self.emit(function, Instruction::Jump(usize::MAX), key.span)?;
                let next_key = self.instruction_count(function, key.span)?;
                self.patch_jump(function, false_jump, next_key, key.span)?;
                clause_jumps.push(body_jump);
            }
            body_jumps.push((clause_jumps, body, clause_span));
        }

        let default_jump = if default_clause.is_some() {
            Some(self.emit(function, Instruction::Jump(usize::MAX), span)?)
        } else {
            None
        };
        let no_match_jump = if default_clause.is_none() {
            if operator.eq_ignore_ascii_case("ECASE") {
                self.emit(
                    function,
                    Instruction::FunctionLoad("__NCL_ECASE_ERROR".to_string()),
                    span,
                )?;
                self.emit(function, Instruction::Call(0), span)?;
            } else {
                self.emit(function, Instruction::Constant(Constant::Nil), span)?;
            }
            Some(self.emit(function, Instruction::Jump(usize::MAX), span)?)
        } else {
            None
        };

        let mut end_jumps = Vec::new();
        for (jumps, body, clause_span) in body_jumps {
            let target = self.instruction_count(function, clause_span)?;
            for jump in jumps {
                self.patch_jump(function, jump, target, clause_span)?;
            }
            self.compile_sequence(function, &body)?;
            end_jumps.push(self.emit(function, Instruction::Jump(usize::MAX), clause_span)?);
        }

        if let Some((body, clause_span)) = default_clause {
            let target = self.instruction_count(function, clause_span)?;
            if let Some(jump) = default_jump {
                self.patch_jump(function, jump, target, clause_span)?;
            }
            self.compile_sequence(function, &body)?;
            end_jumps.push(self.emit(function, Instruction::Jump(usize::MAX), clause_span)?);
        }

        let end = self.instruction_count(function, span)?;
        if let Some(jump) = no_match_jump {
            self.patch_jump(function, jump, end, span)?;
        }
        for jump in end_jumps {
            self.patch_jump(function, jump, end, span)?;
        }
        self.emit(function, Instruction::ExitScope, span)?;
        Ok(())
    }

    fn compile_typecase(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        let operator = items
            .first()
            .and_then(|form| match &form.kind {
                FormKind::Atom(atom) => Some(normalize_name(atom)),
                _ => None,
            })
            .unwrap_or_else(|| "TYPECASE".to_string());
        if items.len() < 2 {
            return Err(self.arity_error(items, &operator, "at least one", span));
        }

        let mut clauses = Vec::new();
        let mut default_clause: Option<(Vec<Form>, Span)> = None;
        for clause in items.iter().skip(2) {
            let FormKind::List(clause_items) = &clause.kind else {
                return Err(CompileError::new(
                    CompileErrorKind::ExpectedList {
                        context: "typecase clause".to_string(),
                    },
                    clause.span,
                ));
            };
            let Some(type_specifier) = clause_items.first() else {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "typecase clause cannot be empty".to_string(),
                    },
                    clause.span,
                ));
            };
            if case_default_clause(type_specifier) {
                default_clause = Some((clause_items.get(1..).unwrap_or(&[]).to_vec(), clause.span));
                continue;
            }
            clauses.push((
                type_specifier.clone(),
                clause_items.get(1..).unwrap_or(&[]).to_vec(),
                clause.span,
            ));
        }

        let key_name = self.fresh_name("TYPECASE_KEY");
        self.emit(function, Instruction::EnterScope, span)?;
        self.compile_expression(function, &items[1])?;
        self.emit(
            function,
            Instruction::Define(key_name.clone()),
            items[1].span,
        )?;
        self.emit(function, Instruction::Pop, items[1].span)?;

        let mut body_jumps: Vec<(usize, Vec<Form>, Span)> = Vec::new();
        for (type_specifier, body, clause_span) in clauses {
            self.emit(
                function,
                Instruction::FunctionLoad("TYPEP".to_string()),
                type_specifier.span,
            )?;
            self.emit(function, Instruction::Load(key_name.clone()), items[1].span)?;
            self.emit(
                function,
                Instruction::Quote(type_specifier.clone()),
                type_specifier.span,
            )?;
            self.emit(function, Instruction::Call(2), type_specifier.span)?;
            let false_jump = self.emit(
                function,
                Instruction::JumpIfFalse(usize::MAX),
                type_specifier.span,
            )?;
            let body_jump =
                self.emit(function, Instruction::Jump(usize::MAX), type_specifier.span)?;
            let next_clause = self.instruction_count(function, type_specifier.span)?;
            self.patch_jump(function, false_jump, next_clause, type_specifier.span)?;
            body_jumps.push((body_jump, body, clause_span));
        }

        let default_jump = if default_clause.is_some() {
            Some(self.emit(function, Instruction::Jump(usize::MAX), span)?)
        } else {
            None
        };
        let no_match_jump = if default_clause.is_none() {
            if operator.eq_ignore_ascii_case("ETYPECASE") {
                self.emit(
                    function,
                    Instruction::FunctionLoad("__NCL_ETYPECASE_ERROR".to_string()),
                    span,
                )?;
                self.emit(function, Instruction::Call(0), span)?;
            } else {
                self.emit(function, Instruction::Constant(Constant::Nil), span)?;
            }
            Some(self.emit(function, Instruction::Jump(usize::MAX), span)?)
        } else {
            None
        };

        let mut end_jumps = Vec::new();
        for (body_jump, body, clause_span) in body_jumps {
            let target = self.instruction_count(function, clause_span)?;
            self.patch_jump(function, body_jump, target, clause_span)?;
            self.compile_sequence(function, &body)?;
            end_jumps.push(self.emit(function, Instruction::Jump(usize::MAX), clause_span)?);
        }

        if let Some((body, clause_span)) = default_clause {
            let target = self.instruction_count(function, clause_span)?;
            if let Some(jump) = default_jump {
                self.patch_jump(function, jump, target, clause_span)?;
            }
            self.compile_sequence(function, &body)?;
            end_jumps.push(self.emit(function, Instruction::Jump(usize::MAX), clause_span)?);
        }

        let end = self.instruction_count(function, span)?;
        if let Some(jump) = no_match_jump {
            self.patch_jump(function, jump, end, span)?;
        }
        for jump in end_jumps {
            self.patch_jump(function, jump, end, span)?;
        }
        self.emit(function, Instruction::ExitScope, span)?;
        Ok(())
    }

    fn compile_lambda(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "lambda needs parameters and a body".to_string(),
                },
                operator_span(items, span),
            ));
        }
        let Some(parameter_form) = items.get(1) else {
            return Err(self.internal_error(span, "missing lambda parameters after arity check"));
        };
        let lambda_list = self.parameters(parameter_form)?;
        let child = self.reserve_function_with_rest(
            None,
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
        let (documentation, body) = split_documentation_body(items.get(2..).unwrap_or(&[]));
        self.functions[child].documentation = documentation;
        let (special_names, special_exact_names) =
            split_special_names(self.declared_special_names(body)?);
        self.functions[child].special_names = special_names;
        self.functions[child].special_exact_names = special_exact_names;
        self.compile_sequence(child, body)?;
        self.emit(child, Instruction::Return, span)?;
        self.emit(function, Instruction::MakeClosure(child), span)?;
        Ok(())
    }

    fn compile_function(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        self.require_arity(items, "FUNCTION", "one", 1, span)?;
        let Some(argument) = items.get(1) else {
            return Err(self.internal_error(span, "missing function argument after arity check"));
        };
        if let FormKind::List(function_name) = &argument.kind {
            let operator = function_name.first().and_then(|form| match &form.kind {
                FormKind::Atom(name) => special_operator_name(name),
                _ => None,
            });
            match operator.as_deref() {
                Some("SETF") => {
                    if function_name.len() != 2 {
                        return Err(CompileError::new(
                            CompileErrorKind::InvalidForm {
                                message: "FUNCTION SETF designator needs a symbol".to_string(),
                            },
                            argument.span,
                        ));
                    }
                    let (name, _) =
                        self.symbol_name_info(&function_name[1], "SETF function name")?;
                    self.emit(
                        function,
                        Instruction::SetfFunctionLoad(unqualified_name(&name)),
                        function_name[1].span,
                    )?;
                }
                Some("LAMBDA") => self.compile_expression(function, argument)?,
                _ => {
                    return Err(CompileError::new(
                        CompileErrorKind::InvalidForm {
                            message:
                                "FUNCTION argument must be a symbol, LAMBDA form, or (SETF symbol)"
                                    .to_string(),
                        },
                        argument.span,
                    ));
                }
            }
        } else if matches!(argument.kind, FormKind::Atom(_)) {
            let (name, escaped) = self.symbol_name_info(argument, "function name")?;
            let local_function = self.is_local_function(&Self::local_function_key(&name, escaped));
            self.emit(
                function,
                if local_function && escaped {
                    Instruction::FunctionLoadExact(name)
                } else if local_function {
                    Instruction::FunctionLoad(name)
                } else if escaped {
                    Instruction::FunctionLoadExact(name)
                } else {
                    Instruction::FunctionLoad(name)
                },
                argument.span,
            )?;
        } else {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "FUNCTION argument must be a symbol, LAMBDA form, or (SETF symbol)"
                        .to_string(),
                },
                argument.span,
            ));
        }
        Ok(())
    }

    fn compile_define(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        self.require_arity(items, "DEFINE", "two", 2, span)?;
        let Some(name_form) = items.get(1) else {
            return Err(self.internal_error(span, "missing define name after arity check"));
        };
        let Some(value_form) = items.get(2) else {
            return Err(self.internal_error(span, "missing define value after arity check"));
        };
        let (name, escaped) = self.symbol_name_info(name_form, "define name")?;
        self.compile_expression(function, value_form)?;
        let instruction = if escaped {
            Instruction::DefineExact(name)
        } else {
            Instruction::Define(name)
        };
        self.emit(function, instruction, value_form.span)?;
        Ok(())
    }

    fn compile_defun(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 4 {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "defun needs a name, parameters, and a body".to_string(),
                },
                operator_span(items, span),
            ));
        }
        let Some(name_form) = items.get(1) else {
            return Err(self.internal_error(span, "missing defun name after arity check"));
        };
        let Some(parameter_form) = items.get(2) else {
            return Err(self.internal_error(span, "missing defun parameters after arity check"));
        };
        let (name, name_escaped) = self.symbol_name_info(name_form, "defun name")?;
        let lambda_list = self.parameters(parameter_form)?;
        let child = self.reserve_function_with_rest(
            Some(name.clone()),
            lambda_list.required,
            lambda_list.required_escaped,
            lambda_list.rest,
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
        let (documentation, body) = split_documentation_body(items.get(3..).unwrap_or(&[]));
        self.functions[child].documentation = documentation;
        let (special_names, special_exact_names) =
            split_special_names(self.declared_special_names(body)?);
        self.functions[child].special_names = special_names;
        self.functions[child].special_exact_names = special_exact_names;
        self.compile_sequence(child, body)?;
        self.emit(child, Instruction::Return, span)?;

        self.emit(function, Instruction::MakeClosure(child), span)?;
        let define = if name_escaped {
            Instruction::DefineFunctionExact(name.clone())
        } else {
            Instruction::DefineFunction(name.clone())
        };
        self.emit(function, define, span)?;
        let constant = if name_escaped {
            Constant::SymbolExact(name)
        } else {
            Constant::Symbol(name)
        };
        self.emit(function, Instruction::Constant(constant), span)?;
        Ok(())
    }

    fn compile_setq(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 || items.len() % 2 == 0 {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "setq needs variable/value pairs".to_string(),
                },
                operator_span(items, span),
            ));
        }
        let operands = items.get(1..).unwrap_or(&[]);
        let pair_count = operands.len() / 2;
        for (index, pair) in operands.chunks_exact(2).enumerate() {
            let Some(name_form) = pair.first() else {
                return Err(self.internal_error(span, "missing setq target"));
            };
            let Some(value_form) = pair.get(1) else {
                return Err(self.internal_error(span, "missing setq value"));
            };
            let (name, escaped) = self.symbol_name_info(name_form, "setq target")?;
            self.compile_expression(function, value_form)?;
            let instruction = if escaped {
                Instruction::SetExact(name)
            } else {
                Instruction::Set(name)
            };
            self.emit(function, instruction, value_form.span)?;
            if index + 1 < pair_count {
                self.emit(function, Instruction::Pop, value_form.span)?;
            }
        }
        Ok(())
    }

    fn compile_psetq(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 || items.len() % 2 == 0 {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "psetq needs variable/value pairs".to_string(),
                },
                operator_span(items, span),
            ));
        }
        let operands = items.get(1..).unwrap_or(&[]);
        let mut names = Vec::with_capacity(operands.len() / 2);
        for pair in operands.chunks_exact(2) {
            let Some(name_form) = pair.first() else {
                return Err(self.internal_error(span, "missing psetq target"));
            };
            names.push(self.symbol_name_info(name_form, "psetq target")?);
        }
        for pair in operands.chunks_exact(2) {
            let Some(value_form) = pair.get(1) else {
                return Err(self.internal_error(span, "missing psetq value"));
            };
            self.compile_expression(function, value_form)?;
        }
        let has_exact = names.iter().any(|(_, escaped)| *escaped);
        let instruction = if has_exact {
            Instruction::PsetqExact(names)
        } else {
            Instruction::Psetq(names.into_iter().map(|(name, _)| name).collect())
        };
        self.emit(function, instruction, span)?;
        Ok(())
    }

    fn compile_multiple_value_setq(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        self.require_arity(items, "MULTIPLE-VALUE-SETQ", "two", 2, span)?;
        let Some(variable_form) = items.get(1) else {
            return Err(self.internal_error(span, "missing MULTIPLE-VALUE-SETQ variables"));
        };
        let FormKind::List(variables) = &variable_form.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedList {
                    context: "MULTIPLE-VALUE-SETQ variables".to_string(),
                },
                variable_form.span,
            ));
        };
        let names = variables
            .iter()
            .map(|variable| self.symbol_name_info(variable, "MULTIPLE-VALUE-SETQ variable"))
            .collect::<Result<Vec<_>, _>>()?;
        let Some(value_form) = items.get(2) else {
            return Err(self.internal_error(span, "missing MULTIPLE-VALUE-SETQ value"));
        };
        self.compile_expression(function, value_form)?;
        let has_exact = names.iter().any(|(_, escaped)| *escaped);
        let instruction = if has_exact {
            Instruction::MultipleValueSetqExact(names)
        } else {
            Instruction::MultipleValueSetq(names.into_iter().map(|(name, _)| name).collect())
        };
        self.emit(function, instruction, value_form.span)?;
        Ok(())
    }

    fn compile_setf(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 || items.len() % 2 == 0 {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "setf needs place/value pairs".to_string(),
                },
                operator_span(items, span),
            ));
        }
        let operands = items.get(1..).unwrap_or(&[]);
        let pair_count = operands.len() / 2;
        for (index, pair) in operands.chunks_exact(2).enumerate() {
            let Some(place) = pair.first() else {
                return Err(self.internal_error(span, "missing setf place"));
            };
            let Some(value_form) = pair.get(1) else {
                return Err(self.internal_error(span, "missing setf value"));
            };
            self.compile_expression(function, value_form)?;
            self.emit(function, Instruction::Setf(place.clone()), place.span)?;
            if index + 1 < pair_count {
                self.emit(function, Instruction::Pop, value_form.span)?;
            }
        }
        Ok(())
    }

    fn compile_psetf(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 || items.len() % 2 == 0 {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "psetf needs place/value pairs".to_string(),
                },
                operator_span(items, span),
            ));
        }
        if items.len() == 3 {
            self.compile_setf(function, span, items)?;
            self.emit(function, Instruction::Pop, span)?;
            self.emit(function, Instruction::Constant(Constant::Nil), span)?;
            return Ok(());
        }

        let operands = items.get(1..).unwrap_or(&[]);
        let mut names = Vec::with_capacity(operands.len() / 2);
        for pair in operands.chunks_exact(2) {
            let Some(place) = pair.first() else {
                return Err(self.internal_error(span, "missing psetf place"));
            };
            if !matches!(&place.kind, FormKind::Atom(_)) {
                return self.compile_runtime_definition(function, span, items);
            }
            let Ok(name) = self.symbol_name_info(place, "psetf target") else {
                return self.compile_runtime_definition(function, span, items);
            };
            names.push(name);
        }

        for pair in operands.chunks_exact(2) {
            let Some(value_form) = pair.get(1) else {
                return Err(self.internal_error(span, "missing psetf value"));
            };
            self.compile_expression(function, value_form)?;
        }

        let has_exact = names.iter().any(|(_, escaped)| *escaped);
        let instruction = if has_exact {
            Instruction::PsetqExact(names)
        } else {
            Instruction::Psetq(names.into_iter().map(|(name, _)| name).collect())
        };
        self.emit(function, instruction, span)?;
        Ok(())
    }

    fn compile_push(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() != 3 {
            return self.compile_runtime_definition(function, span, items);
        }
        let place = items
            .get(2)
            .ok_or_else(|| self.internal_error(span, "missing push place"))?;
        if !matches!(&place.kind, FormKind::Atom(_)) {
            return self.compile_runtime_definition(function, span, items);
        }
        let Ok((name, escaped)) = self.symbol_name_info(place, "push target") else {
            return self.compile_runtime_definition(function, span, items);
        };
        self.compile_expression(function, &items[1])?;
        let instruction = if escaped {
            Instruction::PushExact(name)
        } else {
            Instruction::Push(name)
        };
        self.emit(function, instruction, span)?;
        Ok(())
    }

    fn compile_pop(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() != 2 {
            return self.compile_runtime_definition(function, span, items);
        }
        let place = items
            .get(1)
            .ok_or_else(|| self.internal_error(span, "missing pop place"))?;
        if !matches!(&place.kind, FormKind::Atom(_)) {
            return self.compile_runtime_definition(function, span, items);
        }
        let Ok((name, escaped)) = self.symbol_name_info(place, "pop target") else {
            return self.compile_runtime_definition(function, span, items);
        };
        let instruction = if escaped {
            Instruction::PopPlaceExact(name)
        } else {
            Instruction::PopPlace(name)
        };
        self.emit(function, instruction, span)?;
        Ok(())
    }

    fn compile_pushnew(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() != 3 {
            return self.compile_runtime_definition(function, span, items);
        }
        let place = items
            .get(2)
            .ok_or_else(|| self.internal_error(span, "missing pushnew place"))?;
        if !matches!(&place.kind, FormKind::Atom(_)) {
            return self.compile_runtime_definition(function, span, items);
        }
        let Ok((name, escaped)) = self.symbol_name_info(place, "pushnew target") else {
            return self.compile_runtime_definition(function, span, items);
        };
        self.compile_expression(function, &items[1])?;
        let instruction = if escaped {
            Instruction::PushNewExact(name)
        } else {
            Instruction::PushNew(name)
        };
        self.emit(function, instruction, span)?;
        Ok(())
    }

    fn compile_rotatef(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        let places = items.get(1..).unwrap_or(&[]);
        let mut names = Vec::with_capacity(places.len());
        for place in places {
            if !matches!(&place.kind, FormKind::Atom(_)) {
                return self.compile_runtime_definition(function, span, items);
            }
            let Ok(name) = self.symbol_name_info(place, "rotatef target") else {
                return self.compile_runtime_definition(function, span, items);
            };
            names.push(name);
        }

        if names.is_empty() {
            self.emit(function, Instruction::Constant(Constant::Nil), span)?;
            return Ok(());
        }

        for place in places {
            self.compile_expression(function, place)?;
        }
        if names.len() == 1 {
            self.emit(function, Instruction::Pop, span)?;
            self.emit(function, Instruction::Constant(Constant::Nil), span)?;
            return Ok(());
        }

        let has_exact = names.iter().any(|(_, escaped)| *escaped);
        let instruction = if has_exact {
            Instruction::RotatefExact(names)
        } else {
            Instruction::Rotatef(names.into_iter().map(|(name, _)| name).collect())
        };
        self.emit(function, instruction, span)?;
        Ok(())
    }

    fn compile_shiftf(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 {
            return self.compile_runtime_definition(function, span, items);
        }
        let places = items.get(1..items.len() - 1).unwrap_or(&[]);
        let mut names = Vec::with_capacity(places.len());
        for place in places {
            if !matches!(&place.kind, FormKind::Atom(_)) {
                return self.compile_runtime_definition(function, span, items);
            }
            let Ok(name) = self.symbol_name_info(place, "shiftf target") else {
                return self.compile_runtime_definition(function, span, items);
            };
            names.push(name);
        }

        for place in places {
            self.compile_expression(function, place)?;
        }
        let new_value = items
            .last()
            .ok_or_else(|| self.internal_error(span, "missing shiftf value"))?;
        self.compile_expression(function, new_value)?;

        let has_exact = names.iter().any(|(_, escaped)| *escaped);
        let instruction = if has_exact {
            Instruction::ShiftfExact(names)
        } else {
            Instruction::Shiftf(names.into_iter().map(|(name, _)| name).collect())
        };
        self.emit(function, instruction, span)?;
        Ok(())
    }

    fn compile_modify_symbol(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        operator: &str,
        arithmetic: &str,
    ) -> Result<(), CompileError> {
        if !(items.len() == 2 || items.len() == 3) {
            return Err(self.arity_error(items, operator, "one or two", span));
        }
        let place = items
            .get(1)
            .ok_or_else(|| self.internal_error(span, "missing modifying place"))?;
        let (name, escaped) = self.symbol_name_info(place, &format!("{operator} target"))?;
        self.emit(
            function,
            Instruction::FunctionLoad(arithmetic.to_string()),
            place.span,
        )?;
        self.compile_expression(function, place)?;
        if let Some(delta) = items.get(2) {
            self.compile_expression(function, delta)?;
        } else {
            self.emit(function, Instruction::Constant(Constant::Integer(1)), span)?;
        }
        self.emit(function, Instruction::Call(2), span)?;
        self.emit(
            function,
            if escaped {
                Instruction::SetExact(name)
            } else {
                Instruction::Set(name)
            },
            place.span,
        )?;
        Ok(())
    }

    fn compile_defvar(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        force: bool,
    ) -> Result<(), CompileError> {
        let operator = if force { "DEFPARAMETER" } else { "DEFVAR" };
        if !(items.len() == 2 || items.len() == 3) {
            return Err(self.arity_error(items, operator, "one or two", span));
        }
        let name_form = items
            .get(1)
            .ok_or_else(|| self.internal_error(span, "missing defvar name"))?;
        let (name, escaped) = self.symbol_name_info(
            name_form,
            if force {
                "defparameter name"
            } else {
                "defvar name"
            },
        )?;
        if force {
            if let Some(initializer) = items.get(2) {
                self.compile_expression(function, initializer)?;
            } else {
                self.emit(function, Instruction::Constant(Constant::Nil), span)?;
            }
            self.emit(
                function,
                if escaped {
                    Instruction::DefineSpecialExact { name, force: true }
                } else {
                    Instruction::DefineSpecial { name, force: true }
                },
                span,
            )?;
            return Ok(());
        }

        self.emit(
            function,
            if escaped {
                Instruction::IsBoundExact(name.clone())
            } else {
                Instruction::IsBound(name.clone())
            },
            name_form.span,
        )?;
        let initialize_jump = self.emit(function, Instruction::JumpIfFalse(usize::MAX), span)?;
        self.emit(
            function,
            if escaped {
                Instruction::LoadExact(name.clone())
            } else {
                Instruction::Load(name.clone())
            },
            name_form.span,
        )?;
        let end_jump = self.emit(function, Instruction::Jump(usize::MAX), span)?;
        let initialize_target = self.instruction_count(function, span)?;
        if let Some(initializer) = items.get(2) {
            self.compile_expression(function, initializer)?;
        } else {
            self.emit(function, Instruction::Constant(Constant::Nil), span)?;
        }
        self.emit(
            function,
            if escaped {
                Instruction::DefineSpecialExact { name, force: false }
            } else {
                Instruction::DefineSpecial { name, force: false }
            },
            span,
        )?;
        let end_target = self.instruction_count(function, span)?;
        self.patch_jump(function, initialize_jump, initialize_target, span)?;
        self.patch_jump(function, end_jump, end_target, span)?;
        Ok(())
    }

    fn compile_defstruct(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(self.arity_error(items, "DEFSTRUCT", "at least one", span));
        }
        self.emit(
            function,
            Instruction::Quote(Form::list(items.to_vec(), span)),
            span,
        )?;
        self.emit(function, Instruction::Eval(span), span)?;
        Ok(())
    }

    fn compile_runtime_definition(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(self.arity_error(items, "runtime definition", "at least one", span));
        }
        self.emit(
            function,
            Instruction::Quote(Form::list(items.to_vec(), span)),
            span,
        )?;
        self.emit(function, Instruction::Eval(span), span)?;
        Ok(())
    }

    fn compile_macrolet_environment(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() != 3 {
            return Err(self.arity_error(items, "NCL-MACROLET-ENVIRONMENT", "two", span));
        }
        self.emit(
            function,
            Instruction::EnterMacroletEnvironment(items[1].clone()),
            items[1].span,
        )?;
        self.compile_expression(function, &items[2])?;
        self.emit(function, Instruction::ExitScope, span)?;
        Ok(())
    }

    fn compile_funcall(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(self.arity_error(items, "FUNCALL", "at least one", span));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::Call(items.len().saturating_sub(2)),
            span,
        )?;
        Ok(())
    }

    fn compile_eval(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if !(2..=3).contains(&items.len()) {
            return Err(self.arity_error(items, "EVAL", "one or two", span));
        }
        let Some(argument) = items.get(1) else {
            return Err(self.internal_error(span, "missing eval argument after arity check"));
        };
        self.compile_expression(function, argument)?;
        if let Some(environment) = items.get(2) {
            self.compile_expression(function, environment)?;
            self.emit(
                function,
                Instruction::EvalWithEnvironment(argument.span),
                span,
            )?;
        } else {
            self.emit(function, Instruction::Eval(argument.span), span)?;
        }
        Ok(())
    }

    fn compile_apply(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 {
            return Err(self.arity_error(items, "APPLY", "at least two", span));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::Apply(items.len().saturating_sub(2)),
            span,
        )?;
        Ok(())
    }

    fn compile_mapcar(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 {
            return Err(self.arity_error(items, "MAPCAR", "at least two", span));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::MapCar(items.len().saturating_sub(2)),
            span,
        )?;
        Ok(())
    }

    fn compile_maphash(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() != 3 {
            return Err(self.arity_error(items, "MAPHASH", "two", span));
        }
        self.emit(
            function,
            Instruction::FunctionLoad("MAPHASH".to_string()),
            items[0].span,
        )?;
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(function, Instruction::Call(2), span)?;
        Ok(())
    }

    fn compile_map_into(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 {
            return Err(self.arity_error(items, "MAP-INTO", "at least two", span));
        }
        let destination = items[1].clone();
        self.emit(
            function,
            Instruction::FunctionLoad("MAP-INTO".to_string()),
            items[0].span,
        )?;
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::Call(items.len().saturating_sub(1)),
            span,
        )?;
        self.emit(
            function,
            Instruction::MapIntoSetf(destination.clone()),
            destination.span,
        )?;
        Ok(())
    }

    fn compile_dotimes(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(self.arity_error(items, "DOTIMES", "at least one", span));
        }
        let Some(spec_form) = items.get(1) else {
            return Err(self.internal_error(span, "missing DOTIMES binding after arity check"));
        };
        let FormKind::List(spec) = &spec_form.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedList {
                    context: "DOTIMES binding".to_string(),
                },
                spec_form.span,
            ));
        };
        if !(spec.len() == 2 || spec.len() == 3) {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "DOTIMES binding needs a variable, count, and optional result"
                        .to_string(),
                },
                spec_form.span,
            ));
        }
        let Some(variable_form) = spec.first() else {
            return Err(self.internal_error(spec_form.span, "missing DOTIMES variable"));
        };
        let (variable, variable_escaped) =
            self.symbol_name_info(variable_form, "DOTIMES variable")?;
        let Some(count) = spec.get(1) else {
            return Err(self.internal_error(spec_form.span, "missing DOTIMES count"));
        };
        let result = spec.get(2);
        let limit = self.fresh_name("DOTIMES_LIMIT");

        self.emit(function, Instruction::EnterScope, spec_form.span)?;
        self.compile_expression(function, count)?;
        self.emit(function, Instruction::Define(limit.clone()), spec_form.span)?;
        self.emit(function, Instruction::Pop, spec_form.span)?;
        self.emit(
            function,
            Instruction::Constant(Constant::Integer(0)),
            spec_form.span,
        )?;
        let define_variable = if variable_escaped {
            Instruction::DefineExact(variable.clone())
        } else {
            Instruction::Define(variable.clone())
        };
        self.emit(function, define_variable, spec_form.span)?;
        self.emit(function, Instruction::Pop, spec_form.span)?;

        let loop_start = self.instruction_count(function, span)?;
        self.emit(
            function,
            Instruction::FunctionLoad("<".to_string()),
            spec_form.span,
        )?;
        let load_variable = if variable_escaped {
            Instruction::LoadExact(variable.clone())
        } else {
            Instruction::Load(variable.clone())
        };
        self.emit(function, load_variable, spec_form.span)?;
        self.emit(function, Instruction::Load(limit), spec_form.span)?;
        self.emit(function, Instruction::Call(2), spec_form.span)?;
        let exit_jump = self.emit(
            function,
            Instruction::JumpIfFalse(usize::MAX),
            spec_form.span,
        )?;

        let body = items.get(2..).unwrap_or(&[]);
        self.compile_sequence(function, body)?;
        self.emit(function, Instruction::Pop, span)?;
        self.emit(
            function,
            Instruction::FunctionLoad("+".to_string()),
            spec_form.span,
        )?;
        let load_variable = if variable_escaped {
            Instruction::LoadExact(variable.clone())
        } else {
            Instruction::Load(variable.clone())
        };
        self.emit(function, load_variable, spec_form.span)?;
        self.emit(
            function,
            Instruction::Constant(Constant::Integer(1)),
            spec_form.span,
        )?;
        self.emit(function, Instruction::Call(2), spec_form.span)?;
        let set_variable = if variable_escaped {
            Instruction::SetExact(variable)
        } else {
            Instruction::Set(variable)
        };
        self.emit(function, set_variable, spec_form.span)?;
        self.emit(function, Instruction::Pop, spec_form.span)?;
        self.emit(function, Instruction::Jump(loop_start), spec_form.span)?;

        let end = self.instruction_count(function, span)?;
        self.patch_jump(function, exit_jump, end, span)?;
        if let Some(result) = result {
            self.compile_expression(function, result)?;
        } else {
            self.emit(function, Instruction::Constant(Constant::Nil), span)?;
        }
        self.emit(function, Instruction::ExitScope, span)?;
        Ok(())
    }

    fn compile_dolist(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(self.arity_error(items, "DOLIST", "at least one", span));
        }
        let Some(spec_form) = items.get(1) else {
            return Err(self.internal_error(span, "missing DOLIST binding after arity check"));
        };
        let FormKind::List(spec) = &spec_form.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedList {
                    context: "DOLIST binding".to_string(),
                },
                spec_form.span,
            ));
        };
        if !(spec.len() == 2 || spec.len() == 3) {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "DOLIST binding needs a variable, list, and optional result"
                        .to_string(),
                },
                spec_form.span,
            ));
        }
        let Some(variable_form) = spec.first() else {
            return Err(self.internal_error(spec_form.span, "missing DOLIST variable"));
        };
        let (variable, variable_escaped) =
            self.symbol_name_info(variable_form, "DOLIST variable")?;
        let Some(list) = spec.get(1) else {
            return Err(self.internal_error(spec_form.span, "missing DOLIST list"));
        };
        let result = spec.get(2);
        let tail = self.fresh_name("DOLIST_TAIL");

        self.emit(function, Instruction::EnterScope, spec_form.span)?;
        self.compile_expression(function, list)?;
        self.emit(function, Instruction::Define(tail.clone()), spec_form.span)?;
        self.emit(function, Instruction::Pop, spec_form.span)?;
        self.emit(
            function,
            Instruction::Constant(Constant::Nil),
            spec_form.span,
        )?;
        let define_variable = if variable_escaped {
            Instruction::DefineExact(variable.clone())
        } else {
            Instruction::Define(variable.clone())
        };
        self.emit(function, define_variable, spec_form.span)?;
        self.emit(function, Instruction::Pop, spec_form.span)?;

        let loop_start = self.instruction_count(function, span)?;
        self.emit(
            function,
            Instruction::FunctionLoad("ENDP".to_string()),
            spec_form.span,
        )?;
        self.emit(function, Instruction::Load(tail.clone()), spec_form.span)?;
        self.emit(function, Instruction::Call(1), spec_form.span)?;
        let body_jump = self.emit(
            function,
            Instruction::JumpIfFalse(usize::MAX),
            spec_form.span,
        )?;
        let exit_jump = self.emit(function, Instruction::Jump(usize::MAX), spec_form.span)?;

        let body_start = self.instruction_count(function, span)?;
        self.patch_jump(function, body_jump, body_start, span)?;
        self.emit(
            function,
            Instruction::FunctionLoad("CAR".to_string()),
            spec_form.span,
        )?;
        self.emit(function, Instruction::Load(tail.clone()), spec_form.span)?;
        self.emit(function, Instruction::Call(1), spec_form.span)?;
        let set_variable = if variable_escaped {
            Instruction::SetExact(variable.clone())
        } else {
            Instruction::Set(variable.clone())
        };
        self.emit(function, set_variable, spec_form.span)?;
        self.emit(function, Instruction::Pop, spec_form.span)?;

        let body = items.get(2..).unwrap_or(&[]);
        self.compile_sequence(function, body)?;
        self.emit(function, Instruction::Pop, span)?;
        self.emit(
            function,
            Instruction::FunctionLoad("CDR".to_string()),
            spec_form.span,
        )?;
        self.emit(function, Instruction::Load(tail.clone()), spec_form.span)?;
        self.emit(function, Instruction::Call(1), spec_form.span)?;
        self.emit(function, Instruction::Set(tail), spec_form.span)?;
        self.emit(function, Instruction::Pop, spec_form.span)?;
        self.emit(function, Instruction::Jump(loop_start), spec_form.span)?;

        let end = self.instruction_count(function, span)?;
        self.patch_jump(function, exit_jump, end, span)?;
        self.emit(
            function,
            Instruction::Constant(Constant::Nil),
            spec_form.span,
        )?;
        let set_variable = if variable_escaped {
            Instruction::SetExact(variable)
        } else {
            Instruction::Set(variable)
        };
        self.emit(function, set_variable, spec_form.span)?;
        self.emit(function, Instruction::Pop, spec_form.span)?;
        if let Some(result) = result {
            self.compile_expression(function, result)?;
        } else {
            self.emit(function, Instruction::Constant(Constant::Nil), span)?;
        }
        self.emit(function, Instruction::ExitScope, span)?;
        Ok(())
    }

    fn compile_do(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        sequential: bool,
    ) -> Result<(), CompileError> {
        let operator = if sequential { "DO*" } else { "DO" };
        if items.len() < 3 {
            return Err(self.arity_error(items, operator, "at least two", span));
        }
        let Some(binding_form) = items.get(1) else {
            return Err(self.internal_error(span, "missing DO bindings after arity check"));
        };
        let FormKind::List(binding_forms) = &binding_form.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedList {
                    context: "DO bindings".to_string(),
                },
                binding_form.span,
            ));
        };
        let Some(termination_form) = items.get(2) else {
            return Err(self.internal_error(span, "missing DO termination after arity check"));
        };
        let FormKind::List(termination) = &termination_form.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedList {
                    context: "DO termination".to_string(),
                },
                termination_form.span,
            ));
        };
        if termination.is_empty() {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "DO termination needs an end test".to_string(),
                },
                termination_form.span,
            ));
        }

        let mut names = HashSet::new();
        let mut parsed = Vec::with_capacity(binding_forms.len());
        for binding in binding_forms {
            let FormKind::List(parts) = &binding.kind else {
                return Err(CompileError::new(
                    CompileErrorKind::ExpectedList {
                        context: "DO binding".to_string(),
                    },
                    binding.span,
                ));
            };
            if !(1..=3).contains(&parts.len()) {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "DO binding needs a name, optional init, and optional step"
                            .to_string(),
                    },
                    binding.span,
                ));
            }
            let Some(name_form) = parts.first() else {
                return Err(self.internal_error(binding.span, "missing DO binding name"));
            };
            let (name, escaped) = self.symbol_name_info(name_form, "DO binding name")?;
            let key = if escaped {
                format!("\0{name}")
            } else {
                name.clone()
            };
            if !names.insert(key) {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "DO binding names must be unique".to_string(),
                    },
                    name_form.span,
                ));
            }
            parsed.push((name, escaped, parts.get(1).cloned(), parts.get(2).cloned()));
        }

        let loop_function = self.reserve_function(None, Vec::new());
        self.emit(loop_function, Instruction::EnterScope, binding_form.span)?;

        if sequential {
            for (name, escaped, init, _) in &parsed {
                if let Some(init) = init {
                    self.compile_expression(loop_function, init)?;
                } else {
                    self.emit(
                        loop_function,
                        Instruction::Constant(Constant::Nil),
                        binding_form.span,
                    )?;
                }
                let define = if *escaped {
                    Instruction::DefineExact(name.clone())
                } else {
                    Instruction::Define(name.clone())
                };
                self.emit(loop_function, define, binding_form.span)?;
                self.emit(loop_function, Instruction::Pop, binding_form.span)?;
            }
        } else {
            let mut initial_temporaries = Vec::with_capacity(parsed.len());
            for (_, _, init, _) in &parsed {
                if let Some(init) = init {
                    self.compile_expression(loop_function, init)?;
                } else {
                    self.emit(
                        loop_function,
                        Instruction::Constant(Constant::Nil),
                        binding_form.span,
                    )?;
                }
                let temporary = self.fresh_name("DO_INIT");
                self.emit(
                    loop_function,
                    Instruction::Define(temporary.clone()),
                    binding_form.span,
                )?;
                self.emit(loop_function, Instruction::Pop, binding_form.span)?;
                initial_temporaries.push(temporary);
            }
            for ((name, escaped, _, _), temporary) in parsed.iter().zip(initial_temporaries) {
                self.emit(
                    loop_function,
                    Instruction::Load(temporary),
                    binding_form.span,
                )?;
                let define = if *escaped {
                    Instruction::DefineExact(name.clone())
                } else {
                    Instruction::Define(name.clone())
                };
                self.emit(loop_function, define, binding_form.span)?;
                self.emit(loop_function, Instruction::Pop, binding_form.span)?;
            }
        }

        let loop_start = self.instruction_count(loop_function, span)?;
        self.compile_expression(loop_function, &termination[0])?;
        let body_jump = self.emit(
            loop_function,
            Instruction::JumpIfFalse(usize::MAX),
            termination_form.span,
        )?;
        let result_jump = self.emit(
            loop_function,
            Instruction::Jump(usize::MAX),
            termination_form.span,
        )?;
        let body_start = self.instruction_count(loop_function, span)?;
        self.patch_jump(loop_function, body_jump, body_start, span)?;

        self.compile_tagbody_forms(loop_function, span, items.get(3..).unwrap_or(&[]))?;
        self.emit(loop_function, Instruction::Pop, span)?;

        if sequential {
            for (name, escaped, _, step) in &parsed {
                if let Some(step) = step {
                    self.compile_expression(loop_function, step)?;
                    let set = if *escaped {
                        Instruction::SetExact(name.clone())
                    } else {
                        Instruction::Set(name.clone())
                    };
                    self.emit(loop_function, set, binding_form.span)?;
                    self.emit(loop_function, Instruction::Pop, binding_form.span)?;
                }
            }
        } else {
            let mut step_temporaries = Vec::with_capacity(parsed.len());
            for (_, _, _, step) in &parsed {
                if let Some(step) = step {
                    self.compile_expression(loop_function, step)?;
                    let temporary = self.fresh_name("DO_STEP");
                    self.emit(
                        loop_function,
                        Instruction::Define(temporary.clone()),
                        binding_form.span,
                    )?;
                    self.emit(loop_function, Instruction::Pop, binding_form.span)?;
                    step_temporaries.push(Some(temporary));
                } else {
                    step_temporaries.push(None);
                }
            }
            for ((name, escaped, _, _), temporary) in parsed.iter().zip(step_temporaries) {
                if let Some(temporary) = temporary {
                    self.emit(
                        loop_function,
                        Instruction::Load(temporary),
                        binding_form.span,
                    )?;
                    let set = if *escaped {
                        Instruction::SetExact(name.clone())
                    } else {
                        Instruction::Set(name.clone())
                    };
                    self.emit(loop_function, set, binding_form.span)?;
                    self.emit(loop_function, Instruction::Pop, binding_form.span)?;
                }
            }
        }
        self.emit(loop_function, Instruction::Jump(loop_start), span)?;

        let result_start = self.instruction_count(loop_function, span)?;
        self.patch_jump(loop_function, result_jump, result_start, span)?;
        self.compile_sequence(loop_function, termination.get(1..).unwrap_or(&[]))?;
        self.emit(loop_function, Instruction::ExitScope, span)?;
        self.emit(loop_function, Instruction::Return, span)?;
        self.emit(
            function,
            Instruction::Block {
                function: loop_function,
                name: "NIL".to_string(),
            },
            span,
        )?;
        Ok(())
    }

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
            return Err(self.internal_error(span, "missing let bindings after arity check"));
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
                return Err(self.internal_error(binding.span, "missing let binding name"));
            };
            let (name, escaped) = self.symbol_name_info(name_form, "let binding name")?;
            if !sequential && !names.insert((name.clone(), escaped)) {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "let bindings must have distinct names".to_string(),
                    },
                    name_form.span,
                ));
            }
            parsed.push((name, escaped, binding_items.get(1)));
        }

        let body = items.get(2..).unwrap_or(&[]);
        let declared_special_names = self.declared_special_names(body)?;
        let (special_names, exact_special_names) =
            split_special_names(declared_special_names.clone());
        let has_specials = !special_names.is_empty() || !exact_special_names.is_empty();

        self.emit(function, Instruction::EnterScope, binding_form.span)?;
        if has_specials {
            self.emit(
                function,
                Instruction::EnterSpecialScope {
                    names: special_names,
                    exact_names: exact_special_names,
                },
                binding_form.span,
            )?;
        }
        if sequential {
            for (name, escaped, value) in &parsed {
                if let Some(value) = value {
                    self.compile_expression(function, value)?;
                } else {
                    self.emit(
                        function,
                        Instruction::Constant(Constant::Nil),
                        binding_form.span,
                    )?;
                }
                let define = if declared_special_names.contains(&(name.clone(), *escaped)) {
                    if *escaped {
                        Instruction::DefineDynamicExact(name.clone())
                    } else {
                        Instruction::DefineDynamic(name.clone())
                    }
                } else if *escaped {
                    Instruction::DefineExact(name.clone())
                } else {
                    Instruction::Define(name.clone())
                };
                self.emit(function, define, binding_form.span)?;
                self.emit(function, Instruction::Pop, binding_form.span)?;
            }
        } else {
            for (_, _, value) in &parsed {
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
            for (name, escaped, _) in parsed.iter().rev() {
                let define = if declared_special_names.contains(&(name.clone(), *escaped)) {
                    if *escaped {
                        Instruction::DefineDynamicExact(name.clone())
                    } else {
                        Instruction::DefineDynamic(name.clone())
                    }
                } else if *escaped {
                    Instruction::DefineExact(name.clone())
                } else {
                    Instruction::Define(name.clone())
                };
                self.emit(function, define, binding_form.span)?;
                self.emit(function, Instruction::Pop, binding_form.span)?;
            }
        }

        self.compile_sequence(function, body)?;
        if has_specials {
            self.emit(function, Instruction::ExitSpecialScope, span)?;
        }
        self.emit(function, Instruction::ExitScope, span)?;
        Ok(())
    }

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
            return Err(
                self.internal_error(span, "missing local function bindings after arity check")
            );
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
                return Err(self.internal_error(
                    binding.span,
                    "missing local function name after arity check",
                ));
            };
            let (name, name_escaped) = self.symbol_name_info(name_form, "local function name")?;
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
                return Err(self.internal_error(
                    binding.span,
                    "missing local function parameters after arity check",
                ));
            };
            let lambda_list = self.parameters(parameter_form)?;
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
            let (special_names, special_exact_names) =
                split_special_names(self.declared_special_names(body)?);
            self.functions[child].special_names = special_names;
            self.functions[child].special_exact_names = special_exact_names;
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

    fn parameters(&self, form: &Form) -> Result<OrdinaryLambdaList, CompileError> {
        parse_ordinary_lambda_list(form).map_err(|error| {
            let span = error.span;
            let kind = match error.kind {
                LambdaListErrorKind::ExpectedList => CompileErrorKind::ExpectedList {
                    context: "parameters".to_string(),
                },
                LambdaListErrorKind::ExpectedSymbol { context } => {
                    CompileErrorKind::ExpectedSymbol {
                        context: context.to_string(),
                    }
                }
                LambdaListErrorKind::InvalidForm { message } => {
                    CompileErrorKind::InvalidForm { message }
                }
            };
            CompileError::new(kind, span)
        })
    }

    fn compile_optional_parameters(
        &mut self,
        specifications: &[LambdaListOptionalParameter],
    ) -> Result<Vec<OptionalParameter>, CompileError> {
        let mut optional = Vec::with_capacity(specifications.len());
        for specification in specifications {
            let default_function = self.reserve_function(None, Vec::new());
            self.compile_expression(default_function, &specification.init_form)?;
            self.emit(
                default_function,
                Instruction::Return,
                specification.init_form.span,
            )?;
            optional.push(OptionalParameter {
                name: specification.name.clone(),
                name_escaped: specification.name_escaped,
                default_function,
                supplied_p: specification.supplied_p.clone(),
                supplied_p_escaped: specification.supplied_p_escaped,
            });
        }
        Ok(optional)
    }

    fn compile_auxiliary_parameters(
        &mut self,
        specifications: &[LambdaListAuxiliaryParameter],
    ) -> Result<Vec<AuxiliaryParameter>, CompileError> {
        let mut auxiliary = Vec::with_capacity(specifications.len());
        for specification in specifications {
            let default_function = self.reserve_function(None, Vec::new());
            self.compile_expression(default_function, &specification.init_form)?;
            self.emit(
                default_function,
                Instruction::Return,
                specification.init_form.span,
            )?;
            auxiliary.push(AuxiliaryParameter {
                name: specification.name.clone(),
                name_escaped: specification.name_escaped,
                default_function,
            });
        }
        Ok(auxiliary)
    }

    fn compile_keyword_parameters(
        &mut self,
        specifications: &[LambdaListKeywordParameter],
    ) -> Result<Vec<KeywordParameter>, CompileError> {
        let mut keywords = Vec::with_capacity(specifications.len());
        for specification in specifications {
            let default_function = self.reserve_function(None, Vec::new());
            self.compile_expression(default_function, &specification.init_form)?;
            self.emit(
                default_function,
                Instruction::Return,
                specification.init_form.span,
            )?;
            keywords.push(KeywordParameter {
                keyword_name: specification.keyword_name.clone(),
                keyword_name_escaped: specification.keyword_name_escaped,
                name: specification.name.clone(),
                name_escaped: specification.name_escaped,
                default_function,
                supplied_p: specification.supplied_p.clone(),
                supplied_p_escaped: specification.supplied_p_escaped,
            });
        }
        Ok(keywords)
    }

    fn symbol_name_info(&self, form: &Form, context: &str) -> Result<(String, bool), CompileError> {
        let FormKind::Atom(name) = &form.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedSymbol {
                    context: context.to_string(),
                },
                form.span,
            ));
        };
        let Ok(token) = parse_symbol_token(name) else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedSymbol {
                    context: context.to_string(),
                },
                form.span,
            ));
        };
        if token.kind != SymbolTokenKind::Symbol || token.name.is_empty() {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedSymbol {
                    context: context.to_string(),
                },
                form.span,
            ));
        }
        if token.escaped {
            if token.package.is_some() {
                return Err(CompileError::new(
                    CompileErrorKind::ExpectedSymbol {
                        context: context.to_string(),
                    },
                    form.span,
                ));
            }
            return Ok((token.name, true));
        }
        if literal_constant(name).is_some() || name.starts_with(':') {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedSymbol {
                    context: context.to_string(),
                },
                form.span,
            ));
        }
        Ok((normalize_name(name), false))
    }

    fn symbol_name(&self, form: &Form, context: &str) -> Result<String, CompileError> {
        self.symbol_name_info(form, context).map(|(name, _)| name)
    }

    fn condition_name(&self, form: &Form, context: &str) -> Result<String, CompileError> {
        Ok(self
            .control_name(form, context)?
            .trim_start_matches(':')
            .to_string())
    }

    fn control_name(&self, form: &Form, context: &str) -> Result<String, CompileError> {
        match &form.kind {
            FormKind::Atom(name)
                if !name.is_empty()
                    && ((name.starts_with(':') && name.len() > 1)
                        || (!name.starts_with(':')
                            && (literal_constant(name).is_none()
                                || name.eq_ignore_ascii_case("nil")
                                || name.eq_ignore_ascii_case("t")))) =>
            {
                Ok(normalize_name(name))
            }
            _ => Err(CompileError::new(
                CompileErrorKind::ExpectedSymbol {
                    context: context.to_string(),
                },
                form.span,
            )),
        }
    }

    fn control_tag(&self, form: &Form, context: &str) -> Result<String, CompileError> {
        tag_name(form).ok_or_else(|| {
            CompileError::new(
                CompileErrorKind::ExpectedSymbol {
                    context: context.to_string(),
                },
                form.span,
            )
        })
    }

    fn require_arity(
        &self,
        items: &[Form],
        operator: &str,
        expected: &str,
        expected_count: usize,
        span: Span,
    ) -> Result<(), CompileError> {
        if items.len().saturating_sub(1) != expected_count {
            return Err(self.arity_error(items, operator, expected, span));
        }
        Ok(())
    }

    fn arity_error(
        &self,
        items: &[Form],
        operator: &str,
        expected: &str,
        span: Span,
    ) -> CompileError {
        CompileError::new(
            CompileErrorKind::Arity {
                operator: operator.to_string(),
                expected: expected.to_string(),
                actual: items.len().saturating_sub(1),
            },
            span,
        )
    }

    fn internal_error(&self, span: Span, message: &str) -> CompileError {
        CompileError::new(
            CompileErrorKind::Internal {
                message: message.to_string(),
            },
            span,
        )
    }
}

fn normalize_name(name: &str) -> String {
    name.to_ascii_uppercase()
}

fn macro_keyword_name(form: &Form) -> Option<(String, bool)> {
    let FormKind::Atom(name) = &form.kind else {
        return None;
    };
    let token = parse_symbol_token(name).ok()?;
    if token.kind != SymbolTokenKind::Keyword || token.package.is_some() || token.name.is_empty() {
        return None;
    }
    if token.escaped {
        Some((token.name, true))
    } else {
        Some((normalize_name(&token.name), false))
    }
}

fn unqualified_name(name: &str) -> String {
    let normalized = normalize_name(name);
    normalized
        .split_once("::")
        .or_else(|| normalized.split_once(':'))
        .map_or(normalized.clone(), |(_, symbol)| symbol.to_string())
}

fn is_no_error_marker(form: &Form) -> bool {
    let FormKind::Atom(name) = &form.kind else {
        return false;
    };
    let Ok(token) = parse_symbol_token(name) else {
        return false;
    };
    token.kind == SymbolTokenKind::Keyword
        && token.package.is_none()
        && !token.escaped
        && token.name.eq_ignore_ascii_case("NO-ERROR")
}

fn operator_span(items: &[Form], fallback: Span) -> Span {
    items.first().map_or(fallback, |form| form.span)
}

fn symbol_reference(atom: &str) -> Option<(String, bool)> {
    let token = parse_symbol_token(atom).ok()?;
    if token.kind != SymbolTokenKind::Symbol {
        return None;
    }
    if token.escaped {
        return token.package.is_none().then_some((token.name, true));
    }
    Some((normalize_name(atom), false))
}

fn split_documentation_body(forms: &[Form]) -> (Option<String>, &[Form]) {
    match forms.first().map(|form| &form.kind) {
        Some(FormKind::String(documentation)) => (Some(documentation.clone()), &forms[1..]),
        _ => (None, forms),
    }
}

fn split_special_names(names: HashSet<(String, bool)>) -> (Vec<String>, Vec<String>) {
    let mut normal = Vec::new();
    let mut exact = Vec::new();
    for (name, escaped) in names {
        if escaped {
            exact.push(name);
        } else {
            normal.push(name);
        }
    }
    (normal, exact)
}

fn special_operator_name(atom: &str) -> Option<String> {
    let token = parse_symbol_token(atom).ok()?;
    if token.kind == SymbolTokenKind::Symbol && token.package.is_none() && !token.escaped {
        Some(normalize_name(&token.name))
    } else {
        None
    }
}

fn case_default_clause(form: &Form) -> bool {
    let FormKind::Atom(atom) = &form.kind else {
        return false;
    };
    let Ok(token) = parse_symbol_token(atom) else {
        return false;
    };
    token.kind == SymbolTokenKind::Symbol
        && !token.escaped
        && (token.name.eq_ignore_ascii_case("T") || token.name.eq_ignore_ascii_case("OTHERWISE"))
}

fn compile_eval_when_executes(form: &Form) -> Result<bool, CompileError> {
    let FormKind::List(situations) = &form.kind else {
        return Err(CompileError::new(
            CompileErrorKind::ExpectedList {
                context: "EVAL-WHEN situations".to_string(),
            },
            form.span,
        ));
    };
    let mut executes = false;
    for situation in situations {
        let FormKind::Atom(name) = &situation.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedSymbol {
                    context: "EVAL-WHEN situation".to_string(),
                },
                situation.span,
            ));
        };
        let Ok(token) = parse_symbol_token(name) else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedSymbol {
                    context: "EVAL-WHEN situation".to_string(),
                },
                situation.span,
            ));
        };
        if token.kind == SymbolTokenKind::Uninterned
            || (token.kind == SymbolTokenKind::Symbol && literal_constant(name).is_some())
        {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedSymbol {
                    context: "EVAL-WHEN situation".to_string(),
                },
                situation.span,
            ));
        }
        if token.package.is_none() && token.name.eq_ignore_ascii_case("execute") {
            executes = true;
        }
    }
    Ok(executes)
}

fn literal_constant(atom: &str) -> Option<Constant> {
    let token = parse_symbol_token(atom).ok()?;
    match token.kind {
        SymbolTokenKind::Keyword => {
            if token.escaped {
                Some(Constant::KeywordExact(token.name))
            } else {
                Some(Constant::Keyword(normalize_name(&token.name)))
            }
        }
        SymbolTokenKind::Symbol if token.package.is_none() && !token.escaped => {
            if token.name.eq_ignore_ascii_case("nil") || token.name.eq_ignore_ascii_case("#f") {
                return Some(Constant::Nil);
            }
            if token.name.eq_ignore_ascii_case("t") || token.name.eq_ignore_ascii_case("#t") {
                return Some(Constant::Boolean(true));
            }
            if let Ok(value) = token.name.parse::<i64>() {
                return Some(Constant::Integer(value));
            }
            if let Some(value) = parse_radix_integer_literal(&token.name) {
                return Some(Constant::Integer(value));
            }
            if let Some((numerator, denominator)) = rational_literal_parts(&token.name) {
                return if denominator == 1 {
                    Some(Constant::Integer(numerator))
                } else {
                    Some(Constant::Rational {
                        numerator,
                        denominator,
                    })
                };
            }
            parse_float_literal(&token.name).map(Constant::Float)
        }
        _ => None,
    }
}

fn rational_literal_parts(name: &str) -> Option<(i64, i64)> {
    let (numerator, denominator) = name.split_once('/')?;
    if numerator.is_empty()
        || denominator.is_empty()
        || numerator.contains('/')
        || denominator.contains('/')
    {
        return None;
    }
    let numerator = numerator.parse::<i128>().ok()?;
    let denominator = denominator.parse::<i128>().ok()?;
    if denominator == 0 {
        return None;
    }
    let (numerator, denominator) = if denominator < 0 {
        (numerator.checked_neg()?, denominator.checked_neg()?)
    } else {
        (numerator, denominator)
    };
    let numerator_abs = if numerator < 0 {
        numerator.checked_neg()? as u128
    } else {
        numerator as u128
    };
    let divisor = gcd(numerator_abs, denominator as u128);
    let numerator = i64::try_from(numerator / divisor as i128).ok()?;
    let denominator = i64::try_from(denominator / divisor as i128).ok()?;
    Some((numerator, denominator))
}

fn gcd(mut left: u128, mut right: u128) -> u128 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

fn tag_name(form: &Form) -> Option<String> {
    let FormKind::Atom(name) = &form.kind else {
        return None;
    };
    if name.is_empty() || name == ":" {
        return None;
    }
    if name.starts_with(':') {
        return (name.len() > 1).then(|| normalize_name(name));
    }
    if name.eq_ignore_ascii_case("nil")
        || name.eq_ignore_ascii_case("t")
        || name.parse::<i64>().is_ok()
        || literal_constant(name).is_none()
    {
        Some(normalize_name(name))
    } else {
        None
    }
}
