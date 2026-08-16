use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::rc::Rc;

use ncl_compiler::{Compiler, Program};
use ncl_syntax::{
    Form, FormKind, LambdaListAuxiliaryParameter, LambdaListKeywordParameter,
    LambdaListOptionalParameter, OrdinaryLambdaList, Span, SymbolTokenKind,
    parse_ordinary_lambda_list, parse_symbol_token, read,
};

use crate::builtins;
use crate::environment::normalize_name;
use crate::error::ThrowTag;
use crate::package::{self, PackageState};
use crate::value::{
    ClassDefinition, ClassSlot, ClosureData, ConditionDefinition, ConditionSlot,
    MacroAuxiliaryParameter, MacroKeywordParameter, MacroLambdaList, MacroOptionalParameter,
    MacroPattern, MethodDefinition, MethodSpecializer, StructureDefinition, StructureSlot,
};
use crate::{Environment, ReturnValue, RuntimeError, Value};

const MAX_MACRO_EXPANSIONS: usize = 64;

#[derive(Clone, Copy, Eq, PartialEq)]
enum MacroLambdaListSection {
    Required,
    Optional,
    Rest,
    Keyword,
    Auxiliary,
}

#[derive(Default)]
struct DynamicState {
    special_names: HashSet<String>,
    exact_special_names: HashSet<String>,
    constants: HashSet<String>,
    exact_constants: HashSet<String>,
    globals: HashMap<String, Value>,
    exact_globals: HashMap<String, Value>,
    bindings: Vec<(String, Value)>,
    exact_bindings: Vec<(String, Value)>,
    condition_handlers: Vec<ConditionHandlerBinding>,
    restart_bindings: Vec<RestartBinding>,
    condition_restart_bindings: Vec<ConditionRestartBinding>,
}

#[derive(Clone)]
pub(crate) struct ConditionHandlerBinding {
    pub(crate) condition: String,
    pub(crate) function: Option<Value>,
    pub(crate) catch: bool,
}

#[derive(Clone)]
pub(crate) struct RestartBinding {
    pub(crate) name: String,
    pub(crate) function: Option<Value>,
    restart: Value,
}

impl RestartBinding {
    pub(crate) fn new(name: String, function: Option<Value>) -> Self {
        let restart = Value::restart(&name);
        Self {
            name,
            function,
            restart,
        }
    }
}

#[derive(Clone)]
pub(crate) struct ConditionRestartBinding {
    pub(crate) condition: Value,
    pub(crate) restarts: Vec<Value>,
}

struct SetfExpansion {
    temporaries: Vec<Form>,
    values: Vec<Form>,
    store: Form,
    store_form: Form,
    access_form: Form,
}

struct MacroInvocation<'a> {
    form: &'a Form,
    arguments: &'a [Form],
    macro_name: &'a str,
    lambda_list: &'a MacroLambdaList,
    macro_environment: &'a Environment,
    environment: &'a Environment,
}

struct LongDefsetfInvocation<'a> {
    place: &'a Form,
    accessor_name: &'a str,
    arguments: &'a [Form],
    lambda_list: &'a MacroLambdaList,
    store_variable: &'a str,
    body: &'a [Form],
    macro_environment: &'a Environment,
    environment: &'a Environment,
}

#[derive(Clone, Copy)]
struct EvaluationContext<'a> {
    environment: &'a Environment,
    span: Span,
}

struct CoreMethodInvocation<'a> {
    dispatch: &'a GenericDispatch,
    before: &'a [MethodDefinition],
    primary: &'a [MethodDefinition],
    after: &'a [MethodDefinition],
    default: Option<&'a GenericDefaultAction>,
    arguments: &'a [Value],
    context: EvaluationContext<'a>,
}

struct StructureConstructorInvocation<'a> {
    name: &'a str,
    slots: &'a [StructureSlot],
    structure_types: &'a [String],
    lambda_list: &'a OrdinaryLambdaList,
    definition_environment: &'a Environment,
    arguments: &'a [Value],
    span: Span,
}

struct ClosureLambdaForm<'a> {
    parameters: &'a [String],
    required_escaped: &'a [bool],
    optional: &'a [LambdaListOptionalParameter],
    rest: &'a Option<String>,
    rest_escaped: bool,
    keywords: &'a [LambdaListKeywordParameter],
    has_keyword_section: bool,
    allow_other_keys: bool,
    auxiliary: &'a [LambdaListAuxiliaryParameter],
    body: &'a [Form],
}

