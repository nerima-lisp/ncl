use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::rc::Rc;

use ncl_compiler::Compiler;
use ncl_syntax::{
    parse_float_literal, parse_ordinary_lambda_list, parse_radix_integer_literal, parse_symbol_token,
    read_with_features, Form, FormKind, LambdaListKeywordParameter, OrdinaryLambdaList, Span,
    SymbolTokenKind,
};

use crate::builtins;
use crate::environment::normalize_name;
use crate::error::ThrowTag;
use crate::package::{self, PackageState, COMMON_LISP_PACKAGE};
use crate::value::{
    ArrayElementType, ClassDefinition, ClassSlot, DefsetfDefinition, MacroAuxiliaryParameter,
    MacroBinding, MacroKeywordParameter, MacroLambdaList, MacroOptionalParameter, MacroPattern,
    MethodDefinition, MethodSpecializer, RandomState, StructureDefinition,
    StructureRepresentation, StructureSlot,
};
use crate::{Environment, ReturnValue, RuntimeError, Value};

#[path = "evaluator/conditions.rs"]
mod conditions;

#[path = "evaluator/macros.rs"]
mod macros;

#[path = "evaluator/invocation.rs"]
mod invocation;

#[path = "evaluator/sequences.rs"]
mod sequences;

const MAX_MACRO_EXPANSIONS: usize = 64;

const BOOLE_CONSTANTS: [(&str, i64); 16] = [
    ("BOOLE-CLR", 0),
    ("BOOLE-SET", 1),
    ("BOOLE-1", 2),
    ("BOOLE-2", 3),
    ("BOOLE-C1", 4),
    ("BOOLE-C2", 5),
    ("BOOLE-AND", 6),
    ("BOOLE-IOR", 7),
    ("BOOLE-XOR", 8),
    ("BOOLE-EQV", 9),
    ("BOOLE-NAND", 10),
    ("BOOLE-NOR", 11),
    ("BOOLE-ANDC1", 12),
    ("BOOLE-ANDC2", 13),
    ("BOOLE-ORC1", 14),
    ("BOOLE-ORC2", 15),
];

fn c3_class_precedence(
    class_name: &str,
    direct_superclasses: &[String],
    environment: &Environment,
) -> Result<Vec<String>, &'static str> {
    let mut direct = Vec::with_capacity(direct_superclasses.len().max(1));
    for superclass in direct_superclasses {
        let superclass = match superclass.as_str() {
            "OBJECT" | "STANDARD-OBJECT" => "STANDARD-OBJECT".to_owned(),
            _ => superclass.clone(),
        };
        if direct.iter().any(|name| name == &superclass) {
            return Err("duplicate defclass superclass");
        }
        direct.push(superclass);
    }
    if direct.is_empty() {
        direct.push("STANDARD-OBJECT".to_owned());
    }

    let mut sequences = Vec::with_capacity(direct.len() + 1);
    for superclass in &direct {
        if superclass == "STANDARD-OBJECT" {
            sequences.push(vec![superclass.clone()]);
            continue;
        }
        let Some(definition) = environment.lookup_class(superclass) else {
            return Err("unknown defclass superclass");
        };
        sequences.push(definition.precedence.clone());
    }
    sequences.push(direct);

    let mut merged = Vec::new();
    while sequences.iter().any(|sequence| !sequence.is_empty()) {
        let mut candidate = None;
        'candidate: for sequence in &sequences {
            let Some(head) = sequence.first() else {
                continue;
            };
            if sequences
                .iter()
                .any(|other| other.iter().skip(1).any(|name| name == head))
            {
                continue 'candidate;
            }
            candidate = Some(head.clone());
            break;
        }
        let Some(candidate) = candidate else {
            return Err("inconsistent defclass precedence");
        };
        if candidate == class_name {
            return Err("cyclic defclass inheritance");
        }
        merged.push(candidate.clone());
        for sequence in &mut sequences {
            if sequence.first() == Some(&candidate) {
                sequence.remove(0);
            }
        }
    }

    let mut precedence = Vec::with_capacity(merged.len() + 1);
    precedence.push(class_name.to_owned());
    precedence.extend(merged);
    Ok(precedence)
}

fn split_documentation_body(forms: &[Form]) -> (Option<String>, &[Form]) {
    match forms.first().map(|form| &form.kind) {
        Some(FormKind::String(documentation)) => (Some(documentation.clone()), &forms[1..]),
        _ => (None, forms),
    }
}

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
    scoped_special_names: Vec<String>,
    scoped_exact_special_names: Vec<String>,
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
    stores: Vec<Form>,
    store_form: Form,
    access_form: Form,
    current_place: Option<Form>,
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

pub(crate) struct SpecialGuard {
    state: Rc<RefCell<DynamicState>>,
    depth: usize,
    exact_depth: usize,
}

impl Drop for SpecialGuard {
    fn drop(&mut self) {
        let mut state = self.state.borrow_mut();
        state.scoped_special_names.truncate(self.depth);
        state.scoped_exact_special_names.truncate(self.exact_depth);
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
enum MethodContinuation {
    Chain {
        methods: Vec<MethodDefinition>,
        index: usize,
        fallback: Option<Box<MethodContinuation>>,
    },
    Core {
        before: Vec<MethodDefinition>,
        primary: Vec<MethodDefinition>,
        after: Vec<MethodDefinition>,
    },
}

struct MethodContext {
    arguments: Vec<Value>,
    next: Option<MethodContinuation>,
}

pub struct Runtime {
    global: Environment,
    packages: Rc<RefCell<PackageState>>,
    dynamic: Rc<RefCell<DynamicState>>,
    next_block_target: Cell<u64>,
    gensym_counter: Cell<u64>,
    method_context: RefCell<Vec<MethodContext>>,
    reader_features: Vec<String>,
}

impl Runtime {
    pub fn new() -> Self {
        let global = Environment::new();
        builtins::install(&global);
        let packages = Rc::new(RefCell::new(PackageState::new()));
        let dynamic = Rc::new(RefCell::new(DynamicState::default()));
        let current_package = packages.borrow().current().to_string();
        let random_state = Rc::new(RefCell::new(RandomState::seeded()));
        let mut dynamic_state = dynamic.borrow_mut();
        dynamic_state
            .globals
            .insert("*PACKAGE*".to_string(), Value::package(&current_package));
        dynamic_state.special_names.insert("*PACKAGE*".to_string());
        dynamic_state.globals.insert(
            "*STANDARD-INPUT*".to_string(),
            Value::string_input_stream("", 0, 0),
        );
        dynamic_state
            .special_names
            .insert("*STANDARD-INPUT*".to_string());
        for binding_name in [
            "*RANDOM-STATE*".to_string(),
            format!("{COMMON_LISP_PACKAGE}::*RANDOM-STATE*"),
        ] {
            dynamic_state.special_names.insert(binding_name.clone());
            dynamic_state
                .globals
                .insert(binding_name, Value::random_state_value(Rc::clone(&random_state)));
        }
        for binding_name in [
            "*FEATURES*".to_string(),
            format!("{COMMON_LISP_PACKAGE}::*FEATURES*"),
        ] {
            dynamic_state.special_names.insert(binding_name.clone());
            dynamic_state.globals.insert(binding_name, Value::Nil);
        }
        for (name, value) in BOOLE_CONSTANTS {
            for binding_name in [name.to_string(), format!("{COMMON_LISP_PACKAGE}::{name}")] {
                dynamic_state.special_names.insert(binding_name.clone());
                dynamic_state.constants.insert(binding_name.clone());
                dynamic_state
                    .globals
                    .insert(binding_name, Value::Integer(value));
            }
        }
        drop(dynamic_state);
        Self {
            global,
            packages,
            dynamic,
            next_block_target: Cell::new(1),
            gensym_counter: Cell::new(0),
            method_context: RefCell::new(Vec::new()),
            reader_features: Vec::new(),
        }
    }

    pub fn with_reader_features<I, S>(mut self, features: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.reader_features = features
            .into_iter()
            .map(|feature| feature.as_ref().to_owned())
            .collect();
        let feature_value = Value::list(
            self.reader_features
                .iter()
                .map(|feature| Value::keyword(feature))
                .collect(),
        );
        let mut dynamic = self.dynamic.borrow_mut();
        for binding_name in [
            "*FEATURES*".to_string(),
            format!("{COMMON_LISP_PACKAGE}::*FEATURES*"),
        ] {
            dynamic.special_names.insert(binding_name.clone());
            dynamic.globals.insert(binding_name, feature_value.clone());
        }
        drop(dynamic);
        self
    }

    fn random_state_for(
        &self,
        environment: &Environment,
        span: Span,
    ) -> Result<Rc<RefCell<RandomState>>, RuntimeError> {
        let value = self
            .lookup_in("*RANDOM-STATE*", environment)
            .ok_or_else(|| RuntimeError::UnboundVariable {
                name: "*RANDOM-STATE*".to_string(),
                span: Some(span),
            })?;
        let actual = value.type_name().to_string();
        value.random_state_reference().ok_or(RuntimeError::Type {
            expected: "RANDOM-STATE".to_string(),
            actual,
            span: Some(span),
        })
    }

    fn reader_features_for(
        &self,
        environment: &Environment,
        span: Span,
    ) -> Result<Vec<String>, RuntimeError> {
        let feature_value = self
            .lookup_symbol_value_in("*FEATURES*", environment)
            .unwrap_or_else(|| {
                Value::list(
                    self.reader_features
                        .iter()
                        .map(|feature| Value::keyword(feature))
                        .collect(),
                )
            });
        let Some(features) = feature_value.list_items() else {
            return Err(RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: feature_value.type_name().to_string(),
                span: Some(span),
            });
        };
        features
            .into_iter()
            .map(|feature| {
                if !matches!(
                    &feature,
                    Value::Symbol(_)
                        | Value::SymbolExact(_)
                        | Value::QualifiedSymbolExact { .. }
                        | Value::UninternedSymbol(_)
                        | Value::Keyword(_)
                        | Value::KeywordExact(_)
                ) {
                    return Err(RuntimeError::Type {
                        expected: "SYMBOL".to_string(),
                        actual: feature.type_name().to_string(),
                        span: Some(span),
                    });
                }
                let name = feature.symbol_name().ok_or_else(|| RuntimeError::Type {
                    expected: "SYMBOL".to_string(),
                    actual: feature.type_name().to_string(),
                    span: Some(span),
                })?;
                Ok(name.to_owned())
            })
            .collect()
    }

    pub fn global_environment(&self) -> Environment {
        self.global.clone()
    }

    pub fn current_package(&self) -> String {
        self.packages.borrow().current().to_string()
    }

    fn active_package_name(&self) -> String {
        match self.lookup_in("*PACKAGE*", &self.global) {
            Some(Value::Package(package)) => self
                .packages
                .borrow()
                .package_object_name(package.as_ref()),
            _ => self.current_package(),
        }
    }

    pub(crate) fn fresh_block_target(&self) -> u64 {
        let target = self.next_block_target.get();
        self.next_block_target.set(target.wrapping_add(1));
        target
    }

    pub fn eval(&self, form: &Form) -> Result<Value, RuntimeError> {
        let resolved = self.resolve_form(form)?;
        self.eval_in(&resolved, &self.global)
    }

    pub fn eval_source(&self, source: &str) -> Result<Vec<Value>, RuntimeError> {
        read_with_features(source, self.reader_features.iter())?
            .iter()
            .map(|form| self.eval(form))
            .collect()
    }

    pub fn eval_compiled(&self, form: &Form) -> Result<Value, RuntimeError> {
        let resolved = self.resolve_form(form)?;
        let expanded = self.prepare_compiled_form(&resolved, &self.global)?;
        let program = Rc::new(Compiler::compile_form(&expanded)?);
        crate::vm::run_entry(self, program, 0, self.global.clone(), expanded.span)
            .map(|value| value.primary_value())
    }

    pub fn eval_compiled_source(&self, source: &str) -> Result<Vec<Value>, RuntimeError> {
        read_with_features(source, self.reader_features.iter())?
            .iter()
            .map(|form| self.eval_compiled(form))
            .collect()
    }

    fn resolve_form(&self, form: &Form) -> Result<Form, RuntimeError> {
        let current = self.current_package();
        self.resolve_form_in(form, &current)
    }

    fn resolve_form_in(&self, form: &Form, current: &str) -> Result<Form, RuntimeError> {
        let kind = match &form.kind {
            FormKind::ReadTimeEval(inner) => {
                let resolved_inner = self.resolve_form_in(inner, current)?;
                let value = self.eval_in(&resolved_inner, &self.global)?;
                let evaluated = self.form_from_value(&value, form.span)?;
                return self.resolve_form_in(&evaluated, current);
            }
            FormKind::Atom(atom) => {
                let escaped = parse_symbol_token(atom)
                    .map(|token| token.escaped)
                    .unwrap_or(false);
                if escaped {
                    FormKind::Atom(atom.clone())
                } else {
                    FormKind::Atom(self.resolve_atom(atom, current, form.span)?)
                }
            }
            FormKind::String(value) => FormKind::String(value.clone()),
            FormKind::Character(value) => FormKind::Character(*value),
            FormKind::Complex { real, imaginary } => FormKind::Complex {
                real: Box::new(self.resolve_form_in(real, current)?),
                imaginary: Box::new(self.resolve_form_in(imaginary, current)?),
            },
            FormKind::List(items) => {
                let mut resolved = Vec::with_capacity(items.len());
                for (index, item) in items.iter().enumerate() {
                    if index == 0 && is_special_form(item) {
                        resolved.push(Form::atom(
                            normalize_name(atom_name(item).unwrap_or_default()),
                            item.span,
                        ));
                    } else {
                        resolved.push(self.resolve_form_in(item, current)?);
                    }
                }
                FormKind::List(resolved)
            }
            FormKind::DottedList { items, tail } => FormKind::DottedList {
                items: items
                    .iter()
                    .map(|item| self.resolve_form_in(item, current))
                    .collect::<Result<Vec<_>, _>>()?,
                tail: Box::new(self.resolve_form_in(tail, current)?),
            },
            FormKind::BitVector(bits) => FormKind::BitVector(bits.clone()),
            FormKind::Vector(items) => FormKind::Vector(
                items
                    .iter()
                    .map(|item| self.resolve_form_in(item, current))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
        };
        Ok(Form::new(kind, form.span))
    }

    fn resolve_atom(&self, atom: &str, current: &str, span: Span) -> Result<String, RuntimeError> {
        let token =
            parse_symbol_token(atom).map_err(|_| self.package_error("invalid symbol", span))?;
        match token.kind {
            SymbolTokenKind::Uninterned => {
                return Ok(format!("#:{}", token.name));
            }
            SymbolTokenKind::Keyword => {
                return Ok(format!(":{}", token.name));
            }
            SymbolTokenKind::Symbol => {}
        }

        if token.package.is_none()
            && !token.escaped
            && (literal_atom(atom).is_some() || token.name.starts_with('&'))
        {
            return Ok(normalize_name(&token.name));
        }

        if let Some(package_name) = token.package.as_deref() {
            let package_name = package::normalize_package_name(package_name);
            let symbol_name = normalize_name(&token.name);
            if package_name.is_empty() || symbol_name.is_empty() {
                return Err(self.package_error("invalid package-qualified symbol", span));
            }
            let package_name = {
                let packages = self.packages.borrow();
                let package_name = packages.canonical_package_name_for(current, &package_name);
                if !packages.package_exists(&package_name) {
                    return Err(
                        self.package_error(&format!("unknown package {package_name}"), span)
                    );
                }
                if token.external && !packages.is_exported(&package_name, &symbol_name) {
                    return Err(self.package_error(
                        &format!(
                            "symbol {symbol_name} is not exported from package {package_name}"
                        ),
                        span,
                    ));
                }
                package_name
            };
            self.packages
                .borrow_mut()
                .ensure_symbol(&package_name, &symbol_name);
            return Ok(package::canonical_symbol_name(&package_name, &symbol_name));
        }

        let normalized = normalize_name(&token.name);
        if !token.escaped && self.dynamic.borrow().special_names.contains(&normalized) {
            return Ok(normalized);
        }

        let package_name = if current == package::DEFAULT_PACKAGE {
            package::DEFAULT_PACKAGE.to_string()
        } else {
            current.to_string()
        };
        self.packages
            .borrow_mut()
            .ensure_symbol(&package_name, &normalized);
        Ok(package::canonical_symbol_name(&package_name, &normalized))
    }

    fn package_error(&self, message: &str, span: Span) -> RuntimeError {
        RuntimeError::Package {
            message: message.to_string(),
            span: Some(span),
        }
    }

    pub(crate) fn lookup_in(&self, name: &str, environment: &Environment) -> Option<Value> {
        let candidates = self.dynamic_candidates(name);
        let dynamic = self.dynamic.borrow();
        if let Some(value) = dynamic
            .bindings
            .iter()
            .rev()
            .find(|(binding, _)| candidates.iter().any(|candidate| candidate == binding))
            .map(|(_, value)| value.clone())
        {
            return Some(value);
        }
        if let Some(value) = candidates
            .iter()
            .find_map(|candidate| dynamic.globals.get(candidate).cloned())
        {
            return Some(value);
        }
        if candidates.iter().any(|candidate| {
            dynamic.special_names.contains(candidate)
                || dynamic
                    .scoped_special_names
                    .iter()
                    .any(|special| special == candidate)
        }) {
            return None;
        }
        drop(dynamic);
        if let Some(value) = environment.lookup(name) {
            return Some(value);
        }
        candidates
            .into_iter()
            .find_map(|candidate| environment.lookup(&candidate))
    }

    pub(crate) fn lookup_function_in(
        &self,
        name: &str,
        environment: &Environment,
    ) -> Option<Value> {
        if let Some(value) = environment.lookup_function(name) {
            return Some(value);
        }
        self.dynamic_candidates(name)
            .into_iter()
            .find_map(|candidate| environment.lookup_function(&candidate))
    }

    pub(crate) fn lookup_callable_in(
        &self,
        name: &str,
        environment: &Environment,
    ) -> Option<Value> {
        self.lookup_function_in(name, environment)
            .or_else(|| self.lookup_in(name, environment))
    }

    pub(crate) fn lookup_exact_in(&self, name: &str, environment: &Environment) -> Option<Value> {
        let dynamic = self.dynamic.borrow();
        if let Some(value) = dynamic
            .exact_bindings
            .iter()
            .rev()
            .find(|(binding, _)| binding == name)
            .map(|(_, value)| value.clone())
        {
            return Some(value);
        }
        if let Some(value) = dynamic.exact_globals.get(name).cloned() {
            return Some(value);
        }
        if dynamic.exact_special_names.contains(name)
            || dynamic
                .scoped_exact_special_names
                .iter()
                .any(|special| special == name)
        {
            return None;
        }
        drop(dynamic);
        environment.lookup_exact(name)
    }

    pub(crate) fn lookup_symbol_value_in(
        &self,
        name: &str,
        _environment: &Environment,
    ) -> Option<Value> {
        let candidates = self.dynamic_candidates(name);
        let dynamic = self.dynamic.borrow();
        if let Some(value) = dynamic
            .bindings
            .iter()
            .rev()
            .find(|(binding, _)| candidates.iter().any(|candidate| candidate == binding))
            .map(|(_, value)| value.clone())
        {
            return Some(value);
        }
        if let Some(value) = candidates
            .iter()
            .find_map(|candidate| dynamic.globals.get(candidate).cloned())
        {
            return Some(value);
        }
        if candidates.iter().any(|candidate| {
            dynamic.special_names.contains(candidate)
                || dynamic
                    .scoped_special_names
                    .iter()
                    .any(|special| special == candidate)
        }) {
            return None;
        }
        drop(dynamic);
        candidates
            .into_iter()
            .find_map(|candidate| self.global.lookup(&candidate))
    }

    pub(crate) fn lookup_symbol_value_exact_in(
        &self,
        name: &str,
        _environment: &Environment,
    ) -> Option<Value> {
        let dynamic = self.dynamic.borrow();
        if let Some(value) = dynamic
            .exact_bindings
            .iter()
            .rev()
            .find(|(binding, _)| binding == name)
            .map(|(_, value)| value.clone())
        {
            return Some(value);
        }
        if let Some(value) = dynamic.exact_globals.get(name).cloned() {
            return Some(value);
        }
        if dynamic.exact_special_names.contains(name)
            || dynamic
                .scoped_exact_special_names
                .iter()
                .any(|special| special == name)
        {
            return None;
        }
        drop(dynamic);
        self.global.lookup_exact(name)
    }

    pub(crate) fn lookup_function_exact_in(
        &self,
        name: &str,
        environment: &Environment,
    ) -> Option<Value> {
        environment.lookup_function_exact(name)
    }

    pub(crate) fn lookup_callable_exact_in(
        &self,
        name: &str,
        environment: &Environment,
    ) -> Option<Value> {
        self.lookup_function_exact_in(name, environment)
            .or_else(|| self.lookup_exact_in(name, environment))
    }

    pub(crate) fn is_bound_in(&self, name: &str, environment: &Environment) -> bool {
        self.lookup_symbol_value_in(name, environment).is_some()
    }

    pub(crate) fn is_bound_exact_in(&self, name: &str, environment: &Environment) -> bool {
        self.lookup_symbol_value_exact_in(name, environment)
            .is_some()
    }

    pub(crate) fn define_in(&self, name: &str, value: Value, environment: &Environment) {
        let candidates = self.dynamic_candidates(name);
        if let Some(binding_name) = candidates
            .iter()
            .find(|candidate| {
                let dynamic = self.dynamic.borrow();
                dynamic.special_names.contains(*candidate)
                    || dynamic
                        .scoped_special_names
                        .iter()
                        .any(|special| special == *candidate)
            })
            .cloned()
        {
            self.dynamic
                .borrow_mut()
                .bindings
                .push((binding_name, value));
            return;
        }
        environment.define(name, value);
    }

    pub(crate) fn set_in(&self, name: &str, value: Value, environment: &Environment) -> bool {
        let candidates = self.dynamic_candidates(name);
        {
            let mut dynamic = self.dynamic.borrow_mut();
            if let Some(index) =
                dynamic.bindings.iter().rev().position(|(binding, _)| {
                    candidates.iter().any(|candidate| candidate == binding)
                })
            {
                let index = dynamic.bindings.len() - 1 - index;
                let binding = dynamic.bindings[index].0.clone();
                if dynamic.constants.contains(&binding) {
                    return false;
                }
                dynamic.bindings[index].1 = value;
                return true;
            }
            if let Some(candidate) = candidates.iter().find(|candidate| {
                dynamic.special_names.contains(*candidate)
                    || dynamic
                        .scoped_special_names
                        .iter()
                        .any(|special| special == *candidate)
            }) {
                if dynamic.constants.contains(candidate) {
                    return false;
                }
                dynamic.globals.insert(candidate.clone(), value);
                return true;
            }
        }
        if environment.set(name, value.clone()) {
            return true;
        }
        candidates
            .into_iter()
            .any(|candidate| environment.set(&candidate, value.clone()))
    }

    pub(crate) fn define_exact_in(&self, name: &str, value: Value, environment: &Environment) {
        let is_special = {
            let dynamic = self.dynamic.borrow();
            dynamic.exact_special_names.contains(name)
                || dynamic
                    .scoped_exact_special_names
                    .iter()
                    .any(|special| special == name)
        };
        if is_special {
            self.dynamic
                .borrow_mut()
                .exact_bindings
                .push((name.to_string(), value));
            return;
        }
        environment.define_exact(name, value);
    }

    pub(crate) fn set_exact_in(&self, name: &str, value: Value, environment: &Environment) -> bool {
        {
            let mut dynamic = self.dynamic.borrow_mut();
            if let Some(index) = dynamic
                .exact_bindings
                .iter()
                .rev()
                .position(|(binding, _)| binding == name)
            {
                let index = dynamic.exact_bindings.len() - 1 - index;
                let binding = dynamic.exact_bindings[index].0.clone();
                if dynamic.exact_constants.contains(&binding) {
                    return false;
                }
                dynamic.exact_bindings[index].1 = value;
                return true;
            }
            if dynamic.exact_special_names.contains(name)
                || dynamic
                    .scoped_exact_special_names
                    .iter()
                    .any(|special| special == name)
            {
                if dynamic.exact_constants.contains(name) {
                    return false;
                }
                dynamic.exact_globals.insert(name.to_string(), value);
                return true;
            }
        }
        environment.set_exact(name, value)
    }

    pub(crate) fn dynamic_guard(&self) -> DynamicGuard {
        DynamicGuard {
            state: self.dynamic.clone(),
            depth: self.dynamic.borrow().bindings.len(),
            exact_depth: self.dynamic.borrow().exact_bindings.len(),
        }
    }

    pub(crate) fn special_declaration_guard(
        &self,
        names: &[String],
        exact_names: &[String],
    ) -> SpecialGuard {
        let mut state = self.dynamic.borrow_mut();
        let depth = state.scoped_special_names.len();
        let exact_depth = state.scoped_exact_special_names.len();
        state.scoped_special_names.extend(names.iter().cloned());
        state
            .scoped_exact_special_names
            .extend(exact_names.iter().cloned());
        SpecialGuard {
            state: self.dynamic.clone(),
            depth,
            exact_depth,
        }
    }

    pub(crate) fn condition_handler_guard(
        &self,
        handlers: Vec<ConditionHandlerBinding>,
    ) -> ConditionHandlerGuard {
        let mut state = self.dynamic.borrow_mut();
        let depth = state.condition_handlers.len();
        state.condition_handlers.extend(handlers);
        ConditionHandlerGuard {
            state: self.dynamic.clone(),
            depth,
        }
    }

    pub(crate) fn condition_handlers(&self) -> Vec<ConditionHandlerBinding> {
        self.dynamic.borrow().condition_handlers.clone()
    }

    pub(crate) fn suspend_condition_handler(
        &self,
        condition: &str,
    ) -> Option<ConditionHandlerSuspension> {
        let condition = normalize_name(condition);
        let mut state = self.dynamic.borrow_mut();
        let index = state
            .condition_handlers
            .iter()
            .rposition(|handler| normalize_name(&handler.condition) == condition)?;
        let binding = state.condition_handlers.remove(index);
        Some(ConditionHandlerSuspension {
            state: self.dynamic.clone(),
            index,
            binding: Some(binding),
        })
    }

    pub(crate) fn restart_guard(&self, bindings: Vec<RestartBinding>) -> RestartGuard {
        let mut state = self.dynamic.borrow_mut();
        let depth = state.restart_bindings.len();
        state.restart_bindings.extend(bindings);
        RestartGuard {
            state: self.dynamic.clone(),
            depth,
        }
    }

    pub(crate) fn restart_bindings(&self) -> Vec<RestartBinding> {
        self.dynamic.borrow().restart_bindings.clone()
    }

    pub(crate) fn condition_restart_guard(
        &self,
        condition: Value,
        restarts: Vec<Value>,
    ) -> ConditionRestartGuard {
        let mut state = self.dynamic.borrow_mut();
        let depth = state.condition_restart_bindings.len();
        state
            .condition_restart_bindings
            .push(ConditionRestartBinding {
                condition,
                restarts,
            });
        ConditionRestartGuard {
            state: self.dynamic.clone(),
            depth,
        }
    }

    pub(crate) fn condition_restart_bindings(&self) -> Vec<ConditionRestartBinding> {
        self.dynamic.borrow().condition_restart_bindings.clone()
    }

    pub(crate) fn restart_bindings_for_condition(
        &self,
        condition: Option<&Value>,
    ) -> Vec<RestartBinding> {
        let bindings = self.restart_bindings();
        let Some(condition) = condition else {
            return bindings;
        };
        let associations = self.condition_restart_bindings();
        bindings
            .into_iter()
            .filter(|binding| {
                let associated_with_condition = associations.iter().any(|association| {
                    association.condition.eq_value(condition)
                        && association
                            .restarts
                            .iter()
                            .any(|restart| restart.eq_value(&binding.restart))
                });
                let associated_with_any_condition = associations.iter().any(|association| {
                    association
                        .restarts
                        .iter()
                        .any(|restart| restart.eq_value(&binding.restart))
                });
                associated_with_condition || !associated_with_any_condition
            })
            .collect()
    }

    pub(crate) fn dynamic_depth(&self) -> usize {
        self.dynamic.borrow().bindings.len()
    }

    pub(crate) fn truncate_dynamic(&self, depth: usize) {
        self.dynamic.borrow_mut().bindings.truncate(depth);
    }

    pub(crate) fn exact_dynamic_depth(&self) -> usize {
        self.dynamic.borrow().exact_bindings.len()
    }

    pub(crate) fn truncate_exact_dynamic(&self, depth: usize) {
        self.dynamic.borrow_mut().exact_bindings.truncate(depth);
    }

    pub(crate) fn define_dynamic(&self, name: &str, value: Value) {
        let binding_name = self
            .dynamic_candidates(name)
            .into_iter()
            .next()
            .unwrap_or_else(|| normalize_name(name));
        self.dynamic
            .borrow_mut()
            .bindings
            .push((binding_name, value));
    }

    pub(crate) fn define_dynamic_exact(&self, name: &str, value: Value) {
        self.dynamic
            .borrow_mut()
            .exact_bindings
            .push((name.to_string(), value));
    }

    pub(crate) fn declare_special(&self, name: &str, escaped: bool) {
        let mut dynamic = self.dynamic.borrow_mut();
        if escaped {
            dynamic.exact_special_names.insert(name.to_string());
        } else {
            dynamic.special_names.insert(normalize_name(name));
        }
    }

    pub(crate) fn define_special_value(&self, name: &str, value: Value, force: bool) -> Value {
        let name = normalize_name(name);
        let mut dynamic = self.dynamic.borrow_mut();
        dynamic.special_names.insert(name.clone());
        if !force {
            if let Some(existing) = dynamic.globals.get(&name) {
                return existing.clone();
            }
        }
        dynamic.globals.insert(name, value.clone());
        value
    }

    pub(crate) fn define_special_value_exact(
        &self,
        name: &str,
        value: Value,
        force: bool,
    ) -> Value {
        let mut dynamic = self.dynamic.borrow_mut();
        dynamic.exact_special_names.insert(name.to_string());
        if !force {
            if let Some(existing) = dynamic.exact_globals.get(name) {
                return existing.clone();
            }
        }
        dynamic
            .exact_globals
            .insert(name.to_string(), value.clone());
        value
    }

    pub(crate) fn define_constant_value(&self, name: &str, value: Value) -> Value {
        let name = normalize_name(name);
        let mut dynamic = self.dynamic.borrow_mut();
        dynamic.special_names.insert(name.clone());
        dynamic.constants.insert(name.clone());
        dynamic.globals.insert(name, value.clone());
        value
    }

    pub(crate) fn define_constant_value_exact(&self, name: &str, value: Value) -> Value {
        let mut dynamic = self.dynamic.borrow_mut();
        dynamic.exact_special_names.insert(name.to_string());
        dynamic.exact_constants.insert(name.to_string());
        dynamic
            .exact_globals
            .insert(name.to_string(), value.clone());
        value
    }

    pub(crate) fn lookup_special(&self, name: &str) -> Option<Value> {
        let candidates = self.dynamic_candidates(name);
        candidates
            .iter()
            .find_map(|candidate| self.dynamic.borrow().globals.get(candidate).cloned())
    }

    pub(crate) fn lookup_special_exact(&self, name: &str) -> Option<Value> {
        self.dynamic.borrow().exact_globals.get(name).cloned()
    }

    pub(crate) fn is_constant_in(&self, name: &str) -> bool {
        self.dynamic_candidates(name)
            .into_iter()
            .any(|candidate| self.dynamic.borrow().constants.contains(&candidate))
    }

    pub(crate) fn is_constant_exact_in(&self, name: &str) -> bool {
        self.dynamic.borrow().exact_constants.contains(name)
    }

    pub(crate) fn constantp(&self, value: &Value) -> bool {
        match value {
            Value::Nil
            | Value::Boolean(_)
            | Value::Integer(_)
            | Value::Rational(_)
            | Value::Float(_)
            | Value::String(_)
            | Value::Character(_)
            | Value::Keyword(_)
            | Value::KeywordExact(_) => true,
            Value::Symbol(name) => {
                name.eq_ignore_ascii_case("T")
                    || name.eq_ignore_ascii_case("NIL")
                    || self.is_constant_in(name)
            }
            Value::SymbolExact(name) => {
                name.eq_ignore_ascii_case("T")
                    || name.eq_ignore_ascii_case("NIL")
                    || self.is_constant_exact_in(name)
            }
            Value::QualifiedSymbolExact {
                reference,
                package_len,
            } => {
                let name = &reference[*package_len + 2..];
                name.eq_ignore_ascii_case("T")
                    || name.eq_ignore_ascii_case("NIL")
                    || self.is_constant_exact_in(reference)
            }
            _ => false,
        }
    }

    pub(crate) fn constant_modification_error(&self, name: &str, span: Span) -> RuntimeError {
        RuntimeError::InvalidForm {
            message: format!("cannot modify constant {name}"),
            span: Some(span),
        }
    }

    pub(crate) fn set_or_define_in(
        &self,
        name: &str,
        value: Value,
        environment: &Environment,
        span: Span,
    ) -> Result<(), RuntimeError> {
        if self.set_in(name, value.clone(), environment) {
            return Ok(());
        }
        if self.is_constant_in(name) {
            return Err(self.constant_modification_error(name, span));
        }
        self.define_in(name, value, environment);
        Ok(())
    }

    pub(crate) fn set_or_define_exact_in(
        &self,
        name: &str,
        value: Value,
        environment: &Environment,
        span: Span,
    ) -> Result<(), RuntimeError> {
        if self.set_exact_in(name, value.clone(), environment) {
            return Ok(());
        }
        if self.is_constant_exact_in(name) {
            return Err(self.constant_modification_error(name, span));
        }
        self.define_exact_in(name, value, environment);
        Ok(())
    }

    pub(crate) fn set_symbol_value(&self, name: &str, value: Value) -> Value {
        let candidates = self.dynamic_candidates(name);
        let mut dynamic = self.dynamic.borrow_mut();
        if let Some((_, current)) = dynamic
            .bindings
            .iter_mut()
            .rev()
            .find(|(binding, _)| candidates.iter().any(|candidate| candidate == binding))
        {
            *current = value.clone();
            return value;
        }
        let global_name = candidates
            .iter()
            .find(|candidate| dynamic.special_names.contains(*candidate))
            .cloned()
            .unwrap_or_else(|| normalize_name(name));
        dynamic.special_names.insert(global_name.clone());
        dynamic.globals.insert(global_name, value.clone());
        value
    }

    pub(crate) fn set_symbol_value_exact(&self, name: &str, value: Value) -> Value {
        let mut dynamic = self.dynamic.borrow_mut();
        if let Some((_, current)) = dynamic
            .exact_bindings
            .iter_mut()
            .rev()
            .find(|(binding, _)| binding == name)
        {
            *current = value.clone();
            return value;
        }
        dynamic.exact_special_names.insert(name.to_string());
        dynamic
            .exact_globals
            .insert(name.to_string(), value.clone());
        value
    }

    pub(crate) fn makunbound_symbol(&self, name: &str) {
        let candidates = self.dynamic_candidates(name);
        let mut dynamic = self.dynamic.borrow_mut();
        for candidate in candidates {
            dynamic.globals.remove(&candidate);
        }
    }

    fn remove_global_symbol(&self, name: &str) {
        let mut dynamic = self.dynamic.borrow_mut();
        dynamic.globals.remove(name);
        dynamic.special_names.remove(name);
        dynamic.constants.remove(name);
        drop(dynamic);
        self.global.remove(name);
        self.global.remove_function(name);
    }

    pub(crate) fn makunbound_exact_symbol(&self, name: &str) {
        self.dynamic.borrow_mut().exact_globals.remove(name);
    }

    pub(crate) fn fmakunbound_symbol(&self, name: &str) {
        for candidate in self.dynamic_candidates(name) {
            self.global.remove(&candidate);
            self.global.remove_function(&candidate);
        }
    }

    pub(crate) fn fmakunbound_exact_symbol(&self, name: &str) {
        self.global.remove_exact(name);
        self.global.remove_function_exact(name);
    }

    fn dynamic_candidates(&self, name: &str) -> Vec<String> {
        let qualified = package::split_symbol(name).is_some();
        let (package_name, symbol_name) = match package::split_symbol(name) {
            Some((package_name, symbol_name, _)) => (
                package::normalize_package_name(package_name),
                normalize_name(symbol_name),
            ),
            None => (self.current_package(), normalize_name(name)),
        };
        let packages = self.packages.borrow();
        let package_name = packages.canonical_package_name(&package_name);
        let mut candidates = Vec::new();
        if let Some(imported) = packages.imported_symbol_for(&package_name, &symbol_name) {
            candidates.push(imported);
        } else if qualified {
            candidates.push(package::canonical_symbol_name(&package_name, &symbol_name));
        } else {
            candidates.push(normalize_name(name));
        }
        if !packages.is_shadowed(&package_name, &symbol_name) {
            for used in packages.use_packages_for(&package_name) {
                if packages.is_exported(&used, &symbol_name) {
                    let candidate = format!("{used}::{symbol_name}");
                    if !candidates.contains(&candidate) {
                        candidates.push(candidate);
                    }
                }
            }
        }
        candidates
    }

    fn symbol_macro_expansion_for_atom(
        &self,
        atom: &str,
        environment: &Environment,
    ) -> Option<Form> {
        if literal_atom(atom).is_some() {
            return None;
        }

        let (name, escaped) = resolved_symbol(atom);
        if escaped {
            environment.lookup_symbol_macro_exact(&name)
        } else {
            environment.lookup_symbol_macro(&name)
        }
    }

    fn expand_symbol_macro_form(
        &self,
        form: &Form,
        environment: &Environment,
    ) -> Result<Option<Form>, RuntimeError> {
        let mut current = form.clone();
        let mut expanded = false;
        let mut seen = HashSet::<String>::new();

        loop {
            let Some(atom) = atom_name(&current) else {
                return Ok(if expanded { Some(current) } else { None });
            };
            let Some(next) = self.symbol_macro_expansion_for_atom(atom, environment) else {
                return Ok(if expanded { Some(current) } else { None });
            };
            let (name, escaped) = resolved_symbol(atom);
            let key = format!("{}:{}", if escaped { "escaped" } else { "normal" }, name);
            if !seen.insert(key) {
                return Err(self.invalid("recursive symbol macro expansion", form.span));
            }
            expanded = true;
            current = next;
        }
    }

    fn prepare_compiled_form(
        &self,
        form: &Form,
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        if let Some(expanded) = self.expand_symbol_macro_form(form, environment)? {
            return self.prepare_compiled_form(&expanded, environment);
        }

        if is_operator_form(form, "MACROLET") {
            return self.prepare_compiled_macrolet(form, environment);
        }
        if is_operator_form(form, "SYMBOL-MACROLET") {
            return self.prepare_compiled_symbol_macrolet(form, environment);
        }
        if is_operator_form(form, "WITH-OPEN-FILE") {
            let expanded = self.expand_with_open_file(form)?;
            return self.prepare_compiled_form(&expanded, environment);
        }
        if is_operator_form(form, "WITH-OPEN-STREAM") {
            let expanded = self.expand_with_open_stream(form)?;
            return self.prepare_compiled_form(&expanded, environment);
        }
        if is_operator_form(form, "WITH-OUTPUT-TO-STRING") {
            let expanded = self.expand_with_output_to_string(form)?;
            return self.prepare_compiled_form(&expanded, environment);
        }
        if is_operator_form(form, "WITH-INPUT-FROM-STRING") {
            let expanded = self.expand_with_input_from_string(form)?;
            return self.prepare_compiled_form(&expanded, environment);
        }
        if is_operator_form(form, "WITH-HASH-TABLE-ITERATOR") {
            let expanded = self.expand_with_hash_table_iterator(form)?;
            return self.prepare_compiled_form(&expanded, environment);
        }

        if is_operator_form(form, "DEFMACRO")
            || is_operator_form(form, "DEFINE-MODIFY-MACRO")
            || is_operator_form(form, "DEFINE-SETF-EXPANDER")
            || is_operator_form(form, "DEFINE-SYMBOL-MACRO")
            || is_operator_form(form, "DEFPACKAGE")
            || is_operator_form(form, "IN-PACKAGE")
            || Self::is_static_macroexpand_form(form)
        {
            let value = self.eval_values_in(form, environment)?;
            return self.quoted_value_form(&value, form.span);
        }

        let expanded = self.expand_macros(form.clone(), environment)?;
        match &expanded.kind {
            FormKind::List(items) => self.prepare_compiled_list(&expanded, items, environment),
            _ => Ok(expanded),
        }
    }

    fn is_static_macroexpand_form(form: &Form) -> bool {
        let FormKind::List(items) = &form.kind else {
            return false;
        };
        if !(2..=3).contains(&items.len()) {
            return false;
        }
        let Some(operator) = items.first().and_then(atom_name) else {
            return false;
        };
        if !matches!(
            normalize_name(operator).as_str(),
            "MACROEXPAND" | "MACROEXPAND-1"
        ) {
            return false;
        }
        let FormKind::List(argument) = &items[1].kind else {
            return false;
        };
        argument
            .first()
            .and_then(atom_name)
            .is_some_and(|name| normalize_name(name) == "QUOTE")
    }

    fn inject_macrolet_environment(form: &Form, bindings: &Form) -> Form {
        match &form.kind {
            FormKind::Atom(_)
            | FormKind::String(_)
            | FormKind::Character(_)
            | FormKind::Complex { .. }
            | FormKind::BitVector(_)
            | FormKind::ReadTimeEval(_) => form.clone(),
            FormKind::List(items) => {
                if let Some(operator) = items.first().and_then(atom_name) {
                    let name = normalize_name(operator);
                    if matches!(name.as_str(), "MACROEXPAND" | "MACROEXPAND-1" | "EVAL")
                        && items.len() == 2
                    {
                        let environment_form = Form::list(
                            vec![
                                Form::atom("NCL-MACRO-ENVIRONMENT", form.span),
                                bindings.clone(),
                            ],
                            form.span,
                        );
                        let mut rewritten = items.clone();
                        rewritten.push(environment_form);
                        return Form::list(rewritten, form.span);
                    }
                    if matches!(name.as_str(), "QUOTE" | "QUASIQUOTE") {
                        return form.clone();
                    }
                }
                Form::list(
                    items
                        .iter()
                        .map(|item| Self::inject_macrolet_environment(item, bindings))
                        .collect(),
                    form.span,
                )
            }
            FormKind::DottedList { items, tail } => Form::dotted_list(
                items
                    .iter()
                    .map(|item| Self::inject_macrolet_environment(item, bindings))
                    .collect(),
                Self::inject_macrolet_environment(tail, bindings),
                form.span,
            ),
            FormKind::Vector(items) => Form::new(
                FormKind::Vector(
                    items
                        .iter()
                        .map(|item| Self::inject_macrolet_environment(item, bindings))
                        .collect(),
                ),
                form.span,
            ),
        }
    }

    pub(crate) fn make_macrolet_environment(
        &self,
        bindings: &[Form],
        environment: &Environment,
    ) -> Result<Environment, RuntimeError> {
        let local = environment.child();
        let captured = environment.clone();
        let mut names = HashSet::new();
        for binding in bindings {
            let FormKind::List(parts) = &binding.kind else {
                return Err(self.invalid("local macro binding must be a list", binding.span));
            };
            if parts.len() < 3 {
                return Err(self.invalid(
                    "local macro needs a name, parameters, and a body",
                    binding.span,
                ));
            }
            let (name, escaped) =
                self.variable_name_info(&parts[0], "local macro name must be a symbol")?;
            if !names.insert(name.clone()) {
                return Err(self.invalid("local macro names must be unique", parts[0].span));
            }
            let lambda_list = self.macro_parameters(&parts[1])?;
            let function =
                Value::macro_function(lambda_list, parts[2..].to_vec(), captured.clone());
            if escaped {
                local.define_function_exact(name, function);
            } else {
                local.define_function(name, function);
            }
        }
        Ok(local)
    }

    fn prepare_compiled_macrolet(
        &self,
        form: &Form,
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        let FormKind::List(items) = &form.kind else {
            return Ok(form.clone());
        };
        if items.len() < 2 {
            return Err(self.arity("macrolet", "at least one", items.len().saturating_sub(1)));
        }
        let FormKind::List(bindings) = &items[1].kind else {
            return Err(self.invalid("local macro bindings must be a list", items[1].span));
        };

        let local = self.make_macrolet_environment(bindings, environment)?;

        let mut prepared = Vec::with_capacity(items.len().saturating_sub(2) + 1);
        prepared.push(Form::atom("PROGN", form.span));
        for body_form in &items[2..] {
            let prepared_form = self.prepare_compiled_form(body_form, &local)?;
            prepared.push(Self::inject_macrolet_environment(&prepared_form, &items[1]));
        }
        let body = Form::list(prepared, form.span);
        Ok(Form::list(
            vec![
                Form::atom("NCL-MACROLET-ENVIRONMENT", form.span),
                items[1].clone(),
                body,
            ],
            form.span,
        ))
    }

    fn prepare_compiled_symbol_macrolet(
        &self,
        form: &Form,
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        let FormKind::List(items) = &form.kind else {
            return Ok(form.clone());
        };
        if items.len() < 2 {
            return Err(self.arity(
                "symbol-macrolet",
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        let FormKind::List(bindings) = &items[1].kind else {
            return Err(self.invalid("symbol macro bindings must be a list", items[1].span));
        };

        let local = environment.child();
        let mut names = HashSet::new();
        for binding in bindings {
            let FormKind::List(parts) = &binding.kind else {
                return Err(self.invalid("symbol macro binding must be a list", binding.span));
            };
            if parts.len() != 2 {
                return Err(self.invalid(
                    "symbol macro binding needs a name and an expansion",
                    binding.span,
                ));
            }
            let (name, escaped) =
                self.variable_name_info(&parts[0], "symbol macro name must be a symbol")?;
            if !names.insert((name.clone(), escaped)) {
                return Err(self.invalid("symbol macro names must be unique", parts[0].span));
            }
            if escaped {
                local.define_symbol_macro_exact(name, parts[1].clone());
            } else {
                local.define_symbol_macro(name, parts[1].clone());
            }
        }

        let mut prepared = Vec::with_capacity(items.len().saturating_sub(2) + 1);
        prepared.push(Form::atom("PROGN", form.span));
        for body_form in &items[2..] {
            prepared.push(self.prepare_compiled_form(body_form, &local)?);
        }
        Ok(Form::list(prepared, form.span))
    }

    fn prepare_compiled_list(
        &self,
        form: &Form,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        let Some(operator) = items.first().and_then(atom_name) else {
            if items.is_empty() {
                return Ok(form.clone());
            }
            let mut prepared = items.to_vec();
            prepared[0] = self.prepare_compiled_form(&items[0], environment)?;
            self.prepare_tail(&mut prepared, 1, environment)?;
            return Ok(Form::list(prepared, form.span));
        };

        let mut prepared = items.to_vec();
        match normalize_name(operator).as_str() {
            "QUOTE" | "QUASIQUOTE" => return Ok(form.clone()),
            "DECLARE"
            | "DECLAIM"
            | "PROCLAIM"
            | "DEFSTRUCT"
            | "DEFCLASS"
            | "DEFGENERIC"
            | "DEFMETHOD"
            | "DEFSETF"
            | "DEFINE-MODIFY-MACRO"
            | "DEFINE-CONDITION"
            | "DEFCONSTANT" => return Ok(form.clone()),
            "THE" => {
                self.prepare_tail(&mut prepared, 2, environment)?;
            }
            "LOCALLY" => {
                self.prepare_tail(&mut prepared, 1, environment)?;
            }
            "EVAL-WHEN" => {
                if prepared.len() > 1 && self.eval_when_executes(&prepared[1])? {
                    self.prepare_tail(&mut prepared, 2, environment)?;
                }
            }
            "PROGN"
            | "PROG1"
            | "PROG2"
            | "IF"
            | "WHEN"
            | "UNLESS"
            | "AND"
            | "OR"
            | "FUNCALL"
            | "APPLY"
            | "VALUES"
            | "IGNORE-ERRORS"
            | "UNWIND-PROTECT"
            | "MULTIPLE-VALUE-CALL"
            | "MULTIPLE-VALUE-LIST"
            | "MULTIPLE-VALUE-PROG1" => {
                self.prepare_tail(&mut prepared, 1, environment)?;
            }
            "WITH-SIMPLE-RESTART" => {
                self.prepare_tail(&mut prepared, 2, environment)?;
            }
            "RESTART-CASE" => {
                if prepared.len() > 1 {
                    prepared[1] = self.prepare_compiled_form(&prepared[1], environment)?;
                }
                for clause in prepared.iter_mut().skip(2) {
                    *clause = self.prepare_restart_case_clause(clause, environment)?;
                }
            }
            "CATCH" => {
                if prepared.len() > 1 {
                    prepared[1] = self.prepare_compiled_form(&prepared[1], environment)?;
                }
                self.prepare_tail(&mut prepared, 2, environment)?;
            }
            "PROGV" => {
                if prepared.len() > 1 {
                    prepared[1] = self.prepare_compiled_form(&prepared[1], environment)?;
                }
                if prepared.len() > 2 {
                    prepared[2] = self.prepare_compiled_form(&prepared[2], environment)?;
                }
                self.prepare_tail(&mut prepared, 3, environment)?;
            }
            "PROG" | "PROG*" => {
                if prepared.len() > 1 {
                    prepared[1] = self.prepare_prog_bindings(&prepared[1], environment)?;
                }
                self.prepare_tail(&mut prepared, 2, environment)?;
            }
            "DESTRUCTURING-BIND" => {
                if prepared.len() > 2 {
                    prepared[2] = self.prepare_compiled_form(&prepared[2], environment)?;
                }
                self.prepare_tail(&mut prepared, 3, environment)?;
            }
            "THROW" => {
                self.prepare_tail(&mut prepared, 1, environment)?;
            }
            "BLOCK" => {
                self.prepare_tail(&mut prepared, 2, environment)?;
            }
            "RETURN" => {
                if prepared.len() > 1 {
                    prepared[1] = self.prepare_compiled_form(&prepared[1], environment)?;
                }
            }
            "RETURN-FROM" => {
                if prepared.len() > 2 {
                    prepared[2] = self.prepare_compiled_form(&prepared[2], environment)?;
                }
            }
            "MULTIPLE-VALUE-BIND" => {
                if prepared.len() > 2 {
                    prepared[2] = self.prepare_compiled_form(&prepared[2], environment)?;
                }
                self.prepare_tail(&mut prepared, 3, environment)?;
            }
            "MULTIPLE-VALUE-SETQ" => {
                return self.prepare_compiled_multiple_value_setq(form, &prepared, environment);
            }
            "LAMBDA" => {
                if prepared.len() > 1 {
                    let parameter_form = prepared[1].clone();
                    let local =
                        self.prepare_compiled_lambda_environment(&parameter_form, environment)?;
                    prepared[1] = self.prepare_compiled_lambda_list(&parameter_form, &local)?;
                    self.prepare_tail(&mut prepared, 2, &local)?;
                } else {
                    self.prepare_tail(&mut prepared, 2, environment)?;
                }
            }
            "DEFUN" => {
                if prepared.len() > 2 {
                    let parameter_form = prepared[2].clone();
                    let local =
                        self.prepare_compiled_lambda_environment(&parameter_form, environment)?;
                    prepared[2] = self.prepare_compiled_lambda_list(&parameter_form, &local)?;
                    self.prepare_tail(&mut prepared, 3, &local)?;
                } else {
                    self.prepare_tail(&mut prepared, 3, environment)?;
                }
            }
            "FUNCTION" => {
                if prepared.len() == 2 && is_operator_form(&prepared[1], "LAMBDA") {
                    prepared[1] = self.prepare_compiled_form(&prepared[1], environment)?;
                }
            }
            "COND" => {
                for clause in prepared.iter_mut().skip(1) {
                    *clause = self.prepare_cond_clause(clause, environment)?;
                }
            }
            "CASE" | "ECASE" | "CCASE" | "TYPECASE" | "ETYPECASE" | "CTYPECASE" => {
                if prepared.len() > 1 {
                    prepared[1] = self.prepare_compiled_form(&items[1], environment)?;
                }
                for clause in prepared.iter_mut().skip(2) {
                    *clause = self.prepare_case_clause(clause, environment)?;
                }
            }
            "HANDLER-CASE" => {
                if prepared.len() > 1 {
                    prepared[1] = self.prepare_compiled_form(&prepared[1], environment)?;
                }
                for clause in prepared.iter_mut().skip(2) {
                    *clause = self.prepare_handler_case_clause(clause, environment)?;
                }
            }
            "HANDLER-BIND" => {
                if prepared.len() > 1 {
                    let FormKind::List(handlers) = &prepared[1].kind else {
                        return Ok(Form::list(prepared, form.span));
                    };
                    let mut prepared_handlers = Vec::with_capacity(handlers.len());
                    for handler in handlers {
                        let FormKind::List(parts) = &handler.kind else {
                            prepared_handlers.push(handler.clone());
                            continue;
                        };
                        let mut prepared_parts = parts.to_vec();
                        if prepared_parts.len() > 1 {
                            prepared_parts[1] =
                                self.prepare_compiled_form(&parts[1], environment)?;
                        }
                        prepared_handlers.push(Form::list(prepared_parts, handler.span));
                    }
                    prepared[1] = Form::list(prepared_handlers, prepared[1].span);
                }
                self.prepare_tail(&mut prepared, 2, environment)?;
            }
            "RESTART-BIND" => {
                if prepared.len() > 1 {
                    let FormKind::List(bindings) = &prepared[1].kind else {
                        return Ok(Form::list(prepared, form.span));
                    };
                    let mut prepared_bindings = Vec::with_capacity(bindings.len());
                    for binding in bindings {
                        let FormKind::List(parts) = &binding.kind else {
                            prepared_bindings.push(binding.clone());
                            continue;
                        };
                        let mut prepared_parts = parts.to_vec();
                        if prepared_parts.len() > 1 {
                            prepared_parts[1] =
                                self.prepare_compiled_form(&parts[1], environment)?;
                        }
                        prepared_bindings.push(Form::list(prepared_parts, binding.span));
                    }
                    prepared[1] = Form::list(prepared_bindings, prepared[1].span);
                }
                self.prepare_tail(&mut prepared, 2, environment)?;
            }
            "LET" | "LET*" => {
                if prepared.len() > 1 {
                    let current = Form::list(prepared.clone(), form.span);
                    return Ok(self.prepare_compiled_let(
                        &current,
                        &prepared,
                        environment,
                        normalize_name(operator) == "LET*",
                    )?);
                } else {
                    self.prepare_tail(&mut prepared, 2, environment)?;
                }
            }
            "FLET" | "LABELS" => {
                if prepared.len() > 1 {
                    prepared[1] =
                        self.prepare_local_function_bindings(&prepared[1], environment)?;
                }
                self.prepare_tail(&mut prepared, 2, environment)?;
            }
            "DOTIMES" | "DOLIST" => {
                if prepared.len() > 1 {
                    prepared[1] = self.prepare_iteration_binding(&prepared[1], environment)?;
                }
                self.prepare_tail(&mut prepared, 2, environment)?;
            }
            "DO" | "DO*" => {
                if prepared.len() > 1 {
                    prepared[1] = self.prepare_do_bindings(&prepared[1], environment)?;
                }
                if prepared.len() > 2 {
                    prepared[2] = self.prepare_do_termination(&prepared[2], environment)?;
                }
                self.prepare_tail(&mut prepared, 3, environment)?;
            }
            "SETF" => {
                self.prepare_tail(&mut prepared, 1, environment)?;
            }
            "PSETF" => {
                self.prepare_tail(&mut prepared, 1, environment)?;
            }
            "PUSH" | "POP" => {
                self.prepare_tail(&mut prepared, 1, environment)?;
            }
            "PUSHNEW" => {
                self.prepare_tail(&mut prepared, 1, environment)?;
            }
            "ROTATEF" | "SHIFTF" => {
                self.prepare_tail(&mut prepared, 1, environment)?;
            }
            "INCF" | "DECF" => {
                self.prepare_tail(&mut prepared, 2, environment)?;
            }
            "PSETQ" => {
                return self.prepare_compiled_psetq(form, &prepared, environment);
            }
            "SETQ" => {
                return self.prepare_compiled_setq(form, &prepared, environment);
            }
            "DEFINE" | "DEFVAR" | "DEFPARAMETER" => {
                self.prepare_tail(&mut prepared, 2, environment)?;
            }
            _ => {
                self.prepare_tail(&mut prepared, 1, environment)?;
            }
        }

        Ok(Form::list(prepared, form.span))
    }

    fn prepare_compiled_lambda_environment(
        &self,
        form: &Form,
        environment: &Environment,
    ) -> Result<Environment, RuntimeError> {
        let lambda_list = match self.parameters(form) {
            Ok(lambda_list) => lambda_list,
            Err(RuntimeError::InvalidForm { .. }) => return Ok(environment.child()),
            Err(error) => return Err(error),
        };
        let local = environment.child();
        let define = |name: &str, escaped: bool| {
            if escaped {
                local.define_exact(name, Value::Nil);
            } else {
                local.define(name, Value::Nil);
            }
        };

        for (name, escaped) in lambda_list
            .required
            .iter()
            .zip(lambda_list.required_escaped.iter().copied())
        {
            define(name, escaped);
        }
        for parameter in &lambda_list.optional {
            define(&parameter.name, parameter.name_escaped);
            if let Some(name) = &parameter.supplied_p {
                define(name, parameter.supplied_p_escaped.unwrap_or(false));
            }
        }
        if let Some(name) = &lambda_list.rest {
            define(name, lambda_list.rest_escaped);
        }
        for parameter in &lambda_list.keywords {
            define(&parameter.name, parameter.name_escaped);
            if let Some(name) = &parameter.supplied_p {
                define(name, parameter.supplied_p_escaped.unwrap_or(false));
            }
        }
        for parameter in &lambda_list.auxiliary {
            define(&parameter.name, parameter.name_escaped);
        }
        Ok(local)
    }

    fn prepare_compiled_let(
        &self,
        form: &Form,
        items: &[Form],
        environment: &Environment,
        sequential: bool,
    ) -> Result<Form, RuntimeError> {
        let Some(binding_form) = items.get(1) else {
            return Ok(form.clone());
        };
        let FormKind::List(bindings) = &binding_form.kind else {
            let mut prepared = items.to_vec();
            self.prepare_tail(&mut prepared, 2, environment)?;
            return Ok(Form::list(prepared, form.span));
        };

        let local = environment.child();
        let mut prepared_bindings = Vec::with_capacity(bindings.len());
        for binding in bindings {
            let FormKind::List(parts) = &binding.kind else {
                prepared_bindings.push(binding.clone());
                continue;
            };
            if parts.is_empty() {
                prepared_bindings.push(binding.clone());
                continue;
            }

            let (name, escaped) =
                self.variable_name_info(&parts[0], "let binding name must be a symbol")?;
            let mut prepared_parts = parts.to_vec();
            if parts.len() > 1 {
                let initializer_environment = if sequential { &local } else { environment };
                prepared_parts[1] =
                    self.prepare_compiled_form(&parts[1], initializer_environment)?;
            }
            prepared_bindings.push(Form::list(prepared_parts, binding.span));
            if escaped {
                local.define_exact(name, Value::Nil);
            } else {
                local.define(name, Value::Nil);
            }
        }

        let mut prepared = items.to_vec();
        prepared[1] = Form::list(prepared_bindings, binding_form.span);
        self.prepare_tail(&mut prepared, 2, &local)?;
        Ok(Form::list(prepared, form.span))
    }

    fn prepare_compiled_setq(
        &self,
        form: &Form,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        if items.len() < 3 || items.len() % 2 == 0 {
            let mut prepared = items.to_vec();
            self.prepare_tail(&mut prepared, 2, environment)?;
            return Ok(Form::list(prepared, form.span));
        }

        let expansions = items[1..]
            .chunks_exact(2)
            .map(|pair| self.expand_symbol_macro_form(&pair[0], environment))
            .collect::<Result<Vec<_>, _>>()?;
        if !expansions.iter().any(|expansion| expansion.is_some()) {
            let mut prepared = items.to_vec();
            for index in (2..prepared.len()).step_by(2) {
                prepared[index] = self.prepare_compiled_form(&items[index], environment)?;
            }
            return Ok(Form::list(prepared, form.span));
        }

        let mut transformed = vec![Form::atom("PROGN", form.span)];
        for (pair, expansion) in items[1..].chunks_exact(2).zip(expansions) {
            let is_symbol_macro = expansion.is_some();
            let target = expansion.unwrap_or_else(|| pair[0].clone());
            let operator = if is_symbol_macro { "SETF" } else { "SETQ" };
            let assignment = Form::list(
                vec![Form::atom(operator, pair[0].span), target, pair[1].clone()],
                pair[0].span,
            );
            transformed.push(self.prepare_compiled_form(&assignment, environment)?);
        }
        Ok(Form::list(transformed, form.span))
    }

    fn prepare_compiled_psetq(
        &self,
        form: &Form,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        if items.len() < 3 || items.len() % 2 == 0 {
            let mut prepared = items.to_vec();
            self.prepare_tail(&mut prepared, 2, environment)?;
            return Ok(Form::list(prepared, form.span));
        }

        let expansions = items[1..]
            .chunks_exact(2)
            .map(|pair| self.expand_symbol_macro_form(&pair[0], environment))
            .collect::<Result<Vec<_>, _>>()?;
        if !expansions.iter().any(|expansion| expansion.is_some()) {
            let mut prepared = items.to_vec();
            for index in (2..prepared.len()).step_by(2) {
                prepared[index] = self.prepare_compiled_form(&items[index], environment)?;
            }
            return Ok(Form::list(prepared, form.span));
        }

        let mut bindings = Vec::with_capacity(expansions.len());
        let mut body = vec![Form::atom("PROGN", form.span)];
        for (index, (pair, expansion)) in items[1..].chunks_exact(2).zip(expansions).enumerate() {
            let temporary = self.symbol_macro_temporary(form, index, pair[0].span);
            bindings.push(Form::list(
                vec![temporary.clone(), pair[1].clone()],
                pair[0].span,
            ));
            let is_symbol_macro = expansion.is_some();
            let target = expansion.unwrap_or_else(|| pair[0].clone());
            let operator = if is_symbol_macro { "SETF" } else { "SETQ" };
            body.push(Form::list(
                vec![Form::atom(operator, pair[0].span), target, temporary],
                pair[0].span,
            ));
        }
        body.push(Form::atom("NIL", form.span));

        let mut transformed = vec![
            Form::atom("LET", form.span),
            Form::list(bindings, form.span),
        ];
        transformed.push(Form::list(body, form.span));
        self.prepare_compiled_form(&Form::list(transformed, form.span), environment)
    }

    fn prepare_compiled_multiple_value_setq(
        &self,
        form: &Form,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        let Some(variable_form) = items.get(1) else {
            let mut prepared = items.to_vec();
            self.prepare_tail(&mut prepared, 2, environment)?;
            return Ok(Form::list(prepared, form.span));
        };
        let FormKind::List(variable_forms) = &variable_form.kind else {
            let mut prepared = items.to_vec();
            if prepared.len() > 2 {
                prepared[2] = self.prepare_compiled_form(&items[2], environment)?;
            }
            return Ok(Form::list(prepared, form.span));
        };

        let expansions = variable_forms
            .iter()
            .map(|variable| self.expand_symbol_macro_form(variable, environment))
            .collect::<Result<Vec<_>, _>>()?;
        if !expansions.iter().any(|expansion| expansion.is_some()) {
            let mut prepared = items.to_vec();
            if prepared.len() > 2 {
                prepared[2] = self.prepare_compiled_form(&items[2], environment)?;
            }
            return Ok(Form::list(prepared, form.span));
        }

        let temporaries = variable_forms
            .iter()
            .enumerate()
            .map(|(index, variable)| self.symbol_macro_temporary(form, index, variable.span))
            .collect::<Vec<_>>();
        let mut body = Vec::with_capacity(variable_forms.len() + 1);
        for ((variable, expansion), temporary) in variable_forms
            .iter()
            .zip(expansions)
            .zip(temporaries.iter())
        {
            let is_symbol_macro = expansion.is_some();
            let target = expansion.unwrap_or_else(|| variable.clone());
            let operator = if is_symbol_macro { "SETF" } else { "SETQ" };
            body.push(Form::list(
                vec![
                    Form::atom(operator, variable.span),
                    target,
                    temporary.clone(),
                ],
                variable.span,
            ));
        }
        body.push(temporaries[0].clone());

        let mut transformed = vec![
            Form::atom("MULTIPLE-VALUE-BIND", form.span),
            Form::list(temporaries, variable_form.span),
            items[2].clone(),
        ];
        transformed.extend(body);
        self.prepare_compiled_form(&Form::list(transformed, form.span), environment)
    }

    fn symbol_macro_temporary(&self, form: &Form, index: usize, span: Span) -> Form {
        Form::atom(
            format!(
                "NCL-SYMBOL-MACRO-TEMP-{}-{}-{}",
                form.span.start, form.span.end, index
            ),
            span,
        )
    }

    fn prepare_compiled_lambda_list(
        &self,
        form: &Form,
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        let FormKind::List(parameters) = &form.kind else {
            return Ok(form.clone());
        };

        let mut prepared = parameters.to_vec();
        let mut default_section = false;
        for (index, parameter) in parameters.iter().enumerate() {
            if let Some(name) = atom_name(parameter) {
                match normalize_name(name).as_str() {
                    "&OPTIONAL" | "&KEY" | "&AUX" => default_section = true,
                    "&REST" => default_section = false,
                    _ => {}
                }
                continue;
            }
            if !default_section {
                continue;
            }
            let FormKind::List(specification) = &parameter.kind else {
                continue;
            };
            if let Some(default) = specification.get(1) {
                let mut prepared_specification = specification.to_vec();
                prepared_specification[1] = self.prepare_compiled_form(default, environment)?;
                prepared[index] = Form::list(prepared_specification, parameter.span);
            }
        }
        Ok(Form::list(prepared, form.span))
    }

    fn prepare_local_function_bindings(
        &self,
        form: &Form,
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        let FormKind::List(bindings) = &form.kind else {
            return Ok(form.clone());
        };

        let mut prepared_bindings = Vec::with_capacity(bindings.len());
        for binding in bindings {
            let FormKind::List(parts) = &binding.kind else {
                prepared_bindings.push(binding.clone());
                continue;
            };
            let mut prepared_parts = parts.to_vec();
            if prepared_parts.len() > 1 {
                let parameter_form = parts[1].clone();
                let local =
                    self.prepare_compiled_lambda_environment(&parameter_form, environment)?;
                prepared_parts[1] = self.prepare_compiled_lambda_list(&parameter_form, &local)?;
                for index in 2..prepared_parts.len() {
                    prepared_parts[index] = self.prepare_compiled_form(&parts[index], &local)?;
                }
            } else {
                for index in 2..prepared_parts.len() {
                    prepared_parts[index] =
                        self.prepare_compiled_form(&parts[index], environment)?;
                }
            }
            prepared_bindings.push(Form::list(prepared_parts, binding.span));
        }
        Ok(Form::list(prepared_bindings, form.span))
    }

    fn prepare_tail(
        &self,
        items: &mut [Form],
        start: usize,
        environment: &Environment,
    ) -> Result<(), RuntimeError> {
        for item in items.iter_mut().skip(start) {
            *item = self.prepare_compiled_form(item, environment)?;
        }
        Ok(())
    }

    fn prepare_iteration_binding(
        &self,
        binding: &Form,
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        let FormKind::List(items) = &binding.kind else {
            return Ok(binding.clone());
        };

        let mut prepared = items.to_vec();
        if prepared.len() > 1 {
            prepared[1] = self.prepare_compiled_form(&items[1], environment)?;
        }
        if prepared.len() > 2 {
            prepared[2] = self.prepare_compiled_form(&items[2], environment)?;
        }
        Ok(Form::list(prepared, binding.span))
    }

    fn prepare_do_bindings(
        &self,
        bindings: &Form,
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        let FormKind::List(binding_forms) = &bindings.kind else {
            return Ok(bindings.clone());
        };

        let mut prepared_bindings = Vec::with_capacity(binding_forms.len());
        for binding in binding_forms {
            let FormKind::List(parts) = &binding.kind else {
                prepared_bindings.push(binding.clone());
                continue;
            };

            let mut prepared_parts = parts.to_vec();
            if prepared_parts.len() > 1 {
                prepared_parts[1] = self.prepare_compiled_form(&parts[1], environment)?;
            }
            if prepared_parts.len() > 2 {
                prepared_parts[2] = self.prepare_compiled_form(&parts[2], environment)?;
            }
            prepared_bindings.push(Form::list(prepared_parts, binding.span));
        }

        Ok(Form::list(prepared_bindings, bindings.span))
    }

    fn prepare_prog_bindings(
        &self,
        bindings: &Form,
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        let FormKind::List(binding_forms) = &bindings.kind else {
            return Ok(bindings.clone());
        };

        let mut prepared_bindings = Vec::with_capacity(binding_forms.len());
        for binding in binding_forms {
            let FormKind::List(parts) = &binding.kind else {
                prepared_bindings.push(binding.clone());
                continue;
            };

            let mut prepared_parts = parts.to_vec();
            if prepared_parts.len() > 1 {
                prepared_parts[1] = self.prepare_compiled_form(&parts[1], environment)?;
            }
            prepared_bindings.push(Form::list(prepared_parts, binding.span));
        }

        Ok(Form::list(prepared_bindings, bindings.span))
    }

    fn prepare_do_termination(
        &self,
        termination: &Form,
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        let FormKind::List(parts) = &termination.kind else {
            return Ok(termination.clone());
        };

        let mut prepared = Vec::with_capacity(parts.len());
        for part in parts {
            prepared.push(self.prepare_compiled_form(part, environment)?);
        }
        Ok(Form::list(prepared, termination.span))
    }

    fn prepare_cond_clause(
        &self,
        clause: &Form,
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        let FormKind::List(items) = &clause.kind else {
            return Ok(clause.clone());
        };

        let mut prepared = items.to_vec();
        for item in &mut prepared {
            *item = self.prepare_compiled_form(item, environment)?;
        }
        Ok(Form::list(prepared, clause.span))
    }

    fn prepare_case_clause(
        &self,
        clause: &Form,
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        let FormKind::List(items) = &clause.kind else {
            return Ok(clause.clone());
        };

        let mut prepared = items.to_vec();
        self.prepare_tail(&mut prepared, 1, environment)?;
        Ok(Form::list(prepared, clause.span))
    }

    fn prepare_handler_case_clause(
        &self,
        clause: &Form,
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        let FormKind::List(items) = &clause.kind else {
            return Ok(clause.clone());
        };

        let mut prepared = items.to_vec();
        self.prepare_tail(&mut prepared, 2, environment)?;
        Ok(Form::list(prepared, clause.span))
    }

    fn prepare_restart_case_clause(
        &self,
        clause: &Form,
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        let FormKind::List(items) = &clause.kind else {
            return Ok(clause.clone());
        };

        let mut prepared = items.to_vec();
        if prepared.len() > 1 {
            prepared[1] = self.prepare_compiled_lambda_list(&items[1], environment)?;
        }
        self.prepare_tail(&mut prepared, 2, environment)?;
        Ok(Form::list(prepared, clause.span))
    }

    fn prepare_bindings(
        &self,
        bindings: &Form,
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        let FormKind::List(binding_forms) = &bindings.kind else {
            return Ok(bindings.clone());
        };

        let mut prepared_bindings = Vec::with_capacity(binding_forms.len());
        for binding in binding_forms {
            let FormKind::List(parts) = &binding.kind else {
                prepared_bindings.push(binding.clone());
                continue;
            };
            if parts.is_empty() {
                prepared_bindings.push(binding.clone());
                continue;
            }

            let mut prepared_parts = parts.to_vec();
            self.prepare_tail(&mut prepared_parts, 1, environment)?;
            prepared_bindings.push(Form::list(prepared_parts, binding.span));
        }

        Ok(Form::list(prepared_bindings, bindings.span))
    }

    fn quoted_value_form(&self, value: &Value, span: Span) -> Result<Form, RuntimeError> {
        if let Value::Values(values) = value {
            let mut forms = vec![Form::atom("VALUES", span)];
            for value in values.iter() {
                forms.push(self.quoted_value_form(value, span)?);
            }
            return Ok(Form::list(forms, span));
        }

        Ok(Form::list(
            vec![
                Form::atom("QUOTE", span),
                self.form_from_value(value, span)?,
            ],
            span,
        ))
    }

    pub(crate) fn eval_in(
        &self,
        form: &Form,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        self.eval_values_in(form, environment)
            .map(|value| value.primary_value())
    }

    pub(crate) fn eval_values_in(
        &self,
        form: &Form,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        match &form.kind {
            FormKind::Atom(atom) => {
                if let Some(expanded) = self.expand_symbol_macro_form(form, environment)? {
                    return self.eval_values_in(&expanded, environment);
                }
                self.eval_atom(atom, form.span, environment)
            }
            FormKind::String(value) => Ok(Value::string(value.clone())),
            FormKind::Character(value) => Ok(Value::Character(*value)),
            FormKind::Complex { .. } => self.quoted_value(form),
            FormKind::ReadTimeEval(_) => Err(self.invalid(
                "read-time evaluation must be resolved before evaluation",
                form.span,
            )),
            FormKind::BitVector(bits) => Ok(Value::array_with_element_type(
                vec![bits.len()],
                bits.iter()
                    .map(|bit| Value::Integer(i64::from(*bit)))
                    .collect(),
                ArrayElementType::Bit,
            )),
            FormKind::Vector(items) => Ok(Value::vector(
                items
                    .iter()
                    .map(|item| self.quoted_value(item))
                    .collect::<Result<Vec<_>, _>>()?,
            )),
            FormKind::DottedList { .. } => {
                Err(self.invalid("cannot evaluate a dotted list", form.span))
            }
            FormKind::List(items) => self.eval_list_values(items, form.span, environment),
        }
    }

    fn eval_atom(
        &self,
        atom: &str,
        span: Span,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if let Some(value) = literal_atom(atom) {
            return Ok(value);
        }
        let (name, escaped) = resolved_symbol(atom);
        let value = if escaped {
            self.lookup_exact_in(&name, environment)
        } else {
            self.lookup_in(&name, environment)
        };
        value.ok_or_else(|| RuntimeError::UnboundVariable {
            name: normalize_name(&name),
            span: Some(span),
        })
    }

    fn eval_list_values(
        &self,
        items: &[Form],
        span: Span,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        let form = Form::list(items.to_vec(), span);
        let expanded = self.expand_macros(form, environment)?;
        self.eval_expanded_values(&expanded, environment)
    }

    fn eval_expanded_values(
        &self,
        form: &Form,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        let FormKind::List(items) = &form.kind else {
            return self.eval_values_in(form, environment);
        };
        let Some(operator) = items.first() else {
            return Ok(Value::Nil);
        };
        if let Some(name) = atom_name(operator) {
            let escaped = parse_symbol_token(name)
                .map(|token| token.escaped)
                .unwrap_or(false);
            if !escaped {
                match normalize_name(name).as_str() {
                    "QUOTE" => return self.special_quote(items, form.span),
                    "QUASIQUOTE" => return self.special_quasiquote(items, environment),
                    "DECLARE" => return Ok(Value::Nil),
                    "LOCALLY" => return self.special_locally(items, environment),
                    "EVAL-WHEN" => return self.special_eval_when(items, environment),
                    "DECLAIM" => return self.special_global_declaration(items, environment, false),
                    "PROCLAIM" => return self.special_global_declaration(items, environment, true),
                    "THE" => return self.special_the(items, environment),
                    "LOAD-TIME-VALUE" => {
                        return self.special_load_time_value(items, environment);
                    }
                    "NTH-VALUE" => return self.special_nth_value(items, environment),
                    "IF" => return self.special_if(items, environment),
                    "PROGN" => return self.special_progn(&items[1..], environment),
                    "PROG1" => return self.special_prog1(items, environment),
                    "PROG2" => return self.special_prog2(items, environment),
                    "PROG" => return self.special_prog(items, environment, false),
                    "PROG*" => return self.special_prog(items, environment, true),
                    "VALUES" => return self.special_values(items, environment),
                    "IGNORE-ERRORS" => return self.special_ignore_errors(items, environment),
                    "HANDLER-CASE" => return self.special_handler_case(items, environment),
                    "HANDLER-BIND" => return self.special_handler_bind(items, environment),
                    "RESTART-BIND" => return self.special_restart_bind(items, environment),
                    "CATCH" => return self.special_catch(items, environment),
                    "PROGV" => return self.special_progv(items, environment),
                    "THROW" => return self.special_throw(items, environment),
                    "WITH-CONDITION-RESTARTS" => {
                        return self.special_with_condition_restarts(items, environment);
                    }
                    "WITH-SIMPLE-RESTART" => {
                        return self.special_with_simple_restart(items, environment);
                    }
                    "WITH-OPEN-FILE" => {
                        let expanded = self.expand_with_open_file(form)?;
                        return self.eval_expanded_values(&expanded, environment);
                    }
                    "WITH-OPEN-STREAM" => {
                        let expanded = self.expand_with_open_stream(form)?;
                        return self.eval_expanded_values(&expanded, environment);
                    }
                    "WITH-OUTPUT-TO-STRING" => {
                        let expanded = self.expand_with_output_to_string(form)?;
                        return self.eval_expanded_values(&expanded, environment);
                    }
                    "WITH-INPUT-FROM-STRING" => {
                        let expanded = self.expand_with_input_from_string(form)?;
                        return self.eval_expanded_values(&expanded, environment);
                    }
                    "WITH-HASH-TABLE-ITERATOR" => {
                        let expanded = self.expand_with_hash_table_iterator(form)?;
                        return self.eval_expanded_values(&expanded, environment);
                    }
                    "RESTART-CASE" => return self.special_restart_case(items, environment),
                    "UNWIND-PROTECT" => {
                        return self.special_unwind_protect(items, environment);
                    }
                    "BLOCK" => return self.special_block(items, environment),
                    "RETURN" => return self.special_return(items, environment),
                    "RETURN-FROM" => return self.special_return_from(items, environment),
                    "TAGBODY" => return self.special_tagbody(items, environment),
                    "GO" => return self.special_go(items, environment),
                    "MULTIPLE-VALUE-BIND" => {
                        return self.special_multiple_value_bind(items, environment);
                    }
                    "MULTIPLE-VALUE-CALL" => {
                        return self.special_multiple_value_call(items, environment);
                    }
                    "MULTIPLE-VALUE-LIST" => {
                        return self.special_multiple_value_list(items, environment);
                    }
                    "MULTIPLE-VALUE-PROG1" => {
                        return self.special_multiple_value_prog1(items, environment);
                    }
                    "AND" => return self.special_and(&items[1..], environment),
                    "OR" => return self.special_or(&items[1..], environment),
                    "WHEN" => return self.special_when(items, environment, true),
                    "UNLESS" => return self.special_when(items, environment, false),
                    "COND" => return self.special_cond(&items[1..], environment),
                    "CASE" => return self.special_case(items, environment, false),
                    "ECASE" => return self.special_case(items, environment, true),
                    "CCASE" => return self.special_change_case(items, environment, false),
                    "TYPECASE" => return self.special_typecase(items, environment, false),
                    "ETYPECASE" => return self.special_typecase(items, environment, true),
                    "CTYPECASE" => return self.special_change_case(items, environment, true),
                    "DESTRUCTURING-BIND" => {
                        return self.special_destructuring_bind(items, environment);
                    }
                    "LET" => return self.special_let(items, environment, false),
                    "LET*" => return self.special_let(items, environment, true),
                    "FLET" => return self.special_flet(items, environment, false),
                    "LABELS" => return self.special_flet(items, environment, true),
                    "MACROLET" => return self.special_macrolet(items, environment),
                    "SYMBOL-MACROLET" => return self.special_symbol_macrolet(items, environment),
                    "NCL-MACRO-ENVIRONMENT" => {
                        return self.special_macrolet_environment(items, environment);
                    }
                    "DOTIMES" => return self.special_dotimes(items, environment),
                    "DOLIST" => return self.special_dolist(items, environment),
                    "DO" => return self.special_do(items, environment, false),
                    "DO*" => return self.special_do(items, environment, true),
                    "LAMBDA" => return self.special_lambda(items, environment),
                    "FUNCTION" => return self.special_function(items, environment),
                    "DEFUN" => return self.special_defun(items, environment),
                    "DEFMACRO" => return self.special_defmacro(items, environment),
                    "DEFINE-MODIFY-MACRO" => {
                        return self.special_define_modify_macro(items, environment);
                    }
                    "MACROEXPAND-1" => return self.special_macroexpand_1(items, environment),
                    "MACROEXPAND" => return self.special_macroexpand(items, environment),
                    "DEFPACKAGE" => return self.special_defpackage(items),
                    "IN-PACKAGE" => return self.special_in_package(items),
                    "DEFINE" => return self.special_define(items, environment),
                    "DEFINE-SYMBOL-MACRO" => {
                        return self.special_define_symbol_macro(items, environment);
                    }
                    "SETQ" => return self.special_setq(items, environment),
                    "PSETQ" => return self.special_psetq(items, environment),
                    "MULTIPLE-VALUE-SETQ" => {
                        return self.special_multiple_value_setq(items, environment);
                    }
                    "SETF" => return self.special_setf(items, environment),
                    "PSETF" => return self.special_psetf(items, environment),
                    "PUSH" => return self.special_push(items, environment),
                    "POP" => return self.special_pop(items, environment),
                    "PUSHNEW" => return self.special_pushnew(items, environment),
                    "ROTATEF" => return self.special_rotatef(items, environment),
                    "SHIFTF" => return self.special_shiftf(items, environment),
                    "INCF" => {
                        return self.special_modify_symbol(items, environment, "INCF", "+");
                    }
                    "DECF" => {
                        return self.special_modify_symbol(items, environment, "DECF", "-");
                    }
                    "DEFSTRUCT" => return self.special_defstruct(items, environment),
                    "DEFCLASS" => return self.special_defclass(items, environment),
                    "DEFINE-CONDITION" => {
                        return conditions::define_condition(self, items, environment);
                    }
                    "DEFGENERIC" => return self.special_defgeneric(items, environment),
                    "DEFMETHOD" => return self.special_defmethod(items, environment),
                    "DEFSETF" => return self.special_defsetf(items, environment),
                    "DEFINE-SETF-EXPANDER" => {
                        return self.special_define_setf_expander(items, environment);
                    }
                    "GET-SETF-EXPANSION" => {
                        return self.special_get_setf_expansion(items, environment);
                    }
                    "DEFVAR" => return self.special_defvar(items, environment, false),
                    "DEFPARAMETER" => return self.special_defvar(items, environment, true),
                    "DEFCONSTANT" => return self.special_defconstant(items, environment),
                    "EVAL" => return self.special_eval(items, environment),
                    "FUNCALL" => return self.special_funcall(items, environment),
                    "APPLY" => return self.special_apply(items, environment),
                    "MAP-INTO" => return self.special_map_into(items, environment),
                    "MAPHASH" => return self.special_maphash(items, environment),
                    "MAPCAR" => return self.special_mapcar(items, environment),
                    _ => {}
                }
            }
        }

        let function = if let Some(name) = atom_name(operator) {
            let (resolved_name, escaped) = resolved_symbol(name);
            let function = if escaped {
                self.lookup_callable_exact_in(&resolved_name, environment)
            } else {
                self.lookup_callable_in(&resolved_name, environment)
            };
            function.ok_or_else(|| RuntimeError::UnboundVariable {
                name: if escaped {
                    resolved_name
                } else {
                    normalize_name(&resolved_name)
                },
                span: Some(operator.span),
            })?
        } else {
            self.eval_in(operator, environment)?
        };
        let arguments = items[1..]
            .iter()
            .map(|item| self.eval_in(item, environment))
            .collect::<Result<Vec<_>, _>>()?;
        self.apply_in(&function, &arguments, form.span, environment)
    }

    fn expand_macros(&self, form: Form, environment: &Environment) -> Result<Form, RuntimeError> {
        self.expand_macros_with_flag(form, environment)
            .map(|(form, _)| form)
    }

    fn expand_macros_with_flag(
        &self,
        mut form: Form,
        environment: &Environment,
    ) -> Result<(Form, bool), RuntimeError> {
        let mut expanded_p = false;
        for _ in 0..MAX_MACRO_EXPANSIONS {
            let Some(expanded) = self.expand_macro_once(&form, environment)? else {
                return Ok((form, expanded_p));
            };
            expanded_p = true;
            form = expanded;
        }
        Err(self.invalid("macro expansion exceeded its limit", form.span))
    }

    fn expand_macro_once(
        &self,
        form: &Form,
        environment: &Environment,
    ) -> Result<Option<Form>, RuntimeError> {
        let FormKind::List(items) = &form.kind else {
            return Ok(None);
        };
        let Some(operator) = items.first() else {
            return Ok(None);
        };
        let Some(name) = atom_name(operator) else {
            return Ok(None);
        };
        let (resolved_name, escaped) = resolved_symbol(name);
        let function = if escaped {
            self.lookup_function_exact_in(&resolved_name, environment)
        } else {
            self.lookup_function_in(&resolved_name, environment)
        };
        let Some(function) = function else {
            if !escaped {
                match normalize_name(&resolved_name).as_str() {
                    "WITH-SLOTS" => {
                        return self.expand_builtin_with_slots(form, false).map(Some);
                    }
                    "WITH-ACCESSORS" => {
                        return self.expand_builtin_with_slots(form, true).map(Some);
                    }
                    _ => {}
                }
            }
            return Ok(None);
        };
        let Value::Function(function) = function else {
            return Ok(None);
        };
        let expansion = match function.as_ref() {
            crate::Function::Macro {
                lambda_list,
                body,
                environment: macro_environment,
            } => {
                let expansion = self.invoke_macro(
                    form,
                    &items[1..],
                    name,
                    lambda_list,
                    body,
                    macro_environment,
                    environment,
                )?;
                let expansion = expansion.primary_value();
                self.form_from_value(&expansion, form.span)?
            }
            crate::Function::ModifyMacro {
                lambda_list,
                function,
                environment: macro_environment,
            } => self.invoke_modify_macro(
                form,
                &items[1..],
                name,
                lambda_list,
                function,
                macro_environment,
                environment,
            )?,
            _ => return Ok(None),
        };
        Ok(Some(expansion))
    }

    fn invoke_macro(
        &self,
        form: &Form,
        arguments: &[Form],
        macro_name: &str,
        lambda_list: &MacroLambdaList,
        body: &[Form],
        macro_environment: &Environment,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        let local = self.bind_macro_arguments(
            form,
            arguments,
            macro_name,
            lambda_list,
            macro_environment,
            environment,
        )?;
        self.eval_sequence_values(body, &local)
    }

    fn bind_macro_arguments(
        &self,
        form: &Form,
        arguments: &[Form],
        macro_name: &str,
        lambda_list: &MacroLambdaList,
        macro_environment: &Environment,
        environment: &Environment,
    ) -> Result<Environment, RuntimeError> {
        let argument_count = arguments.len();
        let required_count = lambda_list.required.len();
        if argument_count < required_count {
            return Err(self.arity(
                &normalize_name(macro_name),
                &format!("at least {required_count}"),
                argument_count,
            ));
        }

        let optional_supplied_count = if lambda_list.has_keyword_section {
            let available = argument_count
                .saturating_sub(required_count)
                .min(lambda_list.optional.len());
            (0..available)
                .take_while(|index| !is_macro_keyword_form(&arguments[index + required_count]))
                .count()
        } else {
            argument_count
                .saturating_sub(required_count)
                .min(lambda_list.optional.len())
        };
        let key_start = required_count + optional_supplied_count;
        if !lambda_list.has_keyword_section
            && lambda_list.rest.is_none()
            && argument_count > required_count + lambda_list.optional.len()
        {
            let maximum = required_count + lambda_list.optional.len();
            return Err(self.arity(
                &normalize_name(macro_name),
                &format!("at most {maximum}"),
                argument_count,
            ));
        }

        let keyword_arguments = if lambda_list.has_keyword_section {
            let keyword_arguments = &arguments[key_start..];
            if keyword_arguments.len() % 2 != 0 {
                return Err(self.invalid("keyword arguments must be supplied in pairs", form.span));
            }
            let mut supplied = Vec::new();
            let mut accepts_unknown_keywords = lambda_list.allow_other_keys;
            for pair in keyword_arguments.chunks_exact(2) {
                let Some((keyword_name, keyword_name_escaped)) = macro_keyword_name(&pair[0])
                else {
                    return Err(
                        self.invalid("keyword argument name must be a keyword", pair[0].span)
                    );
                };
                if macro_keyword_matches(
                    "ALLOW-OTHER-KEYS",
                    false,
                    &keyword_name,
                    keyword_name_escaped,
                ) && self.quoted_value(&pair[1])?.is_truthy()
                {
                    accepts_unknown_keywords = true;
                }
                supplied.push((keyword_name, keyword_name_escaped, pair[1].clone()));
            }
            if !accepts_unknown_keywords {
                for (keyword_name, keyword_name_escaped, _) in &supplied {
                    if !macro_keyword_matches(
                        "ALLOW-OTHER-KEYS",
                        false,
                        keyword_name,
                        *keyword_name_escaped,
                    ) && !lambda_list.keywords.iter().any(|specification| {
                        macro_keyword_matches(
                            &specification.keyword_name,
                            specification.keyword_name_escaped,
                            keyword_name,
                            *keyword_name_escaped,
                        )
                    }) {
                        return Err(RuntimeError::InvalidForm {
                            message: format!("unknown keyword :{keyword_name}"),
                            span: Some(form.span),
                        });
                    }
                }
            }
            Some(supplied)
        } else {
            None
        };

        let local = macro_environment.child();
        if let Some(environment_name) = &lambda_list.environment {
            self.define_macro_binding_in(
                environment_name,
                Value::environment(environment.clone()),
                &local,
            );
        }
        if let Some(whole) = &lambda_list.whole {
            self.define_macro_binding_in(whole, self.quoted_value(form)?, &local);
        }
        for (pattern, argument) in lambda_list
            .required
            .iter()
            .zip(arguments[..required_count].iter())
        {
            self.bind_macro_pattern(pattern, self.quoted_value(argument)?, &local, argument.span)?;
        }
        for (index, specification) in lambda_list.optional.iter().enumerate() {
            let supplied =
                (index < optional_supplied_count).then(|| &arguments[required_count + index]);
            let value = match supplied {
                Some(argument) => self.quoted_value(argument)?,
                None => self.eval_in(&specification.init_form, &local)?,
            };
            self.bind_macro_pattern(&specification.pattern, value, &local, form.span)?;
            if let Some(supplied_p) = &specification.supplied_p {
                self.define_macro_binding_in(
                    supplied_p,
                    Value::boolean(supplied.is_some()),
                    &local,
                );
            }
        }
        if let Some(rest_name) = &lambda_list.rest {
            let rest_values = arguments[key_start..]
                .iter()
                .map(|argument| self.quoted_value(argument))
                .collect::<Result<Vec<_>, _>>()?;
            self.define_macro_binding_in(rest_name, Value::list(rest_values), &local);
        }

        if let Some(supplied_keywords) = keyword_arguments {
            for specification in &lambda_list.keywords {
                let supplied = supplied_keywords
                    .iter()
                    .find(|(keyword_name, keyword_name_escaped, _)| {
                        macro_keyword_matches(
                            &specification.keyword_name,
                            specification.keyword_name_escaped,
                            keyword_name,
                            *keyword_name_escaped,
                        )
                    })
                    .map(|(_, _, argument)| argument);
                let value = match supplied {
                    Some(argument) => self.quoted_value(argument)?,
                    None => self.eval_in(&specification.init_form, &local)?,
                };
                self.bind_macro_pattern(&specification.pattern, value, &local, form.span)?;
                if let Some(supplied_p) = &specification.supplied_p {
                    self.define_macro_binding_in(
                        supplied_p,
                        Value::boolean(supplied.is_some()),
                        &local,
                    );
                }
            }
        }
        for specification in &lambda_list.auxiliary {
            let value = self.eval_in(&specification.init_form, &local)?;
            self.define_macro_binding_in(&specification.name, value, &local);
        }

        Ok(local)
    }

    fn invoke_modify_macro(
        &self,
        form: &Form,
        arguments: &[Form],
        macro_name: &str,
        lambda_list: &MacroLambdaList,
        function: &Form,
        macro_environment: &Environment,
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        let local = self.bind_macro_arguments(
            form,
            arguments,
            macro_name,
            lambda_list,
            macro_environment,
            environment,
        )?;
        let Some(MacroPattern::Name(place_name)) = lambda_list.required.first() else {
            return Err(self.invalid("define-modify-macro requires a place parameter", form.span));
        };
        let place_value = self
            .lookup_macro_binding_in(place_name, &local)
            .ok_or_else(|| {
                self.invalid(
                    "define-modify-macro could not bind its place parameter",
                    form.span,
                )
            })?;
        let place = self.form_from_value(&place_value, form.span)?;
        let expansion = self.get_modify_macro_setf_expansion(&place, environment)?;

        let function_designator = if is_operator_form(function, "FUNCTION") {
            function.clone()
        } else {
            Form::list(
                vec![Form::atom("FUNCTION", function.span), function.clone()],
                function.span,
            )
        };
        let mut call_items = vec![
            Form::atom("FUNCALL", form.span),
            function_designator,
            expansion.access_form.clone(),
        ];
        for pattern in lambda_list.required.iter().skip(1) {
            let MacroPattern::Name(name) = pattern else {
                return Err(self.invalid(
                    "define-modify-macro required parameters must be names",
                    form.span,
                ));
            };
            let value = self.lookup_macro_binding_in(name, &local).ok_or_else(|| {
                self.invalid("define-modify-macro parameter is unbound", form.span)
            })?;
            call_items.push(self.form_from_value(&value, form.span)?);
        }
        for specification in &lambda_list.optional {
            let MacroPattern::Name(name) = &specification.pattern else {
                return Err(self.invalid(
                    "define-modify-macro optional parameters must be names",
                    form.span,
                ));
            };
            let value = self.lookup_macro_binding_in(name, &local).ok_or_else(|| {
                self.invalid("define-modify-macro parameter is unbound", form.span)
            })?;
            call_items.push(self.form_from_value(&value, form.span)?);
        }
        if let Some(rest_name) = &lambda_list.rest {
            let rest_value = self
                .lookup_macro_binding_in(rest_name, &local)
                .ok_or_else(|| {
                    self.invalid("define-modify-macro rest parameter is unbound", form.span)
                })?;
            let rest_values = rest_value.list_items().ok_or_else(|| {
                self.invalid(
                    "define-modify-macro rest parameter is not a list",
                    form.span,
                )
            })?;
            for value in rest_values {
                call_items.push(self.form_from_value(&value, form.span)?);
            }
        } else if lambda_list.has_keyword_section {
            for specification in &lambda_list.keywords {
                let MacroPattern::Name(name) = &specification.pattern else {
                    return Err(self.invalid(
                        "define-modify-macro keyword parameters must be names",
                        form.span,
                    ));
                };
                let value = self.lookup_macro_binding_in(name, &local).ok_or_else(|| {
                    self.invalid(
                        "define-modify-macro keyword parameter is unbound",
                        form.span,
                    )
                })?;
                let keyword = if specification.keyword_name_escaped {
                    format!(":{}", escaped_symbol_atom(&specification.keyword_name))
                } else {
                    format!(":{}", specification.keyword_name)
                };
                call_items.push(Form::atom(keyword, form.span));
                call_items.push(self.form_from_value(&value, form.span)?);
            }
        }
        let call = Form::list(call_items, form.span);
        if expansion.stores.is_empty() {
            return Err(self.invalid(
                "SETF expansion must provide at least one store variable",
                form.span,
            ));
        }
        let update = Form::list(
            vec![
                Form::atom("MULTIPLE-VALUE-BIND", form.span),
                Form::list(expansion.stores.clone(), form.span),
                call,
                Form::list(
                    vec![
                        Form::atom("PROGN", form.span),
                        expansion.store_form.clone(),
                        expansion.stores[0].clone(),
                    ],
                    form.span,
                ),
            ],
            form.span,
        );
        let temporary_bindings = expansion
            .temporaries
            .iter()
            .zip(expansion.values.iter())
            .map(|(temporary, value)| Form::list(vec![temporary.clone(), value.clone()], form.span))
            .collect();
        Ok(Form::list(
            vec![
                Form::atom("LET*", form.span),
                Form::list(temporary_bindings, form.span),
                update,
            ],
            form.span,
        ))
    }

    fn expand_builtin_with_slots(
        &self,
        form: &Form,
        with_accessors: bool,
    ) -> Result<Form, RuntimeError> {
        let FormKind::List(items) = &form.kind else {
            return Ok(form.clone());
        };
        let operator = if with_accessors {
            "WITH-ACCESSORS"
        } else {
            "WITH-SLOTS"
        };
        if items.len() < 3 {
            return Err(self.arity(operator, "at least two", items.len().saturating_sub(1)));
        }
        let FormKind::List(bindings) = &items[1].kind else {
            return Err(self.invalid(
                if with_accessors {
                    "with-accessors bindings must be a list"
                } else {
                    "with-slots bindings must be a list"
                },
                items[1].span,
            ));
        };

        let validate_symbol = |candidate: &Form, context: &str| {
            let Some(name) = atom_name(candidate) else {
                return Err(self.invalid(context, candidate.span));
            };
            let Ok(token) = parse_symbol_token(name) else {
                return Err(self.invalid(context, candidate.span));
            };
            if token.name.is_empty()
                || (!token.escaped
                    && literal_atom(name).is_some()
                    && !name.eq_ignore_ascii_case("nil")
                    && !name.eq_ignore_ascii_case("t"))
            {
                return Err(self.invalid(context, candidate.span));
            }
            Ok(())
        };

        let temporary = self.symbol_macro_temporary(&items[2], 0, form.span);
        let mut symbol_bindings = Vec::with_capacity(bindings.len());
        for entry in bindings {
            let (variable, expansion) = if with_accessors {
                let FormKind::List(parts) = &entry.kind else {
                    return Err(self.invalid(
                        "with-accessors entry must be a (variable accessor) list",
                        entry.span,
                    ));
                };
                if parts.len() != 2 {
                    return Err(self.invalid(
                        "with-accessors entry needs a variable and accessor",
                        entry.span,
                    ));
                }
                self.variable_name_info(&parts[0], "with-accessors variable must be a symbol")?;
                validate_symbol(&parts[1], "with-accessors accessor must be a symbol")?;
                (
                    parts[0].clone(),
                    Form::list(vec![parts[1].clone(), temporary.clone()], entry.span),
                )
            } else {
                let (slot, variable) = match &entry.kind {
                    FormKind::Atom(_) => (entry.clone(), entry.clone()),
                    FormKind::List(parts) if parts.len() == 2 => {
                        (parts[0].clone(), parts[1].clone())
                    }
                    _ => {
                        return Err(self.invalid(
                            "with-slots entry must be a slot or (slot variable) list",
                            entry.span,
                        ));
                    }
                };
                validate_symbol(&slot, "with-slots slot must be a symbol")?;
                self.variable_name_info(&variable, "with-slots variable must be a symbol")?;
                let quoted_slot =
                    Form::list(vec![Form::atom("QUOTE", slot.span), slot], entry.span);
                (
                    variable,
                    Form::list(
                        vec![
                            Form::atom("SLOT-VALUE", entry.span),
                            temporary.clone(),
                            quoted_slot,
                        ],
                        entry.span,
                    ),
                )
            };
            symbol_bindings.push(Form::list(vec![variable, expansion], entry.span));
        }

        let symbol_macrolet = {
            let mut forms = Vec::with_capacity(items.len().saturating_sub(1));
            forms.push(Form::atom("SYMBOL-MACROLET", form.span));
            forms.push(Form::list(symbol_bindings, items[1].span));
            forms.extend(items[3..].iter().cloned());
            Form::list(forms, form.span)
        };
        let let_bindings = Form::list(
            vec![Form::list(vec![temporary, items[2].clone()], items[2].span)],
            items[1].span,
        );
        Ok(Form::list(
            vec![Form::atom("LET", form.span), let_bindings, symbol_macrolet],
            form.span,
        ))
    }

    fn expand_with_open_file(&self, form: &Form) -> Result<Form, RuntimeError> {
        let FormKind::List(items) = &form.kind else {
            return Ok(form.clone());
        };
        if items.len() < 2 {
            return Err(self.arity(
                "with-open-file",
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        let binding_form = &items[1];
        let FormKind::List(binding) = &binding_form.kind else {
            return Err(self.invalid("with-open-file binding must be a list", binding_form.span));
        };
        if binding.len() < 2 {
            return Err(self.invalid(
                "with-open-file binding needs a stream variable and pathname",
                binding_form.span,
            ));
        }
        self.variable_name_info(
            &binding[0],
            "with-open-file stream variable must be a symbol",
        )?;

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
            body_items.push(Form::atom("PROGN", form.span));
            body_items.extend(items[2..].iter().cloned());
            Form::list(body_items, form.span)
        } else {
            Form::atom("NIL", form.span)
        };
        let close_form = Form::list(
            vec![Form::atom("CLOSE", form.span), binding[0].clone()],
            form.span,
        );
        let protected_form = Form::list(
            vec![Form::atom("UNWIND-PROTECT", form.span), body, close_form],
            form.span,
        );
        Ok(Form::list(
            vec![
                Form::atom("LET", form.span),
                generated_binding,
                protected_form,
            ],
            form.span,
        ))
    }

    fn expand_with_open_stream(&self, form: &Form) -> Result<Form, RuntimeError> {
        let FormKind::List(items) = &form.kind else {
            return Ok(form.clone());
        };
        if items.len() < 2 {
            return Err(self.arity(
                "with-open-stream",
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        let binding_form = &items[1];
        let FormKind::List(binding) = &binding_form.kind else {
            return Err(self.invalid("with-open-stream binding must be a list", binding_form.span));
        };
        if binding.len() != 2 {
            return Err(self.invalid(
                "with-open-stream binding needs a stream variable and stream form",
                binding_form.span,
            ));
        }
        self.variable_name_info(
            &binding[0],
            "with-open-stream stream variable must be a symbol",
        )?;

        let generated_binding = Form::list(
            vec![Form::list(
                vec![binding[0].clone(), binding[1].clone()],
                binding_form.span,
            )],
            binding_form.span,
        );
        let body = if items.len() > 2 {
            let mut body_items = Vec::with_capacity(items.len() - 1);
            body_items.push(Form::atom("PROGN", form.span));
            body_items.extend(items[2..].iter().cloned());
            Form::list(body_items, form.span)
        } else {
            Form::atom("NIL", form.span)
        };
        let close_form = Form::list(
            vec![Form::atom("CLOSE", form.span), binding[0].clone()],
            form.span,
        );
        let protected_form = Form::list(
            vec![Form::atom("UNWIND-PROTECT", form.span), body, close_form],
            form.span,
        );
        Ok(Form::list(
            vec![
                Form::atom("LET", form.span),
                generated_binding,
                protected_form,
            ],
            form.span,
        ))
    }

    fn expand_with_output_to_string(&self, form: &Form) -> Result<Form, RuntimeError> {
        let FormKind::List(items) = &form.kind else {
            return Ok(form.clone());
        };
        if items.len() < 2 {
            return Err(self.arity(
                "with-output-to-string",
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        let binding_form = &items[1];
        let FormKind::List(binding) = &binding_form.kind else {
            return Err(self.invalid(
                "with-output-to-string binding must be a list",
                binding_form.span,
            ));
        };
        if binding.is_empty() {
            return Err(self.invalid(
                "with-output-to-string binding needs a stream variable",
                binding_form.span,
            ));
        }
        let has_literal_nil_string_form = binding.get(1).is_some_and(is_nil_form);
        if binding.len() != 1 && !has_literal_nil_string_form {
            return Err(self.invalid(
                "with-output-to-string currently supports only a variable binding or literal NIL string form with :element-type",
                binding_form.span,
            ));
        }
        if has_literal_nil_string_form && !(binding.len() - 2).is_multiple_of(2) {
            return Err(self.invalid(
                "with-output-to-string keyword arguments must be keyword/value pairs",
                binding_form.span,
            ));
        }
        self.variable_name_info(
            &binding[0],
            "with-output-to-string stream variable must be a symbol",
        )?;

        let mut initializer_items =
            vec![Form::atom("MAKE-STRING-OUTPUT-STREAM", binding_form.span)];
        let mut element_type_form = None;
        if has_literal_nil_string_form {
            for pair in binding[2..].chunks_exact(2) {
                let Some((keyword, escaped)) = macro_keyword_name(&pair[0]) else {
                    return Err(self.invalid(
                        "with-output-to-string keyword arguments must use keyword names",
                        pair[0].span,
                    ));
                };
                if !macro_keyword_matches("ELEMENT-TYPE", false, &keyword, escaped) {
                    return Err(self.invalid(
                        "with-output-to-string currently supports only :element-type",
                        pair[0].span,
                    ));
                }
                if element_type_form.is_some() {
                    return Err(self.invalid(
                        "with-output-to-string received duplicate :element-type",
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
                Form::atom("GET-OUTPUT-STREAM-STRING", form.span),
                binding[0].clone(),
            ],
            form.span,
        );
        let body = if items.len() > 2 {
            let mut body_items = Vec::with_capacity(items.len() + 1);
            body_items.push(Form::atom("PROGN", form.span));
            body_items.extend(items[2..].iter().cloned());
            body_items.push(result_form);
            Form::list(body_items, form.span)
        } else {
            result_form
        };
        let close_form = Form::list(
            vec![Form::atom("CLOSE", form.span), binding[0].clone()],
            form.span,
        );
        let protected_form = Form::list(
            vec![Form::atom("UNWIND-PROTECT", form.span), body, close_form],
            form.span,
        );
        Ok(Form::list(
            vec![
                Form::atom("LET", form.span),
                generated_binding,
                protected_form,
            ],
            form.span,
        ))
    }

    fn expand_with_input_from_string(&self, form: &Form) -> Result<Form, RuntimeError> {
        let FormKind::List(items) = &form.kind else {
            return Ok(form.clone());
        };
        if items.len() < 2 {
            return Err(self.arity(
                "with-input-from-string",
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        let binding_form = &items[1];
        let FormKind::List(binding) = &binding_form.kind else {
            return Err(self.invalid(
                "with-input-from-string binding must be a list",
                binding_form.span,
            ));
        };
        if binding.len() < 2 {
            return Err(self.invalid(
                "with-input-from-string binding needs a variable and string form",
                binding_form.span,
            ));
        }
        self.variable_name_info(
            &binding[0],
            "with-input-from-string stream variable must be a symbol",
        )?;

        if !(binding.len() - 2).is_multiple_of(2) {
            return Err(self.invalid(
                "with-input-from-string requires keyword/value pairs after the string form",
                binding_form.span,
            ));
        }
        let mut index_form: Option<Form> = None;
        let mut start_form: Option<Form> = None;
        let mut end_form: Option<Form> = None;
        for pair in binding[2..].chunks_exact(2) {
            let Some((keyword, escaped)) = macro_keyword_name(&pair[0]) else {
                return Err(self.invalid(
                    "with-input-from-string options must use keyword names",
                    pair[0].span,
                ));
            };
            let slot = if macro_keyword_matches("INDEX", false, &keyword, escaped) {
                &mut index_form
            } else if macro_keyword_matches("START", false, &keyword, escaped) {
                &mut start_form
            } else if macro_keyword_matches("END", false, &keyword, escaped) {
                &mut end_form
            } else {
                return Err(self.invalid(
                    &format!("with-input-from-string does not recognize keyword :{keyword}"),
                    pair[0].span,
                ));
            };
            if slot.is_some() {
                return Err(self.invalid(
                    &format!("with-input-from-string received duplicate keyword :{keyword}"),
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
            body_items.push(Form::atom("PROGN", form.span));
            body_items.extend(items[2..].iter().cloned());
            Form::list(body_items, form.span)
        } else {
            Form::atom("NIL", form.span)
        };
        let body = if let Some(index_form) = index_form {
            let position_form = Form::list(
                vec![
                    Form::atom("__NCL-STRING-INPUT-STREAM-POSITION", form.span),
                    binding[0].clone(),
                ],
                form.span,
            );
            let update_form = Form::list(
                vec![Form::atom("SETF", form.span), index_form, position_form],
                form.span,
            );
            Form::list(
                vec![
                    Form::atom("MULTIPLE-VALUE-PROG1", form.span),
                    body,
                    update_form,
                ],
                form.span,
            )
        } else {
            body
        };
        let close_form = Form::list(
            vec![Form::atom("CLOSE", form.span), binding[0].clone()],
            form.span,
        );
        let protected_form = Form::list(
            vec![Form::atom("UNWIND-PROTECT", form.span), body, close_form],
            form.span,
        );
        Ok(Form::list(
            vec![
                Form::atom("LET", form.span),
                generated_binding,
                protected_form,
            ],
            form.span,
        ))
    }

    fn expand_with_hash_table_iterator(&self, form: &Form) -> Result<Form, RuntimeError> {
        let FormKind::List(items) = &form.kind else {
            return Ok(form.clone());
        };
        if items.len() < 2 {
            return Err(self.arity(
                "with-hash-table-iterator",
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        let binding_form = &items[1];
        let FormKind::List(binding) = &binding_form.kind else {
            return Err(self.invalid(
                "with-hash-table-iterator binding must be a list",
                binding_form.span,
            ));
        };
        if binding.len() != 2 {
            return Err(self.invalid(
                "with-hash-table-iterator binding needs an iterator name and a hash table",
                binding_form.span,
            ));
        }
        self.variable_name_info(
            &binding[0],
            "with-hash-table-iterator iterator name must be a symbol",
        )?;

        let state = self.fresh_hash_table_iterator_state(binding_form.span);
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
                        Form::atom("__NCL-HASH-TABLE-ITERATOR-NEXT", form.span),
                        state.clone(),
                    ],
                    form.span,
                ),
            ],
            binding_form.span,
        );
        let local_bindings = Form::list(vec![local_function], binding_form.span);
        let mut flet_items = Vec::with_capacity(items.len());
        flet_items.push(Form::atom("FLET", form.span));
        flet_items.push(local_bindings);
        flet_items.extend(items[2..].iter().cloned());
        let flet = Form::list(flet_items, form.span);
        let state_binding = Form::list(vec![state, initializer], binding_form.span);
        let let_bindings = Form::list(vec![state_binding], binding_form.span);
        Ok(Form::list(
            vec![Form::atom("LET", form.span), let_bindings, flet],
            form.span,
        ))
    }

    fn bind_macro_pattern(
        &self,
        pattern: &MacroPattern,
        value: Value,
        environment: &Environment,
        span: Span,
    ) -> Result<(), RuntimeError> {
        match pattern {
            MacroPattern::Name(name) => {
                self.define_macro_binding_in(name, value, environment);
                Ok(())
            }
            MacroPattern::List(patterns) => {
                let Some(values) = value.list_items() else {
                    return Err(
                        self.invalid("macro destructuring pattern requires a proper list", span)
                    );
                };
                if values.len() != patterns.len() {
                    return Err(self.invalid(
                        "macro destructuring pattern has the wrong number of elements",
                        span,
                    ));
                }
                for (pattern, value) in patterns.iter().zip(values) {
                    self.bind_macro_pattern(pattern, value, environment, span)?;
                }
                Ok(())
            }
            MacroPattern::Dotted { items, tail } => {
                let Some((values, dotted_tail)) = macro_dotted_parts(&value) else {
                    return Err(self.invalid("macro destructuring pattern requires a list", span));
                };
                if values.len() < items.len() {
                    return Err(
                        self.invalid("macro destructuring pattern has too few elements", span)
                    );
                }
                for (pattern, value) in items.iter().zip(values.iter().cloned()) {
                    self.bind_macro_pattern(pattern, value, environment, span)?;
                }
                let remaining = values[items.len()..].to_vec();
                let tail_value = if remaining.is_empty() {
                    dotted_tail
                } else if dotted_tail.is_truthy() {
                    Value::dotted_list(remaining, dotted_tail)
                } else {
                    Value::list(remaining)
                };
                self.bind_macro_pattern(tail, tail_value, environment, span)
            }
        }
    }

    fn bind_destructuring_lambda_list(
        &self,
        lambda_list: &MacroLambdaList,
        value: Value,
        environment: &Environment,
        span: Span,
    ) -> Result<(), RuntimeError> {
        let Some(arguments) = value.list_items() else {
            return Err(self.invalid("destructuring-bind value must be a proper list", span));
        };
        let required_count = lambda_list.required.len();
        let optional_count = lambda_list.optional.len();
        if arguments.len() < required_count {
            return Err(self.arity(
                "destructuring-bind",
                &format!("at least {required_count}"),
                arguments.len(),
            ));
        }

        let optional_supplied_count = if lambda_list.has_keyword_section {
            let available = arguments
                .len()
                .saturating_sub(required_count)
                .min(optional_count);
            (0..available)
                .take_while(|index| {
                    !matches!(
                        arguments[required_count + *index],
                        Value::Keyword(_) | Value::KeywordExact(_)
                    )
                })
                .count()
        } else {
            arguments
                .len()
                .saturating_sub(required_count)
                .min(optional_count)
        };
        let key_start = required_count + optional_supplied_count;
        if !lambda_list.has_keyword_section
            && lambda_list.rest.is_none()
            && arguments.len() > required_count + optional_count
        {
            let maximum = required_count + optional_count;
            return Err(self.arity(
                "destructuring-bind",
                &format!("at most {maximum}"),
                arguments.len(),
            ));
        }

        for (pattern, argument) in lambda_list
            .required
            .iter()
            .zip(arguments.iter().take(required_count).cloned())
        {
            self.bind_macro_pattern(pattern, argument, environment, span)?;
        }
        for (index, specification) in lambda_list.optional.iter().enumerate() {
            let supplied =
                (index < optional_supplied_count).then(|| &arguments[required_count + index]);
            let value = match supplied {
                Some(argument) => argument.clone(),
                None => self.eval_in(&specification.init_form, environment)?,
            };
            self.bind_macro_pattern(&specification.pattern, value, environment, span)?;
            if let Some(supplied_p) = &specification.supplied_p {
                self.define_macro_binding_in(
                    supplied_p,
                    Value::boolean(supplied.is_some()),
                    environment,
                );
            }
        }
        if let Some(rest_name) = &lambda_list.rest {
            self.define_macro_binding_in(
                rest_name,
                Value::list(arguments[key_start..].to_vec()),
                environment,
            );
        }

        if lambda_list.has_keyword_section {
            let keyword_arguments = &arguments[key_start..];
            if keyword_arguments.len() % 2 != 0 {
                return Err(self.invalid("keyword arguments must be supplied in pairs", span));
            }
            let mut supplied_keywords = Vec::new();
            let mut accepts_unknown_keywords = lambda_list.allow_other_keys;
            for pair in keyword_arguments.chunks_exact(2) {
                let (keyword_name, keyword_name_escaped) = match &pair[0] {
                    Value::Keyword(keyword) => (keyword.to_string(), false),
                    Value::KeywordExact(keyword) => (keyword.to_string(), true),
                    _ => {
                        return Err(self.invalid("keyword argument name must be a keyword", span));
                    }
                };
                if macro_keyword_matches(
                    "ALLOW-OTHER-KEYS",
                    false,
                    &keyword_name,
                    keyword_name_escaped,
                ) && pair[1].is_truthy()
                {
                    accepts_unknown_keywords = true;
                }
                supplied_keywords.push((keyword_name, keyword_name_escaped, pair[1].clone()));
            }
            if !accepts_unknown_keywords {
                for (keyword_name, keyword_name_escaped, _) in &supplied_keywords {
                    if !macro_keyword_matches(
                        "ALLOW-OTHER-KEYS",
                        false,
                        keyword_name,
                        *keyword_name_escaped,
                    ) && !lambda_list.keywords.iter().any(|specification| {
                        macro_keyword_matches(
                            &specification.keyword_name,
                            specification.keyword_name_escaped,
                            keyword_name,
                            *keyword_name_escaped,
                        )
                    }) {
                        return Err(self.invalid(&format!("unknown keyword :{keyword_name}"), span));
                    }
                }
            }
            for specification in &lambda_list.keywords {
                let supplied = supplied_keywords
                    .iter()
                    .find(|(keyword_name, keyword_name_escaped, _)| {
                        macro_keyword_matches(
                            &specification.keyword_name,
                            specification.keyword_name_escaped,
                            keyword_name,
                            *keyword_name_escaped,
                        )
                    })
                    .map(|(_, _, argument)| argument);
                let value = match supplied {
                    Some(argument) => argument.clone(),
                    None => self.eval_in(&specification.init_form, environment)?,
                };
                self.bind_macro_pattern(&specification.pattern, value, environment, span)?;
                if let Some(supplied_p) = &specification.supplied_p {
                    self.define_macro_binding_in(
                        supplied_p,
                        Value::boolean(supplied.is_some()),
                        environment,
                    );
                }
            }
        }
        for specification in &lambda_list.auxiliary {
            let value = self.eval_in(&specification.init_form, environment)?;
            self.define_macro_binding_in(&specification.name, value, environment);
        }
        Ok(())
    }

    fn special_quote(&self, items: &[Form], span: Span) -> Result<Value, RuntimeError> {
        if items.len() != 2 {
            return Err(self.arity("quote", "one", items.len().saturating_sub(1)));
        }
        self.quoted_value(&items[1]).map_err(|error| match error {
            RuntimeError::InvalidForm { .. } => self.invalid("invalid quoted form", span),
            error => error,
        })
    }

    fn special_the(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 3 {
            return Err(self.arity("the", "two", items.len().saturating_sub(1)));
        }
        let type_designator = quoted_form_value(&items[1])?;
        let value = self.eval_in(&items[2], environment)?;
        builtins::the_check(&[value, type_designator])
    }

    fn special_load_time_value(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if !(2..=3).contains(&items.len()) {
            return Err(self.arity(
                "load-time-value",
                "one or two",
                items.len().saturating_sub(1),
            ));
        }
        let value = self.eval_values_in(&items[1], environment)?;
        if let Some(read_only_p) = items.get(2) {
            let _ = self.eval_in(read_only_p, environment)?;
        }
        Ok(value)
    }

    fn special_nth_value(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 3 {
            return Err(self.arity("nth-value", "two", items.len().saturating_sub(1)));
        }

        let index_value = self.eval_in(&items[1], environment)?;
        let index = match index_value {
            Value::Integer(index) if index >= 0 => {
                usize::try_from(index).map_err(|_| RuntimeError::NumericOverflow)?
            }
            Value::Integer(_) => {
                return Err(self.invalid("nth-value index must be non-negative", items[1].span));
            }
            value => {
                return Err(RuntimeError::Type {
                    expected: "INTEGER".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(items[1].span),
                });
            }
        };

        let values = self
            .eval_values_in(&items[2], environment)?
            .multiple_values();
        Ok(values.get(index).cloned().unwrap_or(Value::Nil))
    }

    fn special_locally(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        let declared = self.declared_special_names(items.get(1..).unwrap_or(&[]))?;
        let (names, exact_names) = split_special_names(declared);
        let _special_guard = self.special_declaration_guard(&names, &exact_names);
        self.eval_sequence_values(items.get(1..).unwrap_or(&[]), environment)
    }

    fn special_global_declaration(
        &self,
        items: &[Form],
        _environment: &Environment,
        quoted: bool,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(self.arity(
                if quoted { "proclaim" } else { "declaim" },
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        let specs = if quoted {
            let argument = &items[1];
            match &argument.kind {
                FormKind::List(quoted_form)
                    if quoted_form.len() == 2
                        && atom_name(&quoted_form[0])
                            .map(|name| normalize_name(name) == "QUOTE")
                            .unwrap_or(false) =>
                {
                    vec![quoted_form[1].clone()]
                }
                _ => vec![argument.clone()],
            }
        } else {
            items[1..].to_vec()
        };
        let declared = self.special_names_from_specs(&specs)?;
        for (name, escaped) in declared {
            self.declare_special(&name, escaped);
        }
        Ok(Value::Nil)
    }

    fn special_names_from_specs(
        &self,
        specs: &[Form],
    ) -> Result<HashSet<(String, bool)>, RuntimeError> {
        let mut names = HashSet::new();
        for spec in specs {
            let FormKind::List(items) = &spec.kind else {
                continue;
            };
            let Some(operator) = items.first().and_then(atom_name) else {
                continue;
            };
            let Ok(token) = parse_symbol_token(operator) else {
                continue;
            };
            if token.kind != SymbolTokenKind::Symbol
                || token.package.is_some()
                || token.escaped
                || !token.name.eq_ignore_ascii_case("SPECIAL")
            {
                continue;
            }
            for variable in &items[1..] {
                names.insert(
                    self.variable_name_info(variable, "special declaration name must be a symbol")?,
                );
            }
        }
        Ok(names)
    }

    fn declared_special_names(
        &self,
        forms: &[Form],
    ) -> Result<HashSet<(String, bool)>, RuntimeError> {
        let mut names = HashSet::new();
        for form in forms {
            let FormKind::List(items) = &form.kind else {
                break;
            };
            let Some(operator) = items.first().and_then(atom_name) else {
                break;
            };
            let Ok(token) = parse_symbol_token(operator) else {
                break;
            };
            if token.kind != SymbolTokenKind::Symbol
                || token.package.is_some()
                || token.escaped
                || !token.name.eq_ignore_ascii_case("DECLARE")
            {
                break;
            }
            names.extend(self.special_names_from_specs(&items[1..])?);
        }
        Ok(names)
    }

    fn special_eval_when(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(self.arity("eval-when", "at least one", items.len().saturating_sub(1)));
        }
        if self.eval_when_executes(&items[1])? {
            self.eval_sequence_values(items.get(2..).unwrap_or(&[]), environment)
        } else {
            Ok(Value::Nil)
        }
    }

    fn eval_when_executes(&self, form: &Form) -> Result<bool, RuntimeError> {
        let FormKind::List(situations) = &form.kind else {
            return Err(self.invalid("eval-when situations must be a list", form.span));
        };
        let mut executes = false;
        for situation in situations {
            let Some(name) = atom_name(situation) else {
                return Err(
                    self.invalid("eval-when situations must contain symbols", situation.span)
                );
            };
            let token = parse_symbol_token(name).map_err(|_| {
                self.invalid("eval-when situations must contain symbols", situation.span)
            })?;
            if token.kind == SymbolTokenKind::Uninterned
                || (token.kind == SymbolTokenKind::Symbol && literal_atom(name).is_some())
            {
                return Err(
                    self.invalid("eval-when situations must contain symbols", situation.span)
                );
            }
            if token.package.is_none() && token.name.eq_ignore_ascii_case("execute") {
                executes = true;
            }
        }
        Ok(executes)
    }

    fn special_quasiquote(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 2 {
            return Err(self.arity("quasiquote", "one", items.len().saturating_sub(1)));
        }
        self.quasiquote_value(&items[1], environment)
    }

    pub(crate) fn quasiquote_value(
        &self,
        form: &Form,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        self.quasiquote_value_at(form, environment, 1)
    }

    fn quasiquote_value_at(
        &self,
        form: &Form,
        environment: &Environment,
        depth: usize,
    ) -> Result<Value, RuntimeError> {
        match &form.kind {
            FormKind::Atom(_)
            | FormKind::String(_)
            | FormKind::Character(_)
            | FormKind::Complex { .. }
            | FormKind::BitVector(_)
            | FormKind::ReadTimeEval(_) => self.quoted_value(form),
            FormKind::Vector(items) => {
                let mut values = Vec::with_capacity(items.len());
                for item in items {
                    if depth == 1 {
                        if let Some(argument) = prefix_argument(
                            match &item.kind {
                                FormKind::List(items) => items,
                                _ => &[],
                            },
                            "UNQUOTE-SPLICING",
                        ) {
                            values.extend(self.quasiquote_splice(
                                argument,
                                environment,
                                item.span,
                            )?);
                            continue;
                        }
                    }
                    values.push(self.quasiquote_value_at(item, environment, depth)?);
                }
                Ok(Value::vector(values))
            }
            FormKind::List(items) => {
                if let Some(argument) = prefix_argument(items, "UNQUOTE") {
                    if depth == 1 {
                        return self.eval_in(argument, environment);
                    }
                    return Ok(quasiquote_marker(
                        "UNQUOTE",
                        self.quasiquote_value_at(argument, environment, depth - 1)?,
                    ));
                }
                if let Some(item) = prefix_argument(items, "UNQUOTE-SPLICING") {
                    if depth == 1 {
                        return Err(self.invalid(
                            "unquote-splicing is only valid inside a list or vector",
                            item.span,
                        ));
                    }
                    return Ok(quasiquote_marker(
                        "UNQUOTE-SPLICING",
                        self.quasiquote_value_at(item, environment, depth - 1)?,
                    ));
                }
                if let Some(argument) = prefix_argument(items, "QUASIQUOTE") {
                    return Ok(quasiquote_marker(
                        "QUASIQUOTE",
                        self.quasiquote_value_at(argument, environment, depth + 1)?,
                    ));
                }

                let mut values = Vec::with_capacity(items.len());
                for item in items {
                    if depth == 1 {
                        if let Some(argument) = prefix_argument(
                            match &item.kind {
                                FormKind::List(items) => items,
                                _ => &[],
                            },
                            "UNQUOTE-SPLICING",
                        ) {
                            values.extend(self.quasiquote_splice(
                                argument,
                                environment,
                                item.span,
                            )?);
                            continue;
                        }
                    } else {
                        values.push(self.quasiquote_value_at(item, environment, depth)?);
                        continue;
                    }
                    values.push(self.quasiquote_value_at(item, environment, depth)?);
                }
                Ok(Value::list(values))
            }
            FormKind::DottedList { items, tail } => {
                let mut values = Vec::with_capacity(items.len());
                for item in items {
                    if depth == 1 {
                        if let Some(argument) = prefix_argument(
                            match &item.kind {
                                FormKind::List(items) => items,
                                _ => &[],
                            },
                            "UNQUOTE-SPLICING",
                        ) {
                            values.extend(self.quasiquote_splice(
                                argument,
                                environment,
                                item.span,
                            )?);
                            continue;
                        }
                    } else {
                        values.push(self.quasiquote_value_at(item, environment, depth)?);
                        continue;
                    }
                    values.push(self.quasiquote_value_at(item, environment, depth)?);
                }
                if let Some(argument) = prefix_argument(
                    match &tail.kind {
                        FormKind::List(items) => items,
                        _ => &[],
                    },
                    "UNQUOTE-SPLICING",
                ) {
                    if depth == 1 {
                        let mut spliced =
                            self.quasiquote_splice(argument, environment, tail.span)?;
                        values.append(&mut spliced);
                        return Ok(Value::list(values));
                    }
                }
                let tail_value = self.quasiquote_value_at(tail, environment, depth)?;
                if depth == 1 {
                    if let Some(mut tail_items) = tail_value.list_items() {
                        values.append(&mut tail_items);
                        return Ok(Value::list(values));
                    }
                }
                Ok(Value::dotted_list(values, tail_value))
            }
        }
    }

    fn quasiquote_splice(
        &self,
        argument: &Form,
        environment: &Environment,
        span: Span,
    ) -> Result<Vec<Value>, RuntimeError> {
        let value = self.eval_in(argument, environment)?;
        value
            .list_items()
            .ok_or_else(|| self.invalid("unquote-splicing requires a proper list", span))
    }

    fn special_if(&self, items: &[Form], environment: &Environment) -> Result<Value, RuntimeError> {
        if !(items.len() == 3 || items.len() == 4) {
            return Err(self.arity("if", "two or three", items.len().saturating_sub(1)));
        }
        let condition = self.eval_in(&items[1], environment)?;
        if condition.is_truthy() {
            self.eval_values_in(&items[2], environment)
        } else {
            items.get(3).map_or(Ok(Value::Nil), |form| {
                self.eval_values_in(form, environment)
            })
        }
    }

    fn special_progn(
        &self,
        forms: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        self.eval_sequence_values(forms, environment)
    }

    fn special_prog1(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(self.arity("prog1", "at least one", items.len().saturating_sub(1)));
        }
        let result = self.eval_values_in(&items[1], environment)?;
        self.eval_sequence_values(&items[2..], environment)?;
        Ok(result)
    }

    fn special_prog2(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(self.arity("prog2", "at least two", items.len().saturating_sub(1)));
        }
        self.eval_values_in(&items[1], environment)?;
        let result = self.eval_values_in(&items[2], environment)?;
        self.eval_sequence_values(&items[3..], environment)?;
        Ok(result)
    }

    fn special_prog(
        &self,
        items: &[Form],
        environment: &Environment,
        sequential: bool,
    ) -> Result<Value, RuntimeError> {
        let operator = if sequential { "prog*" } else { "prog" };
        if items.len() < 2 {
            return Err(self.arity(operator, "at least one", items.len().saturating_sub(1)));
        }
        let FormKind::List(binding_forms) = &items[1].kind else {
            return Err(self.invalid("prog bindings must be a list", items[1].span));
        };

        let mut names = HashSet::new();
        let mut bindings = Vec::with_capacity(binding_forms.len());
        for binding in binding_forms {
            let (name_form, init) = match &binding.kind {
                FormKind::Atom(_) => (binding, None),
                FormKind::List(parts) => {
                    if !(1..=2).contains(&parts.len()) {
                        return Err(self.invalid(
                            "prog binding needs a name and optional value",
                            binding.span,
                        ));
                    }
                    let Some(name_form) = parts.first() else {
                        return Err(self.invalid("prog binding needs a name", binding.span));
                    };
                    (name_form, parts.get(1).cloned())
                }
                _ => {
                    return Err(self.invalid("prog binding must be a symbol or list", binding.span));
                }
            };
            let (name, escaped) =
                self.variable_name_info(name_form, "prog binding name must be a symbol")?;
            let key = if escaped {
                format!("\0{name}")
            } else {
                name.clone()
            };
            if !names.insert(key) {
                return Err(self.invalid("prog binding names must be unique", name_form.span));
            }
            bindings.push((name, escaped, init));
        }

        let target = self.fresh_block_target();
        let block_environment = environment.child();
        block_environment.define_block("NIL", target);
        let local = block_environment.child();
        let _dynamic_guard = self.dynamic_guard();

        let execute = || -> Result<Value, RuntimeError> {
            if sequential {
                for (name, escaped, init) in &bindings {
                    let value = init
                        .as_ref()
                        .map_or(Ok(Value::Nil), |form| self.eval_in(form, &local))?;
                    self.define_variable_in(name, *escaped, value, &local);
                }
            } else {
                let mut values = Vec::with_capacity(bindings.len());
                for (_, _, init) in &bindings {
                    values.push(init.as_ref().map_or(Ok(Value::Nil), |form| {
                        self.eval_in(form, &block_environment)
                    })?);
                }
                for ((name, escaped, _), value) in bindings.iter().zip(values) {
                    self.define_variable_in(name, *escaped, value, &local);
                }
            }

            self.eval_tagbody_forms(&items[2..], &local)?;
            Ok(Value::Nil)
        };

        match execute() {
            Ok(value) => Ok(value),
            Err(RuntimeError::ReturnFrom {
                target: Some(return_target),
                value,
                ..
            }) if return_target == target => Ok(value.into_value()),
            Err(error) => Err(error),
        }
    }

    fn special_values(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        let values = items[1..]
            .iter()
            .map(|form| self.eval_in(form, environment))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Value::values(values))
    }

    fn special_multiple_value_list(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 2 {
            return Err(self.arity("multiple-value-list", "one", items.len().saturating_sub(1)));
        }
        let values = self
            .eval_values_in(&items[1], environment)?
            .multiple_values();
        Ok(Value::list(values))
    }

    fn special_ignore_errors(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        match self.eval_sequence_values(&items[1..], environment) {
            Ok(value) => Ok(value),
            Err(error @ RuntimeError::ReturnFrom { .. }) => Err(error),
            Err(error @ RuntimeError::Go { .. }) => Err(error),
            Err(error @ RuntimeError::InvokeRestart { .. }) => Err(error),
            Err(error) => Ok(Value::values(vec![Value::Nil, Value::condition(&error)])),
        }
    }

    fn special_handler_case(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(self.arity(
                "handler-case",
                "at least two",
                items.len().saturating_sub(1),
            ));
        }

        let mut handlers = Vec::with_capacity(items.len().saturating_sub(2));
        let mut no_error_clause = None;
        for clause in &items[2..] {
            let FormKind::List(clause_items) = &clause.kind else {
                return Err(self.invalid("handler-case clause must be a list", clause.span));
            };
            if clause_items.len() < 2 {
                return Err(self.invalid(
                    "handler-case clause needs a condition and body",
                    clause.span,
                ));
            }
            let FormKind::List(variables) = &clause_items[1].kind else {
                return Err(self.invalid(
                    "handler-case variable list must be a list",
                    clause_items[1].span,
                ));
            };
            if is_no_error_marker(&clause_items[0]) {
                if no_error_clause.replace(clause_items).is_some() {
                    return Err(self.invalid(
                        "handler-case accepts at most one :NO-ERROR clause",
                        clause.span,
                    ));
                }
                for variable in variables {
                    self.variable_name_info(variable, "handler-case no-error variable")?;
                }
                continue;
            }
            if variables.len() > 1 {
                return Err(self.invalid(
                    "handler-case accepts at most one condition variable",
                    clause_items[1].span,
                ));
            }
            let condition = self.condition_name(&clause_items[0])?;
            if let Some(variable) = variables.first() {
                self.variable_name_info(variable, "handler-case condition variable")?;
            }
            handlers.push(ConditionHandlerBinding {
                condition,
                function: None,
                catch: true,
            });
        }

        let guard = self.condition_handler_guard(handlers);
        let protected_result = self.eval_values_in(&items[1], environment);
        drop(guard);
        let protected = match protected_result {
            Ok(value) => {
                let Some(clause_items) = no_error_clause else {
                    return Ok(value);
                };
                let FormKind::List(variables) = &clause_items[1].kind else {
                    unreachable!("handler-case clauses were validated above");
                };
                let values = value.multiple_values();
                let local = environment.child();
                for (index, variable) in variables.iter().enumerate() {
                    let (name, escaped) =
                        self.variable_name_info(variable, "handler-case no-error variable")?;
                    self.define_variable_in(
                        &name,
                        escaped,
                        values.get(index).cloned().unwrap_or(Value::Nil),
                        &local,
                    );
                }
                return self.eval_sequence_values(&clause_items[2..], &local);
            }
            Err(error @ RuntimeError::ReturnFrom { .. }) => return Err(error),
            Err(error @ RuntimeError::Go { .. }) => return Err(error),
            Err(error @ RuntimeError::InvokeRestart { .. }) => return Err(error),
            Err(error) => error,
        };

        for clause in &items[2..] {
            let FormKind::List(clause_items) = &clause.kind else {
                unreachable!("handler-case clauses were validated above");
            };
            if is_no_error_marker(&clause_items[0]) {
                continue;
            }
            let condition = self.condition_name(&clause_items[0])?;
            if !protected.matches_condition(&condition) {
                continue;
            }
            let local = environment.child();
            if let FormKind::List(variables) = &clause_items[1].kind {
                if let Some(variable) = variables.first() {
                    let (name, escaped) =
                        self.variable_name_info(variable, "handler-case condition variable")?;
                    self.define_variable_in(&name, escaped, Value::condition(&protected), &local);
                }
            }
            return self.eval_sequence_values(&clause_items[2..], &local);
        }

        Err(protected)
    }

    fn special_handler_bind(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(self.arity("handler-bind", "at least one", 0));
        }
        let FormKind::List(handlers) = &items[1].kind else {
            return Err(self.invalid("handler-bind handler list must be a list", items[1].span));
        };
        let mut handler_bindings = Vec::with_capacity(handlers.len());
        for handler in handlers {
            let FormKind::List(parts) = &handler.kind else {
                return Err(self.invalid("handler-bind clause must be a list", handler.span));
            };
            if parts.len() != 2 {
                return Err(self.invalid(
                    "handler-bind clause needs a condition and function",
                    handler.span,
                ));
            }
            let condition = self.condition_name(&parts[0])?;
            let function = self.eval_in(&parts[1], environment)?;
            handler_bindings.push(ConditionHandlerBinding {
                condition,
                function: Some(function),
                catch: false,
            });
        }

        let guard = self.condition_handler_guard(handler_bindings.clone());
        let body_result = self.eval_sequence_values(&items[2..], environment);
        drop(guard);
        let body = match body_result {
            Ok(value) => return Ok(value),
            Err(error @ RuntimeError::ReturnFrom { .. }) => return Err(error),
            Err(error @ RuntimeError::Go { .. }) => return Err(error),
            Err(error @ RuntimeError::InvokeRestart { .. }) => return Err(error),
            Err(error @ RuntimeError::Signaled { .. }) => return Err(error),
            Err(error) => error,
        };

        for (handler, binding) in handlers.iter().zip(handler_bindings.iter()).rev() {
            let FormKind::List(parts) = &handler.kind else {
                unreachable!("handler-bind clauses were validated above");
            };
            if body.matches_condition(&binding.condition) {
                let Some(function) = &binding.function else {
                    return Err(body);
                };
                return self.apply_in(
                    function,
                    &[Value::condition(&body)],
                    parts[1].span,
                    environment,
                );
            }
        }

        Err(body)
    }

    fn special_restart_bind(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(self.arity("restart-bind", "at least one", 0));
        }
        let FormKind::List(bindings) = &items[1].kind else {
            return Err(self.invalid("restart-bind binding list must be a list", items[1].span));
        };

        let mut restarts = Vec::with_capacity(bindings.len());
        for binding in bindings {
            let FormKind::List(parts) = &binding.kind else {
                return Err(self.invalid("restart-bind clause must be a list", binding.span));
            };
            if parts.len() != 2 {
                return Err(self.invalid(
                    "restart-bind clause needs a name and function",
                    binding.span,
                ));
            }
            let name = self.restart_name(&parts[0])?;
            let function = self.eval_in(&parts[1], environment)?;
            restarts.push((name, function, parts[1].span));
        }

        let guard = self.restart_guard(
            restarts
                .iter()
                .map(|(name, function, _)| {
                    RestartBinding::new(name.clone(), Some(function.clone()))
                })
                .collect(),
        );
        let body_result = self.eval_sequence_values(&items[2..], environment);
        drop(guard);
        match body_result {
            Ok(value) => Ok(value),
            Err(error) => {
                let RuntimeError::InvokeRestart {
                    name: invoked,
                    arguments,
                    ..
                } = &error
                else {
                    return Err(error);
                };
                let Some((_, function, binding_span)) = restarts
                    .iter()
                    .rev()
                    .find(|(name, _, _)| normalize_name(invoked.as_str()) == name.as_str())
                else {
                    return Err(error);
                };
                let argument_values = arguments
                    .iter()
                    .cloned()
                    .map(ReturnValue::into_value)
                    .collect::<Vec<_>>();
                self.apply_in(function, &argument_values, *binding_span, environment)
            }
        }
    }

    fn special_catch(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(self.arity("catch", "at least one", 0));
        }

        let tag = self.eval_values_in(&items[1], environment)?.primary_value();
        match self.eval_sequence_values(&items[2..], environment) {
            Ok(value) => Ok(value),
            Err(RuntimeError::Throw {
                tag: thrown_tag,
                value,
                ..
            }) if thrown_tag.matches(&tag) => Ok(value.into_value()),
            Err(error) => Err(error),
        }
    }

    fn special_with_simple_restart(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(self.arity(
                "with-simple-restart",
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        let FormKind::List(clause) = &items[1].kind else {
            return Err(self.invalid(
                "with-simple-restart restart clause must be a list",
                items[1].span,
            ));
        };
        if clause.len() < 2 {
            return Err(self.invalid(
                "with-simple-restart restart clause needs a name and report format",
                items[1].span,
            ));
        }
        let name = self.restart_name(&clause[0])?;
        let guard = self.restart_guard(vec![RestartBinding::new(name.clone(), None)]);
        let body_result = self.eval_sequence_values(&items[2..], environment);
        drop(guard);
        match body_result {
            Ok(value) => Ok(value),
            Err(RuntimeError::InvokeRestart {
                name: invoked,
                value,
                ..
            }) if normalize_name(invoked.as_str()) == name => Ok(value.into_value()),
            Err(error) => Err(error),
        }
    }

    fn special_with_condition_restarts(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 4 {
            return Err(self.arity(
                "with-condition-restarts",
                "at least three",
                items.len().saturating_sub(1),
            ));
        }
        let condition = self.eval_values_in(&items[1], environment)?.primary_value();
        if condition.condition_type_name().is_none() {
            return Err(RuntimeError::Type {
                expected: "CONDITION".to_string(),
                actual: condition.type_name().to_string(),
                span: Some(items[1].span),
            });
        }
        let restarts_value = self.eval_values_in(&items[2], environment)?.primary_value();
        let Some(restarts) = restarts_value.list_items() else {
            return Err(RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: restarts_value.type_name().to_string(),
                span: Some(items[2].span),
            });
        };
        if let Some(restart) = restarts
            .iter()
            .find(|restart| restart.restart_name().is_none())
        {
            return Err(RuntimeError::Type {
                expected: "RESTART".to_string(),
                actual: restart.type_name().to_string(),
                span: Some(items[2].span),
            });
        }
        let guard = self.condition_restart_guard(condition, restarts);
        let result = self.eval_sequence_values(&items[3..], environment);
        drop(guard);
        result
    }

    fn special_restart_case(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(self.arity(
                "restart-case",
                "at least two",
                items.len().saturating_sub(1),
            ));
        }

        for clause in &items[2..] {
            let FormKind::List(parts) = &clause.kind else {
                return Err(self.invalid("restart-case clause must be a list", clause.span));
            };
            if parts.len() < 2 {
                return Err(self.invalid(
                    "restart-case clause needs a name, lambda list, and body",
                    clause.span,
                ));
            }
            self.restart_name(&parts[0])?;
            self.parameters(&parts[1])?;
        }

        let mut clauses = Vec::with_capacity(items.len().saturating_sub(2));
        for clause in &items[2..] {
            let FormKind::List(parts) = &clause.kind else {
                unreachable!("restart-case clauses were validated above");
            };
            let name = self.restart_name(&parts[0])?;
            let lambda_list = self.parameters(&parts[1])?;
            let closure = Value::closure_with_keywords(
                lambda_list.required.clone(),
                lambda_list.required_escaped.clone(),
                lambda_list.optional.clone(),
                lambda_list.rest.clone(),
                lambda_list.rest_escaped,
                lambda_list.keywords.clone(),
                lambda_list.has_keyword_section,
                lambda_list.allow_other_keys,
                lambda_list.auxiliary.clone(),
                parts[2..].to_vec(),
                environment.clone(),
            );
            clauses.push((name, closure, clause.span));
        }

        let guard = self.restart_guard(
            clauses
                .iter()
                .map(|(name, _, _)| RestartBinding::new(name.clone(), None))
                .collect(),
        );
        let protected_result = self.eval_values_in(&items[1], environment);
        drop(guard);
        match protected_result {
            Ok(value) => Ok(value),
            Err(error) => {
                if let RuntimeError::InvokeRestart {
                    name: invoked,
                    arguments,
                    ..
                } = &error
                {
                    if let Some((_, closure, clause_span)) =
                        clauses.iter().find(|(restart, _, _)| {
                            normalize_name(invoked.as_str()) == restart.as_str()
                        })
                    {
                        let argument_values = arguments
                            .iter()
                            .cloned()
                            .map(ReturnValue::into_value)
                            .collect::<Vec<_>>();
                        return self.apply_in(closure, &argument_values, *clause_span, environment);
                    }
                }
                Err(error)
            }
        }
    }

    fn special_throw(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 3 {
            return Err(self.arity("throw", "two", items.len().saturating_sub(1)));
        }

        let tag = self.eval_values_in(&items[1], environment)?.primary_value();
        let value = self.eval_values_in(&items[2], environment)?;
        Err(RuntimeError::Throw {
            tag: ThrowTag::new(tag),
            value: ReturnValue::new(value),
            span: Some(items[0].span),
        })
    }

    fn special_progv(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(self.arity("progv", "at least two", items.len().saturating_sub(1)));
        }

        let symbols_value = self.eval_values_in(&items[1], environment)?.primary_value();
        let symbols = symbols_value
            .list_items()
            .ok_or_else(|| RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: symbols_value.type_name().to_string(),
                span: Some(items[1].span),
            })?;
        let values_value = self.eval_values_in(&items[2], environment)?.primary_value();
        let values = values_value
            .list_items()
            .ok_or_else(|| RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: values_value.type_name().to_string(),
                span: Some(items[2].span),
            })?;

        let _dynamic_guard = self.dynamic_guard();
        for (index, symbol) in symbols.iter().enumerate() {
            let name = symbol.symbol_name().ok_or_else(|| {
                self.invalid("progv symbol list must contain only symbols", items[1].span)
            })?;
            self.define_dynamic(name, values.get(index).cloned().unwrap_or(Value::Nil));
        }

        self.eval_sequence_values(&items[3..], environment)
    }

    fn special_unwind_protect(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(self.arity(
                "unwind-protect",
                "at least one",
                items.len().saturating_sub(1),
            ));
        }

        let protected = self.eval_values_in(&items[1], environment);
        let cleanup = self.eval_sequence_values(&items[2..], environment);
        match cleanup {
            Ok(_) => protected,
            Err(error) => Err(error),
        }
    }

    fn special_block(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(self.arity("block", "at least one", items.len().saturating_sub(1)));
        }
        let name = self.block_name(&items[1])?;
        let target = self.fresh_block_target();
        let block_environment = environment.child();
        block_environment.define_block(&name, target);
        match self.eval_sequence_values(&items[2..], &block_environment) {
            Ok(value) => Ok(value),
            Err(RuntimeError::ReturnFrom {
                target: Some(return_target),
                value,
                ..
            }) if return_target == target => Ok(value.into_value()),
            Err(error) => Err(error),
        }
    }

    fn special_return_from(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if !(items.len() == 2 || items.len() == 3) {
            return Err(self.arity("return-from", "one or two", items.len().saturating_sub(1)));
        }
        let block = self.block_name(&items[1])?;
        let value = items.get(2).map_or(Ok(Value::Nil), |form| {
            self.eval_values_in(form, environment)
        })?;
        let target = environment.lookup_block(&block);
        Err(RuntimeError::ReturnFrom {
            block,
            target,
            value: ReturnValue::new(value),
            span: Some(items[1].span),
        })
    }

    fn special_return(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if !(items.len() == 1 || items.len() == 2) {
            return Err(self.arity("return", "zero or one", items.len().saturating_sub(1)));
        }
        let value = items.get(1).map_or(Ok(Value::Nil), |form| {
            self.eval_values_in(form, environment)
        })?;
        let block = "NIL".to_string();
        let target = environment.lookup_block(&block);
        Err(RuntimeError::ReturnFrom {
            block,
            target,
            value: ReturnValue::new(value),
            span: Some(items[0].span),
        })
    }

    fn special_tagbody(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        self.eval_tagbody_forms(&items[1..], environment)
    }

    fn eval_tagbody_forms(
        &self,
        forms: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        let mut tags: Vec<(String, usize)> = Vec::new();
        for (position, item) in forms.iter().enumerate() {
            if let Some(tag) = control_tag(item) {
                if tags.iter().any(|(known_tag, _)| known_tag == &tag) {
                    return Err(self.invalid("tagbody contains duplicate tag", item.span));
                }
                tags.push((tag, position));
            }
        }

        let target = self.fresh_block_target();
        let tag_environment = environment.child();
        for (tag, _) in &tags {
            tag_environment.define_tag(tag, target);
        }

        let mut position = 0;
        while position < forms.len() {
            let item = &forms[position];
            if control_tag(item).is_some() {
                position += 1;
                continue;
            }
            match self.eval_values_in(item, &tag_environment) {
                Ok(_) => position += 1,
                Err(RuntimeError::Go {
                    tag,
                    target: Some(go_target),
                    ..
                }) if go_target == target => {
                    position = tags
                        .iter()
                        .find(|(known_tag, _)| known_tag == &tag)
                        .map(|(_, tag_position)| *tag_position)
                        .ok_or_else(|| {
                            self.invalid("GO target is missing from TAGBODY", item.span)
                        })?;
                }
                Err(error) => return Err(error),
            }
        }
        Ok(Value::Nil)
    }

    fn special_go(&self, items: &[Form], environment: &Environment) -> Result<Value, RuntimeError> {
        if items.len() != 2 {
            return Err(self.arity("go", "one", items.len().saturating_sub(1)));
        }
        let tag = control_tag(&items[1])
            .ok_or_else(|| self.invalid("go tag must be a symbol or integer", items[1].span))?;
        Err(RuntimeError::Go {
            target: environment.lookup_tag(&tag),
            tag,
            span: Some(items[1].span),
        })
    }

    fn special_multiple_value_bind(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(self.arity(
                "multiple-value-bind",
                "at least two",
                items.len().saturating_sub(1),
            ));
        }
        let FormKind::List(variable_forms) = &items[1].kind else {
            return Err(self.invalid(
                "multiple-value-bind variables must be a list",
                items[1].span,
            ));
        };
        let variables = variable_forms
            .iter()
            .map(|form| {
                self.variable_name_info(form, "multiple-value-bind variable must be a symbol")
            })
            .collect::<Result<Vec<_>, _>>()?;
        let source = self.eval_values_in(&items[2], environment)?;
        let values = source.multiple_values();
        let local = environment.child();
        let _dynamic_guard = self.dynamic_guard();
        for (index, (variable, escaped)) in variables.iter().enumerate() {
            self.define_variable_in(
                variable,
                *escaped,
                values.get(index).cloned().unwrap_or(Value::Nil),
                &local,
            );
        }
        self.eval_sequence_values(&items[3..], &local)
    }

    fn special_multiple_value_call(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(self.arity(
                "multiple-value-call",
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        let function = self.eval_in(&items[1], environment)?;
        let mut arguments = Vec::new();
        for form in &items[2..] {
            arguments.extend(self.eval_values_in(form, environment)?.multiple_values());
        }
        self.apply_in(&function, &arguments, items[0].span, environment)
    }

    fn special_multiple_value_prog1(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(self.arity(
                "multiple-value-prog1",
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        let result = self.eval_values_in(&items[1], environment)?;
        self.eval_sequence_values(&items[2..], environment)?;
        Ok(result)
    }

    fn special_and(
        &self,
        forms: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        let mut result = Value::boolean(true);
        for (index, form) in forms.iter().enumerate() {
            result = self.eval_values_in(form, environment)?;
            if !result.is_truthy() {
                return if index + 1 == forms.len() {
                    Ok(result)
                } else {
                    Ok(result.primary_value())
                };
            }
        }
        Ok(result)
    }

    fn special_or(&self, forms: &[Form], environment: &Environment) -> Result<Value, RuntimeError> {
        for (index, form) in forms.iter().enumerate() {
            let result = self.eval_values_in(form, environment)?;
            if result.is_truthy() {
                return if index + 1 == forms.len() {
                    Ok(result)
                } else {
                    Ok(result.primary_value())
                };
            }
        }
        Ok(Value::Nil)
    }

    fn special_when(
        &self,
        items: &[Form],
        environment: &Environment,
        positive: bool,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(self.arity(
                if positive { "when" } else { "unless" },
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        let condition = self.eval_in(&items[1], environment)?.is_truthy();
        if condition == positive {
            self.eval_sequence_values(&items[2..], environment)
        } else {
            Ok(Value::Nil)
        }
    }

    fn special_cond(
        &self,
        clauses: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        for clause in clauses {
            let FormKind::List(items) = &clause.kind else {
                return Err(self.invalid("cond clauses must be lists", clause.span));
            };
            if items.is_empty() {
                return Err(self.invalid("cond clause cannot be empty", clause.span));
            }
            let condition = self.eval_in(&items[0], environment)?;
            if condition.is_truthy() {
                return if items.len() == 1 {
                    Ok(condition)
                } else {
                    self.eval_sequence_values(&items[1..], environment)
                };
            }
        }
        Ok(Value::Nil)
    }

    fn special_case(
        &self,
        items: &[Form],
        environment: &Environment,
        error_on_miss: bool,
    ) -> Result<Value, RuntimeError> {
        let operator = if error_on_miss { "ecase" } else { "case" };
        if items.len() < 2 {
            return Err(self.arity(operator, "at least one", items.len().saturating_sub(1)));
        }

        let key = self.eval_in(&items[1], environment)?;
        let mut default_body: Option<&[Form]> = None;
        for clause in &items[2..] {
            let FormKind::List(parts) = &clause.kind else {
                return Err(self.invalid("case clauses must be lists", clause.span));
            };
            if parts.is_empty() {
                return Err(self.invalid("case clause cannot be empty", clause.span));
            }
            if is_case_default_form(&parts[0]) {
                default_body = Some(&parts[1..]);
                continue;
            }

            let keys = match &parts[0].kind {
                FormKind::List(keys) => keys.as_slice(),
                _ => std::slice::from_ref(&parts[0]),
            };
            for key_form in keys {
                let candidate = quoted_form_value(key_form)?;
                if builtins::eql_value(&key, &candidate) {
                    return self.eval_sequence_values(&parts[1..], environment);
                }
            }
        }

        if let Some(body) = default_body {
            self.eval_sequence_values(body, environment)
        } else if error_on_miss {
            Err(self.invalid("ecase fell through", items[0].span))
        } else {
            Ok(Value::Nil)
        }
    }

    fn special_typecase(
        &self,
        items: &[Form],
        environment: &Environment,
        error_on_miss: bool,
    ) -> Result<Value, RuntimeError> {
        let operator = if error_on_miss {
            "etypecase"
        } else {
            "typecase"
        };
        if items.len() < 2 {
            return Err(self.arity(operator, "at least one", items.len().saturating_sub(1)));
        }

        let key = self.eval_in(&items[1], environment)?;
        let mut default_body: Option<&[Form]> = None;
        for clause in &items[2..] {
            let FormKind::List(parts) = &clause.kind else {
                return Err(self.invalid("typecase clauses must be lists", clause.span));
            };
            if parts.is_empty() {
                return Err(self.invalid("typecase clause cannot be empty", clause.span));
            }
            if is_case_default_form(&parts[0]) {
                default_body = Some(&parts[1..]);
                continue;
            }

            let type_designator = quoted_form_value(&parts[0])?;
            if builtins::typep_value(&key, &type_designator)? {
                return self.eval_sequence_values(&parts[1..], environment);
            }
        }

        if let Some(body) = default_body {
            self.eval_sequence_values(body, environment)
        } else if error_on_miss {
            Err(self.invalid("etypecase fell through", items[0].span))
        } else {
            Ok(Value::Nil)
        }
    }

    fn special_change_case(
        &self,
        items: &[Form],
        environment: &Environment,
        typecase: bool,
    ) -> Result<Value, RuntimeError> {
        let operator = if typecase { "ctypecase" } else { "ccase" };
        if items.len() < 2 {
            return Err(self.arity(operator, "at least one", items.len().saturating_sub(1)));
        }

        let expansion = self.get_setf_expansion(&items[1], environment)?;
        let local = self.initialize_setf_expansion(&expansion, environment, items[1].span)?;
        loop {
            let key = self.eval_in(&expansion.access_form, &local)?;
            let mut matched = None;
            let mut default_body = None;
            for clause in &items[2..] {
                let FormKind::List(parts) = &clause.kind else {
                    return Err(self.invalid(
                        if typecase {
                            "ctypecase clauses must be lists"
                        } else {
                            "ccase clauses must be lists"
                        },
                        clause.span,
                    ));
                };
                if parts.is_empty() {
                    return Err(self.invalid(
                        if typecase {
                            "ctypecase clause cannot be empty"
                        } else {
                            "ccase clause cannot be empty"
                        },
                        clause.span,
                    ));
                }
                if is_case_default_form(&parts[0]) {
                    default_body = Some(&parts[1..]);
                    continue;
                }

                let is_match = if typecase {
                    let type_designator = quoted_form_value(&parts[0])?;
                    builtins::typep_value(&key, &type_designator)?
                } else {
                    let keys = match &parts[0].kind {
                        FormKind::List(keys) => keys.as_slice(),
                        _ => std::slice::from_ref(&parts[0]),
                    };
                    keys.iter().try_fold(false, |matched, key_form| {
                        Ok::<bool, RuntimeError>(
                            matched || builtins::eql_value(&key, &quoted_form_value(key_form)?),
                        )
                    })?
                };
                if is_match {
                    matched = Some(&parts[1..]);
                    break;
                }
            }

            if let Some(body) = matched.or(default_body) {
                return self.eval_sequence_values(body, &local);
            }

            let error = Self::signaled_error(
                "CASE-FAILURE",
                vec![
                    "CASE-FAILURE".to_owned(),
                    "TYPE-ERROR".to_owned(),
                    "ERROR".to_owned(),
                    "SERIOUS-CONDITION".to_owned(),
                    "CONDITION".to_owned(),
                ],
                format!("{operator} fell through"),
                None,
                &[],
                false,
                items[0].span,
            );
            let condition = Value::condition(&error);
            let guard =
                self.restart_guard(vec![RestartBinding::new("STORE-VALUE".to_owned(), None)]);
            let handled = self.dispatch_condition(error.clone(), &condition, &local, items[0].span);
            drop(guard);
            match handled {
                Ok(()) => return Err(error),
                Err(RuntimeError::InvokeRestart {
                    name, arguments, ..
                }) if normalize_name(name.as_str()) == "STORE-VALUE" => {
                    if arguments.len() != 1 {
                        let actual = arguments.len();
                        return Err(self.arity("store-value", "one", actual));
                    }
                    let value = arguments
                        .into_iter()
                        .next()
                        .expect("STORE-VALUE arity checked")
                        .into_value();
                    self.store_setf_expansion(&expansion, value, &local)?;
                }
                Err(error) => return Err(error),
            }
        }
    }

    fn special_destructuring_bind(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(self.arity(
                "destructuring-bind",
                "two or more",
                items.len().saturating_sub(1),
            ));
        }
        let lambda_list = match &items[1].kind {
            FormKind::List(_) => {
                let lambda_list = self.macro_parameters(&items[1])?;
                if lambda_list.environment.is_some() {
                    return Err(self.invalid(
                        "&environment is only valid in macro lambda lists",
                        items[1].span,
                    ));
                }
                Some(lambda_list)
            }
            _ => None,
        };
        let mut seen = HashSet::new();
        let pattern = lambda_list
            .is_none()
            .then(|| self.macro_pattern(&items[1], &mut seen));
        let pattern = pattern.transpose()?;
        let local = environment.child();
        let _dynamic_guard = self.dynamic_guard();
        let value = self.eval_in(&items[2], environment)?.primary_value();
        if let Some(lambda_list) = &lambda_list {
            if let Some(whole) = &lambda_list.whole {
                self.define_macro_binding_in(whole, value.clone(), &local);
            }
        }
        if let Some(lambda_list) = lambda_list {
            self.bind_destructuring_lambda_list(&lambda_list, value, &local, items[1].span)?;
        } else if let Some(pattern) = pattern {
            self.bind_macro_pattern(&pattern, value, &local, items[1].span)?;
        }
        self.eval_sequence_values(&items[3..], &local)
    }

    fn special_let(
        &self,
        items: &[Form],
        environment: &Environment,
        sequential: bool,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(self.arity(
                if sequential { "let*" } else { "let" },
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        let FormKind::List(bindings) = &items[1].kind else {
            return Err(self.invalid("let bindings must be a list", items[1].span));
        };
        let local = environment.child();
        let _dynamic_guard = self.dynamic_guard();
        let declared_special_names = self.declared_special_names(&items[2..])?;
        let (special_names, exact_special_names) =
            split_special_names(declared_special_names.clone());
        let _special_guard = self.special_declaration_guard(&special_names, &exact_special_names);
        for binding in bindings {
            let FormKind::List(binding_items) = &binding.kind else {
                return Err(self.invalid("let binding must be a list", binding.span));
            };
            if !(binding_items.len() == 1 || binding_items.len() == 2) {
                return Err(
                    self.invalid("let binding needs a name and optional value", binding.span)
                );
            }
            let (name, escaped) =
                self.variable_name_info(&binding_items[0], "let binding name must be a symbol")?;
            let value = binding_items.get(1).map_or(Ok(Value::Nil), |form| {
                self.eval_in(form, if sequential { &local } else { environment })
            })?;
            if declared_special_names.contains(&(name.clone(), escaped)) {
                if escaped {
                    self.define_dynamic_exact(&name, value);
                } else {
                    self.define_dynamic(&name, value);
                }
            } else {
                self.define_variable_in(&name, escaped, value, &local);
            }
        }
        self.eval_sequence_values(&items[2..], &local)
    }

    fn special_flet(
        &self,
        items: &[Form],
        environment: &Environment,
        recursive: bool,
    ) -> Result<Value, RuntimeError> {
        let operator = if recursive { "labels" } else { "flet" };
        if items.len() < 2 {
            return Err(self.arity(operator, "at least one", items.len().saturating_sub(1)));
        }
        let FormKind::List(bindings) = &items[1].kind else {
            return Err(self.invalid("local function bindings must be a list", items[1].span));
        };

        let local = environment.child();
        let captured = if recursive {
            local.clone()
        } else {
            environment.clone()
        };
        let mut names = HashSet::new();
        let mut definitions = Vec::with_capacity(bindings.len());
        for binding in bindings {
            let FormKind::List(parts) = &binding.kind else {
                return Err(self.invalid("local function binding must be a list", binding.span));
            };
            if parts.len() < 3 {
                return Err(self.invalid(
                    "local function needs a name, parameters, and a body",
                    binding.span,
                ));
            }
            let (normalized, escaped) =
                self.variable_name_info(&parts[0], "local function name must be a symbol")?;
            if !names.insert(normalized.clone()) {
                return Err(self.invalid("local function names must be unique", parts[0].span));
            }
            definitions.push((
                normalized,
                escaped,
                self.parameters(&parts[1])?,
                parts[2..].to_vec(),
            ));
        }

        for (name, escaped, lambda_list, body) in definitions {
            let function = Value::closure_with_keywords(
                lambda_list.required,
                lambda_list.required_escaped,
                lambda_list.optional,
                lambda_list.rest,
                lambda_list.rest_escaped,
                lambda_list.keywords,
                lambda_list.has_keyword_section,
                lambda_list.allow_other_keys,
                lambda_list.auxiliary,
                body,
                captured.clone(),
            );
            if escaped {
                local.define_function_exact(name, function);
            } else {
                local.define_function(name, function);
            }
        }
        self.eval_sequence_values(&items[2..], &local)
    }

    fn special_macrolet(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(self.arity("macrolet", "at least one", items.len().saturating_sub(1)));
        }
        let FormKind::List(bindings) = &items[1].kind else {
            return Err(self.invalid("local macro bindings must be a list", items[1].span));
        };

        let local = self.make_macrolet_environment(bindings, environment)?;
        self.eval_sequence_values(&items[2..], &local)
    }

    fn special_macrolet_environment(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 2 {
            return Err(self.arity(
                "ncl-macro-environment",
                "one",
                items.len().saturating_sub(1),
            ));
        }
        let FormKind::List(bindings) = &items[1].kind else {
            return Err(self.invalid("local macro bindings must be a list", items[1].span));
        };
        Ok(Value::environment(
            self.make_macrolet_environment(bindings, environment)?,
        ))
    }

    fn special_symbol_macrolet(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(self.arity(
                "symbol-macrolet",
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        let FormKind::List(bindings) = &items[1].kind else {
            return Err(self.invalid("symbol macro bindings must be a list", items[1].span));
        };

        let local = environment.child();
        let mut names = HashSet::new();
        for binding in bindings {
            let FormKind::List(parts) = &binding.kind else {
                return Err(self.invalid("symbol macro binding must be a list", binding.span));
            };
            if parts.len() != 2 {
                return Err(self.invalid(
                    "symbol macro binding needs a name and an expansion",
                    binding.span,
                ));
            }
            let (name, escaped) =
                self.variable_name_info(&parts[0], "symbol macro name must be a symbol")?;
            if !names.insert((name.clone(), escaped)) {
                return Err(self.invalid("symbol macro names must be unique", parts[0].span));
            }
            if escaped {
                local.define_symbol_macro_exact(name, parts[1].clone());
            } else {
                local.define_symbol_macro(name, parts[1].clone());
            }
        }
        self.eval_sequence_values(&items[2..], &local)
    }

    fn special_define_symbol_macro(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 3 {
            return Err(self.arity("DEFINE-SYMBOL-MACRO", "two", items.len().saturating_sub(1)));
        }
        let (name, escaped) =
            self.variable_name_info(&items[1], "DEFINE-SYMBOL-MACRO name must be a symbol")?;
        if escaped {
            environment.define_symbol_macro_exact(name.clone(), items[2].clone());
        } else {
            environment.define_symbol_macro(name.clone(), items[2].clone());
        }
        Ok(if escaped {
            Value::symbol_exact(name)
        } else {
            Value::symbol(name)
        })
    }

    fn special_dotimes(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(self.arity("dotimes", "at least one", items.len().saturating_sub(1)));
        }
        let FormKind::List(binding) = &items[1].kind else {
            return Err(self.invalid("dotimes binding must be a list", items[1].span));
        };
        if !(binding.len() == 2 || binding.len() == 3) {
            return Err(self.invalid(
                "dotimes binding needs a name, count, and optional result",
                items[1].span,
            ));
        }
        let (name, escaped) =
            self.variable_name_info(&binding[0], "dotimes binding name must be a symbol")?;
        let count_form = &binding[1];
        let count = match self.eval_in(count_form, environment)? {
            Value::Integer(count) => count,
            value => {
                return Err(RuntimeError::Type {
                    expected: "INTEGER".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(count_form.span),
                });
            }
        };

        let local = environment.child();
        let _dynamic_guard = self.dynamic_guard();
        self.define_variable_in(&name, escaped, Value::Integer(0), &local);
        let mut index = 0;
        while index < count {
            self.eval_sequence_values(&items[2..], &local)?;
            index += 1;
            self.set_variable_in(&name, escaped, Value::Integer(index), &local);
        }
        binding
            .get(2)
            .map_or(Ok(Value::Nil), |result| self.eval_values_in(result, &local))
    }

    fn special_dolist(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(self.arity("dolist", "at least one", items.len().saturating_sub(1)));
        }
        let FormKind::List(binding) = &items[1].kind else {
            return Err(self.invalid("dolist binding must be a list", items[1].span));
        };
        if !(binding.len() == 2 || binding.len() == 3) {
            return Err(self.invalid(
                "dolist binding needs a name, list, and optional result",
                items[1].span,
            ));
        }
        let (name, escaped) =
            self.variable_name_info(&binding[0], "dolist binding name must be a symbol")?;
        let list_form = &binding[1];
        let list = self.eval_in(list_form, environment)?;
        let Some(elements) = list.list_items() else {
            return Err(RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: list.type_name().to_string(),
                span: Some(list_form.span),
            });
        };

        let local = environment.child();
        let _dynamic_guard = self.dynamic_guard();
        self.define_variable_in(&name, escaped, Value::Nil, &local);
        for element in elements {
            self.set_variable_in(&name, escaped, element, &local);
            self.eval_sequence_values(&items[2..], &local)?;
        }
        self.set_variable_in(&name, escaped, Value::Nil, &local);
        binding
            .get(2)
            .map_or(Ok(Value::Nil), |result| self.eval_values_in(result, &local))
    }

    fn special_do(
        &self,
        items: &[Form],
        environment: &Environment,
        sequential: bool,
    ) -> Result<Value, RuntimeError> {
        let operator = if sequential { "do*" } else { "do" };
        if items.len() < 3 {
            return Err(self.arity(operator, "at least two", items.len().saturating_sub(1)));
        }
        let FormKind::List(binding_forms) = &items[1].kind else {
            return Err(self.invalid("do bindings must be a list", items[1].span));
        };
        let FormKind::List(termination) = &items[2].kind else {
            return Err(self.invalid("do termination must be a list", items[2].span));
        };
        if termination.is_empty() {
            return Err(self.invalid("do termination needs an end test", items[2].span));
        }

        let mut names = HashSet::new();
        let mut bindings = Vec::with_capacity(binding_forms.len());
        for binding in binding_forms {
            let FormKind::List(parts) = &binding.kind else {
                return Err(self.invalid("do binding must be a list", binding.span));
            };
            if !(1..=3).contains(&parts.len()) {
                return Err(self.invalid(
                    "do binding needs a name, optional init, and optional step",
                    binding.span,
                ));
            }
            let (name, escaped) =
                self.variable_name_info(&parts[0], "do binding name must be a symbol")?;
            let key = if escaped {
                format!("\0{name}")
            } else {
                name.clone()
            };
            if !names.insert(key) {
                return Err(self.invalid("do binding names must be unique", parts[0].span));
            }
            bindings.push((name, escaped, parts.get(1).cloned(), parts.get(2).cloned()));
        }

        let target = self.fresh_block_target();
        let block_environment = environment.child();
        block_environment.define_block("NIL", target);
        let local = block_environment.child();
        let _dynamic_guard = self.dynamic_guard();

        let initialization = (|| -> Result<(), RuntimeError> {
            if sequential {
                for (name, escaped, init, _) in &bindings {
                    let value = init
                        .as_ref()
                        .map_or(Ok(Value::Nil), |form| self.eval_in(form, &local))?;
                    self.define_variable_in(name, *escaped, value, &local);
                }
            } else {
                let mut values = Vec::with_capacity(bindings.len());
                for (_, _, init, _) in &bindings {
                    values.push(init.as_ref().map_or(Ok(Value::Nil), |form| {
                        self.eval_in(form, &block_environment)
                    })?);
                }
                for ((name, escaped, _, _), value) in bindings.iter().zip(values) {
                    self.define_variable_in(name, *escaped, value, &local);
                }
            }
            Ok(())
        })();
        match initialization {
            Ok(()) => {}
            Err(RuntimeError::ReturnFrom {
                target: Some(return_target),
                value,
                ..
            }) if return_target == target => return Ok(value.into_value()),
            Err(error) => return Err(error),
        }

        loop {
            let iteration = (|| -> Result<Option<Value>, RuntimeError> {
                let test = self.eval_in(&termination[0], &local)?;
                if test.is_truthy() {
                    return Ok(Some(self.eval_sequence_values(&termination[1..], &local)?));
                }

                self.eval_tagbody_forms(&items[3..], &local)?;
                if sequential {
                    for (name, escaped, _, step) in &bindings {
                        if let Some(step) = step {
                            let value = self.eval_in(step, &local)?;
                            self.set_variable_in(name, *escaped, value, &local);
                        }
                    }
                } else {
                    let mut values = Vec::with_capacity(bindings.len());
                    for (_, _, _, step) in &bindings {
                        values.push(match step {
                            Some(step) => Some(self.eval_in(step, &local)?),
                            None => None,
                        });
                    }
                    for ((name, escaped, _, _), value) in bindings.iter().zip(values) {
                        if let Some(value) = value {
                            self.set_variable_in(name, *escaped, value, &local);
                        }
                    }
                }
                Ok(None)
            })();

            match iteration {
                Ok(Some(value)) => return Ok(value),
                Ok(None) => {}
                Err(RuntimeError::ReturnFrom {
                    target: Some(return_target),
                    value,
                    ..
                }) if return_target == target => return Ok(value.into_value()),
                Err(error) => return Err(error),
            }
        }
    }

    fn special_lambda(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(self.invalid(
                "lambda needs parameters and a body",
                items
                    .first()
                    .map(|item| item.span)
                    .unwrap_or(Span::new(0, 0)),
            ));
        }
        let lambda_list = self.parameters(&items[1])?;
        let (documentation, body) = split_documentation_body(&items[2..]);
        Ok(Value::closure_with_keywords_and_documentation(
            lambda_list.required,
            lambda_list.required_escaped,
            lambda_list.optional,
            lambda_list.rest,
            lambda_list.rest_escaped,
            lambda_list.keywords,
            lambda_list.has_keyword_section,
            lambda_list.allow_other_keys,
            lambda_list.auxiliary,
            body.to_vec(),
            environment.clone(),
            documentation,
        ))
    }

    fn special_function(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 2 {
            return Err(self.arity("function", "one", items.len().saturating_sub(1)));
        }
        if let FormKind::List(function_name) = &items[1].kind {
            match function_name
                .first()
                .and_then(function_operator_name)
                .as_deref()
            {
                Some("SETF") => {
                    if function_name.len() != 2 {
                        return Err(
                            self.invalid("FUNCTION SETF designator needs a symbol", items[1].span)
                        );
                    }
                    let Some(name) = function_name.get(1).and_then(atom_name) else {
                        return Err(self.invalid(
                            "SETF function name must be a symbol",
                            function_name[1].span,
                        ));
                    };
                    if !is_valid_function_symbol_name(name) {
                        return Err(self.invalid(
                            "SETF function name must be a symbol",
                            function_name[1].span,
                        ));
                    }
                    let (resolved_name, _) = resolved_symbol(name);
                    let lookup_name = unqualified_name(&resolved_name);
                    return environment
                        .lookup_setf_function(&lookup_name)
                        .ok_or_else(|| RuntimeError::UnboundVariable {
                            name: format!("(SETF {lookup_name})"),
                            span: Some(function_name[1].span),
                        });
                }
                Some("LAMBDA") => return self.eval_in(&items[1], environment),
                _ => {
                    return Err(self.invalid(
                        "FUNCTION argument must be a symbol, LAMBDA form, or (SETF symbol)",
                        items[1].span,
                    ));
                }
            }
        }
        if let Some(name) = atom_name(&items[1]) {
            if !is_valid_function_symbol_name(name) {
                return Err(self.invalid(
                    "FUNCTION argument must be a symbol, LAMBDA form, or (SETF symbol)",
                    items[1].span,
                ));
            }
            let (resolved_name, escaped) = resolved_symbol(name);
            let function = if escaped {
                self.lookup_function_exact_in(&resolved_name, environment)
            } else {
                self.lookup_function_in(&resolved_name, environment)
            };
            return function.ok_or_else(|| RuntimeError::UnboundVariable {
                name: if escaped {
                    resolved_name
                } else {
                    normalize_name(&resolved_name)
                },
                span: Some(items[1].span),
            });
        }
        Err(self.invalid(
            "FUNCTION argument must be a symbol, LAMBDA form, or (SETF symbol)",
            items[1].span,
        ))
    }

    fn special_defun(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 4 {
            return Err(self.invalid("defun needs a name, parameters, and a body", items[0].span));
        }
        let Some(name) = atom_name(&items[1]) else {
            return Err(self.invalid("defun name must be a symbol", items[1].span));
        };
        let lambda_list = self.parameters(&items[2])?;
        let (documentation, body) = split_documentation_body(&items[3..]);
        let function = Value::closure_with_keywords_and_documentation(
            lambda_list.required,
            lambda_list.required_escaped,
            lambda_list.optional,
            lambda_list.rest,
            lambda_list.rest_escaped,
            lambda_list.keywords,
            lambda_list.has_keyword_section,
            lambda_list.allow_other_keys,
            lambda_list.auxiliary,
            body.to_vec(),
            environment.clone(),
            documentation,
        );
        let (resolved_name, escaped) = resolved_symbol(name);
        if escaped {
            environment.define_function_exact(&resolved_name, function);
        } else {
            environment.define_function(&resolved_name, function);
        }
        Ok(if escaped {
            Value::symbol_exact(resolved_name)
        } else {
            Value::symbol(resolved_name)
        })
    }

    fn special_defsetf(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        let Some(accessor) = atom_name(&items[1]) else {
            return Err(self.invalid("DEFSETF accessor must be a symbol", items[1].span));
        };

        if items.len() != 3 {
            if items.len() < 5 {
                return Err(self.invalid(
                    "DEFSETF needs an accessor, parameters, stores, and a body",
                    items[0].span,
                ));
            }
            let lambda_list = self.macro_parameters(&items[2])?;
            let FormKind::List(store_forms) = &items[3].kind else {
                return Err(self.invalid("DEFSETF store variables must be a list", items[3].span));
            };
            if store_forms.is_empty() {
                return Err(
                    self.invalid("DEFSETF needs at least one store variable", items[3].span)
                );
            }
            let stores = store_forms
                .iter()
                .map(|form| {
                    let (name, escaped) =
                        self.variable_name_info(form, "DEFSETF store variable must be a symbol")?;
                    Ok(MacroBinding { name, escaped })
                })
                .collect::<Result<Vec<_>, RuntimeError>>()?;
            let (resolved_name, escaped) = resolved_symbol(accessor);
            environment.define_defsetf(
                unqualified_name(&resolved_name),
                DefsetfDefinition {
                    lambda_list,
                    stores,
                    body: items[4..].to_vec(),
                    environment: environment.clone(),
                },
            );
            return Ok(if escaped {
                Value::symbol_exact(resolved_name)
            } else {
                Value::symbol(resolved_name)
            });
        }

        let writer_designator = if let Some(writer) = atom_name(&items[2]) {
            let (resolved_name, escaped) = resolved_symbol(writer);
            if escaped {
                Value::symbol_exact(resolved_name)
            } else {
                Value::symbol(resolved_name)
            }
        } else {
            self.eval_in(&items[2], environment)?
        };
        let writer = Value::Function(self.resolve_function_designator(
            &writer_designator,
            items[2].span,
            environment,
        )?);
        let (resolved_name, escaped) = resolved_symbol(accessor);
        environment.define_setf_function(unqualified_name(&resolved_name), writer);
        Ok(if escaped {
            Value::symbol_exact(resolved_name)
        } else {
            Value::symbol(resolved_name)
        })
    }

    fn special_define_setf_expander(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 4 {
            return Err(self.invalid(
                "DEFINE-SETF-EXPANDER needs a name, parameters, and a body",
                items[0].span,
            ));
        }
        let Some(name) = atom_name(&items[1]) else {
            return Err(self.invalid("DEFINE-SETF-EXPANDER name must be a symbol", items[1].span));
        };
        let lambda_list = self.macro_parameters(&items[2])?;
        let function = Value::macro_function(lambda_list, items[3..].to_vec(), environment.clone());
        let (resolved_name, escaped) = resolved_symbol(name);
        environment.define_setf_expander(unqualified_name(&resolved_name), function);
        Ok(if escaped {
            Value::symbol_exact(resolved_name)
        } else {
            Value::symbol(resolved_name)
        })
    }

    fn special_get_setf_expansion(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if !(2..=3).contains(&items.len()) {
            return Err(self.arity(
                "GET-SETF-EXPANSION",
                "one or two",
                items.len().saturating_sub(1),
            ));
        }
        let place_value = self.eval_in(&items[1], environment)?;
        let place = self.form_from_value(&place_value, items[1].span)?;
        let expansion_environment = if items.len() == 3 {
            let value = self.eval_in(&items[2], environment)?;
            self.macroexpansion_environment(value, items[2].span)?
        } else {
            environment.clone()
        };
        let expansion = self.get_setf_expansion(&place, &expansion_environment)?;
        self.setf_expansion_value(&expansion, items[0].span)
    }

    fn special_defmacro(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 4 {
            return Err(self.invalid(
                "defmacro needs a name, parameters, and a body",
                items[0].span,
            ));
        }
        let Some(name) = atom_name(&items[1]) else {
            return Err(self.invalid("defmacro name must be a symbol", items[1].span));
        };
        let lambda_list = self.macro_parameters(&items[2])?;
        let function = Value::macro_function(lambda_list, items[3..].to_vec(), environment.clone());
        let (resolved_name, escaped) = resolved_symbol(name);
        if escaped {
            environment.define_function_exact(&resolved_name, function);
        } else {
            environment.define_function(&resolved_name, function);
        }
        Ok(if escaped {
            Value::symbol_exact(resolved_name)
        } else {
            Value::symbol(resolved_name)
        })
    }

    fn special_define_modify_macro(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 4 {
            return Err(self.invalid(
                "define-modify-macro needs a name, parameters, and a function",
                items[0].span,
            ));
        }
        let Some(name) = atom_name(&items[1]) else {
            return Err(self.invalid("define-modify-macro name must be a symbol", items[1].span));
        };
        let mut lambda_list = self.macro_parameters(&items[2])?;
        lambda_list.required.insert(
            0,
            MacroPattern::Name(MacroBinding {
                name: "NCL-MODIFY-MACRO-PLACE".to_owned(),
                escaped: false,
            }),
        );
        let function =
            Value::modify_macro_function(lambda_list, items[3].clone(), environment.clone());
        let (resolved_name, escaped) = resolved_symbol(name);
        if escaped {
            environment.define_function_exact(&resolved_name, function);
        } else {
            environment.define_function(&resolved_name, function);
        }
        Ok(if escaped {
            Value::symbol_exact(resolved_name)
        } else {
            Value::symbol(resolved_name)
        })
    }

    fn special_macroexpand_1(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if !(2..=3).contains(&items.len()) {
            return Err(self.arity("macroexpand-1", "one or two", items.len().saturating_sub(1)));
        }
        let value = self.eval_in(&items[1], environment)?;
        let form = self.form_from_value(&value, items[1].span)?;
        let expansion_environment = if items.len() == 3 {
            let value = self.eval_in(&items[2], environment)?;
            self.macroexpansion_environment(value, items[2].span)?
        } else {
            environment.clone()
        };
        let (expanded, expanded_p) = match self.expand_macro_once(&form, &expansion_environment)? {
            Some(expanded) => (expanded, true),
            None => (form, false),
        };
        Ok(Value::values(vec![
            self.quoted_value(&expanded)?,
            Value::boolean(expanded_p),
        ]))
    }

    fn special_macroexpand(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if !(2..=3).contains(&items.len()) {
            return Err(self.arity("macroexpand", "one or two", items.len().saturating_sub(1)));
        }
        let value = self.eval_in(&items[1], environment)?;
        let form = self.form_from_value(&value, items[1].span)?;
        let expansion_environment = if items.len() == 3 {
            let value = self.eval_in(&items[2], environment)?;
            self.macroexpansion_environment(value, items[2].span)?
        } else {
            environment.clone()
        };
        let (expanded, expanded_p) = self.expand_macros_with_flag(form, &expansion_environment)?;
        Ok(Value::values(vec![
            self.quoted_value(&expanded)?,
            Value::boolean(expanded_p),
        ]))
    }

    fn macroexpansion_environment(
        &self,
        value: Value,
        span: Span,
    ) -> Result<Environment, RuntimeError> {
        match value {
            Value::Nil | Value::Boolean(false) => Ok(self.global.clone()),
            Value::Environment(environment) => Ok(environment),
            _ => Err(self.invalid("macro expansion environment must be an environment", span)),
        }
    }

    fn special_define(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 3 {
            return Err(self.arity("define", "two", items.len().saturating_sub(1)));
        }
        let (name, escaped) = self.variable_name_info(&items[1], "define name must be a symbol")?;
        let value = self.eval_in(&items[2], environment)?;
        self.define_variable_in(&name, escaped, value.clone(), environment);
        Ok(value)
    }

    fn special_setq(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 || items.len() % 2 == 0 {
            return Err(self.invalid("setq needs variable/value pairs", items[0].span));
        }
        let mut result = Value::Nil;
        for pair in items[1..].chunks_exact(2) {
            let expansion = self.expand_symbol_macro_form(&pair[0], environment)?;
            let (name, escaped) =
                self.variable_name_info(&pair[0], "setq target must be a symbol")?;
            result = self.eval_in(&pair[1], environment)?;
            if let Some(place) = expansion {
                self.set_place(&place, result.clone(), environment)?;
            } else {
                self.set_or_define_variable_in(
                    &name,
                    escaped,
                    result.clone(),
                    environment,
                    pair[0].span,
                )?;
            }
        }
        Ok(result)
    }

    fn special_psetq(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 || items.len() % 2 == 0 {
            return Err(self.invalid("psetq needs variable/value pairs", items[0].span));
        }
        let mut names = Vec::with_capacity((items.len() - 1) / 2);
        for pair in items[1..].chunks_exact(2) {
            let expansion = self.expand_symbol_macro_form(&pair[0], environment)?;
            names.push((
                self.variable_name_info(&pair[0], "psetq target must be a symbol")?,
                expansion,
            ));
        }
        let values = items[1..]
            .chunks_exact(2)
            .map(|pair| {
                self.eval_values_in(&pair[1], environment)
                    .map(|value| value.primary_value())
            })
            .collect::<Result<Vec<_>, _>>()?;
        for (((name, escaped), expansion), value) in names.iter().zip(values) {
            if let Some(place) = expansion {
                self.set_place(place, value, environment)?;
            } else {
                self.set_or_define_variable_in(name, *escaped, value, environment, items[0].span)?;
            }
        }
        Ok(Value::Nil)
    }

    fn special_multiple_value_setq(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 3 {
            return Err(self.arity("multiple-value-setq", "two", items.len().saturating_sub(1)));
        }
        let FormKind::List(variable_forms) = &items[1].kind else {
            return Err(self.invalid(
                "multiple-value-setq variables must be a list",
                items[1].span,
            ));
        };
        let names = variable_forms
            .iter()
            .map(|form| {
                Ok::<_, RuntimeError>((
                    self.variable_name_info(form, "multiple-value-setq variable must be a symbol")?,
                    self.expand_symbol_macro_form(form, environment)?,
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let source = self.eval_values_in(&items[2], environment)?;
        let values = source.multiple_values();
        for (index, ((name, escaped), expansion)) in names.iter().enumerate() {
            let value = values.get(index).cloned().unwrap_or(Value::Nil);
            if let Some(place) = expansion {
                self.set_place(place, value, environment)?;
            } else {
                self.set_or_define_variable_in(name, *escaped, value, environment, items[0].span)?;
            }
        }
        Ok(source.primary_value())
    }

    fn special_setf(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 || items.len() % 2 == 0 {
            return Err(self.invalid("setf needs place/value pairs", items[0].span));
        }
        let mut result = Value::Nil;
        for pair in items[1..].chunks_exact(2) {
            let value = self.eval_values_in(&pair[1], environment)?;
            self.set_place(&pair[0], value.clone(), environment)?;
            result = value.primary_value();
        }
        Ok(result)
    }

    fn special_psetf(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 || items.len() % 2 == 0 {
            return Err(self.invalid("psetf needs place/value pairs", items[0].span));
        }

        let pairs = items[1..].chunks_exact(2).collect::<Vec<_>>();
        let expansions = pairs
            .iter()
            .map(|pair| self.get_modify_macro_setf_expansion(&pair[0], environment))
            .collect::<Result<Vec<_>, _>>()?;
        let mut assignments = Vec::with_capacity(expansions.len());
        for (pair, expansion) in pairs.iter().zip(expansions) {
            if expansion.temporaries.len() != expansion.values.len() {
                return Err(self.invalid(
                    "SETF expansion temporary and value lists must have the same length",
                    pair[0].span,
                ));
            }
            let local = environment.child();
            for (temporary, value_form) in expansion.temporaries.iter().zip(&expansion.values) {
                let (name, escaped) =
                    self.variable_name_info(temporary, "SETF temporary must be a symbol")?;
                let value = self.eval_in(value_form, &local)?;
                self.define_variable_in(&name, escaped, value, &local);
            }
            let value = self.eval_values_in(&pair[1], &local)?;
            self.bind_setf_stores(&expansion, value, &local)?;
            assignments.push((expansion, local));
        }

        for (expansion, local) in assignments {
            if let Some(current_place) = &expansion.current_place {
                if expansion.stores.len() != 1 {
                    return Err(self.invalid(
                        "SETF expansion with a current place must provide one store variable",
                        current_place.span,
                    ));
                }
                let value = self.eval_in(&expansion.stores[0], &local)?;
                self.set_place(current_place, value, &local)?;
            } else {
                self.eval_in(&expansion.store_form, &local)?;
            }
        }
        Ok(Value::Nil)
    }

    fn special_push(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 3 {
            return Err(self.arity("PUSH", "two", items.len().saturating_sub(1)));
        }

        let value = self.eval_in(&items[1], environment)?;
        let current = self.eval_in(&items[2], environment)?;
        let mut elements = current
            .list_items()
            .ok_or_else(|| self.invalid("PUSH place must contain a proper list", items[2].span))?;
        elements.insert(0, value);
        let result = Value::list(elements);
        self.set_place(&items[2], result.clone(), environment)?;
        Ok(result)
    }

    fn special_pop(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 2 {
            return Err(self.arity("POP", "one", items.len().saturating_sub(1)));
        }

        let current = self.eval_in(&items[1], environment)?;
        let mut elements = current
            .list_items()
            .ok_or_else(|| self.invalid("POP place must contain a proper list", items[1].span))?;
        let popped = if elements.is_empty() {
            Value::Nil
        } else {
            elements.remove(0)
        };
        self.set_place(&items[1], Value::list(elements), environment)?;
        Ok(popped)
    }

    fn special_pushnew(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(self.arity("PUSHNEW", "at least two", items.len().saturating_sub(1)));
        }
        if (items.len() - 3) % 2 != 0 {
            return Err(self.invalid(
                "PUSHNEW keyword arguments must be supplied in pairs",
                items[0].span,
            ));
        }

        let mut test = None;
        let mut test_not = None;
        let mut key = None;
        for pair in items[3..].chunks_exact(2) {
            let Some((keyword_name, _)) = macro_keyword_name(&pair[0]) else {
                return Err(self.invalid(
                    "PUSHNEW keyword argument name must be a keyword",
                    pair[0].span,
                ));
            };
            match keyword_name.as_str() {
                "TEST" => {
                    if test_not.is_some() {
                        return Err(self
                            .invalid("PUSHNEW cannot use both :test and :test-not", pair[0].span));
                    }
                    test = Some(self.eval_in(&pair[1], environment)?);
                }
                "TEST-NOT" => {
                    if test.is_some() {
                        return Err(self
                            .invalid("PUSHNEW cannot use both :test and :test-not", pair[0].span));
                    }
                    test_not = Some(self.eval_in(&pair[1], environment)?);
                }
                "KEY" => {
                    key = Some(self.eval_in(&pair[1], environment)?);
                }
                _ => {
                    return Err(RuntimeError::InvalidForm {
                        message: format!("unknown PUSHNEW keyword :{keyword_name}"),
                        span: Some(pair[0].span),
                    });
                }
            }
        }

        let item = self.eval_in(&items[1], environment)?;
        let current = self.eval_in(&items[2], environment)?;
        let elements = current.list_items().ok_or_else(|| {
            self.invalid("PUSHNEW place must contain a proper list", items[2].span)
        })?;

        let invert_test = test_not.is_some();
        let test_designator = test.or(test_not).unwrap_or_else(|| Value::symbol("EQL"));
        let test_function = Value::Function(self.resolve_function_designator(
            &test_designator,
            items[0].span,
            environment,
        )?);
        let key_function = match key {
            Some(value) if value.is_truthy() => Some(Value::Function(
                self.resolve_function_designator(&value, items[0].span, environment)?,
            )),
            _ => None,
        };
        let item_key = match &key_function {
            Some(key_function) => self
                .apply_in(
                    key_function,
                    std::slice::from_ref(&item),
                    items[0].span,
                    environment,
                )?
                .primary_value(),
            None => item.clone(),
        };

        for candidate in &elements {
            let candidate_key = match &key_function {
                Some(key_function) => self
                    .apply_in(
                        key_function,
                        std::slice::from_ref(candidate),
                        items[0].span,
                        environment,
                    )?
                    .primary_value(),
                None => candidate.clone(),
            };
            let equal = self
                .apply_in(
                    &test_function,
                    &[item_key.clone(), candidate_key],
                    items[0].span,
                    environment,
                )?
                .primary_value()
                .is_truthy();
            if if invert_test { !equal } else { equal } {
                return Ok(current);
            }
        }

        let mut result_elements = elements;
        result_elements.insert(0, item);
        let result = Value::list(result_elements);
        self.set_place(&items[2], result.clone(), environment)?;
        Ok(result)
    }

    fn special_rotatef(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        let places = &items[1..];
        let mut locations = Vec::with_capacity(places.len());
        for place in places {
            let expansion = self.get_modify_macro_setf_expansion(place, environment)?;
            let local = self.initialize_setf_expansion(&expansion, environment, place.span)?;
            let value = self.eval_in(&expansion.access_form, &local)?;
            locations.push((expansion, local, value));
        }
        if locations.len() > 1 {
            let mut rotated = locations
                .iter()
                .skip(1)
                .map(|(_, _, value)| value.clone())
                .collect::<Vec<_>>();
            rotated.push(locations[0].2.clone());
            for ((expansion, local, _), value) in locations.iter().zip(rotated) {
                self.store_modify_macro_setf_expansion(expansion, value, local)?;
            }
        }
        Ok(Value::Nil)
    }

    fn special_shiftf(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(self.arity("SHIFTF", "at least two", items.len().saturating_sub(1)));
        }

        let places = &items[1..items.len() - 1];
        let mut locations = Vec::with_capacity(places.len());
        for place in places {
            let expansion = self.get_modify_macro_setf_expansion(place, environment)?;
            let local = self.initialize_setf_expansion(&expansion, environment, place.span)?;
            let value = self.eval_in(&expansion.access_form, &local)?;
            locations.push((expansion, local, value));
        }
        let new_value = self.eval_in(&items[items.len() - 1], environment)?;
        for (index, (expansion, local, _)) in locations.iter().enumerate() {
            let value = locations
                .get(index + 1)
                .map(|(_, _, value)| value.clone())
                .unwrap_or_else(|| new_value.clone());
            self.store_modify_macro_setf_expansion(expansion, value, local)?;
        }
        Ok(locations
            .into_iter()
            .next()
            .map(|(_, _, value)| value)
            .unwrap_or(Value::Nil))
    }

    fn special_modify_symbol(
        &self,
        items: &[Form],
        environment: &Environment,
        operator: &str,
        arithmetic: &str,
    ) -> Result<Value, RuntimeError> {
        if !(items.len() == 2 || items.len() == 3) {
            return Err(self.arity(operator, "one or two", items.len().saturating_sub(1)));
        }
        let place = &items[1];
        if atom_name(place).is_some()
            && self.expand_symbol_macro_form(place, environment)?.is_none()
        {
            self.variable_name(place, &format!("{operator} target"))?;
        }
        let current = self.eval_in(place, environment)?;
        let delta = items
            .get(2)
            .map(|form| self.eval_in(form, environment))
            .transpose()?
            .unwrap_or(Value::Integer(1));
        let function = self
            .lookup_function_in(arithmetic, environment)
            .ok_or_else(|| RuntimeError::UnboundVariable {
                name: normalize_name(arithmetic),
                span: Some(items[0].span),
            })?;
        let value = self
            .apply_in(&function, &[current, delta], items[0].span, environment)?
            .primary_value();
        self.set_place(place, value.clone(), environment)?;
        Ok(value)
    }

    fn fresh_setf_temporary(&self, span: Span) -> Form {
        let counter = self.gensym_counter.get();
        self.gensym_counter.set(counter.wrapping_add(1));
        Form::atom(format!("NCL-SETF-TEMP-{counter}"), span)
    }

    fn fresh_hash_table_iterator_state(&self, span: Span) -> Form {
        let counter = self.gensym_counter.get();
        self.gensym_counter.set(counter.wrapping_add(1));
        Form::atom(format!("NCL-HASH-TABLE-ITERATOR-STATE-{counter}"), span)
    }

    fn setf_expansion_forms(
        &self,
        value: &Value,
        label: &str,
        span: Span,
    ) -> Result<Vec<Form>, RuntimeError> {
        let Some(values) = value.list_items() else {
            return Err(self.invalid(
                &format!("SETF expansion {label} must be a proper list"),
                span,
            ));
        };
        values
            .iter()
            .map(|value| self.form_from_value(value, span))
            .collect()
    }

    fn parse_setf_expansion(
        &self,
        value: &Value,
        span: Span,
    ) -> Result<SetfExpansion, RuntimeError> {
        let values = value.multiple_values();
        if values.len() != 5 {
            return Err(self.invalid("SETF expander must return five values", span));
        }
        let temporaries = self.setf_expansion_forms(&values[0], "temporary variables", span)?;
        let value_forms = self.setf_expansion_forms(&values[1], "value forms", span)?;
        if temporaries.len() != value_forms.len() {
            return Err(self.invalid(
                "SETF expansion temporary and value lists must have the same length",
                span,
            ));
        }
        let stores = self.setf_expansion_forms(&values[2], "store variables", span)?;
        if stores.is_empty() {
            return Err(self.invalid(
                "SETF expansion must provide at least one store variable",
                span,
            ));
        }
        Ok(SetfExpansion {
            temporaries,
            values: value_forms,
            stores,
            store_form: self.form_from_value(&values[3], span)?,
            access_form: self.form_from_value(&values[4], span)?,
            current_place: None,
        })
    }

    fn setf_expansion_value(
        &self,
        expansion: &SetfExpansion,
        _span: Span,
    ) -> Result<Value, RuntimeError> {
        let list_value = |forms: &[Form]| {
            forms
                .iter()
                .map(|form| self.quoted_value(form))
                .collect::<Result<Vec<_>, _>>()
                .map(Value::list)
        };
        Ok(Value::values(vec![
            list_value(&expansion.temporaries)?,
            list_value(&expansion.values)?,
            list_value(&expansion.stores)?,
            self.quoted_value(&expansion.store_form)?,
            self.quoted_value(&expansion.access_form)?,
        ]))
    }

    fn custom_setf_expansion(
        &self,
        place: &Form,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Option<SetfExpansion>, RuntimeError> {
        let Some(operator) = items.first().and_then(atom_name) else {
            return Ok(None);
        };
        let lookup_name = unqualified_name(operator);
        if let Some(definition) = environment.lookup_defsetf(&lookup_name) {
            return Ok(Some(self.defsetf_setf_expansion(
                place,
                items,
                operator,
                &definition,
                environment,
            )?));
        }
        let Some(function) = environment.lookup_setf_expander(&lookup_name) else {
            return Ok(None);
        };
        let Value::Function(function) = function else {
            return Err(self.invalid("SETF expander is not a function", place.span));
        };
        let crate::Function::Macro {
            lambda_list,
            body,
            environment: macro_environment,
        } = function.as_ref()
        else {
            return Err(self.invalid("SETF expander is not a macro function", place.span));
        };
        let expansion = self.invoke_macro(
            place,
            &items[1..],
            operator,
            lambda_list,
            body,
            macro_environment,
            environment,
        )?;
        Ok(Some(self.parse_setf_expansion(&expansion, place.span)?))
    }

    fn defsetf_setf_expansion(
        &self,
        place: &Form,
        items: &[Form],
        operator: &str,
        definition: &DefsetfDefinition,
        environment: &Environment,
    ) -> Result<SetfExpansion, RuntimeError> {
        let temporaries = items[1..]
            .iter()
            .map(|form| self.fresh_setf_temporary(form.span))
            .collect::<Vec<_>>();
        let local = self.bind_macro_arguments(
            place,
            &temporaries,
            operator,
            &definition.lambda_list,
            &definition.environment,
            environment,
        )?;
        if definition.stores.is_empty() {
            return Err(self.invalid("DEFSETF needs a store variable", place.span));
        }
        let stores = definition
            .stores
            .iter()
            .map(|_| self.fresh_setf_temporary(place.span))
            .collect::<Vec<_>>();
        for (store_binding, store) in definition.stores.iter().zip(&stores) {
            self.define_macro_binding_in(store_binding, self.quoted_value(store)?, &local);
        }
        let writer_value = self.eval_sequence_values(&definition.body, &local)?;
        let store_form = self.form_from_value(&writer_value.primary_value(), place.span)?;
        let mut access_items = Vec::with_capacity(items.len());
        access_items.push(items[0].clone());
        access_items.extend(temporaries.iter().cloned());
        let access_form = Form::list(access_items, place.span);
        Ok(SetfExpansion {
            temporaries,
            values: items[1..].to_vec(),
            stores,
            store_form,
            access_form,
            current_place: None,
        })
    }

    fn get_setf_expansion(
        &self,
        place: &Form,
        environment: &Environment,
    ) -> Result<SetfExpansion, RuntimeError> {
        if let Some(expanded) = self.expand_symbol_macro_form(place, environment)? {
            return self.get_setf_expansion(&expanded, environment);
        }
        if atom_name(place).is_some() {
            self.variable_name_info(place, "SETF place must be a symbol")?;
            let store = self.fresh_setf_temporary(place.span);
            let store_form = Form::list(
                vec![Form::atom("SETQ", place.span), place.clone(), store.clone()],
                place.span,
            );
            return Ok(SetfExpansion {
                temporaries: Vec::new(),
                values: Vec::new(),
                stores: vec![store],
                store_form,
                access_form: place.clone(),
                current_place: Some(place.clone()),
            });
        }

        let FormKind::List(items) = &place.kind else {
            return Err(self.invalid("unsupported SETF place", place.span));
        };
        let Some(operator) = items.first().and_then(atom_name) else {
            return Err(self.invalid("unsupported SETF place", place.span));
        };
        if let Some(expansion) = self.custom_setf_expansion(place, items, environment)? {
            return Ok(expansion);
        }

        let temporaries = items[1..]
            .iter()
            .map(|_| self.fresh_setf_temporary(place.span))
            .collect::<Vec<_>>();
        let values = items[1..].to_vec();
        let store = self.fresh_setf_temporary(place.span);
        let mut access_items = Vec::with_capacity(items.len());
        access_items.push(items[0].clone());
        access_items.extend(temporaries.iter().cloned());
        let access_form = Form::list(access_items, place.span);
        let store_form = Form::list(
            vec![
                Form::atom("SETF", place.span),
                access_form.clone(),
                store.clone(),
            ],
            place.span,
        );
        let _ = operator;
        Ok(SetfExpansion {
            temporaries,
            values,
            stores: vec![store],
            store_form,
            access_form,
            current_place: None,
        })
    }

    fn get_modify_macro_setf_expansion(
        &self,
        place: &Form,
        environment: &Environment,
    ) -> Result<SetfExpansion, RuntimeError> {
        if let Some(expanded) = self.expand_symbol_macro_form(place, environment)? {
            return self.get_modify_macro_setf_expansion(&expanded, environment);
        }
        if atom_name(place).is_some() {
            return self.get_setf_expansion(place, environment);
        }

        let FormKind::List(items) = &place.kind else {
            return Err(self.invalid("unsupported SETF place", place.span));
        };
        let Some(operator) = items.first().and_then(atom_name) else {
            return Err(self.invalid("unsupported SETF place", place.span));
        };
        if let Some(expansion) = self.custom_setf_expansion(place, items, environment)? {
            return Ok(expansion);
        }
        let Some(container_index) =
            Self::modify_macro_container_index(operator, items.len().saturating_sub(1))
        else {
            return self.get_setf_expansion(place, environment);
        };

        let outer_temporaries = items[1..]
            .iter()
            .map(|_| self.fresh_setf_temporary(place.span))
            .collect::<Vec<_>>();
        let outer_values = items[1..].to_vec();
        let nested =
            self.get_modify_macro_setf_expansion(&outer_values[container_index], environment)?;
        if nested.stores.len() != 1 {
            return Err(self.invalid(
                "nested SETF expansion must provide one store variable",
                place.span,
            ));
        }
        let nested_store = nested.stores[0].clone();

        let mut temporaries = Vec::new();
        let mut values = Vec::new();
        for (index, (temporary, value_form)) in outer_temporaries
            .iter()
            .zip(outer_values.iter())
            .enumerate()
        {
            if index == container_index {
                temporaries.extend(nested.temporaries.iter().cloned());
                values.extend(nested.values.iter().cloned());
                temporaries.push(temporary.clone());
                values.push(nested.access_form.clone());
            } else {
                temporaries.push(temporary.clone());
                values.push(value_form.clone());
            }
        }

        let mut access_items = Vec::with_capacity(items.len());
        access_items.push(items[0].clone());
        access_items.extend(outer_temporaries.iter().cloned());
        let access_form = Form::list(access_items, place.span);
        let store = self.fresh_setf_temporary(place.span);
        let outer_store_form = Form::list(
            vec![
                Form::atom("SETF", place.span),
                access_form.clone(),
                store.clone(),
            ],
            place.span,
        );
        let nested_store_form = Form::list(
            vec![
                Form::atom("LET", place.span),
                Form::list(
                    vec![Form::list(
                        vec![nested_store, outer_temporaries[container_index].clone()],
                        place.span,
                    )],
                    place.span,
                ),
                nested.store_form.clone(),
            ],
            place.span,
        );
        let store_form = Form::list(
            vec![
                Form::atom("PROGN", place.span),
                outer_store_form,
                nested_store_form,
            ],
            place.span,
        );
        let current_place = nested.current_place.as_ref().map(|nested_place| {
            let mut current_items = Vec::with_capacity(items.len());
            current_items.push(items[0].clone());
            for (index, temporary) in outer_temporaries.iter().enumerate() {
                if index == container_index {
                    current_items.push(nested_place.clone());
                } else {
                    current_items.push(temporary.clone());
                }
            }
            Form::list(current_items, place.span)
        });

        Ok(SetfExpansion {
            temporaries,
            values,
            stores: vec![store],
            store_form,
            access_form,
            current_place,
        })
    }

    fn modify_macro_container_index(operator: &str, argument_count: usize) -> Option<usize> {
        if cxr_operations(operator).is_some() {
            return (argument_count > 0).then_some(0);
        }
        let index = match unqualified_name(operator).as_str() {
            "CAR" | "FIRST" | "CDR" | "REST" | "GETF" | "ELT" | "CHAR" | "SCHAR" | "BIT"
            | "SBIT" | "AREF" | "ROW-MAJOR-AREF" | "SVREF" | "SUBSEQ" => 0,
            "NTH" => 1,
            _ => return None,
        };
        (index < argument_count).then_some(index)
    }

    fn initialize_setf_expansion(
        &self,
        expansion: &SetfExpansion,
        environment: &Environment,
        span: Span,
    ) -> Result<Environment, RuntimeError> {
        if expansion.temporaries.len() != expansion.values.len() {
            return Err(self.invalid(
                "SETF expansion temporary and value lists must have the same length",
                span,
            ));
        }
        let local = environment.child();
        for (temporary, value_form) in expansion.temporaries.iter().zip(&expansion.values) {
            let (name, escaped) =
                self.variable_name_info(temporary, "SETF temporary must be a symbol")?;
            let value = self.eval_in(value_form, &local)?;
            self.define_variable_in(&name, escaped, value, &local);
        }
        Ok(local)
    }

    fn store_setf_expansion(
        &self,
        expansion: &SetfExpansion,
        value: Value,
        environment: &Environment,
    ) -> Result<(), RuntimeError> {
        self.bind_setf_stores(expansion, value, environment)?;
        self.eval_in(&expansion.store_form, environment)?;
        Ok(())
    }

    fn store_modify_macro_setf_expansion(
        &self,
        expansion: &SetfExpansion,
        value: Value,
        environment: &Environment,
    ) -> Result<(), RuntimeError> {
        self.bind_setf_stores(expansion, value, environment)?;
        if let Some(current_place) = &expansion.current_place {
            if expansion.stores.len() != 1 {
                return Err(self.invalid(
                    "SETF expansion with a current place must provide one store variable",
                    current_place.span,
                ));
            }
            let value = self.eval_in(&expansion.stores[0], environment)?;
            self.set_place(current_place, value, environment)?;
        } else {
            self.eval_in(&expansion.store_form, environment)?;
        }
        Ok(())
    }

    fn bind_setf_stores(
        &self,
        expansion: &SetfExpansion,
        value: Value,
        environment: &Environment,
    ) -> Result<(), RuntimeError> {
        if expansion.stores.is_empty() {
            return Err(self.invalid(
                "SETF expansion must provide at least one store variable",
                expansion.store_form.span,
            ));
        }
        let values = value.multiple_values();
        for (index, store) in expansion.stores.iter().enumerate() {
            let (name, escaped) =
                self.variable_name_info(store, "SETF store variable must be a symbol")?;
            let value = values.get(index).cloned().unwrap_or(Value::Nil);
            self.define_variable_in(&name, escaped, value, environment);
        }
        Ok(())
    }

    fn apply_setf_expansion(
        &self,
        expansion: &SetfExpansion,
        value: Value,
        environment: &Environment,
        span: Span,
    ) -> Result<(), RuntimeError> {
        let local = self.initialize_setf_expansion(expansion, environment, span)?;
        self.store_setf_expansion(expansion, value, &local)
    }

    fn set_cxr_value(
        &self,
        current: Value,
        operations: &[u8],
        value: Value,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let Some(mut elements) = current.list_items() else {
            return Err(RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: current.type_name().to_string(),
                span: Some(span),
            });
        };
        if elements.is_empty() {
            return Err(self.invalid("cannot SETF CXR of NIL", span));
        }

        let Some((&operation, rest)) = operations.split_first() else {
            return Ok(value);
        };
        match operation {
            b'A' => {
                elements[0] = if rest.is_empty() {
                    value
                } else {
                    self.set_cxr_value(elements[0].clone(), rest, value, span)?
                };
                Ok(Value::list(elements))
            }
            b'D' => {
                let tail = Value::list(elements.iter().skip(1).cloned().collect());
                let replacement = if rest.is_empty() {
                    value.list_items().ok_or_else(|| RuntimeError::Type {
                        expected: "LIST".to_string(),
                        actual: value.type_name().to_string(),
                        span: Some(span),
                    })?
                } else {
                    let updated = self.set_cxr_value(tail, rest, value, span)?;
                    updated.list_items().ok_or_else(|| {
                        self.invalid("SETF CXR reconstruction must produce a list", span)
                    })?
                };
                let mut rebuilt = Vec::with_capacity(replacement.len() + 1);
                rebuilt.push(elements[0].clone());
                rebuilt.extend(replacement);
                Ok(Value::list(rebuilt))
            }
            _ => Err(self.invalid("unsupported CXR place", span)),
        }
    }

    pub(crate) fn set_place(
        &self,
        place: &Form,
        value: Value,
        environment: &Environment,
    ) -> Result<(), RuntimeError> {
        if let Some(expanded) = self.expand_symbol_macro_form(place, environment)? {
            return self.set_place(&expanded, value, environment);
        }
        if atom_name(place).is_some() {
            let (resolved_name, escaped) =
                self.variable_name_info(place, "SETF target must be a symbol")?;
            self.set_or_define_variable_in(
                &resolved_name,
                escaped,
                value.primary_value(),
                environment,
                place.span,
            )?;
            return Ok(());
        }

        let FormKind::List(items) = &place.kind else {
            return Err(self.invalid("unsupported SETF place", place.span));
        };
        let Some(operator) = items.first().and_then(atom_name) else {
            return Err(self.invalid("unsupported SETF place", place.span));
        };
        let args = &items[1..];

        let lookup_name = unqualified_name(operator);
        if environment.lookup_setf_expander(&lookup_name).is_some()
            || environment.lookup_defsetf(&lookup_name).is_some()
        {
            let expansion = self.get_setf_expansion(place, environment)?;
            return self.apply_setf_expansion(&expansion, value, environment, place.span);
        }
        let value = value.primary_value();
        if let Some(Value::Function(function)) = self.lookup_function_in(&lookup_name, environment)
        {
            match function.as_ref() {
                crate::Function::SlotReader {
                    class_name,
                    slot_name,
                } => {
                    if args.len() != 1 {
                        return Err(self.arity("setf slot accessor", "one", args.len()));
                    }
                    let current = self.eval_in(&args[0], environment)?;
                    if !current.instance_is_type(class_name) {
                        return Err(RuntimeError::Type {
                            expected: class_name.clone(),
                            actual: current.type_name().to_string(),
                            span: Some(args[0].span),
                        });
                    }
                    self.validate_instance_slot_value(&current, slot_name, &value, place.span)?;
                    if current.set_instance_slot(class_name, slot_name, value) {
                        return Ok(());
                    }
                    return Err(self.invalid("slot is not defined for this class", place.span));
                }
                crate::Function::StructureAccessor {
                    structure_name,
                    slot_index,
                    read_only,
                    ..
                } => {
                    if args.len() != 1 {
                        return Err(self.arity("setf structure accessor", "one", args.len()));
                    }
                    if *read_only {
                        return Err(
                            self.invalid("cannot SETF a read-only structure slot", place.span)
                        );
                    }
                    let current = self.eval_in(&args[0], environment)?;
                    if current.set_structure_slot(structure_name, *slot_index, value) {
                        return Ok(());
                    }
                    return Err(RuntimeError::Type {
                        expected: structure_name.clone(),
                        actual: current.type_name().to_string(),
                        span: Some(args[0].span),
                    });
                }
                _ => {}
            }
        }

        if let Some(updater) = environment.lookup_setf_function(&lookup_name) {
            let mut arguments = args
                .iter()
                .map(|argument| self.eval_in(argument, environment))
                .collect::<Result<Vec<_>, _>>()?;
            arguments.push(value);
            self.apply_in(&updater, &arguments, place.span, environment)?;
            return Ok(());
        }

        match lookup_name.as_str() {
            "SLOT-VALUE" => {
                if args.len() != 2 {
                    return Err(self.arity("setf slot-value", "two", args.len()));
                }
                let current = self.eval_in(&args[0], environment)?;
                let slot = self.eval_in(&args[1], environment)?;
                let slot_name = self.slot_name_from_value(&slot, place.span)?;
                let Some(class) = current.instance_class_definition() else {
                    return Err(RuntimeError::Type {
                        expected: "STANDARD-OBJECT".to_string(),
                        actual: current.type_name().to_string(),
                        span: Some(args[0].span),
                    });
                };
                self.validate_instance_slot_value(&current, &slot_name, &value, place.span)?;
                if current.set_instance_slot(&class.name, &slot_name, value) {
                    Ok(())
                } else {
                    Err(self.invalid("slot is not defined for this class", place.span))
                }
            }
            "CAR" | "FIRST" => {
                if args.len() != 1 {
                    return Err(self.arity("setf car", "one", args.len()));
                }
                let current = self.eval_in(&args[0], environment)?;
                let Some(mut elements) = current.list_items() else {
                    return Err(RuntimeError::Type {
                        expected: "LIST".to_string(),
                        actual: current.type_name().to_string(),
                        span: Some(args[0].span),
                    });
                };
                if elements.is_empty() {
                    return Err(self.invalid("cannot SETF CAR of NIL", args[0].span));
                }
                if current.is_typed_list() && current.set_sequence_item(0, value.clone()) {
                    return Ok(());
                }
                elements[0] = value;
                self.set_place(&args[0], Value::list(elements), environment)
            }
            "CDR" | "REST" => {
                if args.len() != 1 {
                    return Err(self.arity("setf cdr", "one", args.len()));
                }
                let current = self.eval_in(&args[0], environment)?;
                let Some(elements) = current.list_items() else {
                    return Err(RuntimeError::Type {
                        expected: "LIST".to_string(),
                        actual: current.type_name().to_string(),
                        span: Some(args[0].span),
                    });
                };
                if elements.is_empty() {
                    return Err(self.invalid("cannot SETF CDR of NIL", args[0].span));
                }
                let Some(mut replacement) = value.list_items() else {
                    return Err(RuntimeError::Type {
                        expected: "LIST".to_string(),
                        actual: value.type_name().to_string(),
                        span: Some(place.span),
                    });
                };
                if current.is_typed_list() && current.set_typed_list_cdr(&replacement) {
                    return Ok(());
                }
                let mut rebuilt = Vec::with_capacity(elements.len() + replacement.len());
                rebuilt.push(elements[0].clone());
                rebuilt.append(&mut replacement);
                self.set_place(&args[0], Value::list(rebuilt), environment)
            }
            _operator if cxr_operations(&lookup_name).is_some() => {
                if args.len() != 1 {
                    return Err(self.arity("setf CXR", "one", args.len()));
                }
                if atom_name(&args[0]).is_some() {
                    let current = self.eval_in(&args[0], environment)?;
                    let operations = cxr_operations(&lookup_name)
                        .expect("CXR operation validated by match guard");
                    let rebuilt = self.set_cxr_value(current, &operations, value, args[0].span)?;
                    self.set_place(&args[0], rebuilt, environment)
                } else {
                    let expansion = self.get_modify_macro_setf_expansion(place, environment)?;
                    self.apply_setf_expansion(&expansion, value, environment, place.span)
                }
            }
            "NTH" => {
                if args.len() != 2 {
                    return Err(self.arity("setf nth", "two", args.len()));
                }
                let index = self.setf_index(self.eval_in(&args[0], environment)?, args[0].span)?;
                let current = self.eval_in(&args[1], environment)?;
                if current.is_typed_list() {
                    if !current.set_sequence_item(index, value) {
                        return Err(self.invalid("SETF index is out of bounds", args[0].span));
                    }
                    return Ok(());
                }
                let Some(mut elements) = current.list_items() else {
                    return Err(RuntimeError::Type {
                        expected: "LIST".to_string(),
                        actual: current.type_name().to_string(),
                        span: Some(args[1].span),
                    });
                };
                let Some(slot) = elements.get_mut(index) else {
                    return Err(self.invalid("SETF index is out of bounds", args[0].span));
                };
                *slot = value;
                self.set_place(&args[1], Value::list(elements), environment)
            }
            "FILL-POINTER" => {
                if args.len() != 1 {
                    return Err(self.arity("setf fill-pointer", "one", args.len()));
                }
                let current = self.eval_in(&args[0], environment)?;
                if current.array_fill_pointer().is_none() {
                    return Err(RuntimeError::Type {
                        expected: "an array with a fill pointer".to_string(),
                        actual: current.type_name().to_string(),
                        span: Some(args[0].span),
                    });
                }
                let index = self.setf_index(value, place.span)?;
                if !current.set_array_fill_pointer(index) {
                    return Err(self.invalid(
                        "SETF fill-pointer is out of bounds",
                        place.span,
                    ));
                }
                Ok(())
            }
            "ELT" => {
                if args.len() != 2 {
                    return Err(self.arity("setf elt", "two", args.len()));
                }
                let current = self.eval_in(&args[0], environment)?;
                let index = self.setf_index(self.eval_in(&args[1], environment)?, args[1].span)?;
                match current {
                    Value::Nil | Value::List(_) => {
                        let mut elements = current.list_items().unwrap_or_default();
                        let Some(slot) = elements.get_mut(index) else {
                            return Err(self.invalid("SETF index is out of bounds", args[1].span));
                        };
                        *slot = value;
                        self.set_place(&args[0], Value::list(elements), environment)
                    }
                    Value::Vector(elements) => {
                        let mut elements = elements.borrow_mut();
                        let Some(slot) = elements.get_mut(index) else {
                            return Err(self.invalid("SETF index is out of bounds", args[1].span));
                        };
                        *slot = value;
                        Ok(())
                    }
                    container if container.is_typed_list() || container.is_typed_vector() => {
                        if !container.set_sequence_item(index, value) {
                            return Err(self.invalid("SETF index is out of bounds", args[1].span));
                        }
                        Ok(())
                    }
                    Value::Array {
                        ref dimensions,
                        ref elements,
                        ..
                    } if dimensions.len() == 1 => {
                        if !current.accepts_array_element(&value) {
                            let expected = current
                                .array_element_type()
                                .map(|element_type| element_type.name())
                                .unwrap_or("ARRAY");
                            return Err(RuntimeError::Type {
                                expected: expected.to_string(),
                                actual: value.type_name().to_string(),
                                span: Some(place.span),
                            });
                        }
                        let limit = current
                            .array_fill_pointer()
                            .unwrap_or_else(|| elements.borrow().len());
                        if index >= limit {
                            return Err(self.invalid("SETF index is out of bounds", args[1].span));
                        }
                        let mut elements = elements.borrow_mut();
                        let Some(slot) = elements.get_mut(index) else {
                            return Err(self.invalid("SETF index is out of bounds", args[1].span));
                        };
                        *slot = value;
                        Ok(())
                    }
                    Value::String(text) => {
                        let Value::Character(character) = value else {
                            return Err(RuntimeError::Type {
                                expected: "CHARACTER".to_string(),
                                actual: value.type_name().to_string(),
                                span: Some(place.span),
                            });
                        };
                        let mut characters = text.chars().collect::<Vec<_>>();
                        let Some(slot) = characters.get_mut(index) else {
                            return Err(self.invalid("SETF index is out of bounds", args[1].span));
                        };
                        *slot = character;
                        self.set_place(
                            &args[0],
                            Value::string(characters.into_iter().collect::<String>()),
                            environment,
                        )
                    }
                    other => Err(RuntimeError::Type {
                        expected: "SEQUENCE".to_string(),
                        actual: other.type_name().to_string(),
                        span: Some(args[0].span),
                    }),
                }
            }
            "SUBSEQ" => {
                if !(2..=3).contains(&args.len()) {
                    return Err(self.arity("setf subseq", "two or three", args.len()));
                }
                let current = self.eval_in(&args[0], environment)?;
                let mut destination = match &current {
                    Value::Nil => Vec::new(),
                    Value::List(items) => items.as_ref().clone(),
                    Value::Vector(items) => items.borrow().clone(),
                    value if value.is_typed_list() => {
                        value.list_items().expect("typed list items")
                    }
                    value if value.is_typed_vector() => {
                        value.vector_items().expect("typed vector items")
                    }
                    Value::Array { dimensions, .. } if dimensions.len() == 1 => {
                        current.vector_items().expect("vector items")
                    }
                    Value::String(text) => text.chars().map(Value::Character).collect(),
                    other => {
                        return Err(RuntimeError::Type {
                            expected: "SEQUENCE".to_string(),
                            actual: other.type_name().to_string(),
                            span: Some(args[0].span),
                        });
                    }
                };
                let start = self.setf_index(self.eval_in(&args[1], environment)?, args[1].span)?;
                let end = args
                    .get(2)
                    .map(|form| {
                        self.eval_in(form, environment)
                            .and_then(|value| self.setf_index(value, form.span))
                    })
                    .transpose()?
                    .unwrap_or(destination.len());
                if start > end || end > destination.len() {
                    return Err(self.invalid("SETF SUBSEQ bounds are invalid", place.span));
                }

                let replacement = match &value {
                    Value::Nil => Vec::new(),
                    Value::List(items) => items.as_ref().clone(),
                    Value::Vector(items) => items.borrow().clone(),
                    value if value.is_typed_list() => {
                        value.list_items().expect("typed list items")
                    }
                    value if value.is_typed_vector() => {
                        value.vector_items().expect("typed vector items")
                    }
                    Value::String(text) => text.chars().map(Value::Character).collect(),
                    other => {
                        return Err(RuntimeError::Type {
                            expected: "SEQUENCE".to_string(),
                            actual: other.type_name().to_string(),
                            span: Some(place.span),
                        });
                    }
                };
                let count = (end - start).min(replacement.len());
                if current.is_typed_list() || current.is_typed_vector() {
                    for (offset, item) in replacement.iter().take(count).cloned().enumerate() {
                        if !current.set_sequence_item(start + offset, item) {
                            return Err(self.invalid(
                                "SETF SUBSEQ cannot modify a typed structure discriminator",
                                place.span,
                            ));
                        }
                    }
                    return Ok(());
                }
                match &current {
                    Value::Vector(elements) => {
                        let mut elements = elements.borrow_mut();
                        elements[start..start + count].clone_from_slice(&replacement[..count]);
                        return Ok(());
                    }
                    Value::Array {
                        dimensions,
                        elements,
                        ..
                    } if dimensions.len() == 1 => {
                        if replacement[..count]
                            .iter()
                            .any(|item| !current.accepts_array_element(item))
                        {
                            let expected = current
                                .array_element_type()
                                .map(|element_type| element_type.name())
                                .unwrap_or("ARRAY");
                            return Err(RuntimeError::Type {
                                expected: expected.to_string(),
                                actual: replacement
                                    .iter()
                                    .find(|item| !current.accepts_array_element(item))
                                    .map(Value::type_name)
                                    .unwrap_or("VALUE")
                                    .to_string(),
                                span: Some(place.span),
                            });
                        }
                        let mut elements = elements.borrow_mut();
                        elements[start..start + count].clone_from_slice(&replacement[..count]);
                        return Ok(());
                    }
                    _ => {}
                }
                destination[start..start + count].clone_from_slice(&replacement[..count]);

                let rebuilt = match &current {
                    Value::Nil | Value::List(_) => Value::list(destination),
                    Value::Vector(_) => Value::vector(destination),
                    Value::Array { dimensions, .. } if dimensions.len() == 1 => {
                        let element_type = current.array_element_type().expect("array type");
                        Value::array_with_element_type(
                            dimensions.as_ref().clone(),
                            destination,
                            element_type,
                        )
                    }
                    Value::String(_) => {
                        let mut text = String::new();
                        for item in destination {
                            let Value::Character(character) = item else {
                                return Err(RuntimeError::Type {
                                    expected: "CHARACTER".to_string(),
                                    actual: item.type_name().to_string(),
                                    span: Some(place.span),
                                });
                            };
                            text.push(character);
                        }
                        Value::string(text)
                    }
                    _ => unreachable!("setf subseq type checked above"),
                };
                self.set_place(&args[0], rebuilt, environment)
            }
            "CHAR" | "SCHAR" => {
                if args.len() != 2 {
                    return Err(self.arity("setf char", "two", args.len()));
                }
                let current = self.eval_in(&args[0], environment)?;
                let index = self.setf_index(self.eval_in(&args[1], environment)?, args[1].span)?;
                let Value::String(text) = current else {
                    return Err(RuntimeError::Type {
                        expected: "STRING".to_string(),
                        actual: current.type_name().to_string(),
                        span: Some(args[0].span),
                    });
                };
                let Value::Character(character) = value else {
                    return Err(RuntimeError::Type {
                        expected: "CHARACTER".to_string(),
                        actual: value.type_name().to_string(),
                        span: Some(place.span),
                    });
                };
                let mut characters = text.chars().collect::<Vec<_>>();
                let Some(slot) = characters.get_mut(index) else {
                    return Err(self.invalid("SETF index is out of bounds", args[1].span));
                };
                *slot = character;
                self.set_place(
                    &args[0],
                    Value::string(characters.into_iter().collect::<String>()),
                    environment,
                )
            }
            "SVREF" => {
                if args.len() != 2 {
                    return Err(self.arity("setf svref", "two", args.len()));
                }
                let current = self.eval_in(&args[0], environment)?;
                let index = self.setf_index(self.eval_in(&args[1], environment)?, args[1].span)?;
                match &current {
                    Value::Vector(elements) => {
                        let mut elements = elements.borrow_mut();
                        let Some(slot) = elements.get_mut(index) else {
                            return Err(self.invalid("SETF index is out of bounds", args[1].span));
                        };
                        *slot = value;
                        Ok(())
                    }
                    container if container.is_typed_vector() => {
                        if !container.set_sequence_item(index, value) {
                            return Err(self.invalid("SETF index is out of bounds", args[1].span));
                        }
                        Ok(())
                    }
                    Value::Array {
                        dimensions,
                        elements,
                        element_type: ArrayElementType::T,
                        fill_pointer: None,
                        adjustable: false,
                    } if dimensions.len() == 1 => {
                        let mut elements = elements.borrow_mut();
                        let Some(slot) = elements.get_mut(index) else {
                            return Err(self.invalid("SETF index is out of bounds", args[1].span));
                        };
                        *slot = value;
                        Ok(())
                    }
                    _ => Err(RuntimeError::Type {
                        expected: "SIMPLE-VECTOR".to_string(),
                        actual: current.type_name().to_string(),
                        span: Some(args[0].span),
                    }),
                }
            }
            "AREF" => {
                if args.is_empty() {
                    return Err(self.arity("setf aref", "at least one", args.len()));
                }
                let current = self.eval_in(&args[0], environment)?;
                let indices = args[1..]
                    .iter()
                    .map(|argument| self.eval_in(argument, environment))
                    .collect::<Result<Vec<_>, _>>()?;
                match &current {
                    Value::Vector(elements) => {
                        if indices.len() != 1 {
                            return Err(self.arity("setf aref", "two", args.len()));
                        }
                        let index = self.setf_index(indices[0].clone(), args[1].span)?;
                        let mut elements = elements.borrow_mut();
                        let Some(slot) = elements.get_mut(index) else {
                            return Err(self.invalid("SETF index is out of bounds", args[1].span));
                        };
                        *slot = value;
                        Ok(())
                    }
                    container if container.is_typed_vector() => {
                        if indices.len() != 1 {
                            return Err(self.arity("setf aref", "two", args.len()));
                        }
                        let index = self.setf_index(indices[0].clone(), args[1].span)?;
                        if !container.set_sequence_item(index, value) {
                            return Err(self.invalid("SETF index is out of bounds", args[1].span));
                        }
                        Ok(())
                    }
                    Value::Array { elements, .. } => {
                        let dimensions = current
                            .array_dimensions()
                            .expect("array dimensions are available");
                        if args.len() != dimensions.len() + 1 {
                            return Err(self.arity(
                                "setf aref",
                                &format!("{} indices", dimensions.len()),
                                indices.len(),
                            ));
                        }
                        let mut offset = 0_usize;
                        for (axis, (dimension, index_value)) in
                            dimensions.iter().zip(&indices).enumerate()
                        {
                            let index =
                                self.setf_index(index_value.clone(), args[axis + 1].span)?;
                            if index >= *dimension {
                                return Err(self
                                    .invalid("SETF index is out of bounds", args[axis + 1].span));
                            }
                            let stride = dimensions[axis + 1..]
                                .iter()
                                .try_fold(1_usize, |stride, dimension| {
                                    stride.checked_mul(*dimension)
                                })
                                .ok_or_else(|| {
                                    self.invalid("SETF index is too large", place.span)
                                })?;
                            let contribution = index.checked_mul(stride).ok_or_else(|| {
                                self.invalid("SETF index is too large", place.span)
                            })?;
                            offset = offset.checked_add(contribution).ok_or_else(|| {
                                self.invalid("SETF index is too large", place.span)
                            })?;
                        }
                        if !current.accepts_array_element(&value) {
                            let expected = current
                                .array_element_type()
                                .map(|element_type| element_type.name())
                                .unwrap_or("ARRAY");
                            return Err(RuntimeError::Type {
                                expected: expected.to_string(),
                                actual: value.type_name().to_string(),
                                span: Some(place.span),
                            });
                        }
                        let mut elements = elements.borrow_mut();
                        let Some(slot) = elements.get_mut(offset) else {
                            return Err(self.invalid("SETF index is out of bounds", place.span));
                        };
                        *slot = value;
                        Ok(())
                    }
                    Value::String(text) => {
                        if indices.len() != 1 {
                            return Err(self.arity("setf aref", "two", args.len()));
                        }
                        let index = self.setf_index(indices[0].clone(), args[1].span)?;
                        let Value::Character(character) = value else {
                            return Err(RuntimeError::Type {
                                expected: "CHARACTER".to_string(),
                                actual: value.type_name().to_string(),
                                span: Some(place.span),
                            });
                        };
                        let mut characters = text.chars().collect::<Vec<_>>();
                        let Some(slot) = characters.get_mut(index) else {
                            return Err(self.invalid("SETF index is out of bounds", args[1].span));
                        };
                        *slot = character;
                        self.set_place(
                            &args[0],
                            Value::string(characters.into_iter().collect::<String>()),
                            environment,
                        )
                    }
                    other => Err(RuntimeError::Type {
                        expected: "ARRAY, VECTOR, or STRING".to_string(),
                        actual: other.type_name().to_string(),
                        span: Some(args[0].span),
                    }),
                }
            }
            "ROW-MAJOR-AREF" => {
                if args.len() != 2 {
                    return Err(self.arity("setf row-major-aref", "two", args.len()));
                }
                let current = self.eval_in(&args[0], environment)?;
                let index = self.setf_index(self.eval_in(&args[1], environment)?, args[1].span)?;
                match &current {
                    Value::Vector(elements) => {
                        let mut elements = elements.borrow_mut();
                        let Some(slot) = elements.get_mut(index) else {
                            return Err(self.invalid("SETF index is out of bounds", args[1].span));
                        };
                        *slot = value;
                        Ok(())
                    }
                    container if container.is_typed_vector() => {
                        if !container.set_sequence_item(index, value) {
                            return Err(self.invalid("SETF index is out of bounds", args[1].span));
                        }
                        Ok(())
                    }
                    Value::Array { elements, .. } => {
                        if !current.accepts_array_element(&value) {
                            let expected = current
                                .array_element_type()
                                .map(|element_type| element_type.name())
                                .unwrap_or("ARRAY");
                            return Err(RuntimeError::Type {
                                expected: expected.to_string(),
                                actual: value.type_name().to_string(),
                                span: Some(place.span),
                            });
                        }
                        let mut elements = elements.borrow_mut();
                        let Some(slot) = elements.get_mut(index) else {
                            return Err(self.invalid("SETF index is out of bounds", args[1].span));
                        };
                        *slot = value;
                        Ok(())
                    }
                    Value::String(text) => {
                        let Value::Character(character) = value else {
                            return Err(RuntimeError::Type {
                                expected: "CHARACTER".to_string(),
                                actual: value.type_name().to_string(),
                                span: Some(place.span),
                            });
                        };
                        let mut characters = text.chars().collect::<Vec<_>>();
                        let Some(slot) = characters.get_mut(index) else {
                            return Err(self.invalid("SETF index is out of bounds", args[1].span));
                        };
                        *slot = character;
                        self.set_place(
                            &args[0],
                            Value::string(characters.into_iter().collect::<String>()),
                            environment,
                        )
                    }
                    other => Err(RuntimeError::Type {
                        expected: "ARRAY, VECTOR, or STRING".to_string(),
                        actual: other.type_name().to_string(),
                        span: Some(args[0].span),
                    }),
                }
            }
            "SBIT" => {
                if args.len() != 2 {
                    return Err(self.arity("setf sbit", "two", args.len()));
                }
                let current = self.eval_in(&args[0], environment)?;
                let Value::Array {
                    dimensions,
                    elements,
                    element_type: ArrayElementType::Bit,
                    fill_pointer: None,
                    adjustable: false,
                } = &current
                else {
                    return Err(RuntimeError::Type {
                        expected: "SIMPLE-BIT-VECTOR".to_string(),
                        actual: current.type_name().to_string(),
                        span: Some(args[0].span),
                    });
                };
                if dimensions.len() != 1 {
                    return Err(RuntimeError::Type {
                        expected: "SIMPLE-BIT-VECTOR".to_string(),
                        actual: current.type_name().to_string(),
                        span: Some(args[0].span),
                    });
                }
                let index_value = self.eval_in(&args[1], environment)?;
                let index = self.setf_index(index_value, args[1].span)?;
                if index >= elements.borrow().len() {
                    return Err(self.invalid("SETF index is out of bounds", args[1].span));
                }
                if !matches!(&value, Value::Integer(bit) if *bit == 0 || *bit == 1) {
                    return Err(RuntimeError::Type {
                        expected: "BIT".to_string(),
                        actual: value.type_name().to_string(),
                        span: Some(place.span),
                    });
                }
                elements.borrow_mut()[index] = value;
                Ok(())
            }
            "BIT" => {
                if args.is_empty() {
                    return Err(self.arity("setf bit", "array and subscripts", 0));
                }
                let current = self.eval_in(&args[0], environment)?;
                let Value::Array {
                    elements,
                    element_type: ArrayElementType::Bit,
                    ..
                } = &current
                else {
                    return Err(RuntimeError::Type {
                        expected: "BIT-ARRAY".to_string(),
                        actual: current.type_name().to_string(),
                        span: Some(args[0].span),
                    });
                };
                let dimensions = current
                    .array_dimensions()
                    .expect("array dimensions are available");
                if args.len() != dimensions.len() + 1 {
                    return Err(self.arity(
                        "setf bit",
                        &format!("{} subscripts", dimensions.len()),
                        args.len() - 1,
                    ));
                }
                let indices = args[1..]
                    .iter()
                    .map(|argument| self.eval_in(argument, environment))
                    .collect::<Result<Vec<_>, _>>()?;
                let mut offset = 0_usize;
                for (axis, (dimension, index_value)) in dimensions.iter().zip(&indices).enumerate()
                {
                    let index = self.setf_index(index_value.clone(), args[axis + 1].span)?;
                    if index >= *dimension {
                        return Err(
                            self.invalid("SETF index is out of bounds", args[axis + 1].span)
                        );
                    }
                    let stride = dimensions[axis + 1..]
                        .iter()
                        .try_fold(1_usize, |stride, dimension| stride.checked_mul(*dimension))
                        .ok_or_else(|| self.invalid("SETF index is too large", place.span))?;
                    let contribution = index
                        .checked_mul(stride)
                        .ok_or_else(|| self.invalid("SETF index is too large", place.span))?;
                    offset = offset
                        .checked_add(contribution)
                        .ok_or_else(|| self.invalid("SETF index is too large", place.span))?;
                }
                if !matches!(&value, Value::Integer(bit) if *bit == 0 || *bit == 1) {
                    return Err(RuntimeError::Type {
                        expected: "BIT".to_string(),
                        actual: value.type_name().to_string(),
                        span: Some(place.span),
                    });
                }
                let mut elements = elements.borrow_mut();
                let Some(slot) = elements.get_mut(offset) else {
                    return Err(self.invalid("SETF index is out of bounds", place.span));
                };
                *slot = value;
                Ok(())
            }
            "SYMBOL-VALUE" => {
                if args.len() != 1 {
                    return Err(self.arity("setf symbol-value", "one", args.len()));
                }
                let symbol = self.eval_in(&args[0], environment)?;
                let (name, exact) = symbol.symbol_reference().ok_or_else(|| {
                    self.invalid("setf symbol-value target must be a symbol", args[0].span)
                })?;
                self.ensure_symbol_writable(name, exact, args[0].span)?;
                if exact {
                    self.set_symbol_value_exact(name, value);
                } else {
                    self.set_symbol_value(name, value);
                }
                Ok(())
            }
            "FDEFINITION" | "SYMBOL-FUNCTION" => {
                if args.len() != 1 {
                    return Err(self.arity("setf function definition", "one", args.len()));
                }
                if !matches!(&value, Value::Function(_)) {
                    return Err(RuntimeError::Type {
                        expected: "FUNCTION".to_string(),
                        actual: value.type_name().to_string(),
                        span: Some(place.span),
                    });
                }
                let symbol = self.eval_in(&args[0], environment)?;
                let (name, exact) = symbol.symbol_reference().ok_or_else(|| {
                    self.invalid(
                        "setf function definition target must be a symbol",
                        args[0].span,
                    )
                })?;
                if exact {
                    self.global.define_function_exact(name, value);
                } else {
                    let function_name = self
                        .dynamic_candidates(name)
                        .into_iter()
                        .next()
                        .unwrap_or_else(|| normalize_name(name));
                    self.global.define_function(function_name, value);
                }
                Ok(())
            }
            "GET" => {
                if args.len() != 2 {
                    return Err(self.arity("setf get", "two", args.len()));
                }
                let symbol = self.eval_in(&args[0], environment)?;
                if symbol.symbol_reference().is_none() {
                    return Err(self.invalid("setf get target must be a symbol", args[0].span));
                }
                let indicator = self.eval_in(&args[1], environment)?;
                let plist = environment.symbol_plist(&symbol).unwrap_or(Value::Nil);
                let Some(mut properties) = plist.list_items() else {
                    return Err(RuntimeError::Type {
                        expected: "LIST".to_string(),
                        actual: plist.type_name().to_string(),
                        span: Some(args[0].span),
                    });
                };
                if properties.len() % 2 != 0 {
                    return Err(self.invalid("SETF GET needs an even property list", args[0].span));
                }
                let mut found = None;
                for index in (0..properties.len()).step_by(2) {
                    if properties[index].eq_value(&indicator) {
                        found = Some(index + 1);
                        break;
                    }
                }
                if let Some(index) = found {
                    properties[index] = value;
                } else {
                    properties.push(indicator);
                    properties.push(value);
                }
                environment.set_symbol_plist(&symbol, Value::list(properties));
                Ok(())
            }
            "GETHASH" => {
                if !(2..=3).contains(&args.len()) {
                    return Err(self.arity("setf gethash", "two or three", args.len()));
                }
                let key = self.eval_in(&args[0], environment)?;
                let table = self.eval_in(&args[1], environment)?;
                if let Some(default) = args.get(2) {
                    let _ = self.eval_in(default, environment)?;
                }
                let Some(test) = table.hash_table_test() else {
                    return Err(RuntimeError::Type {
                        expected: "HASH-TABLE".to_string(),
                        actual: table.type_name().to_string(),
                        span: Some(args[1].span),
                    });
                };
                let test = test.to_string();
                let Some(entries) = table.hash_table_entries() else {
                    return Err(RuntimeError::Type {
                        expected: "HASH-TABLE".to_string(),
                        actual: table.type_name().to_string(),
                        span: Some(args[1].span),
                    });
                };
                let mut entries = entries.borrow_mut();
                if let Some((_, slot)) = entries.iter_mut().find(|(stored_key, _)| {
                    crate::builtins::hash_table_key_equal(&test, stored_key, &key)
                }) {
                    *slot = value;
                } else {
                    entries.push((key, value));
                }
                Ok(())
            }
            "GETF" => {
                if !(2..=3).contains(&args.len()) {
                    return Err(self.arity("setf getf", "two or three", args.len()));
                }
                let current = self.eval_in(&args[0], environment)?;
                let indicator = self.eval_in(&args[1], environment)?;
                if let Some(default) = args.get(2) {
                    let _ = self.eval_in(default, environment)?;
                }
                let Some(mut properties) = current.list_items() else {
                    return Err(RuntimeError::Type {
                        expected: "LIST".to_string(),
                        actual: current.type_name().to_string(),
                        span: Some(args[0].span),
                    });
                };
                if properties.len() % 2 != 0 {
                    return Err(self.invalid("GETF needs an even property list", args[0].span));
                }
                let mut found = None;
                for index in (0..properties.len()).step_by(2) {
                    if properties[index].eq_value(&indicator) {
                        found = Some(index + 1);
                        break;
                    }
                }
                if let Some(index) = found {
                    properties[index] = value;
                } else {
                    properties.insert(0, value);
                    properties.insert(0, indicator);
                }
                self.set_place(&args[0], Value::list(properties), environment)
            }
            _ => Err(self.invalid("unsupported SETF place", place.span)),
        }
    }

    pub(crate) fn set_map_into_destination(
        &self,
        destination: &Form,
        value: Value,
        environment: &Environment,
    ) -> Result<(), RuntimeError> {
        if atom_name(destination).is_some() {
            match self.variable_name_info(destination, "SETF target must be a symbol") {
                Ok(_) => return self.set_place(destination, value, environment),
                Err(RuntimeError::InvalidForm { message, .. })
                    if message == "SETF target must be a symbol" =>
                {
                    return Ok(());
                }
                Err(error) => return Err(error),
            }
        }

        if !matches!(destination.kind, FormKind::List(_)) {
            return Ok(());
        }

        match self.set_place(destination, value, environment) {
            Err(RuntimeError::InvalidForm { message, .. })
                if message == "unsupported SETF place" =>
            {
                Ok(())
            }
            result => result,
        }
    }

    fn setf_index(&self, value: Value, span: Span) -> Result<usize, RuntimeError> {
        match value {
            Value::Integer(index) if index >= 0 => {
                usize::try_from(index).map_err(|_| self.invalid("SETF index is too large", span))
            }
            Value::Integer(_) => Err(self.invalid("SETF index must be non-negative", span)),
            other => Err(RuntimeError::Type {
                expected: "INTEGER".to_string(),
                actual: other.type_name().to_string(),
                span: Some(span),
            }),
        }
    }

    fn special_defvar(
        &self,
        items: &[Form],
        environment: &Environment,
        force: bool,
    ) -> Result<Value, RuntimeError> {
        let operator = if force { "defparameter" } else { "defvar" };
        if !(items.len() == 2 || items.len() == 3) {
            return Err(self.arity(operator, "one or two", items.len().saturating_sub(1)));
        }
        let context = if force {
            "defparameter name must be a symbol"
        } else {
            "defvar name must be a symbol"
        };
        let (name, escaped) = self.variable_name_info(&items[1], context)?;
        if force
            && if escaped {
                self.is_constant_exact_in(&name)
            } else {
                self.is_constant_in(&name)
            }
        {
            return Err(self.constant_modification_error(&name, items[1].span));
        }
        if !force {
            let existing = if escaped {
                self.lookup_special_exact(&name)
            } else {
                self.lookup_special(&name)
            };
            if let Some(value) = existing {
                return Ok(value);
            }
        };
        let value = items
            .get(2)
            .map_or(Ok(Value::Nil), |form| self.eval_in(form, environment))?;
        Ok(if escaped {
            self.define_special_value_exact(&name, value, force)
        } else {
            self.define_special_value(&name, value, force)
        })
    }

    fn special_defconstant(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if !(items.len() == 3 || items.len() == 4) {
            return Err(self.arity("defconstant", "two or three", items.len().saturating_sub(1)));
        }
        let (name, escaped) =
            self.variable_name_info(&items[1], "defconstant name must be a symbol")?;
        if if escaped {
            self.is_constant_exact_in(&name)
        } else {
            self.is_constant_in(&name)
        } {
            return Err(self.constant_modification_error(&name, items[1].span));
        }
        let value = self.eval_in(&items[2], environment)?;
        Ok(if escaped {
            self.define_constant_value_exact(&name, value)
        } else {
            self.define_constant_value(&name, value)
        })
    }

    fn special_defstruct(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(self.arity("defstruct", "at least one", items.len().saturating_sub(1)));
        }
        let (name_form, option_forms, slot_forms) = match &items[1].kind {
            FormKind::Atom(_) => (&items[1], &items[2..2], &items[2..]),
            FormKind::List(name_and_options) if !name_and_options.is_empty() => {
                (&name_and_options[0], &name_and_options[1..], &items[2..])
            }
            _ => {
                return Err(self.invalid(
                    "defstruct name must be a symbol or a name-and-options list",
                    items[1].span,
                ));
            }
        };
        let (documentation, slot_forms) = match slot_forms.first() {
            Some(Form {
                kind: FormKind::String(value),
                ..
            }) => (Some(value.clone()), &slot_forms[1..]),
            _ => (None, slot_forms),
        };
        let (raw_name, _) =
            self.variable_name_info(name_form, "defstruct name must be a symbol")?;
        let structure_name = unqualified_name(&raw_name);
        let mut conc_name = format!("{structure_name}-");
        let mut predicate_name = Some(format!("{structure_name}-P"));
        let mut predicate_explicit = false;
        let mut copier_name = Some(format!("COPY-{structure_name}"));
        let mut constructor_options: Vec<(Option<String>, Option<OrdinaryLambdaList>)> = Vec::new();
        let mut seen_options = HashSet::new();
        let mut included_structure: Option<(StructureDefinition, Vec<Form>)> = None;
        let mut representation = StructureRepresentation::Structure;
        let mut representation_explicit = false;
        let mut named = false;
        let mut named_explicit = false;
        for option_form in option_forms {
            if matches!(option_form.kind, FormKind::Atom(_)) {
                let Some(option_name) = atom_name(option_form) else {
                    return Err(self.invalid("defstruct option needs a name", option_form.span));
                };
                let normalized_option = normalize_name(option_name);
                if normalized_option.trim_start_matches(':') != "NAMED" {
                    return Err(self.invalid("defstruct option must be a list", option_form.span));
                }
                if !seen_options.insert("NAMED".to_string()) {
                    return Err(self.invalid("defstruct cannot repeat an option", option_form.span));
                }
                named = true;
                named_explicit = true;
                continue;
            }
            let FormKind::List(option_items) = &option_form.kind else {
                return Err(self.invalid("defstruct option must be a list", option_form.span));
            };
            let Some(option_name) = option_items.first().and_then(atom_name) else {
                return Err(self.invalid("defstruct option needs a name", option_form.span));
            };
            let normalized_option = normalize_name(option_name);
            let option_name = normalized_option.trim_start_matches(':');
            if option_name != "CONSTRUCTOR" && !seen_options.insert(option_name.to_string()) {
                return Err(self.invalid("defstruct cannot repeat an option", option_form.span));
            }
            match option_name {
                "CONC-NAME" => {
                    conc_name = self
                        .defstruct_name_option(
                            option_form,
                            option_items,
                            format!("{structure_name}-"),
                            "defstruct :conc-name must name a symbol or NIL",
                        )?
                        .unwrap_or_default();
                }
                "PREDICATE" => {
                    predicate_explicit = true;
                    predicate_name = self.defstruct_name_option(
                        option_form,
                        option_items,
                        format!("{structure_name}-P"),
                        "defstruct :predicate must name a symbol or NIL",
                    )?;
                }
                "COPIER" => {
                    copier_name = self.defstruct_name_option(
                        option_form,
                        option_items,
                        format!("COPY-{structure_name}"),
                        "defstruct :copier must name a symbol or NIL",
                    )?;
                }
                "TYPE" => {
                    if option_items.len() != 2 {
                        return Err(self.invalid(
                            "defstruct :type needs one type name",
                            option_form.span,
                        ));
                    }
                    let Some(type_name) = atom_name(&option_items[1]) else {
                        return Err(self.invalid(
                            "defstruct :type must name LIST or VECTOR",
                            option_items[1].span,
                        ));
                    };
                    representation = match normalize_name(type_name)
                        .trim_start_matches(':')
                    {
                        "LIST" => StructureRepresentation::List,
                        "VECTOR" => StructureRepresentation::Vector,
                        _ => {
                            return Err(self.invalid(
                                "defstruct :type must name LIST or VECTOR",
                                option_items[1].span,
                            ));
                        }
                    };
                    representation_explicit = true;
                }
                "INCLUDE" => {
                    if option_items.len() < 2 {
                        return Err(self.invalid(
                            "defstruct :include needs a structure name",
                            option_form.span,
                        ));
                    }
                    let (raw_parent_name, _) = self.variable_name_info(
                        &option_items[1],
                        "defstruct :include structure name must be a symbol",
                    )?;
                    let parent_name = unqualified_name(&raw_parent_name);
                    let Some(parent) = environment.lookup_structure(&parent_name) else {
                        return Err(self.invalid(
                            "defstruct :include requires a previously defined structure",
                            option_form.span,
                        ));
                    };
                    included_structure = Some((parent, option_items[2..].to_vec()));
                }
                "CONSTRUCTOR" => {
                    let constructor = self.defstruct_constructor_option(
                        option_form,
                        option_items,
                        format!("MAKE-{structure_name}"),
                    )?;
                    if (constructor.0.is_none() && !constructor_options.is_empty())
                        || constructor_options.iter().any(|(name, _)| name.is_none())
                    {
                        return Err(self.invalid(
                            "defstruct :constructor NIL cannot be combined with another constructor",
                            option_form.span,
                        ));
                    }
                    constructor_options.push(constructor);
                }
                _ => {
                    return Err(self.invalid("unsupported defstruct option", option_items[0].span));
                }
            }
        }
        let mut structure_types = vec![structure_name.clone()];
        let mut slots = Vec::new();
        let mut slot_names = HashSet::new();
        if let Some((parent, overrides)) = included_structure {
            if !representation_explicit {
                representation = parent.representation;
            }
            if !named_explicit {
                named = parent.named;
            }
            structure_types.extend(parent.type_names.clone());
            slots = parent.slots.clone();
            for slot in &slots {
                slot_names.insert(slot.name.clone());
            }
            let mut overridden_slots = HashSet::new();
            for slot_form in overrides {
                let (raw_slot_name, init_form, read_only) =
                    self.defstruct_slot_description(&slot_form, environment)?;
                let slot_name = unqualified_name(&raw_slot_name);
                let Some(slot) = slots.iter_mut().find(|slot| slot.name == slot_name) else {
                    return Err(self.invalid(
                        "defstruct :include slot must name an inherited slot",
                        slot_form.span,
                    ));
                };
                if !overridden_slots.insert(slot_name) {
                    return Err(self.invalid(
                        "defstruct :include cannot override a slot more than once",
                        slot_form.span,
                    ));
                }
                if let Some(init_form) = init_form {
                    slot.init_form = Some(init_form);
                }
                if let Some(read_only) = read_only {
                    slot.read_only = read_only;
                }
            }
        }
        for slot_form in slot_forms {
            let (raw_slot_name, init_form, read_only) =
                self.defstruct_slot_description(slot_form, environment)?;
            let slot_name = unqualified_name(&raw_slot_name);
            if !slot_names.insert(slot_name.clone()) {
                return Err(self.invalid("defstruct cannot define duplicate slots", slot_form.span));
            }
            slots.push(StructureSlot {
                name: slot_name,
                init_form,
                read_only: read_only.unwrap_or(false),
            });
        }

        if representation != StructureRepresentation::Structure && !named && !predicate_explicit {
            predicate_name = None;
        }

        environment.define_structure(
            &structure_name,
            StructureDefinition {
                documentation,
                slots: slots.clone(),
                type_names: structure_types.clone(),
                representation,
                named,
            },
        );
        if constructor_options.is_empty() {
            constructor_options.push((Some(format!("MAKE-{structure_name}")), None));
        }
        for (constructor_name, constructor_lambda_list) in constructor_options {
            if let Some(constructor_name) = constructor_name {
                environment.define_function(
                    &constructor_name,
                    Value::Function(Rc::new(crate::Function::StructureConstructor {
                        name: structure_name.clone(),
                        slots: slots.clone(),
                        structure_types: structure_types.clone(),
                        representation,
                        named,
                        constructor_lambda_list,
                        environment: environment.clone(),
                    })),
                );
            }
        }
        if let Some(predicate_name) = predicate_name {
            environment.define_function(
                &predicate_name,
                Value::Function(Rc::new(crate::Function::StructurePredicate {
                    name: structure_name.clone(),
                })),
            );
        }
        if let Some(copier_name) = copier_name {
            environment.define_function(
                &copier_name,
                Value::Function(Rc::new(crate::Function::StructureCopier {
                    name: structure_name.clone(),
                })),
            );
        }
        let conc_name = conc_name;
        for (slot_index, slot) in slots.iter().enumerate() {
            let accessor_name = format!("{conc_name}{}", slot.name);
            environment.define_function(
                &accessor_name,
                Value::Function(Rc::new(crate::Function::StructureAccessor {
                    structure_name: structure_name.clone(),
                    slot_name: slot.name.clone(),
                    slot_index,
                    read_only: slot.read_only,
                })),
            );
        }
        Ok(Value::symbol(structure_name))
    }

    fn special_defclass(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 4 {
            return Err(self.arity("defclass", "four", items.len().saturating_sub(1)));
        }

        let class_name = self.variable_name(&items[1], "defclass name must be a symbol")?;
        let class_name = unqualified_name(&class_name);
        let superclasses = self.list_form_items(&items[2], "defclass superclass list")?;
        let mut direct_superclasses = Vec::with_capacity(superclasses.len());
        for superclass in superclasses {
            let name = self.definition_name_from_form(superclass, "defclass superclass")?;
            if !direct_superclasses.contains(&name) {
                direct_superclasses.push(name);
            }
        }

        let slot_forms = self.list_form_items(&items[3], "defclass slot list")?;
        let mut slots: Vec<ClassSlot> = Vec::new();
        let mut readers = Vec::new();
        let mut writers = Vec::new();
        let mut setf_writers = Vec::new();
        let mut default_initargs = Vec::new();
        let mut documentation = None;

        for slot_form in slot_forms {
            let (slot_name_form, options) = match &slot_form.kind {
                FormKind::Atom(_) => (slot_form, &[][..]),
                FormKind::List(slot_items) if !slot_items.is_empty() => {
                    (&slot_items[0], &slot_items[1..])
                }
                _ => {
                    return Err(self.invalid(
                        "defclass slot must be a symbol or non-empty list",
                        slot_form.span,
                    ));
                }
            };
            let slot_name = self.variable_name(slot_name_form, "defclass slot must be a symbol")?;
            let slot_name = unqualified_name(&slot_name);
            let mut initarg = None;
            let mut init_form = None;
            let mut class_value = None;
            let mut type_specifier = None;

            if options.len() % 2 != 0 {
                return Err(self.invalid("defclass slot options require a value", slot_form.span));
            }
            for option in options.chunks_exact(2) {
                let option_name =
                    self.definition_name_from_form(&option[0], "defclass slot option")?;
                match option_name.as_str() {
                    "INITARG" => {
                        initarg = if is_nil_form(&option[1]) {
                            None
                        } else {
                            Some(self.definition_name_from_form(&option[1], "defclass initarg")?)
                        };
                    }
                    "INITFORM" => init_form = Some(option[1].clone()),
                    "ALLOCATION" => {
                        let allocation =
                            self.definition_name_from_form(&option[1], "defclass allocation")?;
                        match allocation.as_str() {
                            "CLASS" => {
                                class_value = Some(Rc::new(RefCell::new(Value::Unbound)));
                            }
                            "INSTANCE" => class_value = None,
                            _ => {
                                return Err(self.invalid(
                                    "defclass allocation must be :instance or :class",
                                    option[1].span,
                                ));
                            }
                        }
                    }
                    "ACCESSOR" => {
                        let accessor_name =
                            self.variable_name(&option[1], "defclass accessor must be a symbol")?;
                        let accessor_name = unqualified_name(&accessor_name);
                        readers.push((accessor_name.clone(), slot_name.clone()));
                        setf_writers.push((accessor_name, slot_name.clone()));
                    }
                    "READER" => {
                        let reader_name =
                            self.variable_name(&option[1], "defclass reader must be a symbol")?;
                        readers.push((unqualified_name(&reader_name), slot_name.clone()));
                    }
                    "WRITER" => {
                        let writer_name =
                            self.variable_name(&option[1], "defclass writer must be a symbol")?;
                        let writer_name = unqualified_name(&writer_name);
                        writers.push((writer_name.clone(), slot_name.clone()));
                        setf_writers.push((writer_name, slot_name.clone()));
                    }
                    "TYPE" => type_specifier = Some(quoted_form_value(&option[1])?),
                    "DOCUMENTATION" => {}
                    _ => {
                        return Err(
                            self.invalid("unsupported defclass slot option", option[0].span)
                        );
                    }
                }
            }

            if let Some(existing) = slots.iter_mut().find(|slot| slot.name == slot_name) {
                existing.initarg = initarg;
                existing.init_form = init_form;
                existing.class_value = class_value;
                existing.type_specifier = type_specifier;
            } else {
                slots.push(ClassSlot {
                    name: slot_name,
                    initarg,
                    init_form,
                    class_value,
                    type_specifier,
                });
            }
        }

        for option in items.iter().skip(4) {
            let option_items = self.list_form_items(option, "defclass option")?;
            if option_items.is_empty() {
                return Err(self.invalid("defclass option must be a non-empty list", option.span));
            }
            let option_name =
                self.definition_name_from_form(&option_items[0], "defclass option name")?;
            match option_name.as_str() {
                "DEFAULT-INITARGS" => {
                    if option_items.len() < 3 || (option_items.len() - 1) % 2 != 0 {
                        return Err(self.invalid(
                            "defclass :default-initargs requires initarg and form pairs",
                            option.span,
                        ));
                    }
                    for pair in option_items[1..].chunks_exact(2) {
                        let initarg =
                            self.definition_name_from_form(&pair[0], "defclass default initarg")?;
                        if let Some(existing) = default_initargs
                            .iter_mut()
                            .find(|(name, _)| name == &initarg)
                        {
                            existing.1 = pair[1].clone();
                        } else {
                            default_initargs.push((initarg, pair[1].clone()));
                        }
                    }
                }
                "DOCUMENTATION" => {
                    if option_items.len() != 2
                        || !matches!(option_items[1].kind, FormKind::String(_))
                    {
                        return Err(
                            self.invalid("defclass :documentation needs one string", option.span)
                        );
                    }
                    let FormKind::String(value) = &option_items[1].kind else {
                        unreachable!("validated defclass documentation string");
                    };
                    documentation = Some(value.clone());
                }
                "METACLASS" => {
                    if option_items.len() != 2 {
                        return Err(
                            self.invalid("defclass :metaclass needs one class name", option.span)
                        );
                    }
                    let metaclass =
                        self.definition_name_from_form(&option_items[1], "defclass metaclass")?;
                    if metaclass != "STANDARD-CLASS" {
                        return Err(self.invalid(
                            "only standard-class defclass metaclass is supported",
                            option_items[1].span,
                        ));
                    }
                }
                _ => {
                    return Err(self.invalid("unsupported defclass option", option_items[0].span));
                }
            }
        }

        let precedence = c3_class_precedence(&class_name, &direct_superclasses, environment)
            .map_err(|message| self.invalid(message, items[2].span))?;
        for superclass in &direct_superclasses {
            if superclass == "OBJECT" || superclass == "STANDARD-OBJECT" {
                continue;
            }
            let Some(definition) = environment.lookup_class(superclass) else {
                return Err(self.invalid("unknown defclass superclass", items[2].span));
            };
            for inherited in &definition.slots {
                if !slots.iter().any(|slot| slot.name == inherited.name) {
                    slots.push(inherited.clone());
                }
            }
            for inherited in &definition.default_initargs {
                if !default_initargs
                    .iter()
                    .any(|(name, _)| name == &inherited.0)
                {
                    default_initargs.push(inherited.clone());
                }
            }
        }

        let definition = Rc::new(ClassDefinition {
            name: class_name.clone(),
            direct_superclasses,
            precedence,
            slots,
            default_initargs,
            documentation,
        });
        environment.define_class(&class_name, definition);
        for (accessor_name, slot_name) in readers {
            environment.define_function(
                &accessor_name,
                Value::slot_reader(class_name.clone(), slot_name),
            );
        }
        for (writer_name, slot_name) in writers {
            environment.define_function(
                &writer_name,
                Value::slot_writer(class_name.clone(), slot_name),
            );
        }
        for (writer_name, slot_name) in setf_writers {
            environment.define_setf_function(
                &writer_name,
                Value::slot_writer(class_name.clone(), slot_name),
            );
        }
        Ok(Value::symbol(class_name))
    }

    fn special_defgeneric(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(self.arity("defgeneric", "three", items.len().saturating_sub(1)));
        }
        let name = self.variable_name(&items[1], "defgeneric name must be a symbol")?;
        let name = unqualified_name(&name);
        let _ = self.parameters(&items[2])?;
        environment.define_function(&name, Value::generic(name.clone()));
        Ok(Value::symbol(name))
    }

    fn special_defmethod(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(self.arity("defmethod", "three", items.len().saturating_sub(1)));
        }
        let name = self.variable_name(&items[1], "defmethod name must be a symbol")?;
        let name = unqualified_name(&name);
        let lambda_index = items[2..]
            .iter()
            .position(|form| matches!(form.kind, FormKind::List(_)))
            .map(|index| index + 2)
            .ok_or_else(|| {
                self.invalid("defmethod requires a method lambda list", items[1].span)
            })?;

        let qualifiers = items[2..lambda_index]
            .iter()
            .map(|form| {
                let qualifier = self.definition_name_from_form(form, "defmethod qualifier")?;
                match qualifier.as_str() {
                    "BEFORE" | "AFTER" | "AROUND" => Ok(qualifier),
                    _ => Err(self.invalid("unsupported defmethod qualifier", form.span)),
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        if qualifiers.len() > 1 {
            return Err(self.invalid(
                "defmethod accepts at most one method qualifier",
                items[2].span,
            ));
        }
        let FormKind::List(parameters) = &items[lambda_index].kind else {
            return Err(self.invalid(
                "defmethod lambda list must be a list",
                items[lambda_index].span,
            ));
        };

        let mut required = Vec::new();
        let mut required_escaped = Vec::new();
        let mut specializers = Vec::new();
        let mut normalized_parameters = Vec::new();
        let mut required_parameter_count = 0;
        for parameter in parameters {
            if matches!(&parameter.kind, FormKind::Atom(name) if normalize_name(name).starts_with('&'))
            {
                break;
            }
            let (name_form, specializer_form) = match &parameter.kind {
                FormKind::Atom(_) => (parameter, None),
                FormKind::List(parts) if (1..=2).contains(&parts.len()) => {
                    (&parts[0], parts.get(1))
                }
                _ => {
                    return Err(self.invalid(
                        "defmethod parameter must be a variable or (variable class)",
                        parameter.span,
                    ));
                }
            };
            let (parameter_name, escaped) =
                self.variable_name_info(name_form, "defmethod parameter must be a variable")?;
            required.push(unqualified_name(&parameter_name));
            required_escaped.push(escaped);
            let specializer = match specializer_form {
                None => MethodSpecializer::Type("T".to_owned()),
                Some(form) => {
                    let eql_parts = match &form.kind {
                        FormKind::List(parts)
                            if parts
                                .first()
                                .and_then(atom_name)
                                .is_some_and(|name| unqualified_name(name) == "EQL") =>
                        {
                            Some(parts)
                        }
                        _ => None,
                    };
                    if let Some(parts) = eql_parts {
                        if parts.len() != 2 {
                            return Err(self
                                .invalid("defmethod EQL specializer needs one value", form.span));
                        }
                        MethodSpecializer::Eql(self.eval_in(&parts[1], environment)?)
                    } else {
                        let name = self.definition_name_from_form(form, "defmethod specializer")?;
                        if !builtins::known_type_name(&name, environment) {
                            return Err(
                                self.invalid("unknown defmethod specializer", parameter.span)
                            );
                        }
                        MethodSpecializer::Type(name)
                    }
                }
            };
            specializers.push(specializer);
            normalized_parameters.push(name_form.clone());
            required_parameter_count += 1;
        }
        normalized_parameters.extend(
            parameters
                .get(required_parameter_count..)
                .unwrap_or_default()
                .iter()
                .cloned(),
        );
        let normalized_lambda_list = Form::list(normalized_parameters, items[lambda_index].span);
        let lambda_list = self.parameters(&normalized_lambda_list)?;

        let generic = environment.lookup_function(&name).or_else(|| {
            let generic = Value::generic(name.clone());
            environment.define_function(&name, generic.clone());
            Some(generic)
        });
        let Some(Value::Function(generic)) = generic else {
            return Err(self.invalid("defmethod name is not a generic function", items[1].span));
        };
        let crate::Function::Generic { methods, .. } = generic.as_ref() else {
            return Err(self.invalid("defmethod name is not a generic function", items[1].span));
        };
        let closure = Value::closure_with_keywords(
            required,
            required_escaped,
            lambda_list.optional,
            lambda_list.rest,
            lambda_list.rest_escaped,
            lambda_list.keywords,
            lambda_list.has_keyword_section,
            lambda_list.allow_other_keys,
            lambda_list.auxiliary,
            items[lambda_index + 1..].to_vec(),
            environment.clone(),
        );
        let method = MethodDefinition {
            qualifiers,
            specializers,
            function: closure,
        };
        let mut methods = methods.borrow_mut();
        if let Some(existing) = methods.iter_mut().find(|existing| {
            existing.qualifiers == method.qualifiers
                && method_specializers_equal(&existing.specializers, &method.specializers)
        }) {
            *existing = method;
        } else {
            methods.push(method);
        }
        Ok(Value::symbol(name))
    }

    pub(crate) fn list_form_items<'a>(
        &self,
        form: &'a Form,
        context: &str,
    ) -> Result<&'a [Form], RuntimeError> {
        match &form.kind {
            FormKind::List(items) => Ok(items),
            FormKind::Atom(name) if normalize_name(name) == "NIL" => Ok(&[]),
            _ => Err(self.invalid(context, form.span)),
        }
    }

    pub(crate) fn definition_name_from_form(
        &self,
        form: &Form,
        context: &str,
    ) -> Result<String, RuntimeError> {
        self.definition_name_info_from_form(form, context)
            .map(|(name, _)| name)
    }

    pub(crate) fn definition_name_info_from_form(
        &self,
        form: &Form,
        context: &str,
    ) -> Result<(String, bool), RuntimeError> {
        let Some(raw_name) = atom_name(form) else {
            return Err(self.invalid(context, form.span));
        };
        let token = parse_symbol_token(raw_name).map_err(|_| self.invalid(context, form.span))?;
        if !matches!(
            token.kind,
            SymbolTokenKind::Symbol | SymbolTokenKind::Keyword
        ) || token.name.is_empty()
        {
            return Err(self.invalid(context, form.span));
        }
        if token.escaped && token.package.is_some() {
            return Err(self.invalid(context, form.span));
        }
        let normalized = if token.escaped {
            token.name
        } else {
            normalize_name(raw_name)
        };
        let name = if token.escaped {
            normalized.trim_start_matches(':').to_owned()
        } else {
            unqualified_name(normalized.trim_start_matches(':'))
        };
        Ok((name, token.escaped))
    }

    fn defstruct_name_option(
        &self,
        option_form: &Form,
        option_items: &[Form],
        default_name: String,
        context: &str,
    ) -> Result<Option<String>, RuntimeError> {
        if option_items.len() > 2 {
            return Err(self.invalid(
                "defstruct naming options accept at most one name",
                option_form.span,
            ));
        }
        let Some(name_form) = option_items.get(1) else {
            return Ok(Some(default_name));
        };
        if is_nil_form(name_form) {
            return Ok(None);
        }
        let (raw_name, _) = self.variable_name_info(name_form, context)?;
        Ok(Some(unqualified_name(&raw_name)))
    }

    fn defstruct_constructor_option(
        &self,
        option_form: &Form,
        option_items: &[Form],
        default_name: String,
    ) -> Result<(Option<String>, Option<OrdinaryLambdaList>), RuntimeError> {
        if option_items.len() > 3 {
            return Err(self.invalid(
                "defstruct :constructor accepts at most a name and a lambda list",
                option_form.span,
            ));
        }
        let constructor_name = match option_items.get(1) {
            None => Some(default_name),
            Some(name_form) if is_nil_form(name_form) => None,
            Some(name_form) => {
                let (raw_name, _) = self.variable_name_info(
                    name_form,
                    "defstruct :constructor must name a symbol or NIL",
                )?;
                Some(unqualified_name(&raw_name))
            }
        };
        let constructor_lambda_list = option_items
            .get(2)
            .map(|lambda_list_form| {
                if constructor_name.is_none() {
                    return Err(self.invalid(
                        "defstruct :constructor NIL cannot have a lambda list",
                        lambda_list_form.span,
                    ));
                }
                self.parameters(lambda_list_form)
            })
            .transpose()?;
        Ok((constructor_name, constructor_lambda_list))
    }

    fn defstruct_slot_description(
        &self,
        slot_form: &Form,
        environment: &Environment,
    ) -> Result<(String, Option<Form>, Option<bool>), RuntimeError> {
        match &slot_form.kind {
            FormKind::Atom(_) => Ok((
                self.variable_name_info(
                    slot_form,
                    "defstruct slot must be a symbol or a slot specification",
                )?
                .0,
                None,
                None,
            )),
            FormKind::List(slot_items) if (1..=3).contains(&slot_items.len()) => {
                let slot_name = self
                    .variable_name_info(&slot_items[0], "defstruct slot name must be a symbol")?;
                let read_only = slot_items
                    .get(2)
                    .map(|form| {
                        self.eval_in(form, environment)
                            .map(|value| value.is_truthy())
                    })
                    .transpose()?;
                Ok((slot_name.0, slot_items.get(1).cloned(), read_only))
            }
            _ => Err(self.invalid(
                "defstruct slot must be a symbol or a one- to three-element list",
                slot_form.span,
            )),
        }
    }

    fn special_defpackage(&self, items: &[Form]) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(self.arity("defpackage", "at least one", items.len().saturating_sub(1)));
        }
        enum DefpackageOperation {
            Shadow(String, bool),
            Intern(String, bool),
            Unintern(String, bool),
            Import {
                source_package: String,
                source_name: String,
                shadowing: bool,
                exact: bool,
            },
        }

        let name = self.package_name_from_form(&items[1])?;
        let mut nicknames = Vec::new();
        let mut use_packages = vec![package::COMMON_LISP_PACKAGE.to_string()];
        let mut exports = HashSet::new();
        let mut exact_exports = Vec::new();
        let mut operations = Vec::new();
        let mut saw_nicknames = false;
        let mut saw_use = false;
        let mut documentation = None;
        let mut saw_documentation = false;
        let mut saw_size = false;
        let mut local_nicknames = HashMap::new();

        for option in items.iter().skip(2) {
            let FormKind::List(option_items) = &option.kind else {
                return Err(self.invalid("defpackage option must be a list", option.span));
            };
            let Some(option_name) = option_items.first().and_then(atom_name) else {
                return Err(self.invalid("defpackage option needs a name", option.span));
            };
            let normalized_option = normalize_name(option_name);
            match normalized_option.trim_start_matches(':') {
                "NICKNAMES" => {
                    if saw_nicknames {
                        return Err(self
                            .invalid("defpackage has duplicate :nicknames options", option.span));
                    }
                    saw_nicknames = true;
                    for package_form in option_items.iter().skip(1) {
                        nicknames.push(self.package_name_from_form(package_form)?);
                    }
                }
                "USE" => {
                    if saw_use {
                        return Err(
                            self.invalid("defpackage has duplicate :use options", option.span)
                        );
                    }
                    saw_use = true;
                    use_packages.clear();
                    for package_form in option_items.iter().skip(1) {
                        use_packages.push(self.package_name_from_form(package_form)?);
                    }
                }
                "DOCUMENTATION" => {
                    if saw_documentation || option_items.len() != 2 {
                        return Err(
                            self.invalid("defpackage :documentation needs one string", option.span)
                        );
                    }
                    saw_documentation = true;
                    let FormKind::String(value) = &option_items[1].kind else {
                        return Err(self.invalid(
                            "defpackage :documentation needs a string",
                            option_items[1].span,
                        ));
                    };
                    documentation = Some(value.clone());
                }
                "SIZE" => {
                    if saw_size || option_items.len() != 2 {
                        return Err(self.invalid(
                            "defpackage :size needs one non-negative integer",
                            option.span,
                        ));
                    }
                    saw_size = true;
                    let FormKind::Atom(value) = &option_items[1].kind else {
                        return Err(self.invalid(
                            "defpackage :size needs a non-negative integer",
                            option_items[1].span,
                        ));
                    };
                    if value.parse::<i64>().map_or(true, |size| size < 0) {
                        return Err(self.invalid(
                            "defpackage :size needs a non-negative integer",
                            option_items[1].span,
                        ));
                    }
                }
                "LOCAL-NICKNAMES" => {
                    for nickname_option in option_items.iter().skip(1) {
                        let FormKind::List(mapping) = &nickname_option.kind else {
                            return Err(self.invalid(
                                "defpackage local nickname needs a two-element list",
                                nickname_option.span,
                            ));
                        };
                        if mapping.len() != 2 {
                            return Err(self.invalid(
                                "defpackage local nickname needs a two-element list",
                                nickname_option.span,
                            ));
                        }
                        let nickname = self.package_name_from_form(&mapping[0])?;
                        let target = self.package_name_from_form(&mapping[1])?;
                        if local_nicknames.insert(nickname, target).is_some() {
                            return Err(self.invalid(
                                "defpackage has duplicate local package nickname",
                                nickname_option.span,
                            ));
                        }
                    }
                }
                "EXPORT" => {
                    for symbol_form in option_items.iter().skip(1) {
                        let (symbol, exact) = self.symbol_name_from_form(symbol_form)?;
                        if exact {
                            exact_exports.push(symbol);
                        } else {
                            exports.insert(symbol);
                        }
                    }
                }
                "SHADOW" => {
                    for symbol_form in option_items.iter().skip(1) {
                        let (symbol, exact) = self.symbol_name_from_form(symbol_form)?;
                        operations.push(DefpackageOperation::Shadow(symbol, exact));
                    }
                }
                "INTERN" => {
                    for symbol_form in option_items.iter().skip(1) {
                        let (symbol, exact) = self.symbol_name_from_form(symbol_form)?;
                        operations.push(DefpackageOperation::Intern(symbol, exact));
                    }
                }
                "UNINTERN" => {
                    for symbol_form in option_items.iter().skip(1) {
                        let (symbol, exact) = self.symbol_name_from_form(symbol_form)?;
                        operations.push(DefpackageOperation::Unintern(symbol, exact));
                    }
                }
                "IMPORT-FROM" | "SHADOWING-IMPORT-FROM" => {
                    if option_items.len() < 2 {
                        return Err(self.invalid(
                            "defpackage import option needs a package name",
                            option.span,
                        ));
                    }
                    let source_package = self.package_name_from_form(&option_items[1])?;
                    let shadowing =
                        normalized_option.trim_start_matches(':') == "SHADOWING-IMPORT-FROM";
                    for symbol_form in option_items.iter().skip(2) {
                        let (source_name, exact) = self.symbol_name_from_form(symbol_form)?;
                        operations.push(DefpackageOperation::Import {
                            source_package: source_package.clone(),
                            source_name,
                            shadowing,
                            exact,
                        });
                    }
                }
                _ => {
                    return Err(self.invalid("unsupported defpackage option", option_items[0].span));
                }
            }
        }

        {
            let packages = self.packages.borrow();
            if use_packages
                .iter()
                .any(|package_name| !packages.package_exists(package_name))
            {
                let missing = use_packages
                    .iter()
                    .find(|package_name| !packages.package_exists(package_name))
                    .expect("missing package must exist");
                return Err(
                    self.package_error(&format!("unknown package {missing}"), items[1].span)
                );
            }
            for operation in &operations {
                let DefpackageOperation::Import {
                    source_package,
                    source_name,
                    exact,
                    ..
                } = operation
                else {
                    continue;
                };
                if !packages.package_exists(source_package) {
                    return Err(self.package_error(
                        &format!("unknown package {source_package}"),
                        items[1].span,
                    ));
                }
                let symbol_exists = if *exact {
                    packages.symbol_exists_exact(source_package, source_name)
                } else {
                    packages.symbol_exists(source_package, source_name)
                };
                if !symbol_exists {
                    return Err(self.package_error(
                        &format!("unknown symbol {source_package}::{source_name}"),
                        items[1].span,
                    ));
                }
            }
        }

        let mut packages = self.packages.borrow_mut();
        let mut preview = packages.clone();
        if let Err(message) = preview.define_package(
            name.clone(),
            nicknames.clone(),
            use_packages.clone(),
            exports.clone(),
            documentation.clone(),
            local_nicknames.clone(),
        ) {
            return Err(self.package_error(&message, items[1].span));
        }
        preview.export_symbols_exact(&name, &exact_exports);
        for operation in &operations {
            let DefpackageOperation::Import {
                source_package,
                source_name,
                shadowing,
                exact,
            } = operation
            else {
                continue;
            };
            let conflict = if *exact {
                preview.import_conflict_exact(source_package, source_name, &name)
            } else {
                preview.import_conflict(source_package, source_name, &name)
            };
            if !shadowing && conflict {
                return Err(self.package_error(
                    &format!("name conflict for symbol {source_name}"),
                    items[1].span,
                ));
            }
            if *exact {
                preview.import_symbol_exact(source_package, source_name, &name, *shadowing);
            } else {
                preview.import_symbol(source_package, source_name, &name, *shadowing);
            }
        }
        if let Err(message) = packages.define_package(
            name.clone(),
            nicknames,
            use_packages,
            exports,
            documentation,
            local_nicknames,
        ) {
            return Err(self.package_error(&message, items[1].span));
        }
        packages.export_symbols_exact(&name, &exact_exports);
        for operation in operations {
            match operation {
                DefpackageOperation::Shadow(symbol, exact) => {
                    if exact {
                        packages.shadow_symbol_exact(&name, &symbol);
                    } else {
                        packages.shadow_symbol(&name, &symbol);
                    }
                }
                DefpackageOperation::Intern(symbol, exact) => {
                    if exact {
                        let _ = packages.intern_symbol_exact(&name, &symbol);
                    } else {
                        let _ = packages.intern_symbol(&name, &symbol);
                    }
                }
                DefpackageOperation::Unintern(symbol, exact) => {
                    if exact {
                        let _ = packages.unintern_symbol_exact(&name, &symbol);
                    } else {
                        let _ = packages.unintern_symbol(&name, &symbol);
                    }
                }
                DefpackageOperation::Import {
                    source_package,
                    source_name,
                    shadowing,
                    exact,
                } => {
                    if exact {
                        packages.import_symbol_exact(
                            &source_package,
                            &source_name,
                            &name,
                            shadowing,
                        );
                    } else {
                        packages.import_symbol(&source_package, &source_name, &name, shadowing);
                    }
                }
            }
        }
        let canonical_name = packages.canonical_package_name(&name);
        Ok(Value::package(&canonical_name))
    }

    fn special_in_package(&self, items: &[Form]) -> Result<Value, RuntimeError> {
        if items.len() != 2 {
            return Err(self.arity("in-package", "one", items.len().saturating_sub(1)));
        }
        let name = self.package_name_from_form(&items[1])?;
        let mut packages = self.packages.borrow_mut();
        if !packages.package_exists(&name) {
            return Err(self.package_error(&format!("unknown package {name}"), items[1].span));
        }
        let canonical_name = packages.canonical_package_name(&name);
        packages.set_current(canonical_name.clone());
        let package = Value::package(&canonical_name);
        drop(packages);
        self.define_special_value("*PACKAGE*", package.clone(), true);
        Ok(package)
    }

    fn package_name_from_form(&self, form: &Form) -> Result<String, RuntimeError> {
        let name = match &form.kind {
            FormKind::String(value) => value.strip_prefix(':').unwrap_or(value).to_string(),
            FormKind::Atom(value) => {
                let token = parse_symbol_token(value).map_err(|_| {
                    self.invalid("package name must be a symbol or string", form.span)
                })?;
                if token.kind == SymbolTokenKind::Uninterned || token.package.is_some() {
                    return Err(self.package_error("package name cannot be qualified", form.span));
                }
                token.name
            }
            _ => {
                return Err(self.invalid("package name must be a symbol or string", form.span));
            }
        };
        if name.is_empty() || name.contains(':') {
            return Err(self.package_error("invalid package name", form.span));
        }
        Ok(name)
    }

    fn symbol_name_from_form(&self, form: &Form) -> Result<(String, bool), RuntimeError> {
        let (raw, exact) = match &form.kind {
            FormKind::Atom(value) => (value.as_str(), false),
            FormKind::String(value) => (value.as_str(), true),
            _ => return Err(self.invalid("symbol name must be a symbol or string", form.span)),
        };
        let name = raw.strip_prefix(':').unwrap_or(raw);
        if name.is_empty() || name.contains(':') {
            return Err(self.package_error("symbol name cannot be qualified", form.span));
        }
        let name = if exact {
            name.to_string()
        } else {
            normalize_name(name)
        };
        Ok((name, exact))
    }

    fn package_designator_name(&self, value: &Value, span: Span) -> Result<String, RuntimeError> {
        let (raw, exact) = match value {
            Value::Package(name) | Value::String(name) => (name.as_ref(), true),
            _ => value.symbol_reference().ok_or_else(|| RuntimeError::Type {
                expected: "PACKAGE DESIGNATOR".to_string(),
                actual: value.type_name().to_string(),
                span: Some(span),
            })?,
        };
        let raw = raw.strip_prefix(':').unwrap_or(raw);
        if package::split_symbol(raw).is_some() {
            return Err(self.package_error("package name cannot be qualified", span));
        }
        let name = if exact {
            raw.to_string()
        } else {
            package::normalize_package_name(raw)
        };
        if name.is_empty() || name.contains(':') {
            return Err(self.package_error("invalid package name", span));
        }
        Ok(name)
    }

    fn package_name_from_value(&self, value: &Value, span: Span) -> Result<String, RuntimeError> {
        let name = self.package_designator_name(value, span)?;
        let is_package_object = matches!(value, Value::Package(_));
        let packages = self.packages.borrow();
        let package_name = if is_package_object {
            packages.package_object_name(&name)
        } else {
            packages.canonical_package_name(&name)
        };
        if !packages.package_exists(&package_name) {
            return Err(self.package_error(&format!("unknown package {name}"), span));
        }
        Ok(package_name)
    }

    fn symbol_name_from_value(&self, value: &Value, span: Span) -> Result<String, RuntimeError> {
        let raw = match value {
            Value::String(name) => name.as_ref(),
            _ => value.symbol_name().ok_or_else(|| RuntimeError::Type {
                expected: "STRING DESIGNATOR".to_string(),
                actual: value.type_name().to_string(),
                span: Some(span),
            })?,
        };
        let name = raw.strip_prefix(':').unwrap_or(raw);
        if name.is_empty() || package::split_symbol(name).is_some() || name.contains(':') {
            return Err(self.package_error("symbol name cannot be qualified", span));
        }
        Ok(package::normalize_symbol_name(name))
    }

    fn symbol_name_from_value_exact(
        &self,
        value: &Value,
        span: Span,
    ) -> Result<String, RuntimeError> {
        match value {
            Value::String(name) => Ok(name.to_string()),
            _ => self.symbol_name_from_value(value, span),
        }
    }

    pub(crate) fn name_designator_from_value(
        &self,
        value: &Value,
        span: Span,
    ) -> Result<String, RuntimeError> {
        self.name_designator_info_from_value(value, span)
            .map(|(name, _)| name)
    }

    pub(crate) fn name_designator_info_from_value(
        &self,
        value: &Value,
        span: Span,
    ) -> Result<(String, bool), RuntimeError> {
        let (raw, escaped) = match value {
            Value::String(name) => (name.as_ref(), false),
            _ => value.symbol_reference().ok_or_else(|| RuntimeError::Type {
                expected: "SYMBOL DESIGNATOR".to_string(),
                actual: value.type_name().to_string(),
                span: Some(span),
            })?,
        };
        let name = raw.strip_prefix(':').unwrap_or(raw);
        if name.is_empty() {
            return Err(self.invalid("symbol name cannot be empty", span));
        }
        let normalized = if escaped {
            name.to_owned()
        } else {
            unqualified_name(name)
        };
        Ok((normalized, escaped))
    }

    fn slot_name_from_value(&self, value: &Value, span: Span) -> Result<String, RuntimeError> {
        self.name_designator_from_value(value, span)
    }

    fn package_names_from_value(
        &self,
        value: &Value,
        span: Span,
    ) -> Result<Vec<String>, RuntimeError> {
        let values = value
            .list_items()
            .ok_or_else(|| self.invalid("package designators must be a proper list", span))?;
        values
            .iter()
            .map(|value| self.package_name_from_value(value, span))
            .collect()
    }

    fn rename_package(&self, arguments: &[Value], span: Span) -> Result<Value, RuntimeError> {
        if !(2..=3).contains(&arguments.len()) {
            return Err(self.arity("rename-package", "two or three", arguments.len()));
        }
        let package_name = self.package_name_from_value(&arguments[0], span)?;
        let new_name = self.package_designator_name(&arguments[1], span)?;
        let new_nicknames = arguments
            .get(2)
            .map(|value| {
                let values = value.list_items().ok_or_else(|| {
                    self.invalid("rename-package nicknames must be a proper list", span)
                })?;
                values
                    .iter()
                    .map(|value| self.package_designator_name(value, span))
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?
            .unwrap_or_default();
        let renamed_name = self
            .packages
            .borrow_mut()
            .rename_package(&package_name, &new_name, new_nicknames)
            .map_err(|message| self.package_error(&message, span))?;
        if matches!(&arguments[0], Value::Package(_)) {
            Ok(arguments[0].clone())
        } else {
            Ok(Value::package(renamed_name))
        }
    }

    fn make_package(&self, arguments: &[Value], span: Span) -> Result<Value, RuntimeError> {
        if arguments.is_empty() {
            return Err(self.arity("make-package", "at least one", arguments.len()));
        }
        if !(arguments.len() - 1).is_multiple_of(2) {
            return Err(self.invalid(
                "make-package keyword arguments must be supplied in pairs",
                span,
            ));
        }

        let name = self.package_designator_name(&arguments[0], span)?;
        let mut nicknames = Vec::new();
        let mut use_packages = Vec::new();
        let mut documentation = None;
        let mut saw_nicknames = false;
        let mut saw_use = false;
        let mut saw_documentation = false;
        let mut saw_size = false;

        for pair in arguments[1..].chunks_exact(2) {
            let keyword_name = match &pair[0] {
                Value::Keyword(keyword) | Value::KeywordExact(keyword) => normalize_name(keyword),
                other => {
                    return Err(self.invalid(
                        &format!(
                            "make-package keyword must be a keyword, got {}",
                            other.type_name()
                        ),
                        span,
                    ));
                }
            };
            match keyword_name.as_str() {
                "NICKNAMES" => {
                    if saw_nicknames {
                        return Err(
                            self.invalid("make-package received duplicate :nicknames", span)
                        );
                    }
                    saw_nicknames = true;
                    let values = pair[1].list_items().ok_or_else(|| {
                        self.invalid("make-package :nicknames must be a proper list", span)
                    })?;
                    nicknames = values
                        .iter()
                        .map(|value| self.package_designator_name(value, span))
                        .collect::<Result<Vec<_>, _>>()?;
                }
                "USE" => {
                    if saw_use {
                        return Err(self.invalid("make-package received duplicate :use", span));
                    }
                    saw_use = true;
                    use_packages = self.package_names_from_value(&pair[1], span)?;
                }
                "DOCUMENTATION" => {
                    if saw_documentation {
                        return Err(
                            self.invalid("make-package received duplicate :documentation", span)
                        );
                    }
                    saw_documentation = true;
                    let Value::String(value) = &pair[1] else {
                        return Err(
                            self.invalid("make-package :documentation must be a string", span)
                        );
                    };
                    documentation = Some(value.to_string());
                }
                "SIZE" => {
                    if saw_size {
                        return Err(self.invalid("make-package received duplicate :size", span));
                    }
                    saw_size = true;
                    if !matches!(&pair[1], Value::Integer(size) if *size >= 0) {
                        return Err(
                            self.invalid("make-package :size must be a non-negative integer", span)
                        );
                    }
                }
                _ => {
                    return Err(self.invalid(
                        &format!("unsupported make-package keyword :{keyword_name}"),
                        span,
                    ));
                }
            }
        }

        {
            let packages = self.packages.borrow();
            if packages.package_exists(&name) {
                return Err(self.package_error(&format!("package {name} already exists"), span));
            }
        }

        let mut packages = self.packages.borrow_mut();
        packages
            .define_package(
                name.clone(),
                nicknames,
                use_packages,
                HashSet::new(),
                documentation,
                HashMap::new(),
            )
            .map_err(|message| self.package_error(&message, span))?;
        let canonical_name = packages.canonical_package_name(&name);
        Ok(Value::package(&canonical_name))
    }

    fn symbol_names_from_value(
        &self,
        value: &Value,
        span: Span,
    ) -> Result<Vec<String>, RuntimeError> {
        let values = value
            .list_items()
            .ok_or_else(|| self.invalid("symbol designators must be a proper list", span))?;
        values
            .iter()
            .map(|value| self.symbol_name_from_value(value, span))
            .collect()
    }

    fn symbol_names_from_value_partitioned(
        &self,
        value: &Value,
        span: Span,
    ) -> Result<Vec<(String, bool)>, RuntimeError> {
        let values = value
            .list_items()
            .ok_or_else(|| self.invalid("symbol designators must be a proper list", span))?;
        values
            .iter()
            .map(|value| match value {
                Value::SymbolExact(name) | Value::KeywordExact(name) => {
                    Ok((name.to_string(), true))
                }
                Value::QualifiedSymbolExact {
                    reference,
                    package_len,
                } => Ok((reference[*package_len + 2..].to_string(), true)),
                _ => Ok((self.symbol_name_from_value(value, span)?, false)),
            })
            .collect()
    }

    fn symbol_references_from_value_or_single(
        &self,
        value: &Value,
        span: Span,
    ) -> Result<Vec<(String, String, bool)>, RuntimeError> {
        let reference = |value: &Value| -> Result<Option<(String, String, bool)>, RuntimeError> {
            if matches!(value, Value::UninternedSymbol(_)) {
                return Ok(None);
            }
            match value {
                Value::QualifiedSymbolExact {
                    reference,
                    package_len,
                } => Ok(Some((
                    package::normalize_package_name(&reference[..*package_len]),
                    reference[*package_len + 2..].to_string(),
                    true,
                ))),
                Value::SymbolExact(name) => {
                    if let Some((package_name, symbol_name, _)) = package::split_symbol(name) {
                        Ok(Some((
                            package::normalize_package_name(package_name),
                            symbol_name.to_string(),
                            true,
                        )))
                    } else {
                        Ok(Some((self.current_package(), name.to_string(), true)))
                    }
                }
                Value::KeywordExact(name) => Ok(Some((
                    package::KEYWORD_PACKAGE.to_string(),
                    name.to_string(),
                    true,
                ))),
                Value::Keyword(name) => Ok(Some((
                    package::KEYWORD_PACKAGE.to_string(),
                    package::normalize_symbol_name(name),
                    false,
                ))),
                _ => {
                    let raw = match value {
                        Value::String(_) => {
                            return Err(RuntimeError::Type {
                                expected: "SYMBOL".to_string(),
                                actual: value.type_name().to_string(),
                                span: Some(span),
                            });
                        }
                        _ => value.symbol_name().ok_or_else(|| RuntimeError::Type {
                            expected: "SYMBOL".to_string(),
                            actual: value.type_name().to_string(),
                            span: Some(span),
                        })?,
                    };
                    let raw = raw.strip_prefix(':').unwrap_or(raw);
                    if let Some((package_name, symbol_name, _)) = package::split_symbol(raw) {
                        Ok(Some((
                            package::normalize_package_name(package_name),
                            package::normalize_symbol_name(symbol_name),
                            false,
                        )))
                    } else {
                        Ok(Some((
                            self.current_package(),
                            package::normalize_symbol_name(raw),
                            false,
                        )))
                    }
                }
            }
        };
        if let Some(values) = value.list_items() {
            let mut references = Vec::new();
            for value in values.iter() {
                if let Some(reference) = reference(value)? {
                    references.push(reference);
                }
            }
            return Ok(references);
        }
        Ok(reference(value)?.into_iter().collect())
    }

    fn symbol_import_references_from_value(
        &self,
        value: &Value,
        span: Span,
    ) -> Result<Vec<(String, String, bool)>, RuntimeError> {
        let values = value
            .list_items()
            .ok_or_else(|| self.invalid("symbol designators must be a proper list", span))?;
        values
            .iter()
            .map(|value| {
                if matches!(value, Value::UninternedSymbol(_)) {
                    return Err(self.invalid("uninterned symbols cannot be imported", span));
                }
                if let Value::QualifiedSymbolExact {
                    reference,
                    package_len,
                } = value
                {
                    return Ok((
                        package::normalize_package_name(&reference[..*package_len]),
                        reference[*package_len + 2..].to_string(),
                        true,
                    ));
                }
                if let Value::SymbolExact(name) = value {
                    return Ok((self.current_package(), name.to_string(), true));
                }
                if let Value::KeywordExact(name) = value {
                    return Ok((package::KEYWORD_PACKAGE.to_string(), name.to_string(), true));
                }
                let raw = value.symbol_name().ok_or_else(|| RuntimeError::Type {
                    expected: "SYMBOL".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(span),
                })?;
                if matches!(value, Value::Keyword(_)) {
                    return Ok((
                        package::KEYWORD_PACKAGE.to_string(),
                        package::normalize_symbol_name(raw),
                        false,
                    ));
                }
                if let Some((package_name, symbol_name, _)) = package::split_symbol(raw) {
                    return Ok((
                        package::normalize_package_name(package_name),
                        package::normalize_symbol_name(symbol_name),
                        false,
                    ));
                }
                Ok((
                    self.current_package(),
                    package::normalize_symbol_name(raw),
                    false,
                ))
            })
            .collect()
    }

    fn package_symbol_value(&self, package_name: &str, symbol_name: &str) -> Value {
        let state = self.packages.borrow();
        let package_name = state.canonical_package_name(package_name);
        if let Some((reference, _)) = state.find_symbol(&package_name, symbol_name) {
            if reference.package() == package::KEYWORD_PACKAGE {
                Value::keyword(reference.name())
            } else {
                Value::symbol(package::canonical_symbol_name(
                    reference.package(),
                    reference.name(),
                ))
            }
        } else if package_name == package::KEYWORD_PACKAGE {
            Value::keyword(symbol_name)
        } else {
            Value::symbol(state.imported_symbol_name(&package_name, symbol_name))
        }
    }

    fn package_symbol_value_exact(&self, package_name: &str, symbol_name: &str) -> Value {
        let state = self.packages.borrow();
        let package_name = state.canonical_package_name(package_name);
        if let Some((reference, _)) = state.find_symbol_exact(&package_name, symbol_name) {
            if reference.package() == package::KEYWORD_PACKAGE {
                Value::keyword_exact(reference.name())
            } else {
                Value::qualified_symbol_exact(reference.package(), reference.name())
            }
        } else if package_name == package::KEYWORD_PACKAGE {
            Value::keyword_exact(symbol_name)
        } else {
            let (package_name, symbol_name) =
                state.imported_symbol_parts_exact(&package_name, symbol_name);
            Value::qualified_symbol_exact(&package_name, &symbol_name)
        }
    }

    fn symbol_status_value(status: package::SymbolStatus) -> Value {
        match status {
            package::SymbolStatus::Internal => Value::keyword("INTERNAL"),
            package::SymbolStatus::External => Value::keyword("EXTERNAL"),
            package::SymbolStatus::Inherited => Value::keyword("INHERITED"),
        }
    }

    fn validate_slot_value(
        &self,
        slot: &ClassSlot,
        value: &Value,
        span: Span,
    ) -> Result<(), RuntimeError> {
        let Some(type_specifier) = &slot.type_specifier else {
            return Ok(());
        };
        if matches!(value, Value::Unbound) || builtins::typep_value(value, type_specifier)? {
            return Ok(());
        }
        Err(RuntimeError::Type {
            expected: type_specifier.to_string(),
            actual: value.type_name().to_string(),
            span: Some(span),
        })
    }

    fn validate_instance_slot_value(
        &self,
        object: &Value,
        slot_name: &str,
        value: &Value,
        span: Span,
    ) -> Result<(), RuntimeError> {
        let Some(class) = object.instance_class_definition() else {
            return Ok(());
        };
        if let Some(slot) = class
            .slots
            .iter()
            .find(|slot| slot.name.eq_ignore_ascii_case(slot_name))
        {
            self.validate_slot_value(slot, value, span)?;
        }
        Ok(())
    }

    fn make_instance(
        &self,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.is_empty() {
            return Err(self.arity("make-instance", "at least one", arguments.len()));
        }
        if (arguments.len() - 1) % 2 != 0 {
            return Err(self.invalid("make-instance initargs require pairs", span));
        }
        let class_name = self.name_designator_from_value(&arguments[0], span)?;
        let class = environment
            .lookup_class(&class_name)
            .ok_or_else(|| self.invalid("unknown class", span))?;

        let mut initargs = Vec::with_capacity(arguments.len().saturating_sub(1));
        for pair in arguments[1..].chunks_exact(2) {
            let initarg = self.name_designator_from_value(&pair[0], span)?;
            initargs.push((initarg, pair[1].clone()));
        }
        for (initarg, init_form) in &class.default_initargs {
            if initargs.iter().any(|(name, _)| name == initarg) {
                continue;
            }
            initargs.push((initarg.clone(), self.eval_in(init_form, environment)?));
        }
        for (initarg, _) in &initargs {
            if !class
                .slots
                .iter()
                .any(|slot| slot.initarg.as_deref() == Some(initarg.as_str()))
            {
                return Err(self.invalid("unknown make-instance initarg", span));
            }
        }

        let mut slots = Vec::with_capacity(class.slots.len());
        let mut pending_class_values = Vec::new();
        for slot in &class.slots {
            let initarg_value = slot.initarg.as_ref().and_then(|initarg| {
                initargs
                    .iter()
                    .rev()
                    .find(|(name, _)| name == initarg)
                    .map(|(_, value)| value.clone())
            });
            let value = if let Some(initarg_value) = initarg_value {
                initarg_value
            } else if let Some(class_value) = &slot.class_value {
                let current = class_value.borrow().clone();
                if matches!(current, Value::Unbound) {
                    let value = slot
                        .init_form
                        .as_ref()
                        .map(|form| self.eval_in(form, environment))
                        .transpose()?
                        .unwrap_or(Value::Unbound);
                    pending_class_values.push((class_value.clone(), value.clone()));
                    value
                } else {
                    current
                }
            } else {
                slot.init_form
                    .as_ref()
                    .map(|form| self.eval_in(form, environment))
                    .transpose()?
                    .unwrap_or(Value::Unbound)
            };
            self.validate_slot_value(slot, &value, span)?;
            slots.push((slot.name.clone(), value));
        }
        let instance = Value::instance(class.clone(), slots);
        let mut pending_instance_values = Vec::new();
        for (initarg, value) in initargs {
            let Some(index) = class
                .slots
                .iter()
                .position(|slot| slot.initarg.as_deref() == Some(initarg.as_str()))
            else {
                return Err(self.invalid("unknown make-instance initarg", span));
            };
            self.validate_slot_value(&class.slots[index], &value, span)?;
            if let Some(class_value) = &class.slots[index].class_value {
                pending_class_values.push((class_value.clone(), value));
            } else {
                pending_instance_values.push((class.slots[index].name.clone(), value));
            }
        }
        for (class_value, value) in pending_class_values {
            *class_value.borrow_mut() = value;
        }
        for (slot_name, value) in pending_instance_values {
            if !instance.set_instance_slot(&class.name, &slot_name, value) {
                return Err(self.invalid("unknown make-instance initarg", span));
            }
        }
        Ok(instance)
    }

    fn compile_function(
        &self,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if !(1..=2).contains(&arguments.len()) {
            return Err(self.arity("compile", "one or two", arguments.len()));
        }

        let name = match &arguments[0] {
            Value::Nil | Value::Boolean(false) => None,
            value => {
                let (name, exact) = value
                    .symbol_reference()
                    .ok_or_else(|| self.invalid("compile name must be a symbol or NIL", span))?;
                Some((name.to_owned(), exact))
            }
        };

        let function = match arguments.get(1) {
            None | Some(Value::Nil) | Some(Value::Boolean(false)) => {
                let Some((name, exact)) = name.as_ref() else {
                    return Err(self.invalid(
                        "compile needs a function definition when the name is NIL",
                        span,
                    ));
                };
                let function = if *exact {
                    self.lookup_function_exact_in(name, environment)
                } else {
                    self.lookup_function_in(name, environment)
                };
                match function {
                    Some(value @ Value::Function(_)) => value,
                    Some(value) => {
                        return Err(RuntimeError::NotCallable {
                            value: value.to_string(),
                            span: Some(span),
                        });
                    }
                    None => {
                        return Err(RuntimeError::UnboundVariable {
                            name: name.clone(),
                            span: Some(span),
                        });
                    }
                }
            }
            Some(definition) => {
                let form = self.form_from_value(definition, span)?;
                let expanded = self.prepare_compiled_form(&form, environment)?;
                let program = Rc::new(Compiler::compile_form(&expanded)?);
                crate::vm::run_entry(self, program, 0, environment.clone(), expanded.span)?
                    .primary_value()
            }
        };

        if !matches!(function, Value::Function(_)) {
            return Err(RuntimeError::Type {
                expected: "FUNCTION".to_owned(),
                actual: function.type_name().to_owned(),
                span: Some(span),
            });
        }

        if let Some((name, exact)) = name {
            if exact {
                environment.define_function_exact(name, function.clone());
            } else {
                environment.define_function(name, function.clone());
            }
        }

        Ok(Value::values(vec![function, Value::Nil, Value::Nil]))
    }

    fn load_file(&self, arguments: &[Value], span: Span) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 {
            return Err(self.arity("load", "one", arguments.len()));
        }
        let path = match &arguments[0] {
            Value::String(path) => path.to_string(),
            value => {
                return Err(RuntimeError::Type {
                    expected: "PATHNAME-DESIGNATOR".to_owned(),
                    actual: value.type_name().to_owned(),
                    span: Some(span),
                });
            }
        };
        let source = fs::read_to_string(&path)
            .map_err(|error| RuntimeError::Io(format!("cannot load {}: {}", path, error)))?;
        self.eval_source(&source)?;
        Ok(Value::boolean(true))
    }

    fn condition_format_control(value: &Value) -> Option<String> {
        match value {
            Value::String(control) => Some(control.to_string()),
            _ => None,
        }
    }

    fn condition_message(
        value: &Value,
        arguments: &[Value],
        span: Span,
    ) -> Result<String, RuntimeError> {
        match value {
            Value::String(control) => builtins::format_control(control, arguments),
            value if arguments.is_empty() => Ok(value.to_string()),
            value => Err(RuntimeError::Type {
                expected: "a string format control".to_owned(),
                actual: value.type_name().to_owned(),
                span: Some(span),
            }),
        }
    }

    fn signaled_error(
        condition: &str,
        condition_types: Vec<String>,
        message: String,
        format_control: Option<String>,
        format_arguments: &[Value],
        warning: bool,
        span: Span,
    ) -> RuntimeError {
        RuntimeError::Signaled {
            condition: normalize_name(condition).trim_start_matches(':').to_owned(),
            condition_types,
            message,
            format_control,
            format_arguments: format_arguments
                .iter()
                .cloned()
                .map(ReturnValue::new)
                .collect(),
            warning,
            span: Some(span),
        }
    }

    fn condition_error(
        value: &Value,
        warning: bool,
        span: Span,
    ) -> Result<RuntimeError, RuntimeError> {
        let Some(condition) = value.condition_type_name() else {
            return Err(RuntimeError::Type {
                expected: "CONDITION".to_owned(),
                actual: value.type_name().to_owned(),
                span: Some(span),
            });
        };
        let message = value.condition_message().unwrap_or_default().to_owned();
        let format_control = value
            .simple_condition_format_control()
            .map(ToOwned::to_owned);
        let format_arguments = value
            .simple_condition_format_arguments()
            .unwrap_or_default();
        Ok(Self::signaled_error(
            condition,
            value.condition_type_names().unwrap_or_default(),
            message,
            format_control,
            &format_arguments,
            warning,
            span,
        ))
    }

    fn dispatch_condition(
        &self,
        error: RuntimeError,
        condition: &Value,
        environment: &Environment,
        span: Span,
    ) -> Result<(), RuntimeError> {
        let Some(binding) = self
            .condition_handlers()
            .into_iter()
            .rev()
            .find(|handler| error.matches_condition(&handler.condition))
        else {
            return Ok(());
        };
        if binding.catch {
            return Err(error);
        }
        let Some(function) = binding.function else {
            return Ok(());
        };
        let result = if let Some(suspension) = self.suspend_condition_handler(&binding.condition) {
            let result = self.apply_in(
                &function,
                std::slice::from_ref(condition),
                span,
                environment,
            );
            drop(suspension);
            result
        } else {
            self.apply_in(
                &function,
                std::slice::from_ref(condition),
                span,
                environment,
            )
        };
        result.map(|_| ())
    }

    fn signal_condition_value(
        &self,
        condition: &Value,
        warning: bool,
        environment: &Environment,
        span: Span,
    ) -> Result<(), RuntimeError> {
        let error = Self::condition_error(condition, warning, span)?;
        self.dispatch_condition(error, condition, environment, span)
    }

    fn signal_condition(
        &self,
        condition: &str,
        message: String,
        format_control: Option<String>,
        format_arguments: &[Value],
        warning: bool,
        environment: &Environment,
        span: Span,
    ) -> Result<(), RuntimeError> {
        let error = Self::signaled_error(
            condition,
            Vec::new(),
            message,
            format_control,
            format_arguments,
            warning,
            span,
        );
        let condition_value = Value::condition(&error);
        self.dispatch_condition(error, &condition_value, environment, span)
    }

    fn apply_standard_input_primitive(
        &self,
        name: &str,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let stream = self
            .lookup_symbol_value_in("*STANDARD-INPUT*", environment)
            .ok_or_else(|| self.invalid("*STANDARD-INPUT* is unbound", span))?;
        match name {
            "READ" => {
                let features = self.reader_features_for(environment, span)?;
                builtins::read_with_standard_input(arguments, &stream, &features)
            }
            "READ-PRESERVING-WHITESPACE" => {
                let features = self.reader_features_for(environment, span)?;
                builtins::read_preserving_whitespace_with_standard_input(
                    arguments, &stream, &features,
                )
            }
            "READ-CHAR" => builtins::read_char_with_standard_input(arguments, &stream),
            "PEEK-CHAR" => builtins::peek_char_with_standard_input(arguments, &stream),
            "UNREAD-CHAR" => builtins::unread_char_with_standard_input(arguments, &stream),
            "LISTEN" => builtins::listen_with_standard_input(arguments, &stream),
            "CLEAR-INPUT" => builtins::clear_input_with_standard_input(arguments, &stream),
            "READ-LINE" => builtins::read_line_with_standard_input(arguments, &stream),
            "READ-SEQUENCE" => builtins::read_sequence_with_standard_input(arguments, &stream),
            _ => unreachable!(),
        }
    }

    fn apply_primitive(
        &self,
        name: &str,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        match name {
            "FUNCALL" => {
                if arguments.is_empty() {
                    return Err(self.arity("funcall", "at least one", arguments.len()));
                }
                self.apply_in(&arguments[0], &arguments[1..], span, environment)
            }
            "APPLY" => {
                if arguments.len() < 2 {
                    return Err(self.arity("apply", "at least two", arguments.len()));
                }
                let Some(last) = arguments.last() else {
                    return Err(self.invalid("apply needs a final list", span));
                };
                let Some(mut final_arguments) = last.list_items() else {
                    return Err(self.invalid("apply's final argument must be a list", span));
                };
                let mut applied_arguments = arguments[1..arguments.len() - 1].to_vec();
                applied_arguments.append(&mut final_arguments);
                self.apply_in(&arguments[0], &applied_arguments, span, environment)
            }
            "READ"
            | "READ-PRESERVING-WHITESPACE"
            | "READ-CHAR"
            | "PEEK-CHAR"
            | "UNREAD-CHAR"
            | "LISTEN"
            | "CLEAR-INPUT"
            | "READ-LINE"
            | "READ-SEQUENCE" => {
                self.apply_standard_input_primitive(name, arguments, environment, span)
            }
            "ERROR" => {
                if arguments.is_empty() {
                    return Err(self.arity("error", "at least one", arguments.len()));
                }
                if arguments[0].condition_type_name().is_some() {
                    let error = Self::condition_error(&arguments[0], false, span)?;
                    return match self.dispatch_condition(
                        error.clone(),
                        &arguments[0],
                        environment,
                        span,
                    ) {
                        Ok(()) => Err(error),
                        Err(error) => Err(error),
                    };
                }
                let format_arguments = &arguments[1..];
                let format_control = Self::condition_format_control(&arguments[0]);
                let message = Self::condition_message(&arguments[0], format_arguments, span)?;
                let error = Self::signaled_error(
                    "SIMPLE-ERROR",
                    Vec::new(),
                    message.clone(),
                    format_control.clone(),
                    format_arguments,
                    false,
                    span,
                );
                match self.signal_condition(
                    "SIMPLE-ERROR",
                    message.clone(),
                    format_control,
                    format_arguments,
                    false,
                    environment,
                    span,
                ) {
                    Ok(()) => Err(error),
                    Err(error) => Err(error),
                }
            }
            "SIGNAL" => {
                if arguments.is_empty() {
                    return Err(self.arity("signal", "at least one", arguments.len()));
                }
                if arguments[0].condition_type_name().is_some() {
                    if arguments.len() != 1 {
                        return Err(self.invalid(
                            "signal does not accept format arguments with a condition object",
                            span,
                        ));
                    }
                    self.signal_condition_value(&arguments[0], false, environment, span)?;
                    return Ok(Value::Nil);
                }
                let format_arguments = &arguments[1..];
                let format_control = Self::condition_format_control(&arguments[0]);
                self.signal_condition(
                    "SIMPLE-CONDITION",
                    Self::condition_message(&arguments[0], format_arguments, span)?,
                    format_control,
                    format_arguments,
                    false,
                    environment,
                    span,
                )?;
                Ok(Value::Nil)
            }
            "WARN" => {
                if arguments.is_empty() {
                    return Err(self.arity("warn", "at least one", arguments.len()));
                }
                if arguments[0].condition_type_name().is_some() {
                    if arguments.len() != 1 {
                        return Err(self.invalid(
                            "warn does not accept format arguments with a condition object",
                            span,
                        ));
                    }
                    self.signal_condition_value(&arguments[0], true, environment, span)?;
                    return Ok(Value::Nil);
                }
                let format_arguments = &arguments[1..];
                let format_control = Self::condition_format_control(&arguments[0]);
                self.signal_condition(
                    "SIMPLE-WARNING",
                    Self::condition_message(&arguments[0], format_arguments, span)?,
                    format_control,
                    format_arguments,
                    true,
                    environment,
                    span,
                )?;
                Ok(Value::Nil)
            }
            "CERROR" => {
                if arguments.len() < 2 {
                    return Err(self.arity("cerror", "at least two", arguments.len()));
                }
                let format_arguments = &arguments[2..];
                let _continue_message =
                    Self::condition_message(&arguments[0], format_arguments, span)?;
                let condition_object = arguments[1].condition_type_name().is_some();
                if condition_object && !format_arguments.is_empty() {
                    return Err(self.invalid(
                        "cerror does not accept format arguments with a condition object",
                        span,
                    ));
                }
                let format_control = Self::condition_format_control(&arguments[1]);
                let message = Self::condition_message(&arguments[1], format_arguments, span)?;
                let signal_result = if condition_object {
                    let error = Self::condition_error(&arguments[1], false, span)?;
                    self.dispatch_condition(error, &arguments[1], environment, span)
                } else {
                    self.signal_condition(
                        "SIMPLE-ERROR",
                        message.clone(),
                        format_control,
                        format_arguments,
                        false,
                        environment,
                        span,
                    )
                };
                match signal_result {
                    Ok(()) => {}
                    Err(error @ RuntimeError::InvokeRestart { .. }) => {
                        let RuntimeError::InvokeRestart { name, .. } = &error else {
                            unreachable!()
                        };
                        if normalize_name(name) == "CONTINUE" {
                            return Ok(Value::Nil);
                        }
                        return Err(error);
                    }
                    Err(error) => return Err(error),
                }
                if self
                    .restart_bindings()
                    .iter()
                    .any(|binding| normalize_name(&binding.name) == "CONTINUE")
                {
                    self.invoke_restart_named("CONTINUE", &[], environment, span)
                } else {
                    Err(RuntimeError::InvalidForm {
                        message,
                        span: Some(span),
                    })
                }
            }
            "MAKE-CONDITION" => conditions::make_condition(self, arguments, span, environment),
            "EVAL" => {
                if !(1..=2).contains(&arguments.len()) {
                    return Err(self.arity("eval", "one or two", arguments.len()));
                }
                let form = self.form_from_value(&arguments[0], span)?;
                let target_environment = match arguments.get(1) {
                    None => environment.clone(),
                    Some(Value::Environment(environment)) => environment.clone(),
                    Some(value) => {
                        return Err(RuntimeError::Type {
                            expected: "ENVIRONMENT".to_string(),
                            actual: value.type_name().to_string(),
                            span: Some(span),
                        });
                    }
                };
                self.eval_values_in(&form, &target_environment)
            }
            "COMPILE" => self.compile_function(arguments, environment, span),
            "LOAD" => self.load_file(arguments, span),
            "MAKE-INSTANCE" => self.make_instance(arguments, environment, span),
            "SLOT-VALUE" => {
                if arguments.len() != 2 {
                    return Err(self.arity("slot-value", "two", arguments.len()));
                }
                let slot_name = self.slot_name_from_value(&arguments[1], span)?;
                if !matches!(arguments[0], Value::Instance(_)) {
                    return Err(RuntimeError::Type {
                        expected: "STANDARD-OBJECT".to_owned(),
                        actual: arguments[0].type_name().to_string(),
                        span: Some(span),
                    });
                }
                let value = arguments[0]
                    .instance_slot(&slot_name)
                    .ok_or_else(|| self.invalid("slot is not defined for this class", span))?;
                if matches!(value, Value::Unbound) {
                    return Err(self.invalid("slot is unbound", span));
                }
                Ok(value)
            }
            "SUBTYPEP" => {
                if arguments.len() != 2 {
                    return Err(self.arity("subtypep", "two", arguments.len()));
                }
                builtins::subtypep_value(&arguments[0], &arguments[1], environment)
            }
            "CLASS-OF" => {
                if arguments.len() != 1 {
                    return Err(self.arity("class-of", "one", arguments.len()));
                }
                let class = match &arguments[0] {
                    Value::Instance(instance) => instance.class.clone(),
                    value => {
                        let name = value.type_name().to_owned();
                        Rc::new(ClassDefinition {
                            name: name.clone(),
                            direct_superclasses: Vec::new(),
                            precedence: vec![name, "STANDARD-OBJECT".to_owned()],
                            slots: Vec::new(),
                            default_initargs: Vec::new(),
                            documentation: None,
                        })
                    }
                };
                Ok(Value::class_object(class))
            }
            "FIND-CLASS" => {
                if arguments.len() != 1 {
                    return Err(self.arity("find-class", "one", arguments.len()));
                }
                let class_name = self.name_designator_from_value(&arguments[0], span)?;
                let class = environment
                    .lookup_class(&class_name)
                    .ok_or_else(|| self.invalid("unknown class", span))?;
                Ok(Value::class_object(class))
            }
            "CLASS-NAME" => {
                if arguments.len() != 1 {
                    return Err(self.arity("class-name", "one", arguments.len()));
                }
                let Value::Class(class) = &arguments[0] else {
                    return Err(RuntimeError::Type {
                        expected: "CLASS".to_owned(),
                        actual: arguments[0].type_name().to_string(),
                        span: Some(span),
                    });
                };
                Ok(Value::symbol(class.name.clone()))
            }
            "CLASS-PRECEDENCE-LIST" => {
                if arguments.len() != 1 {
                    return Err(self.arity("class-precedence-list", "one", arguments.len()));
                }
                let Value::Class(class) = &arguments[0] else {
                    return Err(RuntimeError::Type {
                        expected: "CLASS".to_owned(),
                        actual: arguments[0].type_name().to_string(),
                        span: Some(span),
                    });
                };
                let precedence = class
                    .precedence
                    .iter()
                    .map(|class_name| {
                        let definition = environment.lookup_class(class_name).unwrap_or_else(|| {
                            Rc::new(ClassDefinition {
                                name: class_name.clone(),
                                direct_superclasses: Vec::new(),
                                precedence: vec![class_name.clone()],
                                slots: Vec::new(),
                                default_initargs: Vec::new(),
                                documentation: None,
                            })
                        });
                        Value::class_object(definition)
                    })
                    .collect();
                Ok(Value::list(precedence))
            }
            "SLOT-EXISTS-P" => {
                if arguments.len() != 2 {
                    return Err(self.arity("slot predicate", "two", arguments.len()));
                }
                let slot_name = self.slot_name_from_value(&arguments[1], span)?;
                if !matches!(arguments[0], Value::Instance(_)) {
                    return Err(RuntimeError::Type {
                        expected: "STANDARD-OBJECT".to_owned(),
                        actual: arguments[0].type_name().to_string(),
                        span: Some(span),
                    });
                }
                Ok(Value::boolean(
                    arguments[0].instance_slot_exists(&slot_name),
                ))
            }
            "SLOT-BOUNDP" => {
                if arguments.len() != 2 {
                    return Err(self.arity("slot predicate", "two", arguments.len()));
                }
                let slot_name = self.slot_name_from_value(&arguments[1], span)?;
                if !matches!(arguments[0], Value::Instance(_)) {
                    return Err(RuntimeError::Type {
                        expected: "STANDARD-OBJECT".to_owned(),
                        actual: arguments[0].type_name().to_string(),
                        span: Some(span),
                    });
                }
                Ok(Value::boolean(
                    arguments[0]
                        .instance_slot_is_bound(&slot_name)
                        .unwrap_or(false),
                ))
            }
            "SLOT-MAKUNBOUND" => {
                if arguments.len() != 2 {
                    return Err(self.arity("slot-makunbound", "two", arguments.len()));
                }
                let slot_name = self.slot_name_from_value(&arguments[1], span)?;
                let Some(class) = arguments[0].instance_class_definition() else {
                    return Err(RuntimeError::Type {
                        expected: "STANDARD-OBJECT".to_owned(),
                        actual: arguments[0].type_name().to_string(),
                        span: Some(span),
                    });
                };
                if !arguments[0].instance_slot_exists(&slot_name)
                    || !arguments[0].set_instance_slot(&class.name, &slot_name, Value::Unbound)
                {
                    return Err(self.invalid("slot is not defined for this class", span));
                }
                Ok(arguments[0].clone())
            }
            "CALL-NEXT-METHOD" => {
                let (continuation, default_arguments) = {
                    let contexts = self.method_context.borrow();
                    let Some(context) = contexts.last() else {
                        return Err(
                            self.invalid("call-next-method is only available in a method", span)
                        );
                    };
                    (context.next.clone(), context.arguments.clone())
                };
                let Some(continuation) = continuation else {
                    return Err(self.invalid("no next method is applicable", span));
                };
                let next_arguments = if arguments.is_empty() {
                    default_arguments
                } else {
                    arguments.to_vec()
                };
                self.invoke_continuation(continuation, &next_arguments, span, environment)
            }
            "NEXT-METHOD-P" => {
                if !arguments.is_empty() {
                    return Err(self.arity("next-method-p", "zero", arguments.len()));
                }
                let has_next = self
                    .method_context
                    .borrow()
                    .last()
                    .and_then(|context| context.next.as_ref())
                    .is_some();
                Ok(Value::boolean(has_next))
            }
            "MAKE-SYMBOL" => {
                if arguments.len() != 1 {
                    return Err(self.arity("make-symbol", "one", arguments.len()));
                }
                let Some(Value::String(name)) = arguments.first() else {
                    return Err(self.invalid("make-symbol argument must be a string", span));
                };
                Ok(Value::uninterned_symbol(name.as_ref()))
            }
            "GENSYM" => {
                if arguments.len() > 1 {
                    return Err(self.arity("gensym", "zero or one", arguments.len()));
                }
                let prefix = match arguments.first() {
                    None => "G".to_string(),
                    Some(Value::String(value)) => value.to_string(),
                    Some(value) => value
                        .symbol_name()
                        .map(|name| name.to_string())
                        .ok_or_else(|| {
                            self.invalid("gensym prefix must be a string designator", span)
                        })?,
                };
                let counter = self.gensym_counter.get();
                self.gensym_counter.set(counter.wrapping_add(1));
                Ok(Value::uninterned_symbol(format!("{prefix}{counter}")))
            }
            "INTERN" => {
                if !(1..=2).contains(&arguments.len()) {
                    return Err(self.arity("intern", "one or two", arguments.len()));
                }
                let symbol_name = self.symbol_name_from_value_exact(&arguments[0], span)?;
                let package_name = arguments
                    .get(1)
                    .map(|value| self.package_name_from_value(value, span))
                    .transpose()?
                    .unwrap_or_else(|| self.current_package());
                let (status, inserted) = match self
                    .packages
                    .borrow_mut()
                    .intern_symbol_exact(&package_name, &symbol_name)
                {
                    Some(result) => result,
                    None => {
                        return Err(
                            self.package_error(&format!("unknown package {package_name}"), span)
                        );
                    }
                };
                let symbol = self.package_symbol_value_exact(&package_name, &symbol_name);
                Ok(Value::values(vec![
                    symbol,
                    if inserted {
                        Value::Nil
                    } else {
                        Self::symbol_status_value(status)
                    },
                ]))
            }
            "FIND-SYMBOL" => {
                if !(1..=2).contains(&arguments.len()) {
                    return Err(self.arity("find-symbol", "one or two", arguments.len()));
                }
                let symbol_name = self.symbol_name_from_value_exact(&arguments[0], span)?;
                let package_name = arguments
                    .get(1)
                    .map(|value| self.package_name_from_value(value, span))
                    .transpose()?
                    .unwrap_or_else(|| self.current_package());
                let status = self
                    .packages
                    .borrow()
                    .symbol_status_exact(&package_name, &symbol_name);
                match status {
                    Some(status) => {
                        let symbol = self.package_symbol_value_exact(&package_name, &symbol_name);
                        Ok(Value::values(vec![
                            symbol,
                            Self::symbol_status_value(status),
                        ]))
                    }
                    None => Ok(Value::values(vec![Value::Nil, Value::Nil])),
                }
            }
            "FIND-ALL-SYMBOLS" => {
                if arguments.len() != 1 {
                    return Err(self.arity("find-all-symbols", "one", arguments.len()));
                }
                let symbol_name = self.symbol_name_from_value_exact(&arguments[0], span)?;
                let symbols = self.packages.borrow().find_all_symbols(&symbol_name);
                Ok(Value::list(
                    symbols
                        .into_iter()
                        .map(|reference| {
                            if reference.package() == package::KEYWORD_PACKAGE {
                                Value::keyword_exact(reference.name())
                            } else {
                                Value::qualified_symbol_exact(
                                    reference.package(),
                                    reference.name(),
                                )
                            }
                        })
                        .collect(),
                ))
            }
            "MAKE-PACKAGE" => self.make_package(arguments, span),
            "DELETE-PACKAGE" => {
                if arguments.len() != 1 {
                    return Err(self.arity("delete-package", "one", arguments.len()));
                }
                let package_name = self.package_name_from_value(&arguments[0], span)?;
                let current = self.active_package_name();
                let result = self
                    .packages
                    .borrow_mut()
                    .delete_package(&package_name, &current);
                result.map_err(|message| self.package_error(&message, span))?;
                Ok(Value::boolean(true))
            }
            "RENAME-PACKAGE" => self.rename_package(arguments, span),
            "FIND-PACKAGE" => {
                if arguments.len() != 1 {
                    return Err(self.arity("find-package", "one", arguments.len()));
                }
                let package = self.package_designator_name(&arguments[0], span)?;
                let is_package_object = matches!(&arguments[0], Value::Package(_));
                let packages = self.packages.borrow();
                let package_name = if is_package_object {
                    packages.package_object_name(&package)
                } else {
                    packages.canonical_package_name(&package)
                };
                if packages.package_exists(&package_name) {
                    if is_package_object {
                        Ok(arguments[0].clone())
                    } else {
                        Ok(Value::package(package_name))
                    }
                } else {
                    Ok(Value::Nil)
                }
            }
            "PACKAGE-NAME" => {
                if arguments.len() != 1 {
                    return Err(self.arity("package-name", "one", arguments.len()));
                }
                match &arguments[0] {
                    Value::Package(package) => Ok(Value::string(
                        self.packages
                            .borrow()
                            .package_object_name(package.as_ref()),
                    )),
                    other => Err(RuntimeError::Type {
                        expected: "PACKAGE".to_string(),
                        actual: other.type_name().to_string(),
                        span: Some(span),
                    }),
                }
            }
            "PACKAGE-USE-LIST" => {
                if arguments.len() != 1 {
                    return Err(self.arity("package-use-list", "one", arguments.len()));
                }
                match &arguments[0] {
                    Value::Package(package) => {
                        let package_name = self
                            .packages
                            .borrow()
                            .package_object_name(package.as_ref());
                        let names = self.packages.borrow().use_packages_for(&package_name);
                        Ok(Value::list(names.into_iter().map(Value::package).collect()))
                    }
                    other => Err(RuntimeError::Type {
                        expected: "PACKAGE".to_string(),
                        actual: other.type_name().to_string(),
                        span: Some(span),
                    }),
                }
            }
            "PACKAGE-NICKNAMES" => {
                if arguments.len() != 1 {
                    return Err(self.arity("package-nicknames", "one", arguments.len()));
                }
                match &arguments[0] {
                    Value::Package(package) => {
                        let package_name = self
                            .packages
                            .borrow()
                            .package_object_name(package.as_ref());
                        let names = self.packages.borrow().nicknames_for(&package_name);
                        Ok(Value::list(
                            names
                                .into_iter()
                                .map(|name| Value::string(name.as_str()))
                                .collect(),
                        ))
                    }
                    other => Err(RuntimeError::Type {
                        expected: "PACKAGE".to_string(),
                        actual: other.type_name().to_string(),
                        span: Some(span),
                    }),
                }
            }
            "PACKAGE-SHADOWING-SYMBOLS" => {
                if arguments.len() != 1 {
                    return Err(self.arity("package-shadowing-symbols", "one", arguments.len()));
                }
                match &arguments[0] {
                    Value::Package(package) => {
                        let package_name = self
                            .packages
                            .borrow()
                            .package_object_name(package.as_ref());
                        let names = self
                            .packages
                            .borrow()
                            .shadowing_symbols_for(&package_name);
                        Ok(Value::list(
                            names
                                .into_iter()
                                .map(|name| self.package_symbol_value(&package_name, &name))
                                .collect(),
                        ))
                    }
                    other => Err(RuntimeError::Type {
                        expected: "PACKAGE".to_string(),
                        actual: other.type_name().to_string(),
                        span: Some(span),
                    }),
                }
            }
            "PACKAGE-USED-BY-LIST" => {
                if arguments.len() != 1 {
                    return Err(self.arity("package-used-by-list", "one", arguments.len()));
                }
                match &arguments[0] {
                    Value::Package(package) => {
                        let package_name = self
                            .packages
                            .borrow()
                            .package_object_name(package.as_ref());
                        let names = self
                            .packages
                            .borrow()
                            .used_by_packages_for(&package_name);
                        Ok(Value::list(names.into_iter().map(Value::package).collect()))
                    }
                    other => Err(RuntimeError::Type {
                        expected: "PACKAGE".to_string(),
                        actual: other.type_name().to_string(),
                        span: Some(span),
                    }),
                }
            }
            "DOCUMENTATION" => {
                if arguments.len() != 2 {
                    return Err(self.arity("documentation", "two", arguments.len()));
                }
                match &arguments[0] {
                    Value::Package(package) => {
                        let package_name = self
                            .packages
                            .borrow()
                            .package_object_name(package.as_ref());
                        Ok(self
                            .packages
                            .borrow()
                            .package_documentation(&package_name)
                            .map_or(Value::Nil, |documentation| {
                                Value::string(documentation.as_str())
                            }))
                    }
                    Value::Class(class) => {
                        let documentation_type =
                            self.name_designator_from_value(&arguments[1], span)?;
                        if documentation_type != "T" {
                            return Ok(Value::Nil);
                        }
                        Ok(class
                            .documentation
                            .as_deref()
                            .map_or(Value::Nil, Value::string))
                    }
                    Value::Function(function) => {
                        let documentation_type =
                            self.name_designator_from_value(&arguments[1], span)?;
                        if documentation_type != "FUNCTION" {
                            return Ok(Value::Nil);
                        }
                        Ok(function
                            .documentation()
                            .map_or(Value::Nil, |documentation| Value::string(documentation)))
                    }
                    other if other.symbol_reference().is_some() => {
                        let documentation_type =
                            self.name_designator_from_value(&arguments[1], span)?;
                        match documentation_type.as_str() {
                            "FUNCTION" => {
                                let (name, escaped) =
                                    self.name_designator_info_from_value(other, span)?;
                                let function = if escaped {
                                    self.lookup_function_exact_in(&name, environment)
                                } else {
                                    self.lookup_function_in(&name, environment)
                                };
                                Ok(function
                                    .and_then(|value| match value {
                                        Value::Function(function) => {
                                            function.documentation().map(str::to_owned)
                                        }
                                        _ => None,
                                    })
                                    .map_or(Value::Nil, |documentation| {
                                        Value::string(documentation.as_str())
                                    }))
                            }
                            "STRUCTURE" => {
                                let name = self.name_designator_from_value(other, span)?;
                                Ok(environment
                                    .lookup_structure(&name)
                                    .and_then(|definition| definition.documentation)
                                    .map_or(Value::Nil, |documentation| {
                                        Value::string(documentation.as_str())
                                    }))
                            }
                            _ => Ok(Value::Nil),
                        }
                    }
                    other => Err(RuntimeError::Type {
                        expected: "PACKAGE, CLASS, FUNCTION, or SYMBOL".to_string(),
                        actual: other.type_name().to_string(),
                        span: Some(span),
                    }),
                }
            }
            "LIST-ALL-PACKAGES" => {
                if !arguments.is_empty() {
                    return Err(self.arity("list-all-packages", "zero", arguments.len()));
                }
                let names = self.packages.borrow().all_package_names();
                Ok(Value::list(names.into_iter().map(Value::package).collect()))
            }
            "USE-PACKAGE" => {
                if arguments.len() != 1 && arguments.len() != 2 {
                    return Err(self.arity("use-package", "one or two", arguments.len()));
                }
                let packages = self.package_names_from_value(&arguments[0], span)?;
                let target = arguments
                    .get(1)
                    .map(|value| self.package_name_from_value(value, span))
                    .transpose()?
                    .unwrap_or_else(|| self.current_package());
                if packages.iter().any(|package| package == &target) {
                    return Err(self.package_error("a package cannot use itself", span));
                }
                {
                    let mut preview = self.packages.borrow().clone();
                    for package in &packages {
                        if let Some(conflict) = preview.use_package_conflict(package, &target) {
                            return Err(self.package_error(
                                &format!("name conflict for symbol {conflict}"),
                                span,
                            ));
                        }
                        preview.use_package(package, &target);
                    }
                }
                let mut state = self.packages.borrow_mut();
                for package in packages {
                    state.use_package(&package, &target);
                }
                Ok(Value::boolean(true))
            }
            "UNUSE-PACKAGE" => {
                if arguments.len() != 1 && arguments.len() != 2 {
                    return Err(self.arity("unuse-package", "one or two", arguments.len()));
                }
                let packages = self.package_names_from_value(&arguments[0], span)?;
                let target = arguments
                    .get(1)
                    .map(|value| self.package_name_from_value(value, span))
                    .transpose()?
                    .unwrap_or_else(|| self.current_package());
                let mut state = self.packages.borrow_mut();
                for package in packages {
                    state.unuse_package(&package, &target);
                }
                Ok(Value::boolean(true))
            }
            "EXPORT" => {
                if arguments.len() != 1 && arguments.len() != 2 {
                    return Err(self.arity("export", "one or two", arguments.len()));
                }
                let symbols = self.symbol_names_from_value_partitioned(&arguments[0], span)?;
                let package = arguments
                    .get(1)
                    .map(|value| self.package_name_from_value(value, span))
                    .transpose()?
                    .unwrap_or_else(|| self.current_package());
                let (normalized, exact): (Vec<_>, Vec<_>) =
                    symbols.into_iter().partition(|(_, is_exact)| !*is_exact);
                let normalized = normalized
                    .into_iter()
                    .map(|(symbol, _)| symbol)
                    .collect::<Vec<_>>();
                let exact = exact
                    .into_iter()
                    .map(|(symbol, _)| symbol)
                    .collect::<Vec<_>>();
                let mut state = self.packages.borrow_mut();
                state.export_symbols(&package, &normalized);
                state.export_symbols_exact(&package, &exact);
                Ok(Value::boolean(true))
            }
            "UNEXPORT" => {
                if arguments.len() != 1 && arguments.len() != 2 {
                    return Err(self.arity("unexport", "one or two", arguments.len()));
                }
                let symbols = self.symbol_names_from_value_partitioned(&arguments[0], span)?;
                let package = arguments
                    .get(1)
                    .map(|value| self.package_name_from_value(value, span))
                    .transpose()?
                    .unwrap_or_else(|| self.current_package());
                let (normalized, exact): (Vec<_>, Vec<_>) =
                    symbols.into_iter().partition(|(_, is_exact)| !*is_exact);
                let normalized = normalized
                    .into_iter()
                    .map(|(symbol, _)| symbol)
                    .collect::<Vec<_>>();
                let exact = exact
                    .into_iter()
                    .map(|(symbol, _)| symbol)
                    .collect::<Vec<_>>();
                let mut state = self.packages.borrow_mut();
                state.unexport_symbols(&package, &normalized);
                state.unexport_symbols_exact(&package, &exact);
                Ok(Value::boolean(true))
            }
            "IMPORT" | "SHADOWING-IMPORT" => {
                if arguments.len() != 1 && arguments.len() != 2 {
                    return Err(self.arity(name, "one or two", arguments.len()));
                }
                let imports = self.symbol_import_references_from_value(&arguments[0], span)?;
                let target = arguments
                    .get(1)
                    .map(|value| self.package_name_from_value(value, span))
                    .transpose()?
                    .unwrap_or_else(|| self.current_package());
                {
                    let state = self.packages.borrow();
                    for (source_package, source_name, is_exact) in &imports {
                        let exists = if *is_exact {
                            state.symbol_exists_exact(source_package, source_name)
                        } else {
                            state.symbol_exists(source_package, source_name)
                        };
                        if !exists {
                            return Err(self.package_error(
                                &format!("unknown symbol {source_package}::{source_name}"),
                                span,
                            ));
                        }
                    }
                }
                let shadowing = name == "SHADOWING-IMPORT";
                {
                    let mut preview = self.packages.borrow().clone();
                    for (source_package, source_name, is_exact) in &imports {
                        if !shadowing {
                            let conflict = if *is_exact {
                                preview.import_conflict_exact(source_package, source_name, &target)
                            } else {
                                preview.import_conflict(source_package, source_name, &target)
                            };
                            if conflict {
                                return Err(self.package_error(
                                    &format!("name conflict for symbol {source_name}"),
                                    span,
                                ));
                            }
                        }
                        if *is_exact {
                            preview.import_symbol_exact(
                                source_package,
                                source_name,
                                &target,
                                shadowing,
                            );
                        } else {
                            preview.import_symbol(source_package, source_name, &target, shadowing);
                        }
                    }
                }
                let mut state = self.packages.borrow_mut();
                for (source_package, source_name, is_exact) in imports {
                    if is_exact {
                        state.import_symbol_exact(
                            &source_package,
                            &source_name,
                            &target,
                            shadowing,
                        );
                    } else {
                        state.import_symbol(&source_package, &source_name, &target, shadowing);
                    }
                }
                Ok(Value::boolean(true))
            }
            "SHADOW" => {
                if arguments.len() != 1 && arguments.len() != 2 {
                    return Err(self.arity("shadow", "one or two", arguments.len()));
                }
                let symbols = self.symbol_names_from_value_partitioned(&arguments[0], span)?;
                let target = arguments
                    .get(1)
                    .map(|value| self.package_name_from_value(value, span))
                    .transpose()?
                    .unwrap_or_else(|| self.current_package());
                let mut state = self.packages.borrow_mut();
                for (symbol, is_exact) in symbols {
                    if is_exact {
                        state.shadow_symbol_exact(&target, &symbol);
                    } else {
                        state.shadow_symbol(&target, &symbol);
                    }
                }
                Ok(Value::boolean(true))
            }
            "UNINTERN" => {
                if arguments.len() != 1 && arguments.len() != 2 {
                    return Err(self.arity("unintern", "one or two", arguments.len()));
                }
                let symbols = self.symbol_references_from_value_or_single(&arguments[0], span)?;
                let target = arguments
                    .get(1)
                    .map(|value| self.package_name_from_value(value, span))
                    .transpose()?
                    .unwrap_or_else(|| self.current_package());
                let mut removed = false;
                let mut state = self.packages.borrow_mut();
                for (source_package, symbol, is_exact) in symbols {
                    let symbol_removed = state.unintern_symbol_reference(
                        &target,
                        &source_package,
                        &symbol,
                        is_exact,
                    );
                    removed |= symbol_removed;
                }
                Ok(Value::boolean(removed))
            }
            "BOUNDP" => {
                if arguments.len() != 1 {
                    return Err(self.arity("boundp", "one", arguments.len()));
                }
                let (name, exact) = arguments[0]
                    .symbol_reference()
                    .ok_or_else(|| self.invalid("boundp argument must be a symbol", span))?;
                Ok(Value::boolean(if exact {
                    self.is_bound_exact_in(name, environment)
                } else {
                    self.is_bound_in(name, environment)
                }))
            }
            "CONSTANTP" => {
                if arguments.len() != 1 && arguments.len() != 2 {
                    return Err(self.arity("constantp", "one or two", arguments.len()));
                }
                Ok(Value::boolean(self.constantp(&arguments[0])))
            }
            "FBOUNDP" => {
                if arguments.len() != 1 {
                    return Err(self.arity("fboundp", "one", arguments.len()));
                }
                let (name, exact) = arguments[0]
                    .symbol_reference()
                    .ok_or_else(|| self.invalid("fboundp argument must be a symbol", span))?;
                let value = if exact {
                    self.lookup_function_exact_in(name, environment)
                } else {
                    self.lookup_function_in(name, environment)
                };
                Ok(Value::boolean(matches!(value, Some(Value::Function(_)))))
            }
            "MACRO-FUNCTION" => {
                if arguments.len() != 1 && arguments.len() != 2 {
                    return Err(self.arity("macro-function", "one or two", arguments.len()));
                }
                let (name, exact) = arguments[0].symbol_reference().ok_or_else(|| {
                    self.invalid("macro-function argument must be a symbol", span)
                })?;
                let lookup_environment = match arguments.get(1) {
                    None | Some(Value::Nil | Value::Boolean(false)) => &self.global,
                    Some(Value::Environment(environment)) => environment,
                    Some(_) => {
                        return Err(
                            self.invalid("macro-function environment must be an environment", span)
                        );
                    }
                };
                let value = if exact {
                    self.lookup_function_exact_in(name, lookup_environment)
                } else {
                    self.lookup_function_in(name, lookup_environment)
                };
                Ok(match value {
                    Some(Value::Function(function))
                        if matches!(
                            function.as_ref(),
                            crate::Function::Macro { .. } | crate::Function::ModifyMacro { .. }
                        ) =>
                    {
                        Value::Function(function)
                    }
                    _ => Value::Nil,
                })
            }
            "SPECIAL-OPERATOR-P" => {
                if arguments.len() != 1 {
                    return Err(self.arity("special-operator-p", "one", arguments.len()));
                }
                let (name, _) = arguments[0].symbol_reference().ok_or_else(|| {
                    self.invalid("special-operator-p argument must be a symbol", span)
                })?;
                Ok(Value::boolean(is_special_operator_name(name)))
            }
            "COMPILED-FUNCTION-P" => {
                if arguments.len() != 1 {
                    return Err(self.arity("compiled-function-p", "one", arguments.len()));
                }
                Ok(Value::boolean(matches!(
                    &arguments[0],
                    Value::Function(function)
                        if matches!(function.as_ref(), crate::Function::Compiled { .. })
                )))
            }
            "FDEFINITION" => {
                if arguments.len() != 1 {
                    return Err(self.arity("fdefinition", "one", arguments.len()));
                }
                let (name, exact) = arguments[0]
                    .symbol_reference()
                    .ok_or_else(|| self.invalid("fdefinition argument must be a symbol", span))?;
                let value = if exact {
                    self.lookup_function_exact_in(name, environment)
                } else {
                    self.lookup_function_in(name, environment)
                };
                match value {
                    Some(Value::Function(function)) => Ok(Value::Function(function)),
                    Some(value) => Err(RuntimeError::NotCallable {
                        value: value.to_string(),
                        span: Some(span),
                    }),
                    None => Err(RuntimeError::UnboundVariable {
                        name: if exact {
                            name.to_string()
                        } else {
                            normalize_name(name)
                        },
                        span: Some(span),
                    }),
                }
            }
            "SYMBOL-FUNCTION" => {
                if arguments.len() != 1 {
                    return Err(self.arity("symbol-function", "one", arguments.len()));
                }
                let (name, exact) = arguments[0].symbol_reference().ok_or_else(|| {
                    self.invalid("symbol-function argument must be a symbol", span)
                })?;
                let value = if exact {
                    self.lookup_function_exact_in(name, environment)
                } else {
                    self.lookup_function_in(name, environment)
                };
                match value {
                    Some(Value::Function(function)) => Ok(Value::Function(function)),
                    Some(value) => Err(RuntimeError::NotCallable {
                        value: value.to_string(),
                        span: Some(span),
                    }),
                    None => Err(RuntimeError::UnboundVariable {
                        name: if exact {
                            name.to_string()
                        } else {
                            normalize_name(name)
                        },
                        span: Some(span),
                    }),
                }
            }
            "SYMBOL-VALUE" => {
                if arguments.len() != 1 {
                    return Err(self.arity("symbol-value", "one", arguments.len()));
                }
                let (name, exact) = arguments[0]
                    .symbol_reference()
                    .ok_or_else(|| self.invalid("symbol-value argument must be a symbol", span))?;
                let value = if exact {
                    self.lookup_symbol_value_exact_in(name, environment)
                } else {
                    self.lookup_symbol_value_in(name, environment)
                };
                value.ok_or_else(|| RuntimeError::UnboundVariable {
                    name: if exact {
                        name.to_string()
                    } else {
                        normalize_name(name)
                    },
                    span: Some(span),
                })
            }
            "GET" => {
                if !(2..=3).contains(&arguments.len()) {
                    return Err(self.arity("get", "two or three", arguments.len()));
                }
                if arguments[0].symbol_reference().is_none() {
                    return Err(self.invalid("get first argument must be a symbol", span));
                }
                let plist = environment
                    .symbol_plist(&arguments[0])
                    .unwrap_or(Value::Nil);
                let Some(properties) = plist.list_items() else {
                    return Err(RuntimeError::Type {
                        expected: "LIST".to_string(),
                        actual: plist.type_name().to_string(),
                        span: Some(span),
                    });
                };
                if properties.len() % 2 != 0 {
                    return Err(self.invalid("GET needs an even property list", span));
                }
                for index in (0..properties.len()).step_by(2) {
                    if properties[index].eq_value(&arguments[1]) {
                        return Ok(properties[index + 1].clone());
                    }
                }
                Ok(arguments.get(2).cloned().unwrap_or(Value::Nil))
            }
            "PUTPROP" => {
                if arguments.len() != 3 {
                    return Err(self.arity("putprop", "three", arguments.len()));
                }
                if arguments[0].symbol_reference().is_none() {
                    return Err(self.invalid("putprop first argument must be a symbol", span));
                }
                let plist = environment
                    .symbol_plist(&arguments[0])
                    .unwrap_or(Value::Nil);
                let Some(mut properties) = plist.list_items() else {
                    return Err(RuntimeError::Type {
                        expected: "LIST".to_string(),
                        actual: plist.type_name().to_string(),
                        span: Some(span),
                    });
                };
                if properties.len() % 2 != 0 {
                    return Err(self.invalid("PUTPROP needs an even property list", span));
                }
                let mut found = None;
                for index in (0..properties.len()).step_by(2) {
                    if properties[index].eq_value(&arguments[2]) {
                        found = Some(index + 1);
                        break;
                    }
                }
                if let Some(index) = found {
                    properties[index] = arguments[1].clone();
                } else {
                    properties.push(arguments[2].clone());
                    properties.push(arguments[1].clone());
                }
                environment.set_symbol_plist(&arguments[0], Value::list(properties));
                Ok(arguments[1].clone())
            }
            "REMPROP" => {
                if arguments.len() != 2 {
                    return Err(self.arity("remprop", "two", arguments.len()));
                }
                if arguments[0].symbol_reference().is_none() {
                    return Err(self.invalid("remprop first argument must be a symbol", span));
                }
                let plist = environment
                    .symbol_plist(&arguments[0])
                    .unwrap_or(Value::Nil);
                let Some(mut properties) = plist.list_items() else {
                    return Err(RuntimeError::Type {
                        expected: "LIST".to_string(),
                        actual: plist.type_name().to_string(),
                        span: Some(span),
                    });
                };
                if properties.len() % 2 != 0 {
                    return Err(self.invalid("REMPROP needs an even property list", span));
                }
                let Some(index) = (0..properties.len())
                    .step_by(2)
                    .find(|index| properties[*index].eq_value(&arguments[1]))
                else {
                    return Ok(Value::Nil);
                };
                properties.drain(index..index + 2);
                if properties.is_empty() {
                    environment.remove_symbol_property(&arguments[0]);
                } else {
                    environment.set_symbol_plist(&arguments[0], Value::list(properties));
                }
                Ok(Value::boolean(true))
            }
            "SYMBOL-PLIST" => {
                if arguments.len() != 1 {
                    return Err(self.arity("symbol-plist", "one", arguments.len()));
                }
                if arguments[0].symbol_reference().is_none() {
                    return Err(self.invalid("symbol-plist argument must be a symbol", span));
                }
                Ok(environment
                    .symbol_plist(&arguments[0])
                    .unwrap_or(Value::Nil))
            }
            "SET" => {
                if arguments.len() != 2 {
                    return Err(self.arity("set", "two", arguments.len()));
                }
                let (name, exact) = arguments[0]
                    .symbol_reference()
                    .ok_or_else(|| self.invalid("set first argument must be a symbol", span))?;
                self.ensure_symbol_writable(name, exact, span)?;
                Ok(if exact {
                    self.set_symbol_value_exact(name, arguments[1].clone())
                } else {
                    self.set_symbol_value(name, arguments[1].clone())
                })
            }
            "MAKUNBOUND" => {
                if arguments.len() != 1 {
                    return Err(self.arity("makunbound", "one", arguments.len()));
                }
                let (name, exact) = arguments[0]
                    .symbol_reference()
                    .ok_or_else(|| self.invalid("makunbound argument must be a symbol", span))?;
                self.ensure_symbol_writable(name, exact, span)?;
                if exact {
                    self.makunbound_exact_symbol(name);
                } else {
                    self.makunbound_symbol(name);
                }
                Ok(arguments[0].clone())
            }
            "FMAKUNBOUND" => {
                if arguments.len() != 1 {
                    return Err(self.arity("fmakunbound", "one", arguments.len()));
                }
                let (name, exact) = arguments[0]
                    .symbol_reference()
                    .ok_or_else(|| self.invalid("fmakunbound argument must be a symbol", span))?;
                if exact {
                    self.fmakunbound_exact_symbol(name);
                } else {
                    self.fmakunbound_symbol(name);
                }
                Ok(arguments[0].clone())
            }
            "COMPUTE-RESTARTS" => {
                if arguments.len() > 1 {
                    return Err(self.arity("compute-restarts", "at most one", arguments.len()));
                }
                let condition = arguments
                    .first()
                    .filter(|condition| !condition.eq_value(&Value::Nil));
                if let Some(condition) = condition {
                    if condition.condition_type_name().is_none() {
                        return Err(RuntimeError::Type {
                            expected: "CONDITION".to_string(),
                            actual: condition.type_name().to_string(),
                            span: Some(span),
                        });
                    }
                }
                Ok(Value::list(
                    self.restart_bindings_for_condition(condition)
                        .into_iter()
                        .rev()
                        .map(|binding| binding.restart)
                        .collect(),
                ))
            }
            "FIND-RESTART" => {
                if arguments.is_empty() || arguments.len() > 2 {
                    return Err(self.arity("find-restart", "one or two", arguments.len()));
                }
                let condition = arguments
                    .get(1)
                    .filter(|condition| !condition.eq_value(&Value::Nil));
                if let Some(condition) = condition {
                    if condition.condition_type_name().is_none() {
                        return Err(RuntimeError::Type {
                            expected: "CONDITION".to_string(),
                            actual: condition.type_name().to_string(),
                            span: Some(span),
                        });
                    }
                }
                let bindings = self.restart_bindings_for_condition(condition);
                Ok(self
                    .restart_binding_for_designator_in(&arguments[0], &bindings, span)?
                    .map(|binding| binding.restart)
                    .unwrap_or(Value::Nil))
            }
            "RESTART-FUNCTION" => {
                if arguments.len() != 1 {
                    return Err(self.arity("restart-function", "one", arguments.len()));
                }
                Ok(self
                    .restart_binding_for_designator(&arguments[0], span)?
                    .and_then(|binding| binding.function)
                    .unwrap_or(Value::Nil))
            }
            "RESTART-NAME" => {
                if arguments.len() != 1 {
                    return Err(self.arity("restart-name", "one", arguments.len()));
                }
                let Some(name) = arguments[0].restart_name() else {
                    return Err(RuntimeError::Type {
                        expected: "RESTART".to_string(),
                        actual: arguments[0].type_name().to_string(),
                        span: Some(span),
                    });
                };
                Ok(Value::symbol(name))
            }
            "INVOKE-RESTART" => {
                if arguments.is_empty() {
                    return Err(self.arity("invoke-restart", "at least one", arguments.len()));
                }
                if let Some((name, _)) = arguments[0].symbol_reference() {
                    return self.invoke_restart_named(name, &arguments[1..], environment, span);
                }
                let Some(binding) = self.restart_binding_for_designator(&arguments[0], span)?
                else {
                    return Err(self.invalid("restart is not active", span));
                };
                self.invoke_restart_binding(binding, &arguments[1..], environment, span)
            }
            "INVOKE-RESTART-INTERACTIVELY" => {
                if arguments.len() != 1 {
                    return Err(self.arity(
                        "invoke-restart-interactively",
                        "one",
                        arguments.len(),
                    ));
                }
                if let Some((name, _)) = arguments[0].symbol_reference() {
                    return self.invoke_restart_named(name, &[], environment, span);
                }
                let Some(binding) = self.restart_binding_for_designator(&arguments[0], span)?
                else {
                    return Err(self.invalid("restart is not active", span));
                };
                self.invoke_restart_binding(binding, &[], environment, span)
            }
            "MAP" => {
                if arguments.len() < 3 {
                    return Err(self.arity("map", "at least three", arguments.len()));
                }
                self.apply_sequence_mapping(
                    &arguments[0],
                    &arguments[1],
                    &arguments[2..],
                    environment,
                    span,
                )
            }
            "REDUCE" => {
                if arguments.len() < 2 {
                    return Err(self.arity("reduce", "at least two", arguments.len()));
                }
                self.apply_sequence_reduce(
                    &arguments[0],
                    &arguments[1],
                    &arguments[2..],
                    environment,
                    span,
                )
            }
            "REMOVE" | "REMOVE-IF" | "REMOVE-IF-NOT" | "DELETE" | "DELETE-IF" | "DELETE-IF-NOT" => {
                if arguments.len() < 2 {
                    let function_name = name.to_ascii_lowercase();
                    return Err(self.arity(&function_name, "at least two", arguments.len()));
                }
                self.apply_sequence_remove(
                    name,
                    &arguments[0],
                    &arguments[1],
                    &arguments[2..],
                    environment,
                    span,
                )
            }
            "REMOVE-DUPLICATES" | "DELETE-DUPLICATES" => {
                if arguments.is_empty() {
                    let function_name = name.to_ascii_lowercase();
                    return Err(self.arity(&function_name, "at least one", arguments.len()));
                }
                self.apply_sequence_remove(
                    name,
                    &Value::Nil,
                    &arguments[0],
                    &arguments[1..],
                    environment,
                    span,
                )
            }
            "SUBSTITUTE" | "SUBSTITUTE-IF" | "SUBSTITUTE-IF-NOT" | "NSUBSTITUTE"
            | "NSUBSTITUTE-IF" | "NSUBSTITUTE-IF-NOT" => {
                if arguments.len() < 3 {
                    let function_name = name.to_ascii_lowercase();
                    return Err(self.arity(&function_name, "at least three", arguments.len()));
                }
                self.apply_sequence_substitute(
                    name,
                    &arguments[0],
                    &arguments[1],
                    &arguments[2],
                    &arguments[3..],
                    environment,
                    span,
                )
            }
            "UNION" | "NUNION" | "INTERSECTION" | "NINTERSECTION" | "SET-DIFFERENCE"
            | "NSET-DIFFERENCE" | "SET-EXCLUSIVE-OR" | "NSET-EXCLUSIVE-OR" | "SUBSETP" => {
                if arguments.len() < 2 {
                    let function_name = name.to_ascii_lowercase();
                    return Err(self.arity(&function_name, "at least two", arguments.len()));
                }
                self.apply_list_set_operation(
                    name,
                    &arguments[0],
                    &arguments[1],
                    &arguments[2..],
                    environment,
                    span,
                )
            }
            "MEMBER" | "MEMBER-IF" | "MEMBER-IF-NOT" | "ADJOIN" => {
                if arguments.len() < 2 {
                    let function_name = name.to_ascii_lowercase();
                    return Err(self.arity(&function_name, "at least two", arguments.len()));
                }
                self.apply_list_membership(
                    name,
                    &arguments[0],
                    &arguments[1],
                    &arguments[2..],
                    environment,
                    span,
                )
            }
            "ASSOC" | "ASSOC-IF" | "ASSOC-IF-NOT" | "RASSOC" | "RASSOC-IF" | "RASSOC-IF-NOT" => {
                if arguments.len() < 2 {
                    let function_name = name.to_ascii_lowercase();
                    return Err(self.arity(&function_name, "at least two", arguments.len()));
                }
                self.apply_association_search(
                    name,
                    &arguments[0],
                    &arguments[1],
                    &arguments[2..],
                    environment,
                    span,
                )
            }
            "FIND" | "POSITION" | "COUNT" => {
                if arguments.len() < 2 {
                    let function_name = name.to_ascii_lowercase();
                    return Err(self.arity(&function_name, "at least two", arguments.len()));
                }
                self.apply_sequence_search(
                    name,
                    &arguments[0],
                    &arguments[1],
                    &arguments[2..],
                    environment,
                    span,
                )
            }
            "SEARCH" | "MISMATCH" => {
                if arguments.len() < 2 {
                    let function_name = name.to_ascii_lowercase();
                    return Err(self.arity(&function_name, "at least two", arguments.len()));
                }
                self.apply_sequence_pair_search(
                    name,
                    &arguments[0],
                    &arguments[1],
                    &arguments[2..],
                    environment,
                    span,
                )
            }
            "SORT" | "STABLE-SORT" => {
                if arguments.len() < 2 {
                    let function_name = name.to_ascii_lowercase();
                    return Err(self.arity(&function_name, "at least two", arguments.len()));
                }
                self.apply_sequence_sort(
                    name,
                    &arguments[0],
                    &arguments[1],
                    &arguments[2..],
                    environment,
                    span,
                )
            }
            "MERGE" => {
                if arguments.len() < 4 {
                    return Err(self.arity("merge", "at least four", arguments.len()));
                }
                self.apply_sequence_merge(
                    &arguments[0],
                    &arguments[1],
                    &arguments[2],
                    &arguments[3],
                    &arguments[4..],
                    environment,
                    span,
                )
            }
            "EVERY" | "SOME" | "NOTANY" | "NOTEVERY" => {
                if arguments.len() < 2 {
                    let function_name = name.to_ascii_lowercase();
                    return Err(self.arity(&function_name, "at least two", arguments.len()));
                }
                self.apply_sequence_quantifier(
                    name,
                    &arguments[0],
                    &arguments[1..],
                    environment,
                    span,
                )
            }
            "MAP-INTO" => {
                if arguments.len() < 2 {
                    return Err(self.arity("map-into", "at least two", arguments.len()));
                }
                self.apply_sequence_map_into(
                    &arguments[0],
                    &arguments[1],
                    &arguments[2..],
                    environment,
                    span,
                )
            }
            "MAPHASH" => {
                if arguments.len() != 2 {
                    return Err(self.arity("maphash", "two", arguments.len()));
                }
                self.apply_hash_table_mapping(&arguments[0], &arguments[1], environment, span)
            }
            "MAPCAR" | "MAPC" | "MAPL" | "MAPLIST" | "MAPCAN" | "MAPCON" => {
                if arguments.len() < 2 {
                    let function_name = name.to_ascii_lowercase();
                    return Err(self.arity(&function_name, "at least two", arguments.len()));
                }
                self.apply_list_mapping(name, &arguments[0], &arguments[1..], environment, span)
            }
            _ => Err(self.invalid("unknown runtime primitive", span)),
        }
    }

    fn method_score(&self, method: &MethodDefinition, arguments: &[Value]) -> Option<Vec<usize>> {
        let required_count = method.specializers.len();
        if arguments.len() < required_count {
            return None;
        }
        if let Value::Function(function) = &method.function {
            if let crate::Function::Closure {
                parameters,
                optional,
                rest,
                has_keyword_section,
                ..
            } = function.as_ref()
            {
                if parameters.len() != required_count
                    || (!*has_keyword_section
                        && rest.is_none()
                        && arguments.len() > required_count + optional.len())
                {
                    return None;
                }
            }
        }
        let mut score = Vec::with_capacity(required_count);
        for (specializer, argument) in method
            .specializers
            .iter()
            .zip(arguments.iter().take(required_count))
        {
            match specializer {
                MethodSpecializer::Eql(expected) => {
                    if !builtins::eql_value(expected, argument) {
                        return None;
                    }
                    score.push(0);
                    continue;
                }
                MethodSpecializer::Type(specializer) => {
                    if specializer == "T" || specializer == "OBJECT" {
                        score.push(1_000_000);
                        continue;
                    }
                    if let Some(class) = argument.instance_class_definition() {
                        if let Some(position) =
                            class.precedence.iter().position(|name| name == specializer)
                        {
                            score.push(position.saturating_add(1));
                            continue;
                        }
                    }
                    let matches =
                        builtins::typep_value(argument, &Value::symbol(specializer)).ok()?;
                    if !matches {
                        return None;
                    }
                    score.push(builtins::builtin_type_specializer_score(specializer));
                }
            }
        }
        Some(score)
    }

    fn invoke_method(
        &self,
        method: &MethodDefinition,
        arguments: &[Value],
        next: Option<MethodContinuation>,
        span: Span,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        self.method_context.borrow_mut().push(MethodContext {
            arguments: arguments.to_vec(),
            next,
        });
        let result = self.apply_in(&method.function, arguments, span, environment);
        self.method_context.borrow_mut().pop();
        result
    }

    fn invoke_core(
        &self,
        before: &[MethodDefinition],
        primary: &[MethodDefinition],
        after: &[MethodDefinition],
        arguments: &[Value],
        span: Span,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        for method in before {
            self.invoke_method(method, arguments, None, span, environment)?;
        }
        let Some(method) = primary.first() else {
            return Err(self.invalid("no primary method is applicable", span));
        };
        let next = (primary.len() > 1).then(|| MethodContinuation::Chain {
            methods: primary.to_vec(),
            index: 1,
            fallback: None,
        });
        let result = self.invoke_method(method, arguments, next, span, environment)?;
        for method in after {
            self.invoke_method(method, arguments, None, span, environment)?;
        }
        Ok(result)
    }

    fn invoke_continuation(
        &self,
        continuation: MethodContinuation,
        arguments: &[Value],
        span: Span,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        match continuation {
            MethodContinuation::Chain {
                methods,
                index,
                fallback,
            } => {
                if index < methods.len() {
                    let method = methods[index].clone();
                    let next = if index + 1 < methods.len() || fallback.is_some() {
                        Some(MethodContinuation::Chain {
                            methods,
                            index: index + 1,
                            fallback,
                        })
                    } else {
                        None
                    };
                    self.invoke_method(&method, arguments, next, span, environment)
                } else if let Some(fallback) = fallback {
                    self.invoke_continuation(*fallback, arguments, span, environment)
                } else {
                    Err(self.invalid("no next method is applicable", span))
                }
            }
            MethodContinuation::Core {
                before,
                primary,
                after,
            } => self.invoke_core(&before, &primary, &after, arguments, span, environment),
        }
    }

    fn apply_generic(
        &self,
        name: &str,
        methods: &RefCell<Vec<MethodDefinition>>,
        arguments: &[Value],
        span: Span,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        let mut applicable = methods
            .borrow()
            .iter()
            .filter_map(|method| {
                self.method_score(method, arguments)
                    .map(|score| (score, method.clone()))
            })
            .collect::<Vec<_>>();
        if applicable.is_empty() {
            return Err(self.invalid(&format!("no applicable method for {name}"), span));
        }
        applicable.sort_by(|(left, _), (right, _)| left.cmp(right));

        let mut around = Vec::new();
        let mut before = Vec::new();
        let mut primary = Vec::new();
        let mut after = Vec::new();
        for (_, method) in applicable {
            match method.qualifiers.first().map(String::as_str) {
                Some("AROUND") => around.push(method),
                Some("BEFORE") => before.push(method),
                Some("AFTER") => after.push(method),
                _ => primary.push(method),
            }
        }
        after.reverse();
        let core = MethodContinuation::Core {
            before,
            primary,
            after,
        };
        if around.is_empty() {
            self.invoke_continuation(core, arguments, span, environment)
        } else {
            let first = around[0].clone();
            let next = MethodContinuation::Chain {
                methods: around,
                index: 1,
                fallback: Some(Box::new(core)),
            };
            self.invoke_method(&first, arguments, Some(next), span, environment)
        }
    }

    pub(crate) fn apply_in(
        &self,
        function: &Value,
        arguments: &[Value],
        span: Span,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        let function = self.resolve_function_designator(function, span, environment)?;
        match function.as_ref() {
            crate::Function::Builtin { name, function } => {
                match name.as_ref() {
                    "random" => {
                        let state = self.random_state_for(environment, span)?;
                        builtins::random::random_with_state(arguments, &state)
                    }
                    "make-random-state" => {
                        let state = self.random_state_for(environment, span)?;
                        builtins::random::make_random_state_with_state(arguments, &state)
                    }
                    "read-from-string" => {
                        let features = self.reader_features_for(environment, span)?;
                        builtins::read_from_string_with_features(arguments, &features)
                    }
                    _ => function(arguments),
                }
            }
            crate::Function::Primitive { name } => {
                self.apply_primitive(name, arguments, environment, span)
            }
            crate::Function::Generic { name, methods } => {
                self.apply_generic(name, methods, arguments, span, environment)
            }
            crate::Function::SlotReader {
                class_name,
                slot_name,
            } => {
                if arguments.len() != 1 {
                    return Err(self.arity("slot reader", "one", arguments.len()));
                }
                if !arguments[0].instance_is_type(class_name) {
                    return Err(RuntimeError::Type {
                        expected: class_name.clone(),
                        actual: arguments[0].type_name().to_string(),
                        span: Some(span),
                    });
                }
                let value = arguments[0]
                    .instance_slot(slot_name)
                    .ok_or_else(|| self.invalid("slot is not defined for this class", span))?;
                if matches!(value, Value::Unbound) {
                    return Err(self.invalid("slot is unbound", span));
                }
                Ok(value)
            }
            crate::Function::SlotWriter {
                class_name,
                slot_name,
            } => {
                if arguments.len() != 2 {
                    return Err(self.arity("slot writer", "two", arguments.len()));
                }
                let value = arguments[0].clone();
                let object = &arguments[1];
                if !object.instance_is_type(class_name) {
                    return Err(RuntimeError::Type {
                        expected: class_name.clone(),
                        actual: object.type_name().to_string(),
                        span: Some(span),
                    });
                }
                self.validate_instance_slot_value(object, slot_name, &value, span)?;
                if object.set_instance_slot(class_name, slot_name, value.clone()) {
                    Ok(value)
                } else {
                    Err(self.invalid("slot is not defined for this class", span))
                }
            }
            crate::Function::ConditionReader {
                condition_name,
                slot_name,
            } => {
                if arguments.len() != 1 {
                    return Err(self.arity("condition reader", "one", arguments.len()));
                }
                arguments[0]
                    .condition_slot(condition_name, slot_name)
                    .ok_or_else(|| self.invalid("condition slot is not defined", span))
            }
            crate::Function::ConditionWriter {
                condition_name,
                slot_name,
            } => {
                if arguments.len() != 2 {
                    return Err(self.arity("condition writer", "two", arguments.len()));
                }
                let value = arguments[0].clone();
                let object = &arguments[1];
                if object.set_condition_slot(condition_name, slot_name, value.clone()) {
                    Ok(value)
                } else {
                    Err(self.invalid("condition slot is not defined", span))
                }
            }
            crate::Function::StructureConstructor {
                name,
                slots,
                structure_types,
                representation,
                named,
                constructor_lambda_list,
                environment: definition_environment,
            } => {
                if let Some(lambda_list) = constructor_lambda_list {
                    self.apply_structure_boa_constructor(
                        name,
                        slots,
                        structure_types,
                        *representation,
                        *named,
                        lambda_list,
                        definition_environment,
                        arguments,
                        span,
                    )
                } else {
                    if arguments.len() % 2 != 0 {
                        return Err(self.arity(
                            "structure constructor",
                            "an even number of",
                            arguments.len(),
                        ));
                    }
                    let mut supplied = vec![None; slots.len()];
                    for pair in arguments.chunks_exact(2) {
                        let keyword_name = match &pair[0] {
                            Value::Keyword(keyword) | Value::KeywordExact(keyword) => {
                                normalize_name(keyword)
                            }
                            _ => {
                                return Err(self.invalid(
                                    "structure constructor keyword name must be a keyword",
                                    span,
                                ));
                            }
                        };
                        let Some(index) = slots.iter().position(|slot| slot.name == keyword_name)
                        else {
                            return Err(RuntimeError::InvalidForm {
                                message: format!("unknown structure keyword :{keyword_name}"),
                                span: Some(span),
                            });
                        };
                        if supplied[index].is_none() {
                            supplied[index] = Some(pair[1].clone());
                        }
                    }
                    let mut values = Vec::with_capacity(slots.len());
                    for (index, slot) in slots.iter().enumerate() {
                        let value = match supplied[index].clone() {
                            Some(value) => value,
                            None => slot
                                .init_form
                                .as_ref()
                                .map(|form| self.eval_in(form, definition_environment))
                                .transpose()?
                                .unwrap_or(Value::Nil),
                        };
                        values.push((slot.name.clone(), value));
                    }
                    Ok(Value::structure_with_representation(
                        name,
                        values,
                        structure_types.clone(),
                        *representation,
                        *named,
                    ))
                }
            }
            crate::Function::StructurePredicate { name } => {
                if arguments.len() != 1 {
                    return Err(self.arity("structure predicate", "one", arguments.len()));
                }
                Ok(Value::boolean(arguments[0].structure_typep_is_type(name)))
            }
            crate::Function::StructureAccessor {
                structure_name,
                slot_name: _,
                slot_index,
                ..
            } => {
                if arguments.len() != 1 {
                    return Err(self.arity("structure accessor", "one", arguments.len()));
                }
                if !arguments[0].structure_is_type(structure_name) {
                    return Err(RuntimeError::Type {
                        expected: structure_name.clone(),
                        actual: arguments[0].type_name().to_string(),
                        span: Some(span),
                    });
                }
                arguments[0]
                    .structure_slot(*slot_index)
                    .ok_or_else(|| self.invalid("structure slot is out of range", span))
            }
            crate::Function::StructureCopier { name } => {
                if arguments.len() != 1 {
                    return Err(self.arity("structure copier", "one", arguments.len()));
                }
                if !arguments[0].structure_is_type(name) {
                    return Err(RuntimeError::Type {
                        expected: name.clone(),
                        actual: arguments[0].type_name().to_string(),
                        span: Some(span),
                    });
                }
                arguments[0]
                    .copy_structure()
                    .ok_or_else(|| self.invalid("structure copy failed", span))
            }
            crate::Function::Closure {
                parameters,
                required_escaped,
                optional,
                rest,
                rest_escaped,
                keywords,
                has_keyword_section,
                allow_other_keys,
                auxiliary,
                body,
                environment,
                ..
            } => {
                let required_count = parameters.len();
                let optional_count = optional.len();
                let maximum_count = required_count + optional_count;
                if arguments.len() < required_count {
                    let expected = if optional_count > 0 || rest.is_some() || *has_keyword_section {
                        format!("at least {required_count}")
                    } else {
                        required_count.to_string()
                    };
                    return Err(self.arity("closure", &expected, arguments.len()));
                }
                let optional_supplied_count = if *has_keyword_section {
                    let available = arguments
                        .len()
                        .saturating_sub(required_count)
                        .min(optional_count);
                    (0..available)
                        .take_while(|index| {
                            !matches!(
                                arguments[required_count + *index],
                                Value::Keyword(_) | Value::KeywordExact(_)
                            )
                        })
                        .count()
                } else {
                    arguments
                        .len()
                        .saturating_sub(required_count)
                        .min(optional_count)
                };
                let key_start = required_count + optional_supplied_count;
                if !*has_keyword_section && rest.is_none() && arguments.len() > maximum_count {
                    let expected = if optional_count > 0 {
                        format!("at most {maximum_count}")
                    } else {
                        maximum_count.to_string()
                    };
                    return Err(self.arity("closure", &expected, arguments.len()));
                }

                let declared_special_names = self.declared_special_names(body)?;
                let (special_names, special_exact_names) =
                    split_special_names(declared_special_names);
                let _special_guard =
                    self.special_declaration_guard(&special_names, &special_exact_names);
                let local = environment.child();
                let _dynamic_guard = self.dynamic_guard();
                for (index, (parameter, argument)) in
                    parameters.iter().zip(arguments.iter()).enumerate()
                {
                    if required_escaped.get(index).copied().unwrap_or(false) {
                        self.define_exact_in(parameter, argument.clone(), &local);
                    } else {
                        self.define_in(parameter, argument.clone(), &local);
                    }
                }
                for (index, specification) in optional.iter().enumerate() {
                    let supplied = (index < optional_supplied_count)
                        .then(|| &arguments[required_count + index]);
                    let value = match supplied {
                        Some(argument) => argument.clone(),
                        None => self.eval_in(&specification.init_form, &local)?,
                    };
                    if specification.name_escaped {
                        self.define_exact_in(&specification.name, value, &local);
                    } else {
                        self.define_in(&specification.name, value, &local);
                    }
                    if let Some(supplied_p) = &specification.supplied_p {
                        if specification.supplied_p_escaped.unwrap_or(false) {
                            self.define_exact_in(
                                supplied_p,
                                Value::boolean(supplied.is_some()),
                                &local,
                            );
                        } else {
                            self.define_in(supplied_p, Value::boolean(supplied.is_some()), &local);
                        }
                    }
                }
                if let Some(rest) = rest {
                    let rest_start = key_start;
                    let value = Value::list(arguments[rest_start..].to_vec());
                    if *rest_escaped {
                        self.define_exact_in(rest, value, &local);
                    } else {
                        self.define_in(rest, value, &local);
                    }
                }
                if *has_keyword_section {
                    let keyword_arguments = &arguments[key_start..];
                    if keyword_arguments.len() % 2 != 0 {
                        return Err(
                            self.invalid("keyword arguments must be supplied in pairs", span)
                        );
                    }
                    let mut supplied_keywords = HashMap::new();
                    let mut accepts_unknown_keywords = *allow_other_keys;
                    for pair in keyword_arguments.chunks_exact(2) {
                        let keyword = match &pair[0] {
                            Value::Keyword(keyword) | Value::KeywordExact(keyword) => keyword,
                            _ => {
                                return Err(
                                    self.invalid("keyword argument name must be a keyword", span)
                                );
                            }
                        };
                        let keyword_name = keyword.to_string();
                        if keyword_name == "ALLOW-OTHER-KEYS" && pair[1].is_truthy() {
                            accepts_unknown_keywords = true;
                        }
                        supplied_keywords
                            .entry(keyword_name)
                            .or_insert_with(|| pair[1].clone());
                    }
                    if !accepts_unknown_keywords {
                        for keyword_name in supplied_keywords.keys() {
                            if keyword_name != "ALLOW-OTHER-KEYS"
                                && !keywords.iter().any(|specification| {
                                    specification.keyword_name == *keyword_name
                                })
                            {
                                return Err(RuntimeError::InvalidForm {
                                    message: format!("unknown keyword :{keyword_name}"),
                                    span: Some(span),
                                });
                            }
                        }
                    }
                    for specification in keywords {
                        let supplied = supplied_keywords.get(&specification.keyword_name);
                        let value = match supplied {
                            Some(argument) => argument.clone(),
                            None => self.eval_in(&specification.init_form, &local)?,
                        };
                        if specification.name_escaped {
                            self.define_exact_in(&specification.name, value, &local);
                        } else {
                            self.define_in(&specification.name, value, &local);
                        }
                        if let Some(supplied_p) = &specification.supplied_p {
                            if specification.supplied_p_escaped.unwrap_or(false) {
                                self.define_exact_in(
                                    supplied_p,
                                    Value::boolean(supplied.is_some()),
                                    &local,
                                );
                            } else {
                                self.define_in(
                                    supplied_p,
                                    Value::boolean(supplied.is_some()),
                                    &local,
                                );
                            }
                        }
                    }
                }
                for specification in auxiliary {
                    let value = self.eval_in(&specification.init_form, &local)?;
                    if specification.name_escaped {
                        self.define_exact_in(&specification.name, value, &local);
                    } else {
                        self.define_in(&specification.name, value, &local);
                    }
                }
                self.eval_sequence_values(body, &local)
            }
            crate::Function::Macro { .. } | crate::Function::ModifyMacro { .. } => {
                Err(RuntimeError::NotCallable {
                    value: Value::Function(function.clone()).to_string(),
                    span: Some(span),
                })
            }
            crate::Function::Compiled {
                program,
                function,
                environment,
                ..
            } => crate::vm::run(
                self,
                program.clone(),
                *function,
                environment.clone(),
                arguments,
                span,
            ),
        }
    }

    fn apply_structure_boa_constructor(
        &self,
        name: &str,
        slots: &[StructureSlot],
        structure_types: &[String],
        representation: StructureRepresentation,
        named: bool,
        lambda_list: &OrdinaryLambdaList,
        definition_environment: &Environment,
        arguments: &[Value],
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let required_count = lambda_list.required.len();
        let optional_count = lambda_list.optional.len();
        if arguments.len() < required_count {
            let expected = if optional_count > 0
                || lambda_list.rest.is_some()
                || lambda_list.has_keyword_section
            {
                format!("at least {required_count}")
            } else {
                required_count.to_string()
            };
            return Err(self.arity("structure constructor", &expected, arguments.len()));
        }
        let optional_supplied_count = if lambda_list.has_keyword_section {
            let available = arguments
                .len()
                .saturating_sub(required_count)
                .min(optional_count);
            (0..available)
                .take_while(|index| {
                    !matches!(
                        arguments[required_count + *index],
                        Value::Keyword(_) | Value::KeywordExact(_)
                    )
                })
                .count()
        } else {
            arguments
                .len()
                .saturating_sub(required_count)
                .min(optional_count)
        };
        let key_start = required_count + optional_supplied_count;
        if !lambda_list.has_keyword_section
            && lambda_list.rest.is_none()
            && arguments.len() > required_count + optional_count
        {
            let maximum = required_count + optional_count;
            let expected = if optional_count > 0 {
                format!("at most {maximum}")
            } else {
                maximum.to_string()
            };
            return Err(self.arity("structure constructor", &expected, arguments.len()));
        }

        let local = definition_environment.child();
        let _dynamic_guard = self.dynamic_guard();
        let mut slot_values = vec![None; slots.len()];
        let slot_index =
            |parameter_name: &str| slots.iter().position(|slot| slot.name == parameter_name);
        let evaluate_slot_default = |parameter_name: &str| -> Result<Value, RuntimeError> {
            slots
                .iter()
                .find(|slot| slot.name == parameter_name)
                .and_then(|slot| slot.init_form.as_ref())
                .map(|form| self.eval_in(form, definition_environment))
                .transpose()
                .map(|value| value.unwrap_or(Value::Nil))
        };

        for (index, (parameter, argument)) in lambda_list
            .required
            .iter()
            .zip(arguments.iter())
            .enumerate()
        {
            if lambda_list
                .required_escaped
                .get(index)
                .copied()
                .unwrap_or(false)
            {
                self.define_exact_in(parameter, argument.clone(), &local);
            } else {
                self.define_in(parameter, argument.clone(), &local);
            }
            if let Some(slot_index) = slot_index(parameter) {
                slot_values[slot_index] = Some(argument.clone());
            }
        }

        for (index, specification) in lambda_list.optional.iter().enumerate() {
            let supplied =
                (index < optional_supplied_count).then(|| &arguments[required_count + index]);
            let value = match supplied {
                Some(argument) => argument.clone(),
                None if specification.init_form_supplied => {
                    self.eval_in(&specification.init_form, &local)?
                }
                None => evaluate_slot_default(&specification.name)?,
            };
            if specification.name_escaped {
                self.define_exact_in(&specification.name, value.clone(), &local);
            } else {
                self.define_in(&specification.name, value.clone(), &local);
            }
            if let Some(slot_index) = slot_index(&specification.name) {
                slot_values[slot_index] = Some(value);
            }
            if let Some(supplied_p) = &specification.supplied_p {
                let supplied_value = Value::boolean(supplied.is_some());
                if specification.supplied_p_escaped.unwrap_or(false) {
                    self.define_exact_in(supplied_p, supplied_value, &local);
                } else {
                    self.define_in(supplied_p, supplied_value, &local);
                }
            }
        }

        if let Some(rest) = &lambda_list.rest {
            let value = Value::list(arguments[key_start..].to_vec());
            if lambda_list.rest_escaped {
                self.define_exact_in(rest, value.clone(), &local);
            } else {
                self.define_in(rest, value.clone(), &local);
            }
            if let Some(slot_index) = slot_index(rest) {
                slot_values[slot_index] = Some(value);
            }
        }

        if lambda_list.has_keyword_section {
            let keyword_arguments = &arguments[key_start..];
            if keyword_arguments.len() % 2 != 0 {
                return Err(self.invalid("keyword arguments must be supplied in pairs", span));
            }
            let mut supplied_keywords = Vec::new();
            let mut accepts_unknown_keywords = lambda_list.allow_other_keys;
            for pair in keyword_arguments.chunks_exact(2) {
                let (keyword_name, keyword_name_escaped) = match &pair[0] {
                    Value::Keyword(keyword) => (keyword.to_string(), false),
                    Value::KeywordExact(keyword) => (keyword.to_string(), true),
                    _ => return Err(self.invalid("keyword argument name must be a keyword", span)),
                };
                if macro_keyword_matches(
                    "ALLOW-OTHER-KEYS",
                    false,
                    &keyword_name,
                    keyword_name_escaped,
                ) && pair[1].is_truthy()
                {
                    accepts_unknown_keywords = true;
                }
                supplied_keywords.push((keyword_name, keyword_name_escaped, pair[1].clone()));
            }
            let keyword_matches = |specification: &LambdaListKeywordParameter,
                                   actual_name: &str,
                                   actual_name_escaped: bool| {
                macro_keyword_matches(
                    &specification.keyword_name,
                    specification.keyword_name_escaped,
                    actual_name,
                    actual_name_escaped,
                )
            };
            if !accepts_unknown_keywords {
                for (keyword_name, keyword_name_escaped, _) in &supplied_keywords {
                    if !macro_keyword_matches(
                        "ALLOW-OTHER-KEYS",
                        false,
                        keyword_name,
                        *keyword_name_escaped,
                    ) && !lambda_list.keywords.iter().any(|specification| {
                        keyword_matches(specification, keyword_name, *keyword_name_escaped)
                    }) {
                        return Err(RuntimeError::InvalidForm {
                            message: format!("unknown keyword :{keyword_name}"),
                            span: Some(span),
                        });
                    }
                }
            }
            for specification in &lambda_list.keywords {
                let supplied =
                    supplied_keywords
                        .iter()
                        .find(|(keyword_name, keyword_name_escaped, _)| {
                            keyword_matches(specification, keyword_name, *keyword_name_escaped)
                        });
                let value = match supplied {
                    Some((_, _, argument)) => argument.clone(),
                    None if specification.init_form_supplied => {
                        self.eval_in(&specification.init_form, &local)?
                    }
                    None => evaluate_slot_default(&specification.name)?,
                };
                if specification.name_escaped {
                    self.define_exact_in(&specification.name, value.clone(), &local);
                } else {
                    self.define_in(&specification.name, value.clone(), &local);
                }
                if let Some(slot_index) = slot_index(&specification.name) {
                    slot_values[slot_index] = Some(value);
                }
                if let Some(supplied_p) = &specification.supplied_p {
                    let supplied_value = Value::boolean(supplied.is_some());
                    if specification.supplied_p_escaped.unwrap_or(false) {
                        self.define_exact_in(supplied_p, supplied_value, &local);
                    } else {
                        self.define_in(supplied_p, supplied_value, &local);
                    }
                }
            }
        }

        for specification in &lambda_list.auxiliary {
            let value = self.eval_in(&specification.init_form, &local)?;
            if specification.name_escaped {
                self.define_exact_in(&specification.name, value.clone(), &local);
            } else {
                self.define_in(&specification.name, value.clone(), &local);
            }
            if let Some(slot_index) = slot_index(&specification.name) {
                slot_values[slot_index] = Some(value);
            }
        }

        let mut values = Vec::with_capacity(slots.len());
        for (index, slot) in slots.iter().enumerate() {
            let value = match slot_values[index].take() {
                Some(value) => value,
                None => evaluate_slot_default(&slot.name)?,
            };
            values.push((slot.name.clone(), value));
        }
        Ok(Value::structure_with_representation(
            name,
            values,
            structure_types.to_vec(),
            representation,
            named,
        ))
    }

    fn parameters(&self, form: &Form) -> Result<OrdinaryLambdaList, RuntimeError> {
        parse_ordinary_lambda_list(form).map_err(|error| {
            let message = error.kind.to_string();
            self.invalid(&message, error.span)
        })
    }

    fn eval_sequence(
        &self,
        forms: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        self.eval_sequence_values(forms, environment)
            .map(|value| value.primary_value())
    }

    fn eval_sequence_values(
        &self,
        forms: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        let mut result = Value::Nil;
        for form in forms {
            result = self.eval_values_in(form, environment)?;
        }
        Ok(result)
    }

    pub(crate) fn quoted_value(&self, form: &Form) -> Result<Value, RuntimeError> {
        quoted_form_value(form)
    }

    pub(crate) fn form_from_value(&self, value: &Value, span: Span) -> Result<Form, RuntimeError> {
        match value {
            Value::Nil | Value::Boolean(false) => Ok(Form::atom("NIL", span)),
            Value::Boolean(true) => Ok(Form::atom("T", span)),
            Value::Integer(value) => Ok(Form::atom(value.to_string(), span)),
            Value::Rational(value) => Ok(Form::atom(
                format!("{}/{}", value.numerator(), value.denominator()),
                span,
            )),
            Value::Float(value) => Ok(Form::atom(value.to_string(), span)),
            Value::Complex { real, imaginary } => Ok(Form::new(
                FormKind::Complex {
                    real: Box::new(self.form_from_value(real.as_ref(), span)?),
                    imaginary: Box::new(self.form_from_value(imaginary.as_ref(), span)?),
                },
                span,
            )),
            Value::String(value) => Ok(Form::new(FormKind::String(value.to_string()), span)),
            Value::Character(value) => Ok(Form::new(FormKind::Character(*value), span)),
            Value::Package(name) => Ok(Form::list(
                vec![
                    Form::atom("FIND-PACKAGE", span),
                    Form::new(FormKind::String(name.to_string()), span),
                ],
                span,
            )),
            Value::Symbol(value) => Ok(Form::atom(value.as_ref(), span)),
            Value::SymbolExact(value) => Ok(Form::atom(escaped_symbol_atom(value), span)),
            Value::QualifiedSymbolExact {
                reference,
                package_len,
            } => Ok(Form::atom(
                format!(
                    "{}{}",
                    &reference[..*package_len + 2],
                    escaped_symbol_atom(&reference[*package_len + 2..])
                ),
                span,
            )),
            Value::UninternedSymbol(value) => Ok(Form::atom(format!("#:{value}"), span)),
            Value::Keyword(value) => Ok(Form::atom(format!(":{value}"), span)),
            Value::KeywordExact(value) => {
                Ok(Form::atom(format!(":{}", escaped_symbol_atom(value)), span))
            }
            Value::List(values) => Ok(Form::list(
                values
                    .iter()
                    .map(|value| self.form_from_value(value, span))
                    .collect::<Result<Vec<_>, _>>()?,
                span,
            )),
            Value::DottedList { items, tail } => Ok(Form::dotted_list(
                items
                    .iter()
                    .map(|value| self.form_from_value(value, span))
                    .collect::<Result<Vec<_>, _>>()?,
                self.form_from_value(tail, span)?,
                span,
            )),
            Value::Vector(values) => Ok(Form::new(
                FormKind::Vector(
                    values
                        .borrow()
                        .iter()
                        .map(|value| self.form_from_value(value, span))
                        .collect::<Result<Vec<_>, _>>()?,
                ),
                span,
            )),
            Value::Array { .. }
            | Value::HashTable { .. }
            | Value::HashTableIterator(_)
            | Value::Stream(_)
            | Value::RandomState(_)
            | Value::Values(_)
            | Value::Condition(_)
            | Value::Restart(_)
            | Value::Unbound
            | Value::Environment(_)
            | Value::Class(_)
            | Value::Instance(_)
            | Value::Structure { .. } => Err(RuntimeError::Type {
                expected: "FORM".to_string(),
                actual: value.type_name().to_string(),
                span: Some(span),
            }),
            Value::Function(_) => Err(RuntimeError::Type {
                expected: "FORM".to_string(),
                actual: value.type_name().to_string(),
                span: Some(span),
            }),
        }
    }

    fn arity(&self, function: &str, expected: &str, actual: usize) -> RuntimeError {
        RuntimeError::Arity {
            function: function.to_string(),
            expected: expected.to_string(),
            actual,
        }
    }

    fn block_name(&self, form: &Form) -> Result<String, RuntimeError> {
        let FormKind::Atom(name) = &form.kind else {
            return Err(self.invalid("block name must be a symbol", form.span));
        };
        if name.is_empty() || (name.starts_with(':') && name.len() == 1) {
            return Err(self.invalid("block name must be a symbol", form.span));
        }
        if !name.starts_with(':')
            && literal_atom(name).is_some()
            && !name.eq_ignore_ascii_case("nil")
            && !name.eq_ignore_ascii_case("t")
        {
            return Err(self.invalid("block name must be a symbol", form.span));
        }
        Ok(normalize_name(name))
    }

    fn restart_name(&self, form: &Form) -> Result<String, RuntimeError> {
        let FormKind::Atom(name) = &form.kind else {
            return Err(self.invalid("restart name must be a symbol", form.span));
        };
        if name.is_empty() || (name.starts_with(':') && name.len() == 1) {
            return Err(self.invalid("restart name must be a symbol", form.span));
        }
        if !name.starts_with(':')
            && literal_atom(name).is_some()
            && !name.eq_ignore_ascii_case("nil")
            && !name.eq_ignore_ascii_case("t")
        {
            return Err(self.invalid("restart name must be a symbol", form.span));
        }
        Ok(normalize_name(name))
    }

    fn condition_name(&self, form: &Form) -> Result<String, RuntimeError> {
        let Some(name) = atom_name(form) else {
            return Err(self.invalid("condition name must be a symbol", form.span));
        };
        if name.is_empty()
            || (name.starts_with(':') && name.len() == 1)
            || (!name.starts_with(':')
                && literal_atom(name).is_some()
                && !name.eq_ignore_ascii_case("nil")
                && !name.eq_ignore_ascii_case("t"))
        {
            return Err(self.invalid("condition name must be a symbol", form.span));
        }
        Ok(normalize_name(name).trim_start_matches(':').to_string())
    }

    fn variable_name_info(
        &self,
        form: &Form,
        context: &str,
    ) -> Result<(String, bool), RuntimeError> {
        let Some(name) = atom_name(form) else {
            return Err(self.invalid(context, form.span));
        };
        let Ok(token) = parse_symbol_token(name) else {
            return Err(self.invalid(context, form.span));
        };
        if token.kind != SymbolTokenKind::Symbol
            || token.name.is_empty()
            || (token.escaped && token.package.is_some())
            || (!token.escaped && (token.name.starts_with('&') || literal_atom(name).is_some()))
        {
            return Err(self.invalid(context, form.span));
        }
        let variable_name = if token.escaped {
            token.name
        } else {
            normalize_name(name)
        };
        Ok((variable_name, token.escaped))
    }

    fn variable_name(&self, form: &Form, context: &str) -> Result<String, RuntimeError> {
        self.variable_name_info(form, context).map(|(name, _)| name)
    }

    fn define_variable_in(
        &self,
        name: &str,
        escaped: bool,
        value: Value,
        environment: &Environment,
    ) {
        if escaped {
            self.define_exact_in(name, value, environment);
        } else {
            self.define_in(name, value, environment);
        }
    }

    fn define_macro_binding_in(
        &self,
        binding: &MacroBinding,
        value: Value,
        environment: &Environment,
    ) {
        self.define_variable_in(&binding.name, binding.escaped, value, environment);
    }

    fn lookup_macro_binding_in(
        &self,
        binding: &MacroBinding,
        environment: &Environment,
    ) -> Option<Value> {
        if binding.escaped {
            self.lookup_exact_in(&binding.name, environment)
        } else {
            self.lookup_in(&binding.name, environment)
        }
    }

    fn set_variable_in(
        &self,
        name: &str,
        escaped: bool,
        value: Value,
        environment: &Environment,
    ) -> bool {
        if escaped {
            self.set_exact_in(name, value, environment)
        } else {
            self.set_in(name, value, environment)
        }
    }

    fn set_or_define_variable_in(
        &self,
        name: &str,
        escaped: bool,
        value: Value,
        environment: &Environment,
        span: Span,
    ) -> Result<(), RuntimeError> {
        if escaped {
            self.set_or_define_exact_in(name, value, environment, span)
        } else {
            self.set_or_define_in(name, value, environment, span)
        }
    }

    fn ensure_symbol_writable(
        &self,
        name: &str,
        escaped: bool,
        span: Span,
    ) -> Result<(), RuntimeError> {
        let constant = if escaped {
            self.is_constant_exact_in(name)
        } else {
            self.is_constant_in(name)
        };
        if constant {
            Err(self.constant_modification_error(name, span))
        } else {
            Ok(())
        }
    }

    fn invalid(&self, message: &str, span: Span) -> RuntimeError {
        RuntimeError::InvalidForm {
            message: message.to_string(),
            span: Some(span),
        }
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

fn atom_name(form: &Form) -> Option<&str> {
    match &form.kind {
        FormKind::Atom(value) => Some(value),
        _ => None,
    }
}

fn method_specializers_equal(left: &[MethodSpecializer], right: &[MethodSpecializer]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| match (left, right) {
                (MethodSpecializer::Type(left), MethodSpecializer::Type(right)) => left == right,
                (MethodSpecializer::Eql(left), MethodSpecializer::Eql(right)) => {
                    builtins::eql_value(left, right)
                }
                _ => false,
            })
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

fn function_operator_name(form: &Form) -> Option<String> {
    let name = atom_name(form)?;
    let token = parse_symbol_token(name).ok()?;
    if token.kind == SymbolTokenKind::Symbol && token.package.is_none() && !token.escaped {
        Some(normalize_name(&token.name))
    } else {
        None
    }
}

fn is_valid_function_symbol_name(name: &str) -> bool {
    let Ok(token) = parse_symbol_token(name) else {
        return false;
    };
    if token.kind != SymbolTokenKind::Symbol || token.name.is_empty() {
        return false;
    }
    if token.escaped {
        return token.package.is_none();
    }
    literal_atom(name).is_none() && !name.starts_with(':')
}

fn is_nil_form(form: &Form) -> bool {
    atom_name(form).is_some_and(|name| name.eq_ignore_ascii_case("nil"))
}

fn is_macro_keyword_form(form: &Form) -> bool {
    macro_keyword_name(form).is_some()
}

fn macro_keyword_name(form: &Form) -> Option<(String, bool)> {
    let name = atom_name(form)?;
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

fn is_no_error_marker(form: &Form) -> bool {
    macro_keyword_name(form).is_some_and(|(name, escaped)| !escaped && name == "NO-ERROR")
}

fn macro_keyword_matches(
    specification_name: &str,
    specification_escaped: bool,
    actual_name: &str,
    _actual_escaped: bool,
) -> bool {
    if specification_escaped {
        specification_name == actual_name
    } else {
        normalize_name(specification_name) == actual_name
    }
}

fn macro_dotted_parts(value: &Value) -> Option<(Vec<Value>, Value)> {
    match value {
        Value::Nil => Some((Vec::new(), Value::Nil)),
        Value::List(values) => Some((values.as_ref().clone(), Value::Nil)),
        Value::DottedList { items, tail } => {
            let mut values = items.as_ref().clone();
            match tail.as_ref() {
                Value::Nil => Some((values, Value::Nil)),
                Value::List(more) => {
                    values.extend(more.as_ref().iter().cloned());
                    Some((values, Value::Nil))
                }
                Value::DottedList { .. } => {
                    let (more, dotted_tail) = macro_dotted_parts(tail)?;
                    values.extend(more);
                    Some((values, dotted_tail))
                }
                other => Some((values, other.clone())),
            }
        }
        _ => None,
    }
}

fn control_tag(form: &Form) -> Option<String> {
    let name = atom_name(form)?;
    if name.is_empty() || name == ":" {
        return None;
    }
    if name.starts_with(':') {
        return (name.len() > 1).then(|| normalize_name(name));
    }
    if name.eq_ignore_ascii_case("nil")
        || name.eq_ignore_ascii_case("t")
        || name.parse::<i64>().is_ok()
        || literal_atom(name).is_none()
    {
        Some(normalize_name(name))
    } else {
        None
    }
}

fn unqualified_name(name: &str) -> String {
    let normalized = normalize_name(name);
    package::split_symbol(&normalized)
        .map(|(_, symbol, _)| symbol.to_string())
        .unwrap_or(normalized)
}

fn cxr_operations(name: &str) -> Option<Vec<u8>> {
    let name = unqualified_name(name);
    let bytes = name.as_bytes();
    if !(4..=6).contains(&bytes.len())
        || bytes.first() != Some(&b'C')
        || bytes.last() != Some(&b'R')
        || bytes[1..bytes.len() - 1]
            .iter()
            .any(|operation| !matches!(operation, b'A' | b'D'))
    {
        return None;
    }

    let mut operations = bytes[1..bytes.len() - 1].to_vec();
    operations.reverse();
    Some(operations)
}

fn is_special_operator_name(name: &str) -> bool {
    matches!(
        unqualified_name(name).as_str(),
        "BLOCK"
            | "CATCH"
            | "EVAL-WHEN"
            | "FLET"
            | "FUNCTION"
            | "GO"
            | "IF"
            | "LABELS"
            | "LET"
            | "LET*"
            | "LOAD-TIME-VALUE"
            | "LOCALLY"
            | "MACROLET"
            | "MULTIPLE-VALUE-CALL"
            | "MULTIPLE-VALUE-PROG1"
            | "PROGN"
            | "PROGV"
            | "QUOTE"
            | "SETQ"
            | "SYMBOL-MACROLET"
            | "TAGBODY"
            | "THE"
            | "THROW"
            | "UNWIND-PROTECT"
    )
}

fn is_case_default_form(form: &Form) -> bool {
    let Some(name) = atom_name(form) else {
        return false;
    };
    let Ok(token) = parse_symbol_token(name) else {
        return false;
    };
    token.kind == SymbolTokenKind::Symbol
        && !token.escaped
        && matches!(unqualified_name(name).as_str(), "T" | "OTHERWISE")
}

fn is_operator_form(form: &Form, name: &str) -> bool {
    match &form.kind {
        FormKind::List(items) => items
            .first()
            .and_then(atom_name)
            .is_some_and(|operator| operator.eq_ignore_ascii_case(name)),
        _ => false,
    }
}

fn is_special_form(form: &Form) -> bool {
    let Some(operator) = atom_name(form) else {
        return false;
    };
    matches!(
        normalize_name(operator).as_str(),
        "QUOTE"
            | "QUASIQUOTE"
            | "DECLARE"
            | "LOCALLY"
            | "EVAL-WHEN"
            | "LOAD-TIME-VALUE"
            | "NTH-VALUE"
            | "DECLAIM"
            | "PROCLAIM"
            | "THE"
            | "IF"
            | "PROGN"
            | "PROG1"
            | "PROG2"
            | "PROG"
            | "PROG*"
            | "VALUES"
            | "IGNORE-ERRORS"
            | "HANDLER-CASE"
            | "HANDLER-BIND"
            | "RESTART-BIND"
            | "WITH-CONDITION-RESTARTS"
            | "CATCH"
            | "PROGV"
            | "THROW"
            | "WITH-SIMPLE-RESTART"
            | "WITH-OPEN-FILE"
            | "WITH-OPEN-STREAM"
            | "WITH-OUTPUT-TO-STRING"
            | "WITH-INPUT-FROM-STRING"
            | "WITH-HASH-TABLE-ITERATOR"
            | "RESTART-CASE"
            | "UNWIND-PROTECT"
            | "BLOCK"
            | "RETURN"
            | "RETURN-FROM"
            | "TAGBODY"
            | "GO"
            | "MULTIPLE-VALUE-BIND"
            | "MULTIPLE-VALUE-CALL"
            | "MULTIPLE-VALUE-LIST"
            | "MULTIPLE-VALUE-PROG1"
            | "AND"
            | "OR"
            | "WHEN"
            | "UNLESS"
            | "COND"
            | "CASE"
            | "CCASE"
            | "ECASE"
            | "TYPECASE"
            | "CTYPECASE"
            | "ETYPECASE"
            | "DESTRUCTURING-BIND"
            | "LET"
            | "LET*"
            | "FLET"
            | "LABELS"
            | "MACROLET"
            | "SYMBOL-MACROLET"
            | "NCL-MACRO-ENVIRONMENT"
            | "DOTIMES"
            | "DOLIST"
            | "DO"
            | "DO*"
            | "LAMBDA"
            | "FUNCTION"
            | "DEFUN"
            | "DEFMACRO"
            | "DEFINE-MODIFY-MACRO"
            | "MACROEXPAND-1"
            | "MACROEXPAND"
            | "DEFPACKAGE"
            | "IN-PACKAGE"
            | "DEFINE"
            | "DEFINE-SYMBOL-MACRO"
            | "SETQ"
            | "PSETQ"
            | "MULTIPLE-VALUE-SETQ"
            | "SETF"
            | "PSETF"
            | "PUSH"
            | "POP"
            | "PUSHNEW"
            | "ROTATEF"
            | "SHIFTF"
            | "DEFSETF"
            | "INCF"
            | "DECF"
            | "DEFSTRUCT"
            | "DEFCLASS"
            | "DEFINE-CONDITION"
            | "DEFGENERIC"
            | "DEFMETHOD"
            | "DEFVAR"
            | "DEFPARAMETER"
            | "DEFCONSTANT"
            | "DEFINE-SETF-EXPANDER"
            | "GET-SETF-EXPANSION"
            | "EVAL"
            | "FUNCALL"
            | "APPLY"
            | "MAP-INTO"
            | "MAPHASH"
            | "MAPCAR"
    )
}

fn prefix_argument<'form>(items: &'form [Form], name: &str) -> Option<&'form Form> {
    if items.len() != 2 {
        return None;
    }
    atom_name(&items[0]).filter(|operator| operator.eq_ignore_ascii_case(name))?;
    Some(&items[1])
}

fn quasiquote_marker(name: &str, value: Value) -> Value {
    Value::list(vec![Value::symbol(name), value])
}

pub(crate) fn quoted_form_value(form: &Form) -> Result<Value, RuntimeError> {
    match &form.kind {
        FormKind::Atom(atom) => {
            if let Ok(token) = parse_symbol_token(atom) {
                match token.kind {
                    SymbolTokenKind::Uninterned => {
                        return Ok(Value::uninterned_symbol(token.name));
                    }
                    SymbolTokenKind::Keyword => {
                        return Ok(if token.escaped {
                            Value::keyword_exact(token.name)
                        } else {
                            Value::keyword(token.name)
                        });
                    }
                    SymbolTokenKind::Symbol => {
                        if let Some(package) = token.package {
                            let name = format!("{}::{}", normalize_name(&package), token.name);
                            return Ok(if token.escaped {
                                Value::symbol_exact(name)
                            } else {
                                Value::symbol(name)
                            });
                        }
                        if token.escaped {
                            return Ok(Value::symbol_exact(token.name));
                        }
                    }
                }
            }
            Ok(literal_atom(atom).unwrap_or_else(|| Value::symbol(atom)))
        }
        FormKind::String(value) => Ok(Value::string(value.clone())),
        FormKind::Character(value) => Ok(Value::Character(*value)),
        FormKind::Complex { real, imaginary } => {
            Value::complex(quoted_form_value(real)?, quoted_form_value(imaginary)?)
        }
        FormKind::ReadTimeEval(_) => Err(RuntimeError::InvalidForm {
            message: "read-time evaluation must be resolved before quoting".to_string(),
            span: Some(form.span),
        }),
        FormKind::BitVector(bits) => Ok(Value::array_with_element_type(
            vec![bits.len()],
            bits.iter()
                .map(|bit| Value::Integer(i64::from(*bit)))
                .collect(),
            ArrayElementType::Bit,
        )),
        FormKind::List(items) => Ok(Value::list(
            items
                .iter()
                .map(quoted_form_value)
                .collect::<Result<Vec<_>, _>>()?,
        )),
        FormKind::DottedList { items, tail } => Ok(Value::dotted_list(
            items
                .iter()
                .map(quoted_form_value)
                .collect::<Result<Vec<_>, _>>()?,
            quoted_form_value(tail)?,
        )),
        FormKind::Vector(items) => Ok(Value::vector(
            items
                .iter()
                .map(quoted_form_value)
                .collect::<Result<Vec<_>, _>>()?,
        )),
    }
}

fn escaped_symbol_atom(value: &str) -> String {
    let mut result = String::with_capacity(value.len() + 2);
    result.push('|');
    for character in value.chars() {
        if matches!(character, '|' | '\\') {
            result.push('\\');
        }
        result.push(character);
    }
    result.push('|');
    result
}

fn literal_atom(atom: &str) -> Option<Value> {
    let token = parse_symbol_token(atom).ok()?;
    match token.kind {
        SymbolTokenKind::Keyword => Some(if token.escaped {
            Value::keyword_exact(token.name)
        } else {
            Value::keyword(token.name)
        }),
        SymbolTokenKind::Symbol if token.package.is_none() && !token.escaped => {
            match token.name.as_str() {
                "NIL" | "#F" => return Some(Value::Nil),
                "T" | "#T" => return Some(Value::boolean(true)),
                _ => {}
            }
            if let Ok(value) = token.name.parse::<i64>() {
                return Some(Value::Integer(value));
            }
            if let Some(value) = parse_radix_integer_literal(&token.name) {
                return Some(Value::Integer(value));
            }
            if let Some((numerator, denominator)) = token.name.split_once('/') {
                if let (Ok(numerator), Ok(denominator)) =
                    (numerator.parse::<i128>(), denominator.parse::<i128>())
                {
                    return Value::rational(numerator, denominator).ok();
                }
            }
            parse_float_literal(&token.name).map(Value::Float)
        }
        _ => None,
    }
}

fn resolved_symbol_name(atom: &str) -> String {
    resolved_symbol(atom).0
}

fn resolved_symbol(atom: &str) -> (String, bool) {
    let Ok(token) = parse_symbol_token(atom) else {
        return (normalize_name(atom), false);
    };
    match token.kind {
        SymbolTokenKind::Uninterned => (format!("#:{}", token.name), token.escaped),
        SymbolTokenKind::Keyword => (format!(":{}", token.name), token.escaped),
        SymbolTokenKind::Symbol => {
            let name = if token.escaped {
                token.name
            } else {
                normalize_name(&token.name)
            };
            let resolved = token.package.map_or(name.clone(), |package| {
                package::canonical_symbol_name(&package, &name)
            });
            (resolved, token.escaped)
        }
    }
}