pub(crate) struct DynamicGuard {
    state: Rc<RefCell<DynamicState>>,
    depth: usize,
    exact_depth: usize,
}

impl Drop for DynamicGuard {
    fn drop(&mut self) {
        let mut state = self.state.borrow_mut();
        state.bindings.truncate(self.depth);
        state.exact_bindings.truncate(self.exact_depth);
    }
}

pub(crate) struct ConditionHandlerGuard {
    state: Rc<RefCell<DynamicState>>,
    depth: usize,
}

impl Drop for ConditionHandlerGuard {
    fn drop(&mut self) {
        self.state
            .borrow_mut()
            .condition_handlers
            .truncate(self.depth);
    }
}

pub(crate) struct RestartGuard {
    state: Rc<RefCell<DynamicState>>,
    depth: usize,
}

impl Drop for RestartGuard {
    fn drop(&mut self) {
        self.state
            .borrow_mut()
            .restart_bindings
            .truncate(self.depth);
    }
}

pub(crate) struct ConditionRestartGuard {
    state: Rc<RefCell<DynamicState>>,
    depth: usize,
}

impl Drop for ConditionRestartGuard {
    fn drop(&mut self) {
        self.state
            .borrow_mut()
            .condition_restart_bindings
            .truncate(self.depth);
    }
}

pub(crate) struct ConditionHandlerSuspension {
    state: Rc<RefCell<DynamicState>>,
    index: usize,
    binding: Option<ConditionHandlerBinding>,
}

impl Drop for ConditionHandlerSuspension {
    fn drop(&mut self) {
        let Some(binding) = self.binding.take() else {
            return;
        };
        let mut state = self.state.borrow_mut();
        let index = self.index.min(state.condition_handlers.len());
        state.condition_handlers.insert(index, binding);
    }
}

#[derive(Clone)]
struct GenericDispatch {
    name: String,
    function: Value,
    methods: Rc<RefCell<Vec<MethodDefinition>>>,
    applicable: Vec<MethodDefinition>,
}

#[derive(Clone)]
enum GenericDefaultAction {
    Value(Value),
    SharedInitialize {
        instance: Value,
        class: Rc<ClassDefinition>,
        slot_names: Value,
        initargs: Vec<(String, Value)>,
        unknown_initarg_message: &'static str,
    },
}

#[derive(Clone)]
enum MethodContinuation {
    Chain {
        dispatch: GenericDispatch,
        methods: Vec<MethodDefinition>,
        index: usize,
        fallback: Option<Box<MethodContinuation>>,
    },
    Core {
        dispatch: GenericDispatch,
        before: Vec<MethodDefinition>,
        primary: Vec<MethodDefinition>,
        after: Vec<MethodDefinition>,
        default: Option<GenericDefaultAction>,
    },
    Default(GenericDefaultAction),
}

struct MethodContext {
    dispatch: GenericDispatch,
    method: Value,
    arguments: Vec<Value>,
    next: Option<MethodContinuation>,
}

pub struct Runtime {
    global: Environment,
    packages: Rc<RefCell<PackageState>>,
    dynamic: Rc<RefCell<DynamicState>>,
    next_block_target: Cell<u64>,
    gensym_counter: Cell<u64>,
    next_method_id: Cell<u64>,
    method_context: RefCell<Vec<MethodContext>>,
}

#[derive(Clone, Debug)]
pub struct CompiledForm {
    form: Form,
    program: Rc<Program>,
}

impl CompiledForm {
    pub fn form(&self) -> &Form {
        &self.form
    }

    pub fn program(&self) -> &Program {
        &self.program
    }

    pub fn function_count(&self) -> usize {
        self.program.function_count()
    }

    pub fn instruction_count(&self) -> usize {
        self.program.instruction_count()
    }
}

include!("runtime_core.rs");
include!("compiler.rs");
include!("evaluation.rs");
include!("macros.rs");
include!("special_forms.rs");
include!("setf.rs");
include!("definitions.rs");
include!("packages.rs");
include!("sequences.rs");
include!("objects.rs");
include!("conditions.rs");
include!("primitives.rs");
include!("generic.rs");
include!("lambda.rs");
include!("evaluation_helpers.rs");
include!("errors.rs");
include!("default.rs");
include!("helpers.rs");
