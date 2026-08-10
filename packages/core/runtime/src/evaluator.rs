use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::rc::Rc;

use ncl_compiler::Compiler;
use ncl_syntax::{
    parse_ordinary_lambda_list, parse_symbol_token, read, Form, FormKind,
    LambdaListKeywordParameter, OrdinaryLambdaList, Span, SymbolTokenKind,
};

use crate::builtins;
use crate::environment::normalize_name;
use crate::error::ThrowTag;
use crate::package::{self, PackageState};
use crate::value::{
    ClassDefinition, ClassSlot, MacroAuxiliaryParameter, MacroKeywordParameter,
    MacroLambdaList, MacroOptionalParameter, MacroPattern, MethodDefinition, StructureDefinition,
    StructureSlot,
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
}

impl Runtime {
    pub fn new() -> Self {
        let global = Environment::new();
        builtins::install(&global);
        Self {
            global,
            packages: Rc::new(RefCell::new(PackageState::new())),
            dynamic: Rc::new(RefCell::new(DynamicState::default())),
            next_block_target: Cell::new(1),
            gensym_counter: Cell::new(0),
            method_context: RefCell::new(Vec::new()),
        }
    }

    pub fn global_environment(&self) -> Environment {
        self.global.clone()
    }

    pub fn current_package(&self) -> String {
        self.packages.borrow().current().to_string()
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
        read(source)?.iter().map(|form| self.eval(form)).collect()
    }

    pub fn eval_compiled(&self, form: &Form) -> Result<Value, RuntimeError> {
        let resolved = self.resolve_form(form)?;
        let expanded = self.prepare_compiled_form(&resolved, &self.global)?;
        let program = Rc::new(Compiler::compile_form(&expanded)?);
        crate::vm::run_entry(self, program, 0, self.global.clone(), expanded.span)
            .map(|value| value.primary_value())
    }

    pub fn eval_compiled_source(&self, source: &str) -> Result<Vec<Value>, RuntimeError> {
        read(source)?
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
        let token = parse_symbol_token(atom)
            .map_err(|_| self.package_error("invalid symbol", span))?;
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
        if let Some(value) = self
            .dynamic
            .borrow()
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
            .find_map(|candidate| self.dynamic.borrow().globals.get(candidate).cloned())
        {
            return Some(value);
        }
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
        environment
            .lookup_function(name)
            .or_else(|| self.lookup_in(name, environment))
    }

    pub(crate) fn lookup_exact_in(
        &self,
        name: &str,
        environment: &Environment,
    ) -> Option<Value> {
        if let Some(value) = self
            .dynamic
            .borrow()
            .exact_bindings
            .iter()
            .rev()
            .find(|(binding, _)| binding == name)
            .map(|(_, value)| value.clone())
        {
            return Some(value);
        }
        if let Some(value) = self.dynamic.borrow().exact_globals.get(name).cloned() {
            return Some(value);
        }
        environment.lookup_exact(name)
    }

    pub(crate) fn lookup_function_exact_in(
        &self,
        name: &str,
        environment: &Environment,
    ) -> Option<Value> {
        environment
            .lookup_function_exact(name)
            .or_else(|| self.lookup_exact_in(name, environment))
    }

    pub(crate) fn is_bound_in(&self, name: &str, environment: &Environment) -> bool {
        self.lookup_in(name, environment).is_some()
    }

    pub(crate) fn is_bound_exact_in(&self, name: &str, environment: &Environment) -> bool {
        self.lookup_exact_in(name, environment).is_some()
    }

    pub(crate) fn define_in(&self, name: &str, value: Value, environment: &Environment) {
        let candidates = self.dynamic_candidates(name);
        if let Some(binding_name) = candidates
            .into_iter()
            .find(|candidate| self.dynamic.borrow().special_names.contains(candidate))
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
            if let Some(index) = dynamic
                .bindings
                .iter()
                .rev()
                .position(|(binding, _)| candidates.iter().any(|candidate| candidate == binding))
            {
                let index = dynamic.bindings.len() - 1 - index;
                let binding = dynamic.bindings[index].0.clone();
                if dynamic.constants.contains(&binding) {
                    return false;
                }
                dynamic.bindings[index].1 = value;
                return true;
            }
            if let Some(candidate) = candidates
                .iter()
                .find(|candidate| dynamic.special_names.contains(*candidate))
            {
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

    pub(crate) fn define_exact_in(
        &self,
        name: &str,
        value: Value,
        environment: &Environment,
    ) {
        if self
            .dynamic
            .borrow()
            .exact_special_names
            .contains(name)
        {
            self.dynamic
                .borrow_mut()
                .exact_bindings
                .push((name.to_string(), value));
            return;
        }
        environment.define_exact(name, value);
    }

    pub(crate) fn set_exact_in(
        &self,
        name: &str,
        value: Value,
        environment: &Environment,
    ) -> bool {
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
            if dynamic.exact_special_names.contains(name) {
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
            .push(ConditionRestartBinding { condition, restarts });
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
        dynamic.exact_globals.insert(name.to_string(), value.clone());
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
        dynamic.exact_globals.insert(name.to_string(), value.clone());
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
        dynamic.exact_globals.insert(name.to_string(), value.clone());
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
        let mut seen = HashSet::new();

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

        if is_operator_form(form, "DEFMACRO")
            || is_operator_form(form, "DEFINE-MODIFY-MACRO")
            || is_operator_form(form, "DEFINE-SETF-EXPANDER")
            || is_operator_form(form, "DEFINE-SYMBOL-MACRO")
            || is_operator_form(form, "MACROEXPAND-1")
            || is_operator_form(form, "MACROEXPAND")
            || is_operator_form(form, "LOAD-TIME-VALUE")
            || is_operator_form(form, "DEFPACKAGE")
            || is_operator_form(form, "IN-PACKAGE")
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
            let function = Value::macro_function(
                lambda_list,
                parts[2..].to_vec(),
                captured.clone(),
            );
            if escaped {
                local.define_exact(name, function);
            } else {
                local.define(name, function);
            }
        }

        let mut prepared = Vec::with_capacity(items.len().saturating_sub(2) + 1);
        prepared.push(Form::atom("PROGN", form.span));
        for body_form in &items[2..] {
            prepared.push(self.prepare_compiled_form(body_form, &local)?);
        }
        Ok(Form::list(prepared, form.span))
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
            return Err(self.invalid(
                "symbol macro bindings must be a list",
                items[1].span,
            ));
        };

        let local = environment.child();
        let mut names = HashSet::new();
        for binding in bindings {
            let FormKind::List(parts) = &binding.kind else {
                return Err(self.invalid(
                    "symbol macro binding must be a list",
                    binding.span,
                ));
            };
            if parts.len() != 2 {
                return Err(self.invalid(
                    "symbol macro binding needs a name and an expansion",
                    binding.span,
                ));
            }
            let (name, escaped) = self.variable_name_info(
                &parts[0],
                "symbol macro name must be a symbol",
            )?;
            if !names.insert((name.clone(), escaped)) {
                return Err(self.invalid(
                    "symbol macro names must be unique",
                    parts[0].span,
                ));
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
                return self.prepare_compiled_multiple_value_setq(
                    form,
                    &prepared,
                    environment,
                );
            }
            "LAMBDA" => {
                if prepared.len() > 1 {
                    let parameter_form = prepared[1].clone();
                    let local =
                        self.prepare_compiled_lambda_environment(&parameter_form, environment)?;
                    prepared[1] =
                        self.prepare_compiled_lambda_list(&parameter_form, &local)?;
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
                    prepared[2] =
                        self.prepare_compiled_lambda_list(&parameter_form, &local)?;
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
            "CASE" | "ECASE" | "TYPECASE" | "ETYPECASE" => {
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
            .map(|(index, variable)| {
                self.symbol_macro_temporary(form, index, variable.span)
            })
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
                vec![Form::atom(operator, variable.span), target, temporary.clone()],
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
                let local = self.prepare_compiled_lambda_environment(&parameter_form, environment)?;
                prepared_parts[1] =
                    self.prepare_compiled_lambda_list(&parameter_form, &local)?;
                for index in 2..prepared_parts.len() {
                    prepared_parts[index] =
                        self.prepare_compiled_form(&parts[index], &local)?;
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
        value
            .ok_or_else(|| RuntimeError::UnboundVariable {
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
                "DECLAIM" | "PROCLAIM" => return Ok(Value::Nil),
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
                "TYPECASE" => return self.special_typecase(items, environment, false),
                "ETYPECASE" => return self.special_typecase(items, environment, true),
                "DESTRUCTURING-BIND" => {
                    return self.special_destructuring_bind(items, environment);
                }
                "LET" => return self.special_let(items, environment, false),
                "LET*" => return self.special_let(items, environment, true),
                "FLET" => return self.special_flet(items, environment, false),
                "LABELS" => return self.special_flet(items, environment, true),
                "MACROLET" => return self.special_macrolet(items, environment),
                "SYMBOL-MACROLET" => return self.special_symbol_macrolet(items, environment),
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
                "MAPCAR" => return self.special_mapcar(items, environment),
                    _ => {}
                }
            }
        }

        let function = if let Some(name) = atom_name(operator) {
            let (resolved_name, escaped) = resolved_symbol(name);
            let function = if escaped {
                self.lookup_function_exact_in(&resolved_name, environment)
            } else {
                self.lookup_function_in(&resolved_name, environment)
            };
            function.ok_or_else(|| {
                RuntimeError::UnboundVariable {
                    name: if escaped {
                        resolved_name
                    } else {
                        normalize_name(&resolved_name)
                    },
                    span: Some(operator.span),
                }
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
            self.lookup_in(&resolved_name, environment)
        };
        let Some(function) = function else {
            if !escaped {
                match normalize_name(&resolved_name).as_str() {
                    "WITH-SLOTS" => {
                        return self
                            .expand_builtin_with_slots(form, false)
                            .map(Some);
                    }
                    "WITH-ACCESSORS" => {
                        return self
                            .expand_builtin_with_slots(form, true)
                            .map(Some);
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
            let mut supplied = HashMap::new();
            let mut accepts_unknown_keywords = lambda_list.allow_other_keys;
            for pair in keyword_arguments.chunks_exact(2) {
                let Some(keyword_name) = macro_keyword_name(&pair[0]) else {
                    return Err(
                        self.invalid("keyword argument name must be a keyword", pair[0].span)
                    );
                };
                if keyword_name == "ALLOW-OTHER-KEYS" && self.quoted_value(&pair[1])?.is_truthy() {
                    accepts_unknown_keywords = true;
                }
                supplied.insert(keyword_name, pair[1].clone());
            }
            if !accepts_unknown_keywords {
                for keyword_name in supplied.keys() {
                    if keyword_name != "ALLOW-OTHER-KEYS"
                        && !lambda_list
                            .keywords
                            .iter()
                            .any(|specification| specification.keyword_name == *keyword_name)
                    {
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
            local.define(environment_name, Value::environment(environment.clone()));
        }
        if let Some(whole) = &lambda_list.whole {
            local.define(whole, self.quoted_value(form)?);
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
                local.define(supplied_p, Value::boolean(supplied.is_some()));
            }
        }
        if let Some(rest_name) = &lambda_list.rest {
            let rest_values = arguments[key_start..]
                .iter()
                .map(|argument| self.quoted_value(argument))
                .collect::<Result<Vec<_>, _>>()?;
            local.define(rest_name, Value::list(rest_values));
        }

        if let Some(supplied_keywords) = keyword_arguments {
            for specification in &lambda_list.keywords {
                let supplied = supplied_keywords.get(&specification.keyword_name);
                let value = match supplied {
                    Some(argument) => self.quoted_value(argument)?,
                    None => self.eval_in(&specification.init_form, &local)?,
                };
                self.bind_macro_pattern(&specification.pattern, value, &local, form.span)?;
                if let Some(supplied_p) = &specification.supplied_p {
                    local.define(supplied_p, Value::boolean(supplied.is_some()));
                }
            }
        }
        for specification in &lambda_list.auxiliary {
            let value = self.eval_in(&specification.init_form, &local)?;
            local.define(&specification.name, value);
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
            return Err(self.invalid(
                "define-modify-macro requires a place parameter",
                form.span,
            ));
        };
        let place_value = self.lookup_in(place_name, &local).ok_or_else(|| {
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
            let value = self.lookup_in(name, &local).ok_or_else(|| {
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
            let value = self.lookup_in(name, &local).ok_or_else(|| {
                self.invalid("define-modify-macro parameter is unbound", form.span)
            })?;
            call_items.push(self.form_from_value(&value, form.span)?);
        }
        if let Some(rest_name) = &lambda_list.rest {
            let rest_value = self.lookup_in(rest_name, &local).ok_or_else(|| {
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
                let value = self.lookup_in(name, &local).ok_or_else(|| {
                    self.invalid("define-modify-macro keyword parameter is unbound", form.span)
                })?;
                call_items.push(Form::atom(
                    format!(":{}", specification.keyword_name),
                    form.span,
                ));
                call_items.push(self.form_from_value(&value, form.span)?);
            }
        }
        let call = Form::list(call_items, form.span);
        let store_binding = Form::list(
            vec![expansion.store.clone(), call],
            form.span,
        );
        let update = Form::list(
            vec![
                Form::atom("LET", form.span),
                Form::list(vec![store_binding], form.span),
                Form::list(
                    vec![
                        Form::atom("PROGN", form.span),
                        expansion.store_form.clone(),
                        expansion.store.clone(),
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
            .map(|(temporary, value)| {
                Form::list(vec![temporary.clone(), value.clone()], form.span)
            })
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
                self.variable_name_info(
                    &parts[0],
                    "with-accessors variable must be a symbol",
                )?;
                validate_symbol(&parts[1], "with-accessors accessor must be a symbol")?;
                (
                    parts[0].clone(),
                    Form::list(
                        vec![parts[1].clone(), temporary.clone()],
                        entry.span,
                    ),
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
                let quoted_slot = Form::list(
                    vec![Form::atom("QUOTE", slot.span), slot],
                    entry.span,
                );
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
            symbol_bindings.push(Form::list(
                vec![variable, expansion],
                entry.span,
            ));
        }

        let symbol_macrolet = {
            let mut forms = Vec::with_capacity(items.len().saturating_sub(1));
            forms.push(Form::atom("SYMBOL-MACROLET", form.span));
            forms.push(Form::list(symbol_bindings, items[1].span));
            forms.extend(items[3..].iter().cloned());
            Form::list(forms, form.span)
        };
        let let_bindings = Form::list(
            vec![Form::list(
                vec![temporary, items[2].clone()],
                items[2].span,
            )],
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
            return Err(self.invalid(
                "with-open-file binding must be a list",
                binding_form.span,
            ));
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
            vec![Form::atom("LET", form.span), generated_binding, protected_form],
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
                environment.define(name, value);
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
            return Err(self.invalid(
                "destructuring-bind value must be a proper list",
                span,
            ));
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
            let supplied = (index < optional_supplied_count)
                .then(|| &arguments[required_count + index]);
            let value = match supplied {
                Some(argument) => argument.clone(),
                None => self.eval_in(&specification.init_form, environment)?,
            };
            self.bind_macro_pattern(&specification.pattern, value, environment, span)?;
            if let Some(supplied_p) = &specification.supplied_p {
                environment.define(supplied_p, Value::boolean(supplied.is_some()));
            }
        }
        if let Some(rest_name) = &lambda_list.rest {
            environment.define(
                rest_name,
                Value::list(arguments[key_start..].to_vec()),
            );
        }

        if lambda_list.has_keyword_section {
            let keyword_arguments = &arguments[key_start..];
            if keyword_arguments.len() % 2 != 0 {
                return Err(self.invalid("keyword arguments must be supplied in pairs", span));
            }
            let mut supplied_keywords = HashMap::new();
            let mut accepts_unknown_keywords = lambda_list.allow_other_keys;
            for pair in keyword_arguments.chunks_exact(2) {
                let keyword = match &pair[0] {
                    Value::Keyword(keyword) | Value::KeywordExact(keyword) => keyword,
                    _ => {
                        return Err(self.invalid(
                            "keyword argument name must be a keyword",
                            span,
                        ));
                    }
                };
                let keyword_name = keyword.to_string();
                if keyword_name == "ALLOW-OTHER-KEYS" && pair[1].is_truthy() {
                    accepts_unknown_keywords = true;
                }
                supplied_keywords.insert(keyword_name, pair[1].clone());
            }
            if !accepts_unknown_keywords {
                for keyword_name in supplied_keywords.keys() {
                    if keyword_name != "ALLOW-OTHER-KEYS"
                        && !lambda_list
                            .keywords
                            .iter()
                            .any(|specification| specification.keyword_name == *keyword_name)
                    {
                        return Err(self.invalid(
                            &format!("unknown keyword :{keyword_name}"),
                            span,
                        ));
                    }
                }
            }
            for specification in &lambda_list.keywords {
                let supplied = supplied_keywords.get(&specification.keyword_name);
                let value = match supplied {
                    Some(argument) => argument.clone(),
                    None => self.eval_in(&specification.init_form, environment)?,
                };
                self.bind_macro_pattern(&specification.pattern, value, environment, span)?;
                if let Some(supplied_p) = &specification.supplied_p {
                    environment.define(supplied_p, Value::boolean(supplied.is_some()));
                }
            }
        }
        for specification in &lambda_list.auxiliary {
            let value = self.eval_in(&specification.init_form, environment)?;
            environment.define(&specification.name, value);
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
                return Err(self.invalid(
                    "nth-value index must be non-negative",
                    items[1].span,
                ));
            }
            value => {
                return Err(RuntimeError::Type {
                    expected: "INTEGER".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(items[1].span),
                });
            }
        };

        let values = self.eval_values_in(&items[2], environment)?.multiple_values();
        Ok(values.get(index).cloned().unwrap_or(Value::Nil))
    }

    fn special_locally(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        self.eval_sequence_values(items.get(1..).unwrap_or(&[]), environment)
    }

    fn special_eval_when(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(self.arity(
                "eval-when",
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        if self.eval_when_executes(&items[1])? {
            self.eval_sequence_values(items.get(2..).unwrap_or(&[]), environment)
        } else {
            Ok(Value::Nil)
        }
    }

    fn eval_when_executes(&self, form: &Form) -> Result<bool, RuntimeError> {
        let FormKind::List(situations) = &form.kind else {
            return Err(self.invalid(
                "eval-when situations must be a list",
                form.span,
            ));
        };
        let mut executes = false;
        for situation in situations {
            let Some(name) = atom_name(situation) else {
                return Err(self.invalid(
                    "eval-when situations must contain symbols",
                    situation.span,
                ));
            };
            let token = parse_symbol_token(name).map_err(|_| {
                self.invalid(
                    "eval-when situations must contain symbols",
                    situation.span,
                )
            })?;
            if token.kind == SymbolTokenKind::Uninterned
                || (token.kind == SymbolTokenKind::Symbol && literal_atom(name).is_some())
            {
                return Err(self.invalid(
                    "eval-when situations must contain symbols",
                    situation.span,
                ));
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
            FormKind::Atom(_) | FormKind::String(_) | FormKind::Character(_) => {
                self.quoted_value(form)
            }
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
                    return Err(self.invalid(
                        "prog binding must be a symbol or list",
                        binding.span,
                    ));
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
                    values.push(
                        init.as_ref()
                            .map_or(Ok(Value::Nil), |form| {
                                self.eval_in(form, &block_environment)
                            })?,
                    );
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
            return Err(self.arity(
                "multiple-value-list",
                "one",
                items.len().saturating_sub(1),
            ));
        }
        let values = self.eval_values_in(&items[1], environment)?.multiple_values();
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
            Ok(value) => return Ok(value),
            Err(error @ RuntimeError::ReturnFrom { .. }) => return Err(error),
            Err(error @ RuntimeError::Go { .. }) => return Err(error),
            Err(error @ RuntimeError::InvokeRestart { .. }) => return Err(error),
            Err(error) => error,
        };

        for clause in &items[2..] {
            let FormKind::List(clause_items) = &clause.kind else {
                unreachable!("handler-case clauses were validated above");
            };
            let condition = self.condition_name(&clause_items[0])?;
            if !protected.matches_condition(&condition) {
                continue;
            }
            let local = environment.child();
            if let FormKind::List(variables) = &clause_items[1].kind {
                if let Some(variable) = variables.first() {
                    let (name, escaped) =
                        self.variable_name_info(variable, "handler-case condition variable")?;
                    self.define_variable_in(
                        &name,
                        escaped,
                        Value::condition(&protected),
                        &local,
                    );
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
                return Err(self.invalid(
                    "restart-case clause must be a list",
                    clause.span,
                ));
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
                    if let Some((_, closure, clause_span)) = clauses.iter().find(
                        |(restart, _, _)| normalize_name(invoked.as_str()) == restart.as_str(),
                    ) {
                        let argument_values = arguments
                            .iter()
                            .cloned()
                            .map(ReturnValue::into_value)
                            .collect::<Vec<_>>();
                        return self.apply_in(
                            closure,
                            &argument_values,
                            *clause_span,
                            environment,
                        );
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
        let operator = if error_on_miss { "etypecase" } else { "typecase" };
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
        let pattern = lambda_list.is_none().then(|| self.macro_pattern(&items[1], &mut seen));
        let pattern = pattern.transpose()?;
        let local = environment.child();
        let _dynamic_guard = self.dynamic_guard();
        let value = self.eval_in(&items[2], environment)?.primary_value();
        if let Some(lambda_list) = &lambda_list {
            if let Some(whole) = &lambda_list.whole {
                local.define(whole, value.clone());
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
            self.define_variable_in(&name, escaped, value, &local);
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
            let function = Value::macro_function(
                lambda_list,
                parts[2..].to_vec(),
                captured.clone(),
            );
            if escaped {
                local.define_exact(name, function);
            } else {
                local.define(name, function);
            }
        }
        self.eval_sequence_values(&items[2..], &local)
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
            return Err(self.invalid(
                "symbol macro bindings must be a list",
                items[1].span,
            ));
        };

        let local = environment.child();
        let mut names = HashSet::new();
        for binding in bindings {
            let FormKind::List(parts) = &binding.kind else {
                return Err(self.invalid(
                    "symbol macro binding must be a list",
                    binding.span,
                ));
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
                return Err(self.invalid(
                    "symbol macro names must be unique",
                    parts[0].span,
                ));
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
            return Err(self.arity(
                "DEFINE-SYMBOL-MACRO",
                "two",
                items.len().saturating_sub(1),
            ));
        }
        let (name, escaped) = self.variable_name_info(
            &items[1],
            "DEFINE-SYMBOL-MACRO name must be a symbol",
        )?;
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
            return Err(self.invalid(
                "do termination needs an end test",
                items[2].span,
            ));
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
            bindings.push((
                name,
                escaped,
                parts.get(1).cloned(),
                parts.get(2).cloned(),
            ));
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
                    values.push(
                        init.as_ref()
                            .map_or(Ok(Value::Nil), |form| self.eval_in(form, &block_environment))?,
                    );
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
        Ok(Value::closure_with_keywords(
            lambda_list.required,
            lambda_list.required_escaped,
            lambda_list.optional,
            lambda_list.rest,
            lambda_list.rest_escaped,
            lambda_list.keywords,
            lambda_list.has_keyword_section,
            lambda_list.allow_other_keys,
            lambda_list.auxiliary,
            items[2..].to_vec(),
            environment.clone(),
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
        if let Some(name) = atom_name(&items[1]) {
            let (resolved_name, escaped) = resolved_symbol(name);
            let function = if escaped {
                self.lookup_function_exact_in(&resolved_name, environment)
            } else {
                self.lookup_function_in(&resolved_name, environment)
            };
            return function.ok_or_else(|| {
                RuntimeError::UnboundVariable {
                    name: if escaped {
                        resolved_name
                    } else {
                        normalize_name(&resolved_name)
                    },
                    span: Some(items[1].span),
                }
            });
        }
        self.eval_in(&items[1], environment)
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
            items[3..].to_vec(),
            environment.clone(),
        );
        let (resolved_name, escaped) = resolved_symbol(name);
        if escaped {
            self.define_exact_in(&resolved_name, function, environment);
        } else {
            self.define_in(&resolved_name, function, environment);
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
        if items.len() != 3 {
            return Err(self.invalid("DEFSETF needs an accessor and a writer", items[0].span));
        }
        let Some(accessor) = atom_name(&items[1]) else {
            return Err(self.invalid("DEFSETF accessor must be a symbol", items[1].span));
        };

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
            return Err(self.invalid(
                "DEFINE-SETF-EXPANDER name must be a symbol",
                items[1].span,
            ));
        };
        let lambda_list = self.macro_parameters(&items[2])?;
        let function =
            Value::macro_function(lambda_list, items[3..].to_vec(), environment.clone());
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
            self.define_exact_in(&resolved_name, function, environment);
        } else {
            self.define_in(&resolved_name, function, environment);
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
            return Err(self.invalid(
                "define-modify-macro name must be a symbol",
                items[1].span,
            ));
        };
        let mut lambda_list = self.macro_parameters(&items[2])?;
        lambda_list
            .required
            .insert(0, MacroPattern::Name("NCL-MODIFY-MACRO-PLACE".to_owned()));
        let function = Value::modify_macro_function(
            lambda_list,
            items[3].clone(),
            environment.clone(),
        );
        let (resolved_name, escaped) = resolved_symbol(name);
        if escaped {
            self.define_exact_in(&resolved_name, function, environment);
        } else {
            self.define_in(&resolved_name, function, environment);
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
            return Err(self.arity(
                "macroexpand-1",
                "one or two",
                items.len().saturating_sub(1),
            ));
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
            return Err(self.arity(
                "macroexpand",
                "one or two",
                items.len().saturating_sub(1),
            ));
        }
        let value = self.eval_in(&items[1], environment)?;
        let form = self.form_from_value(&value, items[1].span)?;
        let expansion_environment = if items.len() == 3 {
            let value = self.eval_in(&items[2], environment)?;
            self.macroexpansion_environment(value, items[2].span)?
        } else {
            environment.clone()
        };
        let (expanded, expanded_p) =
            self.expand_macros_with_flag(form, &expansion_environment)?;
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
            _ => Err(self.invalid(
                "macro expansion environment must be an environment",
                span,
            )),
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
        let (name, escaped) =
            self.variable_name_info(&items[1], "define name must be a symbol")?;
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
            let value = self.eval_in(&pair[1], environment)?;
            self.set_place(&pair[0], value.clone(), environment)?;
            result = value;
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

        let mut assignments = Vec::with_capacity((items.len() - 1) / 2);
        for pair in items[1..].chunks_exact(2) {
            let value = self.eval_in(&pair[1], environment)?;
            assignments.push((pair[0].clone(), value));
        }

        let mut result = Value::Nil;
        for (place, value) in assignments {
            self.set_place(&place, value.clone(), environment)?;
            result = value;
        }
        Ok(result)
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
            let Some(keyword_name) = macro_keyword_name(&pair[0]) else {
                return Err(self.invalid(
                    "PUSHNEW keyword argument name must be a keyword",
                    pair[0].span,
                ));
            };
            match keyword_name.as_str() {
                "TEST" => {
                    if test_not.is_some() {
                        return Err(self.invalid(
                            "PUSHNEW cannot use both :test and :test-not",
                            pair[0].span,
                        ));
                    }
                    test = Some(self.eval_in(&pair[1], environment)?);
                }
                "TEST-NOT" => {
                    if test.is_some() {
                        return Err(self.invalid(
                            "PUSHNEW cannot use both :test and :test-not",
                            pair[0].span,
                        ));
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
        let elements = current
            .list_items()
            .ok_or_else(|| self.invalid("PUSHNEW place must contain a proper list", items[2].span))?;

        let invert_test = test_not.is_some();
        let test_designator = test
            .or(test_not)
            .unwrap_or_else(|| Value::symbol("EQL"));
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
        let values = places
            .iter()
            .map(|place| self.eval_in(place, environment))
            .collect::<Result<Vec<_>, _>>()?;
        if values.len() > 1 {
            let mut rotated = Vec::with_capacity(values.len());
            rotated.push(values.last().cloned().unwrap_or(Value::Nil));
            rotated.extend(values[..values.len() - 1].iter().cloned());
            for (place, value) in places.iter().zip(rotated) {
                self.set_place(place, value, environment)?;
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
        let old_values = places
            .iter()
            .map(|place| self.eval_in(place, environment))
            .collect::<Result<Vec<_>, _>>()?;
        let new_value = self.eval_in(&items[items.len() - 1], environment)?;
        for (index, place) in places.iter().enumerate() {
            let value = old_values
                .get(index + 1)
                .cloned()
                .unwrap_or_else(|| new_value.clone());
            self.set_place(place, value, environment)?;
        }
        Ok(old_values.into_iter().next().unwrap_or(Value::Nil))
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
            && self
                .expand_symbol_macro_form(place, environment)?
                .is_none()
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
            return Err(self.invalid(
                "SETF expander must return five values",
                span,
            ));
        }
        let temporaries = self.setf_expansion_forms(&values[0], "temporary variables", span)?;
        let value_forms = self.setf_expansion_forms(&values[1], "value forms", span)?;
        if temporaries.len() != value_forms.len() {
            return Err(self.invalid(
                "SETF expansion temporary and value lists must have the same length",
                span,
            ));
        }
        let mut stores = self.setf_expansion_forms(&values[2], "store variables", span)?;
        if stores.len() != 1 {
            return Err(self.invalid(
                "SETF expansion must provide exactly one store variable",
                span,
            ));
        }
        Ok(SetfExpansion {
            temporaries,
            values: value_forms,
            store: stores.remove(0),
            store_form: self.form_from_value(&values[3], span)?,
            access_form: self.form_from_value(&values[4], span)?,
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
            Value::list(vec![self.quoted_value(&expansion.store)?]),
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
                vec![
                    Form::atom("SETQ", place.span),
                    place.clone(),
                    store.clone(),
                ],
                place.span,
            );
            return Ok(SetfExpansion {
                temporaries: Vec::new(),
                values: Vec::new(),
                store,
                store_form,
                access_form: place.clone(),
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
            store,
            store_form,
            access_form,
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
        let Some(container_index) = Self::modify_macro_container_index(
            operator,
            items.len().saturating_sub(1),
        ) else {
            return self.get_setf_expansion(place, environment);
        };

        let outer_temporaries = items[1..]
            .iter()
            .map(|_| self.fresh_setf_temporary(place.span))
            .collect::<Vec<_>>();
        let outer_values = items[1..].to_vec();
        let nested = self.get_modify_macro_setf_expansion(
            &outer_values[container_index],
            environment,
        )?;

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
                        vec![
                            nested.store.clone(),
                            outer_temporaries[container_index].clone(),
                        ],
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

        Ok(SetfExpansion {
            temporaries,
            values,
            store,
            store_form,
            access_form,
        })
    }

    fn modify_macro_container_index(operator: &str, argument_count: usize) -> Option<usize> {
        let index = match unqualified_name(operator).as_str() {
            "CAR" | "FIRST" | "CDR" | "REST" | "GETF" | "ELT" | "CHAR" | "SCHAR"
            | "BIT" | "AREF" | "ROW-MAJOR-AREF" | "SVREF" | "SUBSEQ" => 0,
            "NTH" => 1,
            _ => return None,
        };
        (index < argument_count).then_some(index)
    }

    fn apply_setf_expansion(
        &self,
        expansion: &SetfExpansion,
        value: Value,
        environment: &Environment,
        span: Span,
    ) -> Result<(), RuntimeError> {
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
        let (store_name, store_escaped) =
            self.variable_name_info(&expansion.store, "SETF store variable must be a symbol")?;
        self.define_variable_in(&store_name, store_escaped, value, &local);
        self.eval_in(&expansion.store_form, &local)?;
        Ok(())
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
                value,
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
        if environment.lookup_setf_expander(&lookup_name).is_some() {
            let expansion = self.get_setf_expansion(place, environment)?;
            return self.apply_setf_expansion(&expansion, value, environment, place.span);
        }
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
                        return Err(self.invalid(
                            "cannot SETF a read-only structure slot",
                            place.span,
                        ));
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
                let mut rebuilt = Vec::with_capacity(elements.len() + replacement.len());
                rebuilt.push(elements[0].clone());
                rebuilt.append(&mut replacement);
                self.set_place(&args[0], Value::list(rebuilt), environment)
            }
            "NTH" => {
                if args.len() != 2 {
                    return Err(self.arity("setf nth", "two", args.len()));
                }
                let index = self.setf_index(self.eval_in(&args[0], environment)?, args[0].span)?;
                let current = self.eval_in(&args[1], environment)?;
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
                    Value::Vector(_) => {
                        let mut elements = current.vector_items().unwrap_or_default();
                        let Some(slot) = elements.get_mut(index) else {
                            return Err(self.invalid("SETF index is out of bounds", args[1].span));
                        };
                        *slot = value;
                        self.set_place(&args[0], Value::vector(elements), environment)
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
                    Value::List(items) | Value::Vector(items) => items.as_ref().clone(),
                    Value::String(text) => text.chars().map(Value::Character).collect(),
                    other => {
                        return Err(RuntimeError::Type {
                            expected: "SEQUENCE".to_string(),
                            actual: other.type_name().to_string(),
                            span: Some(args[0].span),
                        });
                    }
                };
                let start =
                    self.setf_index(self.eval_in(&args[1], environment)?, args[1].span)?;
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
                    Value::List(items) | Value::Vector(items) => items.as_ref().clone(),
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
                destination[start..start + count].clone_from_slice(&replacement[..count]);

                let rebuilt = match &current {
                    Value::Nil | Value::List(_) => Value::list(destination),
                    Value::Vector(_) => Value::vector(destination),
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
                let Value::Vector(_) = &current else {
                    return Err(RuntimeError::Type {
                        expected: "SIMPLE-VECTOR".to_string(),
                        actual: current.type_name().to_string(),
                        span: Some(args[0].span),
                    });
                };
                let mut elements = current.vector_items().expect("vector items");
                let Some(slot) = elements.get_mut(index) else {
                    return Err(self.invalid("SETF index is out of bounds", args[1].span));
                };
                *slot = value;
                self.set_place(&args[0], Value::vector(elements), environment)
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
                    Value::Vector(_) => {
                        if indices.len() != 1 {
                            return Err(self.arity("setf aref", "two", args.len()));
                        }
                        let index = self.setf_index(indices[0].clone(), args[1].span)?;
                        let mut elements = current.vector_items().expect("vector items");
                        let Some(slot) = elements.get_mut(index) else {
                            return Err(self.invalid("SETF index is out of bounds", args[1].span));
                        };
                        *slot = value;
                        self.set_place(&args[0], Value::vector(elements), environment)
                    }
                    Value::Array { dimensions, .. } => {
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
                        let mut elements = current.array_items().expect("array items");
                        let Some(slot) = elements.get_mut(offset) else {
                            return Err(self.invalid("SETF index is out of bounds", place.span));
                        };
                        *slot = value;
                        self.set_place(
                            &args[0],
                            Value::array(dimensions.as_ref().clone(), elements),
                            environment,
                        )
                    }
                    other => Err(RuntimeError::Type {
                        expected: "ARRAY or VECTOR".to_string(),
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
                let index =
                    self.setf_index(self.eval_in(&args[1], environment)?, args[1].span)?;
                match &current {
                    Value::Vector(_) => {
                        let mut elements = current.vector_items().expect("vector items");
                        let Some(slot) = elements.get_mut(index) else {
                            return Err(self.invalid("SETF index is out of bounds", args[1].span));
                        };
                        *slot = value;
                        self.set_place(&args[0], Value::vector(elements), environment)
                    }
                    Value::Array { .. } => {
                        let mut elements = current.array_items().expect("array items");
                        let Some(slot) = elements.get_mut(index) else {
                            return Err(self.invalid("SETF index is out of bounds", args[1].span));
                        };
                        *slot = value;
                        let dimensions = current
                            .array_dimensions()
                            .expect("array dimensions");
                        self.set_place(
                            &args[0],
                            Value::array(dimensions, elements),
                            environment,
                        )
                    }
                    other => Err(RuntimeError::Type {
                        expected: "ARRAY or VECTOR".to_string(),
                        actual: other.type_name().to_string(),
                        span: Some(args[0].span),
                    }),
                }
            }
            "BIT" => {
                if args.is_empty() {
                    return Err(self.arity("setf bit", "array and subscripts", 0));
                }
                let current = self.eval_in(&args[0], environment)?;
                let dimensions = match &current {
                    Value::Vector(items) => vec![items.len()],
                    Value::Array { dimensions, .. } => dimensions.as_ref().clone(),
                    other => {
                        return Err(RuntimeError::Type {
                            expected: "ARRAY".to_string(),
                            actual: other.type_name().to_string(),
                            span: Some(args[0].span),
                        });
                    }
                };
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
                        .ok_or_else(|| self.invalid("SETF index is too large", place.span))?;
                    let contribution = index.checked_mul(stride).ok_or_else(|| {
                        self.invalid("SETF index is too large", place.span)
                    })?;
                    offset = offset.checked_add(contribution).ok_or_else(|| {
                        self.invalid("SETF index is too large", place.span)
                    })?;
                }
                if !matches!(&value, Value::Integer(bit) if *bit == 0 || *bit == 1) {
                    return Err(RuntimeError::Type {
                        expected: "BIT".to_string(),
                        actual: value.type_name().to_string(),
                        span: Some(place.span),
                    });
                }
                match &current {
                    Value::Vector(_) => {
                        let mut elements = current.vector_items().expect("vector items");
                        let Some(slot) = elements.get_mut(offset) else {
                            return Err(self.invalid("SETF index is out of bounds", place.span));
                        };
                        *slot = value;
                        self.set_place(&args[0], Value::vector(elements), environment)
                    }
                    Value::Array { .. } => {
                        let mut elements = current.array_items().expect("array items");
                        let Some(slot) = elements.get_mut(offset) else {
                            return Err(self.invalid("SETF index is out of bounds", place.span));
                        };
                        *slot = value;
                        let dimensions = current
                            .array_dimensions()
                            .expect("array dimensions");
                        self.set_place(
                            &args[0],
                            Value::array(dimensions, elements),
                            environment,
                        )
                    }
                    _ => unreachable!("bit array type checked above"),
                }
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
            "SYMBOL-FUNCTION" => {
                if args.len() != 1 {
                    return Err(self.arity("setf symbol-function", "one", args.len()));
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
                    self.invalid("setf symbol-function target must be a symbol", args[0].span)
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
                if args.len() != 2 {
                    return Err(self.arity("setf gethash", "two", args.len()));
                }
                let key = self.eval_in(&args[0], environment)?;
                let table = self.eval_in(&args[1], environment)?;
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
                if args.len() != 2 {
                    return Err(self.arity("setf getf", "two", args.len()));
                }
                let current = self.eval_in(&args[0], environment)?;
                let indicator = self.eval_in(&args[1], environment)?;
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
                    properties.push(indicator);
                    properties.push(value);
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
                    if message == "SETF target must be a symbol" => return Ok(()),
                Err(error) => return Err(error),
            }
        }

        if !matches!(destination.kind, FormKind::List(_)) {
            return Ok(());
        }

        match self.set_place(destination, value, environment) {
            Err(RuntimeError::InvalidForm { message, .. })
                if message == "unsupported SETF place" => Ok(()),
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
            return Err(self.arity(
                "defconstant",
                "two or three",
                items.len().saturating_sub(1),
            ));
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
            FormKind::List(name_and_options) if !name_and_options.is_empty() => (
                &name_and_options[0],
                &name_and_options[1..],
                &items[2..],
            ),
            _ => {
                return Err(self.invalid(
                    "defstruct name must be a symbol or a name-and-options list",
                    items[1].span,
                ));
            }
        };
        let (raw_name, _) = self.variable_name_info(name_form, "defstruct name must be a symbol")?;
        let structure_name = unqualified_name(&raw_name);
        let mut conc_name = format!("{structure_name}-");
        let mut predicate_name = Some(format!("{structure_name}-P"));
        let mut copier_name = Some(format!("COPY-{structure_name}"));
        let mut constructor_options: Vec<(Option<String>, Option<OrdinaryLambdaList>)> =
            Vec::new();
        let mut seen_options = HashSet::new();
        let mut included_structure: Option<(StructureDefinition, Vec<Form>)> = None;
        for option_form in option_forms {
            let FormKind::List(option_items) = &option_form.kind else {
                return Err(self.invalid("defstruct option must be a list", option_form.span));
            };
            let Some(option_name) = option_items.first().and_then(atom_name) else {
                return Err(self.invalid("defstruct option needs a name", option_form.span));
            };
            let normalized_option = normalize_name(option_name);
            let option_name = normalized_option.trim_start_matches(':');
            if option_name != "CONSTRUCTOR" && !seen_options.insert(option_name.to_string()) {
                return Err(self.invalid(
                    "defstruct cannot repeat an option",
                    option_form.span,
                ));
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
                    return Err(self.invalid(
                        "unsupported defstruct option",
                        option_items[0].span,
                    ));
                }
            }
        }
        let mut structure_types = vec![structure_name.clone()];
        let mut slots = Vec::new();
        let mut slot_names = HashSet::new();
        if let Some((parent, overrides)) = included_structure {
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
                return Err(self.invalid(
                    "defstruct cannot define duplicate slots",
                    slot_form.span,
                ));
            }
            slots.push(StructureSlot {
                name: slot_name,
                init_form,
                read_only: read_only.unwrap_or(false),
            });
        }

        environment.define_structure(
            &structure_name,
            StructureDefinition {
                slots: slots.clone(),
                type_names: structure_types.clone(),
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
        let mut default_initargs = Vec::new();

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

            if options.len() % 2 != 0 {
                return Err(self.invalid(
                    "defclass slot options require a value",
                    slot_form.span,
                ));
            }
            for option in options.chunks_exact(2) {
                let option_name = self.definition_name_from_form(&option[0], "defclass slot option")?;
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
                        if allocation == "CLASS" {
                            class_value = Some(Rc::new(RefCell::new(Value::Unbound)));
                        } else {
                            class_value = None;
                        }
                    }
                    "ACCESSOR" | "READER" => {
                        let accessor_name =
                            self.variable_name(&option[1], "defclass accessor must be a symbol")?;
                        readers.push((unqualified_name(&accessor_name), slot_name.clone()));
                    }
                    "WRITER" => {
                        let writer_name =
                            self.variable_name(&option[1], "defclass writer must be a symbol")?;
                        writers.push((unqualified_name(&writer_name), slot_name.clone()));
                    }
                    "TYPE" | "DOCUMENTATION" => {}
                    _ => {
                        return Err(self.invalid(
                            "unsupported defclass slot option",
                            option[0].span,
                        ));
                    }
                }
            }

            if let Some(existing) = slots.iter_mut().find(|slot| slot.name == slot_name) {
                existing.initarg = initarg;
                existing.init_form = init_form;
                existing.class_value = class_value;
            } else {
                slots.push(ClassSlot {
                    name: slot_name,
                    initarg,
                    init_form,
                    class_value,
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
                        let initarg = self
                            .definition_name_from_form(&pair[0], "defclass default initarg")?;
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
                        return Err(self.invalid(
                            "defclass :documentation needs one string",
                            option.span,
                        ));
                    }
                }
                _ => {}
            }
        }

        let mut precedence = vec![class_name.clone()];
        for superclass in &direct_superclasses {
            if superclass == "OBJECT" || superclass == "STANDARD-OBJECT" {
                if !precedence.iter().any(|name| name == "STANDARD-OBJECT") {
                    precedence.push("STANDARD-OBJECT".to_owned());
                }
                continue;
            }
            let Some(definition) = environment.lookup_class(superclass) else {
                return Err(self.invalid("unknown defclass superclass", items[2].span));
            };
            for name in &definition.precedence {
                if !precedence.iter().any(|existing| existing == name) {
                    precedence.push(name.clone());
                }
            }
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
        if !precedence.iter().any(|name| name == "STANDARD-OBJECT") {
            precedence.push("STANDARD-OBJECT".to_owned());
        }

        let definition = Rc::new(ClassDefinition {
            name: class_name.clone(),
            direct_superclasses,
            precedence,
            slots,
            default_initargs,
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
                self.invalid(
                    "defmethod requires a method lambda list",
                    items[1].span,
                )
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
            let (parameter_name, escaped) = self.variable_name_info(
                name_form,
                "defmethod parameter must be a variable",
            )?;
            required.push(unqualified_name(&parameter_name));
            required_escaped.push(escaped);
            let specializer = match specializer_form {
                None => "T".to_owned(),
                Some(form) => self.definition_name_from_form(form, "defmethod specializer")?,
            };
            if specializer != "T"
                && specializer != "OBJECT"
                && specializer != "STANDARD-OBJECT"
                && environment.lookup_class(&specializer).is_none()
            {
                return Err(self.invalid("unknown defmethod specializer", parameter.span));
            }
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
        methods.borrow_mut().push(MethodDefinition {
            qualifiers,
            specializers,
            function: closure,
        });
        Ok(Value::symbol(name))
    }

    fn list_form_items<'a>(
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

    fn definition_name_from_form(
        &self,
        form: &Form,
        context: &str,
    ) -> Result<String, RuntimeError> {
        let Some(raw_name) = atom_name(form) else {
            return Err(self.invalid(context, form.span));
        };
        let token = parse_symbol_token(raw_name).map_err(|_| self.invalid(context, form.span))?;
        if !matches!(token.kind, SymbolTokenKind::Symbol | SymbolTokenKind::Keyword)
            || token.name.is_empty()
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
        Ok(unqualified_name(normalized.trim_start_matches(':')))
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
        let constructor_lambda_list = option_items.get(2).map(|lambda_list_form| {
            if constructor_name.is_none() {
                return Err(self.invalid(
                    "defstruct :constructor NIL cannot have a lambda list",
                    lambda_list_form.span,
                ));
            }
            self.parameters(lambda_list_form)
        }).transpose()?;
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
                let slot_name = self.variable_name_info(
                    &slot_items[0],
                    "defstruct slot name must be a symbol",
                )?;
                let read_only = slot_items
                    .get(2)
                    .map(|form| self.eval_in(form, environment).map(|value| value.is_truthy()))
                    .transpose()?;
                Ok((
                    slot_name.0,
                    slot_items.get(1).cloned(),
                    read_only,
                ))
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
            Shadow(String),
            Intern(String),
            Import {
                source_package: String,
                source_name: String,
                shadowing: bool,
            },
        }

        let name = self.package_name_from_form(&items[1])?;
        let mut nicknames = Vec::new();
        let mut use_packages = vec![package::COMMON_LISP_PACKAGE.to_string()];
        let mut exports = HashSet::new();
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
                        return Err(self.invalid(
                            "defpackage has duplicate :nicknames options",
                            option.span,
                        ));
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
                        return Err(self.invalid(
                            "defpackage :documentation needs one string",
                            option.span,
                        ));
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
                        exports.insert(self.symbol_name_from_form(symbol_form)?);
                    }
                }
                "SHADOW" => {
                    for symbol_form in option_items.iter().skip(1) {
                        operations.push(DefpackageOperation::Shadow(
                            self.symbol_name_from_form(symbol_form)?,
                        ));
                    }
                }
                "INTERN" => {
                    for symbol_form in option_items.iter().skip(1) {
                        operations.push(DefpackageOperation::Intern(
                            self.symbol_name_from_form(symbol_form)?,
                        ));
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
                    let shadowing = normalized_option.trim_start_matches(':')
                        == "SHADOWING-IMPORT-FROM";
                    for symbol_form in option_items.iter().skip(2) {
                        operations.push(DefpackageOperation::Import {
                            source_package: source_package.clone(),
                            source_name: self.symbol_name_from_form(symbol_form)?,
                            shadowing,
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
                if !packages.symbol_exists(source_package, source_name) {
                    return Err(self.package_error(
                        &format!(
                            "unknown symbol {source_package}::{source_name}"
                        ),
                        items[1].span,
                    ));
                }
            }
        }

        let mut packages = self.packages.borrow_mut();
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
        for operation in operations {
            match operation {
                DefpackageOperation::Shadow(symbol) => packages.shadow_symbol(&name, &symbol),
                DefpackageOperation::Intern(symbol) => {
                    let _ = packages.intern_symbol(&name, &symbol);
                }
                DefpackageOperation::Import {
                    source_package,
                    source_name,
                    shadowing,
                } => packages.import_symbol(&source_package, &source_name, &name, shadowing),
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
        Ok(Value::package(&canonical_name))
    }

    fn package_name_from_form(&self, form: &Form) -> Result<String, RuntimeError> {
        let raw = match &form.kind {
            FormKind::Atom(value) | FormKind::String(value) => value.as_str(),
            _ => {
                return Err(self.invalid("package name must be a symbol or string", form.span));
            }
        };
        if !raw.starts_with(':') && package::split_symbol(raw).is_some() {
            return Err(self.package_error("package name cannot be qualified", form.span));
        }
        let name = package::normalize_package_name(raw);
        if name.is_empty() || name.contains(':') {
            return Err(self.package_error("invalid package name", form.span));
        }
        Ok(name)
    }

    fn symbol_name_from_form(&self, form: &Form) -> Result<String, RuntimeError> {
        let raw = match &form.kind {
            FormKind::Atom(value) | FormKind::String(value) => value.as_str(),
            _ => return Err(self.invalid("symbol name must be a symbol or string", form.span)),
        };
        let name = raw.strip_prefix(':').unwrap_or(raw);
        if name.is_empty() || name.contains(':') {
            return Err(self.package_error("symbol name cannot be qualified", form.span));
        }
        Ok(normalize_name(name))
    }

    fn package_designator_name(
        &self,
        value: &Value,
        span: Span,
    ) -> Result<String, RuntimeError> {
        let raw = match value {
            Value::Package(name) | Value::String(name) => name.as_ref(),
            _ => value.symbol_name().ok_or_else(|| RuntimeError::Type {
                expected: "PACKAGE DESIGNATOR".to_string(),
                actual: value.type_name().to_string(),
                span: Some(span),
            })?,
        };
        if package::split_symbol(raw).is_some() {
            return Err(self.package_error("package name cannot be qualified", span));
        }
        let name = package::normalize_package_name(raw);
        if name.is_empty() || name.contains(':') {
            return Err(self.package_error("invalid package name", span));
        }
        Ok(name)
    }

    fn package_name_from_value(&self, value: &Value, span: Span) -> Result<String, RuntimeError> {
        let name = self.package_designator_name(value, span)?;
        let packages = self.packages.borrow();
        if !packages.package_exists(&name) {
            return Err(self.package_error(&format!("unknown package {name}"), span));
        }
        Ok(packages.canonical_package_name(&name))
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

    fn name_designator_from_value(
        &self,
        value: &Value,
        span: Span,
    ) -> Result<String, RuntimeError> {
        let raw = match value {
            Value::String(name) => name.as_ref(),
            _ => value.symbol_name().ok_or_else(|| RuntimeError::Type {
                expected: "SYMBOL DESIGNATOR".to_string(),
                actual: value.type_name().to_string(),
                span: Some(span),
            })?,
        };
        let name = raw.strip_prefix(':').unwrap_or(raw);
        if name.is_empty() {
            return Err(self.invalid("symbol name cannot be empty", span));
        }
        Ok(unqualified_name(name))
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

    fn symbol_import_references_from_value(
        &self,
        value: &Value,
        span: Span,
    ) -> Result<Vec<(String, String)>, RuntimeError> {
        let values = value
            .list_items()
            .ok_or_else(|| self.invalid("symbol designators must be a proper list", span))?;
        values
            .iter()
            .map(|value| {
                if matches!(value, Value::UninternedSymbol(_)) {
                    return Err(self.invalid("uninterned symbols cannot be imported", span));
                }
                let raw = value.symbol_name().ok_or_else(|| RuntimeError::Type {
                    expected: "SYMBOL".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(span),
                })?;
                if matches!(value, Value::Keyword(_) | Value::KeywordExact(_)) {
                    return Ok((
                        package::KEYWORD_PACKAGE.to_string(),
                        package::normalize_symbol_name(raw),
                    ));
                }
                if let Some((package_name, symbol_name, _)) = package::split_symbol(raw) {
                    return Ok((
                        package::normalize_package_name(package_name),
                        package::normalize_symbol_name(symbol_name),
                    ));
                }
                Ok((
                    self.current_package(),
                    package::normalize_symbol_name(raw),
                ))
            })
            .collect()
    }

    fn package_symbol_value(&self, package_name: &str, symbol_name: &str) -> Value {
        let package_name = self
            .packages
            .borrow()
            .canonical_package_name(package_name);
        if package_name == package::KEYWORD_PACKAGE {
            Value::keyword(symbol_name)
        } else {
            let symbol_name = self
                .packages
                .borrow()
                .imported_symbol_name(&package_name, symbol_name);
            Value::symbol(symbol_name)
        }
    }

    fn symbol_status_value(status: package::SymbolStatus) -> Value {
        match status {
            package::SymbolStatus::Internal => Value::keyword("INTERNAL"),
            package::SymbolStatus::External => Value::keyword("EXTERNAL"),
        }
    }

    fn special_funcall(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(self.arity("funcall", "at least one", 0));
        }
        let function = self.eval_in(&items[1], environment)?;
        let arguments = items[2..]
            .iter()
            .map(|form| self.eval_in(form, environment))
            .collect::<Result<Vec<_>, _>>()?;
        self.apply_in(&function, &arguments, items[0].span, environment)
    }

    fn special_eval(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 2 {
            return Err(self.arity("eval", "one", items.len().saturating_sub(1)));
        }
        let value = self.eval_in(&items[1], environment)?;
        let form = self.form_from_value(&value, items[1].span)?;
        self.eval_values_in(&form, environment)
    }

    fn special_apply(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(self.arity("apply", "at least two", items.len().saturating_sub(1)));
        }
        let function = self.eval_in(&items[1], environment)?;
        let evaluated = items[2..]
            .iter()
            .map(|form| self.eval_in(form, environment))
            .collect::<Result<Vec<_>, _>>()?;
        let Some(last) = evaluated.last() else {
            return Err(self.invalid("apply needs a final list", items[0].span));
        };
        let Some(mut final_arguments) = last.list_items() else {
            return Err(self.invalid("apply's final argument must be a list", items[0].span));
        };
        let mut arguments = evaluated[..evaluated.len() - 1].to_vec();
        arguments.append(&mut final_arguments);
        self.apply_in(&function, &arguments, items[0].span, environment)
    }

    fn resolve_function_designator(
        &self,
        function: &Value,
        span: Span,
        environment: &Environment,
    ) -> Result<Rc<crate::Function>, RuntimeError> {
        if let Value::Function(function) = function {
            return Ok(function.clone());
        }

        let Some((name, exact)) = function.symbol_reference() else {
            return Err(RuntimeError::NotCallable {
                value: function.to_string(),
                span: Some(span),
            });
        };
        let resolved = if exact {
            self.lookup_function_exact_in(name, environment)
        } else {
            self.lookup_function_in(name, environment)
        };
        match resolved {
            Some(Value::Function(function)) => Ok(function),
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

    fn apply_list_mapping(
        &self,
        operation: &str,
        function: &Value,
        sequences: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let (uses_tails, concatenates, returns_first) = match operation {
            "MAPC" => (false, false, true),
            "MAPCAR" => (false, false, false),
            "MAPL" => (true, false, true),
            "MAPLIST" => (true, false, false),
            "MAPCAN" => (false, true, false),
            "MAPCON" => (true, true, false),
            _ => return Err(self.invalid("unknown list mapping operation", span)),
        };
        let operation_name = operation.to_ascii_lowercase();
        let lists = sequences
            .iter()
            .map(|value| {
                value.list_items().ok_or_else(|| {
                    self.invalid(
                        &format!("{operation_name} arguments must be lists"),
                        span,
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let length = lists.iter().map(Vec::len).min().unwrap_or(0);
        let mut results = Vec::with_capacity(length);
        for index in 0..length {
            let arguments = if uses_tails {
                lists
                    .iter()
                    .map(|items| Value::list(items[index..].to_vec()))
                    .collect::<Vec<_>>()
            } else {
                lists
                    .iter()
                    .map(|items| items[index].clone())
                    .collect::<Vec<_>>()
            };
            let result = self
                .apply_in(function, &arguments, span, environment)?
                .primary_value();
            if concatenates {
                let items = result.list_items().ok_or_else(|| {
                    self.invalid(
                        &format!("{operation_name} function results must be lists"),
                        span,
                    )
                })?;
                results.extend(items);
            } else if !returns_first {
                results.push(result);
            }
        }
        if returns_first {
            Ok(sequences.first().cloned().unwrap_or(Value::Nil))
        } else {
            Ok(Value::list(results))
        }
    }

    fn apply_sequence_mapping(
        &self,
        result_type: &Value,
        function: &Value,
        sequences: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let result_type_name = result_type.symbol_name().map(normalize_name);
        let result_kind = match result_type_name.as_deref() {
            Some("NIL") => "NIL",
            Some("LIST") => "LIST",
            Some("VECTOR") | Some("SIMPLE-VECTOR") => "VECTOR",
            Some("STRING") | Some("SIMPLE-STRING") => "STRING",
            _ => {
                return Err(self.invalid(
                    "map result type must be LIST, VECTOR, STRING, or NIL",
                    span,
                ));
            }
        };
        let function = Value::Function(self.resolve_function_designator(
            function,
            span,
            environment,
        )?);
        let sequences = sequences
            .iter()
            .map(|value| match value {
                Value::Nil => Ok(Vec::new()),
                Value::List(items) | Value::Vector(items) => Ok(items.as_ref().clone()),
                Value::String(value) => Ok(value.chars().map(Value::Character).collect()),
                value => Err(RuntimeError::Type {
                    expected: "SEQUENCE".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(span),
                }),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let length = sequences.iter().map(Vec::len).min().unwrap_or(0);
        let mut results = Vec::with_capacity(length);
        for index in 0..length {
            let arguments = sequences
                .iter()
                .map(|items| items[index].clone())
                .collect::<Vec<_>>();
            let result = self
                .apply_in(&function, &arguments, span, environment)?
                .primary_value();
            if result_kind != "NIL" {
                results.push(result);
            }
        }
        match result_kind {
            "NIL" => Ok(Value::Nil),
            "LIST" => Ok(Value::list(results)),
            "VECTOR" => Ok(Value::vector(results)),
            "STRING" => {
                let mut string = String::new();
                for value in results {
                    let Value::Character(character) = value else {
                        return Err(RuntimeError::Type {
                            expected: "CHARACTER".to_string(),
                            actual: value.type_name().to_string(),
                            span: Some(span),
                        });
                    };
                    string.push(character);
                }
                Ok(Value::string(string))
            }
            _ => unreachable!("validated MAP result type"),
        }
    }

    fn apply_sequence_reduce(
        &self,
        function: &Value,
        sequence: &Value,
        options: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if options.len() % 2 != 0 {
            return Err(self.invalid("reduce keyword arguments must be supplied in pairs", span));
        }

        let mut from_end = false;
        let mut start = 0;
        let mut end = None;
        let mut initial_value = None;
        let mut key = None;

        let index_argument = |option: &str, value: &Value| -> Result<usize, RuntimeError> {
            let Value::Integer(index) = value else {
                return Err(RuntimeError::Type {
                    expected: "INTEGER".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(span),
                });
            };
            if *index < 0 {
                return Err(self.invalid(
                    &format!("reduce {option} must be non-negative"),
                    span,
                ));
            }
            usize::try_from(*index).map_err(|_| {
                self.invalid(&format!("reduce {option} is out of range"), span)
            })
        };

        for pair in options.chunks_exact(2) {
            let keyword_name = match &pair[0] {
                Value::Keyword(keyword) | Value::KeywordExact(keyword) => normalize_name(keyword),
                _ => {
                    return Err(self.invalid(
                        "reduce keyword argument name must be a keyword",
                        span,
                    ));
                }
            };
            match keyword_name.as_str() {
                "FROM-END" => from_end = pair[1].is_truthy(),
                "START" => start = index_argument(":start", &pair[1])?,
                "END" => end = Some(index_argument(":end", &pair[1])?),
                "INITIAL-VALUE" => initial_value = Some(pair[1].clone()),
                "KEY" => key = Some(pair[1].clone()),
                _ => {
                    return Err(RuntimeError::InvalidForm {
                        message: format!("unknown reduce keyword :{keyword_name}"),
                        span: Some(span),
                    });
                }
            }
        }

        let function = Value::Function(self.resolve_function_designator(
            function,
            span,
            environment,
        )?);
        let items = match sequence {
            Value::Nil => Vec::new(),
            Value::List(items) | Value::Vector(items) => items.as_ref().clone(),
            Value::String(value) => value.chars().map(Value::Character).collect(),
            value => {
                return Err(RuntimeError::Type {
                    expected: "SEQUENCE".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(span),
                });
            }
        };
        let end = end.unwrap_or(items.len());
        if start > end || end > items.len() {
            return Err(self.invalid("reduce sequence bounds are invalid", span));
        }

        let key_function = match key {
            Some(value) if value.is_truthy() => Some(self.resolve_function_designator(
                &value,
                span,
                environment,
            )?),
            _ => None,
        };
        let apply_key = |value: &Value| -> Result<Value, RuntimeError> {
            match &key_function {
                Some(key_function) => self
                    .apply_in(
                        &Value::Function(key_function.clone()),
                        std::slice::from_ref(value),
                        span,
                        environment,
                    )
                    .map(|result| result.primary_value()),
                None => Ok(value.clone()),
            }
        };

        let selected = &items[start..end];
        if selected.is_empty() {
            return initial_value
                .ok_or_else(|| self.invalid("reduce of an empty sequence", span));
        }

        if from_end {
            let mut values = selected.iter().rev();
            let mut accumulator = match initial_value {
                Some(value) => value,
                None => apply_key(values.next().expect("non-empty REDUCE selection"))?,
            };
            for value in values {
                let value = apply_key(value)?;
                accumulator = self
                    .apply_in(&function, &[value, accumulator], span, environment)?
                    .primary_value();
            }
            Ok(accumulator)
        } else {
            let mut values = selected.iter();
            let mut accumulator = match initial_value {
                Some(value) => value,
                None => apply_key(values.next().expect("non-empty REDUCE selection"))?,
            };
            for value in values {
                let value = apply_key(value)?;
                accumulator = self
                    .apply_in(&function, &[accumulator, value], span, environment)?
                    .primary_value();
            }
            Ok(accumulator)
        }
    }

    fn apply_sequence_search(
        &self,
        operation: &str,
        item: &Value,
        sequence: &Value,
        options: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if options.len() % 2 != 0 {
            return Err(self.invalid(
                "sequence search keyword arguments must be supplied in pairs",
                span,
            ));
        }

        let mut from_end = false;
        let mut test = None;
        let mut test_not = None;
        let mut key = None;
        let mut start = 0;
        let mut end = None;

        let index_argument = |option: &str, value: &Value| -> Result<usize, RuntimeError> {
            let Value::Integer(index) = value else {
                return Err(RuntimeError::Type {
                    expected: "INTEGER".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(span),
                });
            };
            if *index < 0 {
                return Err(self.invalid(
                    &format!("sequence search {option} must be non-negative"),
                    span,
                ));
            }
            usize::try_from(*index).map_err(|_| {
                self.invalid(&format!("sequence search {option} is out of range"), span)
            })
        };

        for pair in options.chunks_exact(2) {
            let keyword_name = match &pair[0] {
                Value::Keyword(keyword) | Value::KeywordExact(keyword) => normalize_name(keyword),
                _ => {
                    return Err(self.invalid(
                        "sequence search keyword argument name must be a keyword",
                        span,
                    ));
                }
            };
            match keyword_name.as_str() {
                "FROM-END" => from_end = pair[1].is_truthy(),
                "TEST" => {
                    if test_not.is_some() {
                        return Err(self.invalid(
                            "sequence search cannot use both :test and :test-not",
                            span,
                        ));
                    }
                    test = Some(pair[1].clone());
                }
                "TEST-NOT" => {
                    if test.is_some() {
                        return Err(self.invalid(
                            "sequence search cannot use both :test and :test-not",
                            span,
                        ));
                    }
                    test_not = Some(pair[1].clone());
                }
                "KEY" => key = Some(pair[1].clone()),
                "START" => start = index_argument(":start", &pair[1])?,
                "END" => {
                    end = match &pair[1] {
                        Value::Nil => None,
                        value => Some(index_argument(":end", value)?),
                    }
                }
                _ => {
                    return Err(RuntimeError::InvalidForm {
                        message: format!("unknown sequence search keyword :{keyword_name}"),
                        span: Some(span),
                    });
                }
            }
        }

        let items = match sequence {
            Value::Nil => Vec::new(),
            Value::List(items) | Value::Vector(items) => items.as_ref().clone(),
            Value::String(value) => value.chars().map(Value::Character).collect(),
            value => {
                return Err(RuntimeError::Type {
                    expected: "SEQUENCE".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(span),
                });
            }
        };
        let end = end.unwrap_or(items.len());
        if start > end || end > items.len() {
            return Err(self.invalid("sequence search bounds are invalid", span));
        }

        let invert_test = test_not.is_some();
        let test_designator = test
            .or(test_not)
            .unwrap_or_else(|| Value::symbol("EQL"));
        let test_function = Value::Function(self.resolve_function_designator(
            &test_designator,
            span,
            environment,
        )?);
        let key_function = match key {
            Some(value) if value.is_truthy() => Some(self.resolve_function_designator(
                &value,
                span,
                environment,
            )?),
            _ => None,
        };

        let indexes: Vec<usize> = if from_end {
            (start..end).rev().collect()
        } else {
            (start..end).collect()
        };
        let mut count = 0;
        for index in indexes {
            let candidate = match &key_function {
                Some(key_function) => self
                    .apply_in(
                        &Value::Function(key_function.clone()),
                        std::slice::from_ref(&items[index]),
                        span,
                        environment,
                    )?
                    .primary_value(),
                None => items[index].clone(),
            };
            let matches = self
                .apply_in(
                    &test_function,
                    &[item.clone(), candidate],
                    span,
                    environment,
                )?
                .primary_value()
                .is_truthy();
            let matches = if invert_test { !matches } else { matches };
            if matches {
                match operation {
                    "FIND" => return Ok(items[index].clone()),
                    "POSITION" => return Ok(Value::Integer(index as i64)),
                    "COUNT" => count += 1,
                    _ => return Err(self.invalid("unknown sequence search operation", span)),
                }
            }
        }

        match operation {
            "FIND" => Ok(Value::Nil),
            "POSITION" => Ok(Value::Nil),
            "COUNT" => Ok(Value::Integer(count)),
            _ => Err(self.invalid("unknown sequence search operation", span)),
        }
    }

    fn apply_sequence_pair_search(
        &self,
        operation: &str,
        sequence1: &Value,
        sequence2: &Value,
        options: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if !matches!(operation, "SEARCH" | "MISMATCH") {
            return Err(self.invalid("unknown sequence pair search operation", span));
        }
        if options.len() % 2 != 0 {
            return Err(self.invalid(
                "sequence pair search keyword arguments must be supplied in pairs",
                span,
            ));
        }

        let mut from_end = false;
        let mut test = None;
        let mut test_not = None;
        let mut key = None;
        let mut start1 = 0;
        let mut start2 = 0;
        let mut end1 = None;
        let mut end2 = None;

        let index_argument = |option: &str, value: &Value| -> Result<usize, RuntimeError> {
            let Value::Integer(index) = value else {
                return Err(RuntimeError::Type {
                    expected: "INTEGER".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(span),
                });
            };
            if *index < 0 {
                return Err(self.invalid(
                    &format!("sequence pair search {option} must be non-negative"),
                    span,
                ));
            }
            usize::try_from(*index).map_err(|_| {
                self.invalid(
                    &format!("sequence pair search {option} is out of range"),
                    span,
                )
            })
        };

        for pair in options.chunks_exact(2) {
            let keyword_name = match &pair[0] {
                Value::Keyword(keyword) | Value::KeywordExact(keyword) => normalize_name(keyword),
                _ => {
                    return Err(self.invalid(
                        "sequence pair search keyword argument name must be a keyword",
                        span,
                    ));
                }
            };
            match keyword_name.as_str() {
                "FROM-END" => from_end = pair[1].is_truthy(),
                "TEST" => {
                    if test_not.is_some() {
                        return Err(self.invalid(
                            "sequence pair search cannot use both :test and :test-not",
                            span,
                        ));
                    }
                    test = Some(pair[1].clone());
                }
                "TEST-NOT" => {
                    if test.is_some() {
                        return Err(self.invalid(
                            "sequence pair search cannot use both :test and :test-not",
                            span,
                        ));
                    }
                    test_not = Some(pair[1].clone());
                }
                "KEY" => key = Some(pair[1].clone()),
                "START1" => start1 = index_argument(":start1", &pair[1])?,
                "END1" => {
                    end1 = match &pair[1] {
                        Value::Nil => None,
                        value => Some(index_argument(":end1", value)?),
                    }
                }
                "START2" => start2 = index_argument(":start2", &pair[1])?,
                "END2" => {
                    end2 = match &pair[1] {
                        Value::Nil => None,
                        value => Some(index_argument(":end2", value)?),
                    }
                }
                _ => {
                    return Err(RuntimeError::InvalidForm {
                        message: format!("unknown sequence pair search keyword :{keyword_name}"),
                        span: Some(span),
                    });
                }
            }
        }

        let items1 = match sequence1 {
            Value::Nil => Vec::new(),
            Value::List(items) | Value::Vector(items) => items.as_ref().clone(),
            Value::String(value) => value.chars().map(Value::Character).collect(),
            value => {
                return Err(RuntimeError::Type {
                    expected: "SEQUENCE".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(span),
                });
            }
        };
        let items2 = match sequence2 {
            Value::Nil => Vec::new(),
            Value::List(items) | Value::Vector(items) => items.as_ref().clone(),
            Value::String(value) => value.chars().map(Value::Character).collect(),
            value => {
                return Err(RuntimeError::Type {
                    expected: "SEQUENCE".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(span),
                });
            }
        };

        let end1 = end1.unwrap_or(items1.len());
        let end2 = end2.unwrap_or(items2.len());
        if start1 > end1 || end1 > items1.len() || start2 > end2 || end2 > items2.len() {
            return Err(self.invalid("sequence pair search bounds are invalid", span));
        }

        let invert_test = test_not.is_some();
        let test_designator = test
            .or(test_not)
            .unwrap_or_else(|| Value::symbol("EQL"));
        let test_function = Value::Function(self.resolve_function_designator(
            &test_designator,
            span,
            environment,
        )?);
        let key_function = match key {
            Some(value) if value.is_truthy() => Some(self.resolve_function_designator(
                &value,
                span,
                environment,
            )?),
            _ => None,
        };
        let apply_key = |value: &Value| -> Result<Value, RuntimeError> {
            match &key_function {
                Some(key_function) => self
                    .apply_in(
                        &Value::Function(key_function.clone()),
                        std::slice::from_ref(value),
                        span,
                        environment,
                    )
                    .map(|result| result.primary_value()),
                None => Ok(value.clone()),
            }
        };
        let elements_match = |left: &Value, right: &Value| -> Result<bool, RuntimeError> {
            let left = apply_key(left)?;
            let right = apply_key(right)?;
            let matches = self
                .apply_in(&test_function, &[left, right], span, environment)?
                .primary_value()
                .is_truthy();
            Ok(if invert_test { !matches } else { matches })
        };

        let length1 = end1 - start1;
        let length2 = end2 - start2;
        match operation {
            "SEARCH" => {
                if length1 > length2 {
                    return Ok(Value::Nil);
                }
                let last_start = end2 - length1;
                if from_end {
                    for candidate in (start2..=last_start).rev() {
                        let mut matches = true;
                        for offset in 0..length1 {
                            if !elements_match(
                                &items1[start1 + offset],
                                &items2[candidate + offset],
                            )? {
                                matches = false;
                                break;
                            }
                        }
                        if matches {
                            return Ok(Value::Integer(candidate as i64));
                        }
                    }
                } else {
                    for candidate in start2..=last_start {
                        let mut matches = true;
                        for offset in 0..length1 {
                            if !elements_match(
                                &items1[start1 + offset],
                                &items2[candidate + offset],
                            )? {
                                matches = false;
                                break;
                            }
                        }
                        if matches {
                            return Ok(Value::Integer(candidate as i64));
                        }
                    }
                }
                Ok(Value::Nil)
            }
            "MISMATCH" => {
                let compared_length = length1.min(length2);
                if from_end {
                    for offset in 0..compared_length {
                        let index1 = end1 - 1 - offset;
                        let index2 = end2 - 1 - offset;
                        if !elements_match(&items1[index1], &items2[index2])? {
                            return Ok(Value::Integer((index1 + 1) as i64));
                        }
                    }
                    if length1 == length2 {
                        Ok(Value::Nil)
                    } else {
                        Ok(Value::Integer(
                            (start1 + length1.saturating_sub(length2)) as i64,
                        ))
                    }
                } else {
                    for offset in 0..compared_length {
                        let index1 = start1 + offset;
                        let index2 = start2 + offset;
                        if !elements_match(&items1[index1], &items2[index2])? {
                            return Ok(Value::Integer(index1 as i64));
                        }
                    }
                    if length1 == length2 {
                        Ok(Value::Nil)
                    } else {
                        Ok(Value::Integer((start1 + compared_length) as i64))
                    }
                }
            }
            _ => Err(self.invalid("unknown sequence pair search operation", span)),
        }
    }

    fn apply_sequence_sort(
        &self,
        operation: &str,
        sequence: &Value,
        predicate: &Value,
        options: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if !matches!(operation, "SORT" | "STABLE-SORT") {
            return Err(self.invalid("unknown sequence sort operation", span));
        }
        if options.len() % 2 != 0 {
            return Err(self.invalid(
                "sequence sort keyword arguments must be supplied in pairs",
                span,
            ));
        }

        let mut key = None;
        for pair in options.chunks_exact(2) {
            let keyword_name = match &pair[0] {
                Value::Keyword(keyword) | Value::KeywordExact(keyword) => normalize_name(keyword),
                _ => {
                    return Err(self.invalid(
                        "sequence sort keyword argument name must be a keyword",
                        span,
                    ));
                }
            };
            match keyword_name.as_str() {
                "KEY" => key = Some(pair[1].clone()),
                _ => {
                    return Err(RuntimeError::InvalidForm {
                        message: format!("unknown sequence sort keyword :{keyword_name}"),
                        span: Some(span),
                    });
                }
            }
        }

        enum SequenceKind {
            List,
            Vector,
            String,
        }
        let (kind, items) = match sequence {
            Value::Nil => (SequenceKind::List, Vec::new()),
            Value::List(items) => (SequenceKind::List, items.as_ref().clone()),
            Value::Vector(items) => (SequenceKind::Vector, items.as_ref().clone()),
            Value::String(value) => (
                SequenceKind::String,
                value.chars().map(Value::Character).collect(),
            ),
            value => {
                return Err(RuntimeError::Type {
                    expected: "SEQUENCE".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(span),
                });
            }
        };

        let predicate = Value::Function(self.resolve_function_designator(
            predicate,
            span,
            environment,
        )?);
        let key_function = match key {
            Some(value) if value.is_truthy() => Some(self.resolve_function_designator(
                &value,
                span,
                environment,
            )?),
            _ => None,
        };

        let mut sorted: Vec<(Value, Value)> = Vec::with_capacity(items.len());
        for item in items {
            let item_key = match &key_function {
                Some(key_function) => self
                    .apply_in(
                        &Value::Function(key_function.clone()),
                        std::slice::from_ref(&item),
                        span,
                        environment,
                    )?
                    .primary_value(),
                None => item.clone(),
            };
            let mut insert_at = sorted.len();
            for (index, (_, existing_key)) in sorted.iter().enumerate() {
                let precedes = self
                    .apply_in(
                        &predicate,
                        &[item_key.clone(), existing_key.clone()],
                        span,
                        environment,
                    )?
                    .primary_value()
                    .is_truthy();
                if precedes {
                    insert_at = index;
                    break;
                }
            }
            sorted.insert(insert_at, (item, item_key));
        }

        let result = sorted
            .into_iter()
            .map(|(item, _)| item)
            .collect::<Vec<_>>();
        match kind {
            SequenceKind::List => Ok(Value::list(result)),
            SequenceKind::Vector => Ok(Value::vector(result)),
            SequenceKind::String => {
                let mut value = String::new();
                for item in result {
                    let Value::Character(character) = item else {
                        return Err(RuntimeError::Type {
                            expected: "CHARACTER".to_string(),
                            actual: item.type_name().to_string(),
                            span: Some(span),
                        });
                    };
                    value.push(character);
                }
                Ok(Value::string(value))
            }
        }
    }

    fn apply_sequence_merge(
        &self,
        result_type: &Value,
        sequence1: &Value,
        sequence2: &Value,
        predicate: &Value,
        options: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if options.len() % 2 != 0 {
            return Err(self.invalid(
                "merge keyword arguments must be supplied in pairs",
                span,
            ));
        }

        let mut key = None;
        for pair in options.chunks_exact(2) {
            let keyword_name = match &pair[0] {
                Value::Keyword(keyword) | Value::KeywordExact(keyword) => normalize_name(keyword),
                _ => {
                    return Err(self.invalid(
                        "merge keyword argument name must be a keyword",
                        span,
                    ));
                }
            };
            match keyword_name.as_str() {
                "KEY" => key = Some(pair[1].clone()),
                _ => {
                    return Err(RuntimeError::InvalidForm {
                        message: format!("unknown merge keyword :{keyword_name}"),
                        span: Some(span),
                    });
                }
            }
        }

        let result_type_name = result_type.symbol_name().map(normalize_name);
        let result_kind = match result_type_name.as_deref() {
            Some("NIL") => "NIL",
            Some("LIST") => "LIST",
            Some("VECTOR") | Some("SIMPLE-VECTOR") => "VECTOR",
            Some("STRING") | Some("SIMPLE-STRING") => "STRING",
            _ => {
                return Err(self.invalid(
                    "merge result type must be LIST, VECTOR, STRING, or NIL",
                    span,
                ));
            }
        };

        let sequence_items = |value: &Value| match value {
            Value::Nil => Ok(Vec::new()),
            Value::List(items) | Value::Vector(items) => Ok(items.as_ref().clone()),
            Value::String(value) => Ok(value.chars().map(Value::Character).collect()),
            value => Err(RuntimeError::Type {
                expected: "SEQUENCE".to_string(),
                actual: value.type_name().to_string(),
                span: Some(span),
            }),
        };
        let items1 = sequence_items(sequence1)?;
        let items2 = sequence_items(sequence2)?;

        let predicate = Value::Function(self.resolve_function_designator(
            predicate,
            span,
            environment,
        )?);
        let key_function = match key {
            Some(value) if value.is_truthy() => Some(self.resolve_function_designator(
                &value,
                span,
                environment,
            )?),
            _ => None,
        };

        let mut keyed1 = Vec::with_capacity(items1.len());
        for item in items1 {
            let item_key = match &key_function {
                Some(key_function) => self
                    .apply_in(
                        &Value::Function(key_function.clone()),
                        std::slice::from_ref(&item),
                        span,
                        environment,
                    )?
                    .primary_value(),
                None => item.clone(),
            };
            keyed1.push((item, item_key));
        }

        let mut keyed2 = Vec::with_capacity(items2.len());
        for item in items2 {
            let item_key = match &key_function {
                Some(key_function) => self
                    .apply_in(
                        &Value::Function(key_function.clone()),
                        std::slice::from_ref(&item),
                        span,
                        environment,
                    )?
                    .primary_value(),
                None => item.clone(),
            };
            keyed2.push((item, item_key));
        }

        let mut merged = Vec::with_capacity(keyed1.len() + keyed2.len());
        let mut index1 = 0;
        let mut index2 = 0;
        while index1 < keyed1.len() && index2 < keyed2.len() {
            let (_, first_key) = &keyed1[index1];
            let (_, second_key) = &keyed2[index2];
            let second_precedes = self
                .apply_in(
                    &predicate,
                    &[second_key.clone(), first_key.clone()],
                    span,
                    environment,
                )?
                .primary_value()
                .is_truthy();
            if second_precedes {
                merged.push(keyed2[index2].0.clone());
                index2 += 1;
            } else {
                merged.push(keyed1[index1].0.clone());
                index1 += 1;
            }
        }
        merged.extend(
            keyed1[index1..]
                .iter()
                .map(|(item, _)| item.clone()),
        );
        merged.extend(
            keyed2[index2..]
                .iter()
                .map(|(item, _)| item.clone()),
        );

        match result_kind {
            "NIL" => Ok(Value::Nil),
            "LIST" => Ok(Value::list(merged)),
            "VECTOR" => Ok(Value::vector(merged)),
            "STRING" => {
                let mut value = String::new();
                for item in merged {
                    let Value::Character(character) = item else {
                        return Err(RuntimeError::Type {
                            expected: "CHARACTER".to_string(),
                            actual: item.type_name().to_string(),
                            span: Some(span),
                        });
                    };
                    value.push(character);
                }
                Ok(Value::string(value))
            }
            _ => unreachable!("validated MERGE result type"),
        }
    }

    fn apply_sequence_quantifier(
        &self,
        operation: &str,
        predicate: &Value,
        sequences: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if !matches!(operation, "EVERY" | "SOME" | "NOTANY" | "NOTEVERY") {
            return Err(self.invalid("unknown sequence quantifier operation", span));
        }

        let predicate = Value::Function(self.resolve_function_designator(
            predicate,
            span,
            environment,
        )?);
        let sequences = sequences
            .iter()
            .map(|value| match value {
                Value::Nil => Ok(Vec::new()),
                Value::List(items) | Value::Vector(items) => Ok(items.as_ref().clone()),
                Value::String(value) => Ok(value.chars().map(Value::Character).collect()),
                value => Err(RuntimeError::Type {
                    expected: "SEQUENCE".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(span),
                }),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let length = sequences.iter().map(Vec::len).min().unwrap_or(0);

        for index in 0..length {
            let arguments = sequences
                .iter()
                .map(|items| items[index].clone())
                .collect::<Vec<_>>();
            let result = self
                .apply_in(&predicate, &arguments, span, environment)?
                .primary_value();
            match operation {
                "SOME" if result.is_truthy() => return Ok(result),
                "EVERY" if !result.is_truthy() => return Ok(Value::Nil),
                "NOTANY" if result.is_truthy() => return Ok(Value::Nil),
                "NOTEVERY" if !result.is_truthy() => return Ok(Value::boolean(true)),
                _ => {}
            }
        }

        match operation {
            "EVERY" | "NOTANY" => Ok(Value::boolean(true)),
            "SOME" | "NOTEVERY" => Ok(Value::Nil),
            _ => Err(self.invalid("unknown sequence quantifier operation", span)),
        }
    }

    fn apply_list_membership(
        &self,
        operation: &str,
        item_or_predicate: &Value,
        list: &Value,
        options: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if !matches!(
            operation,
            "MEMBER" | "MEMBER-IF" | "MEMBER-IF-NOT" | "ADJOIN"
        ) {
            return Err(self.invalid("unknown list membership operation", span));
        }
        if options.len() % 2 != 0 {
            return Err(self.invalid(
                "list membership keyword arguments must be supplied in pairs",
                span,
            ));
        }

        let is_predicate = matches!(operation, "MEMBER-IF" | "MEMBER-IF-NOT");
        let mut test = None;
        let mut test_not = None;
        let mut key = None;
        for pair in options.chunks_exact(2) {
            let keyword_name = match &pair[0] {
                Value::Keyword(keyword) | Value::KeywordExact(keyword) => normalize_name(keyword),
                _ => {
                    return Err(self.invalid(
                        "list membership keyword argument name must be a keyword",
                        span,
                    ));
                }
            };
            match keyword_name.as_str() {
                "KEY" => key = Some(pair[1].clone()),
                "TEST" if !is_predicate => {
                    if test_not.is_some() {
                        return Err(self.invalid(
                            "list membership cannot use both :test and :test-not",
                            span,
                        ));
                    }
                    test = Some(pair[1].clone());
                }
                "TEST-NOT" if !is_predicate => {
                    if test.is_some() {
                        return Err(self.invalid(
                            "list membership cannot use both :test and :test-not",
                            span,
                        ));
                    }
                    test_not = Some(pair[1].clone());
                }
                _ => {
                    return Err(RuntimeError::InvalidForm {
                        message: format!("unknown list membership keyword :{keyword_name}"),
                        span: Some(span),
                    });
                }
            }
        }

        let Some(items) = list.list_items() else {
            return Err(RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: list.type_name().to_string(),
                span: Some(span),
            });
        };
        let invert_test = test_not.is_some() || operation == "MEMBER-IF-NOT";
        let test_designator = if is_predicate {
            item_or_predicate.clone()
        } else {
            test.or(test_not)
                .unwrap_or_else(|| Value::symbol("EQL"))
        };
        let test_function = Value::Function(self.resolve_function_designator(
            &test_designator,
            span,
            environment,
        )?);
        let key_function = match key {
            Some(value) if value.is_truthy() => Some(self.resolve_function_designator(
                &value,
                span,
                environment,
            )?),
            _ => None,
        };

        for index in 0..items.len() {
            let candidate = match &key_function {
                Some(key_function) => self
                    .apply_in(
                        &Value::Function(key_function.clone()),
                        std::slice::from_ref(&items[index]),
                        span,
                        environment,
                    )?
                    .primary_value(),
                None => items[index].clone(),
            };
            let matches = if is_predicate {
                self.apply_in(
                    &test_function,
                    std::slice::from_ref(&candidate),
                    span,
                    environment,
                )?
                .primary_value()
                .is_truthy()
            } else {
                self.apply_in(
                    &test_function,
                    &[item_or_predicate.clone(), candidate],
                    span,
                    environment,
                )?
                .primary_value()
                .is_truthy()
            };
            let matches = if invert_test { !matches } else { matches };
            if matches {
                return match operation {
                    "ADJOIN" => Ok(list.clone()),
                    "MEMBER" | "MEMBER-IF" | "MEMBER-IF-NOT" => {
                        Ok(Value::list(items[index..].to_vec()))
                    }
                    _ => Err(self.invalid("unknown list membership operation", span)),
                };
            }
        }

        if operation == "ADJOIN" {
            let mut result = Vec::with_capacity(items.len() + 1);
            result.push(item_or_predicate.clone());
            result.extend(items);
            Ok(Value::list(result))
        } else {
            Ok(Value::Nil)
        }
    }

    fn association_entry_parts(entry: &Value) -> Option<(Value, Value)> {
        match entry {
            Value::List(items) => {
                let (key, rest) = items.split_first()?;
                Some((key.clone(), Value::list(rest.to_vec())))
            }
            Value::DottedList { items, tail } => {
                let (key, rest) = items.split_first()?;
                let value = if rest.is_empty() {
                    tail.as_ref().clone()
                } else {
                    Value::dotted_list(rest.to_vec(), tail.as_ref().clone())
                };
                Some((key.clone(), value))
            }
            _ => None,
        }
    }

    fn apply_association_search(
        &self,
        operation: &str,
        item_or_predicate: &Value,
        alist: &Value,
        options: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if !matches!(
            operation,
            "ASSOC"
                | "ASSOC-IF"
                | "ASSOC-IF-NOT"
                | "RASSOC"
                | "RASSOC-IF"
                | "RASSOC-IF-NOT"
        ) {
            return Err(self.invalid("unknown association search operation", span));
        }
        if options.len() % 2 != 0 {
            return Err(self.invalid(
                "association search keyword arguments must be supplied in pairs",
                span,
            ));
        }

        let is_predicate = matches!(
            operation,
            "ASSOC-IF" | "ASSOC-IF-NOT" | "RASSOC-IF" | "RASSOC-IF-NOT"
        );
        let reverse = matches!(
            operation,
            "RASSOC" | "RASSOC-IF" | "RASSOC-IF-NOT"
        );
        let mut test = None;
        let mut test_not = None;
        let mut key = None;
        for pair in options.chunks_exact(2) {
            let keyword_name = match &pair[0] {
                Value::Keyword(keyword) | Value::KeywordExact(keyword) => normalize_name(keyword),
                _ => {
                    return Err(self.invalid(
                        "association search keyword argument name must be a keyword",
                        span,
                    ));
                }
            };
            match keyword_name.as_str() {
                "KEY" => key = Some(pair[1].clone()),
                "TEST" if !is_predicate => {
                    if test_not.is_some() {
                        return Err(self.invalid(
                            "association search cannot use both :test and :test-not",
                            span,
                        ));
                    }
                    test = Some(pair[1].clone());
                }
                "TEST-NOT" if !is_predicate => {
                    if test.is_some() {
                        return Err(self.invalid(
                            "association search cannot use both :test and :test-not",
                            span,
                        ));
                    }
                    test_not = Some(pair[1].clone());
                }
                _ => {
                    return Err(RuntimeError::InvalidForm {
                        message: format!("unknown association search keyword :{keyword_name}"),
                        span: Some(span),
                    });
                }
            }
        }

        let Some(entries) = alist.list_items() else {
            return Err(RuntimeError::Type {
                expected: "ASSOCIATION LIST".to_string(),
                actual: alist.type_name().to_string(),
                span: Some(span),
            });
        };
        let invert_test = test_not.is_some()
            || matches!(operation, "ASSOC-IF-NOT" | "RASSOC-IF-NOT");
        let test_designator = if is_predicate {
            item_or_predicate.clone()
        } else {
            test.or(test_not)
                .unwrap_or_else(|| Value::symbol("EQL"))
        };
        let test_function = Value::Function(self.resolve_function_designator(
            &test_designator,
            span,
            environment,
        )?);
        let key_function = match key {
            Some(value) if value.is_truthy() => Some(self.resolve_function_designator(
                &value,
                span,
                environment,
            )?),
            _ => None,
        };

        for entry in entries {
            let Some((entry_key, entry_value)) = Self::association_entry_parts(&entry) else {
                return Err(RuntimeError::Type {
                    expected: "ASSOCIATION LIST ENTRY".to_string(),
                    actual: entry.type_name().to_string(),
                    span: Some(span),
                });
            };
            let candidate = if reverse { entry_value } else { entry_key };
            let candidate = match &key_function {
                Some(key_function) => self
                    .apply_in(
                        &Value::Function(key_function.clone()),
                        std::slice::from_ref(&candidate),
                        span,
                        environment,
                    )?
                    .primary_value(),
                None => candidate,
            };
            let matches = if is_predicate {
                self.apply_in(
                    &test_function,
                    std::slice::from_ref(&candidate),
                    span,
                    environment,
                )?
                .primary_value()
                .is_truthy()
            } else {
                self.apply_in(
                    &test_function,
                    &[item_or_predicate.clone(), candidate],
                    span,
                    environment,
                )?
                .primary_value()
                .is_truthy()
            };
            let matches = if invert_test { !matches } else { matches };
            if matches {
                return Ok(entry);
            }
        }
        Ok(Value::Nil)
    }

    fn apply_sequence_remove(
        &self,
        operation: &str,
        item_or_predicate: &Value,
        sequence: &Value,
        options: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if !matches!(
            operation,
            "REMOVE"
                | "REMOVE-IF"
                | "REMOVE-IF-NOT"
                | "DELETE"
                | "DELETE-IF"
                | "DELETE-IF-NOT"
                | "REMOVE-DUPLICATES"
                | "DELETE-DUPLICATES"
        ) {
            return Err(self.invalid("unknown sequence removal operation", span));
        }
        if options.len() % 2 != 0 {
            return Err(self.invalid(
                "sequence removal keyword arguments must be supplied in pairs",
                span,
            ));
        }

        let is_predicate = matches!(
            operation,
            "REMOVE-IF" | "REMOVE-IF-NOT" | "DELETE-IF" | "DELETE-IF-NOT"
        );
        let removes_duplicates = matches!(
            operation,
            "REMOVE-DUPLICATES" | "DELETE-DUPLICATES"
        );
        let mut from_end = false;
        let mut test = None;
        let mut test_not = None;
        let mut key = None;
        let mut start = 0;
        let mut end = None;
        let mut count = None;

        let index_argument = |option: &str, value: &Value| -> Result<usize, RuntimeError> {
            let Value::Integer(index) = value else {
                return Err(RuntimeError::Type {
                    expected: "INTEGER".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(span),
                });
            };
            if *index < 0 {
                return Err(self.invalid(
                    &format!("sequence removal {option} must be non-negative"),
                    span,
                ));
            }
            usize::try_from(*index).map_err(|_| {
                self.invalid(&format!("sequence removal {option} is out of range"), span)
            })
        };

        for pair in options.chunks_exact(2) {
            let keyword_name = match &pair[0] {
                Value::Keyword(keyword) | Value::KeywordExact(keyword) => normalize_name(keyword),
                _ => {
                    return Err(self.invalid(
                        "sequence removal keyword argument name must be a keyword",
                        span,
                    ));
                }
            };
            match keyword_name.as_str() {
                "FROM-END" => from_end = pair[1].is_truthy(),
                "TEST" if !is_predicate => {
                    if test_not.is_some() {
                        return Err(self.invalid(
                            "sequence removal cannot use both :test and :test-not",
                            span,
                        ));
                    }
                    test = Some(pair[1].clone());
                }
                "TEST-NOT" if !is_predicate => {
                    if test.is_some() {
                        return Err(self.invalid(
                            "sequence removal cannot use both :test and :test-not",
                            span,
                        ));
                    }
                    test_not = Some(pair[1].clone());
                }
                "KEY" => key = Some(pair[1].clone()),
                "START" => start = index_argument(":start", &pair[1])?,
                "END" => {
                    end = match &pair[1] {
                        Value::Nil => None,
                        value => Some(index_argument(":end", value)?),
                    }
                }
                "COUNT" if !removes_duplicates => {
                    count = match &pair[1] {
                        Value::Nil => None,
                        value => Some(index_argument(":count", value)?),
                    };
                }
                _ => {
                    return Err(RuntimeError::InvalidForm {
                        message: format!("unknown sequence removal keyword :{keyword_name}"),
                        span: Some(span),
                    });
                }
            }
        }

        enum SequenceKind {
            List,
            Vector,
            String,
        }
        let (kind, items) = match sequence {
            Value::Nil => (SequenceKind::List, Vec::new()),
            Value::List(items) => (SequenceKind::List, items.as_ref().clone()),
            Value::Vector(items) => (SequenceKind::Vector, items.as_ref().clone()),
            Value::String(value) => (
                SequenceKind::String,
                value.chars().map(Value::Character).collect(),
            ),
            value => {
                return Err(RuntimeError::Type {
                    expected: "SEQUENCE".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(span),
                });
            }
        };
        let end = end.unwrap_or(items.len());
        if start > end || end > items.len() {
            return Err(self.invalid("sequence removal bounds are invalid", span));
        }

        let invert_test = test_not.is_some()
            || matches!(operation, "REMOVE-IF-NOT" | "DELETE-IF-NOT");
        let test_designator = if is_predicate {
            item_or_predicate.clone()
        } else {
            test.or(test_not)
                .unwrap_or_else(|| Value::symbol("EQL"))
        };
        let test_function = Value::Function(self.resolve_function_designator(
            &test_designator,
            span,
            environment,
        )?);
        let key_function = match key {
            Some(value) if value.is_truthy() => Some(self.resolve_function_designator(
                &value,
                span,
                environment,
            )?),
            _ => None,
        };
        let mut candidates = items.clone();
        for index in start..end {
            candidates[index] = match &key_function {
                Some(key_function) => self
                    .apply_in(
                        &Value::Function(key_function.clone()),
                        std::slice::from_ref(&items[index]),
                        span,
                        environment,
                    )?
                    .primary_value(),
                None => items[index].clone(),
            };
        }

        let mut remove = vec![false; items.len()];
        if removes_duplicates {
            let mut kept: Vec<usize> = Vec::new();
            if from_end {
                for index in (start..end).rev() {
                    let mut duplicate = false;
                    for kept_index in &kept {
                        let matches = self
                            .apply_in(
                                &test_function,
                                &[candidates[index].clone(), candidates[*kept_index].clone()],
                                span,
                                environment,
                            )?
                            .primary_value()
                            .is_truthy();
                        duplicate = if invert_test { !matches } else { matches };
                        if duplicate {
                            break;
                        }
                    }
                    if duplicate {
                        remove[index] = true;
                    } else {
                        kept.push(index);
                    }
                }
            } else {
                for index in start..end {
                    let mut duplicate = false;
                    for kept_index in &kept {
                        let matches = self
                            .apply_in(
                                &test_function,
                                &[candidates[index].clone(), candidates[*kept_index].clone()],
                                span,
                                environment,
                            )?
                            .primary_value()
                            .is_truthy();
                        duplicate = if invert_test { !matches } else { matches };
                        if duplicate {
                            break;
                        }
                    }
                    if duplicate {
                        remove[index] = true;
                    } else {
                        kept.push(index);
                    }
                }
            }
        } else {
            let mut matched = Vec::new();
            for index in start..end {
                let matches = if is_predicate {
                    self.apply_in(
                        &test_function,
                        std::slice::from_ref(&candidates[index]),
                        span,
                        environment,
                    )?
                    .primary_value()
                    .is_truthy()
                } else {
                    self.apply_in(
                        &test_function,
                        &[item_or_predicate.clone(), candidates[index].clone()],
                        span,
                        environment,
                    )?
                    .primary_value()
                    .is_truthy()
                };
                let matches = if invert_test { !matches } else { matches };
                if matches {
                    matched.push(index);
                }
            }
            let limit = count.unwrap_or(matched.len()).min(matched.len());
            if from_end {
                for index in matched.into_iter().rev().take(limit) {
                    remove[index] = true;
                }
            } else {
                for index in matched.into_iter().take(limit) {
                    remove[index] = true;
                }
            }
        }

        let result = items
            .into_iter()
            .enumerate()
            .filter_map(|(index, value)| (!remove[index]).then_some(value))
            .collect::<Vec<_>>();
        match kind {
            SequenceKind::List => Ok(Value::list(result)),
            SequenceKind::Vector => Ok(Value::vector(result)),
            SequenceKind::String => {
                let mut value = String::new();
                for item in result {
                    let Value::Character(character) = item else {
                        return Err(RuntimeError::Type {
                            expected: "CHARACTER".to_string(),
                            actual: item.type_name().to_string(),
                            span: Some(span),
                        });
                    };
                    value.push(character);
                }
                Ok(Value::string(value))
            }
        }
    }

    fn apply_sequence_substitute(
        &self,
        operation: &str,
        new_item: &Value,
        old_or_predicate: &Value,
        sequence: &Value,
        options: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if !matches!(
            operation,
            "SUBSTITUTE"
                | "SUBSTITUTE-IF"
                | "SUBSTITUTE-IF-NOT"
                | "NSUBSTITUTE"
                | "NSUBSTITUTE-IF"
                | "NSUBSTITUTE-IF-NOT"
        ) {
            return Err(self.invalid("unknown sequence substitution operation", span));
        }
        if options.len() % 2 != 0 {
            return Err(self.invalid(
                "sequence substitution keyword arguments must be supplied in pairs",
                span,
            ));
        }

        let is_predicate = matches!(
            operation,
            "SUBSTITUTE-IF" | "SUBSTITUTE-IF-NOT" | "NSUBSTITUTE-IF" | "NSUBSTITUTE-IF-NOT"
        );
        let mut from_end = false;
        let mut test = None;
        let mut test_not = None;
        let mut key = None;
        let mut start = 0;
        let mut end = None;
        let mut count = None;

        let index_argument = |option: &str, value: &Value| -> Result<usize, RuntimeError> {
            let Value::Integer(index) = value else {
                return Err(RuntimeError::Type {
                    expected: "INTEGER".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(span),
                });
            };
            if *index < 0 {
                return Err(self.invalid(
                    &format!("sequence substitution {option} must be non-negative"),
                    span,
                ));
            }
            usize::try_from(*index).map_err(|_| {
                self.invalid(&format!("sequence substitution {option} is out of range"), span)
            })
        };

        for pair in options.chunks_exact(2) {
            let keyword_name = match &pair[0] {
                Value::Keyword(keyword) | Value::KeywordExact(keyword) => normalize_name(keyword),
                _ => {
                    return Err(self.invalid(
                        "sequence substitution keyword argument name must be a keyword",
                        span,
                    ));
                }
            };
            match keyword_name.as_str() {
                "FROM-END" => from_end = pair[1].is_truthy(),
                "TEST" if !is_predicate => {
                    if test_not.is_some() {
                        return Err(self.invalid(
                            "sequence substitution cannot use both :test and :test-not",
                            span,
                        ));
                    }
                    test = Some(pair[1].clone());
                }
                "TEST-NOT" if !is_predicate => {
                    if test.is_some() {
                        return Err(self.invalid(
                            "sequence substitution cannot use both :test and :test-not",
                            span,
                        ));
                    }
                    test_not = Some(pair[1].clone());
                }
                "KEY" => key = Some(pair[1].clone()),
                "START" => start = index_argument(":start", &pair[1])?,
                "END" => {
                    end = match &pair[1] {
                        Value::Nil => None,
                        value => Some(index_argument(":end", value)?),
                    }
                }
                "COUNT" => {
                    count = match &pair[1] {
                        Value::Nil => None,
                        value => Some(index_argument(":count", value)?),
                    };
                }
                _ => {
                    return Err(RuntimeError::InvalidForm {
                        message: format!("unknown sequence substitution keyword :{keyword_name}"),
                        span: Some(span),
                    });
                }
            }
        }

        enum SequenceKind {
            List,
            Vector,
            String,
        }
        let (kind, items) = match sequence {
            Value::Nil => (SequenceKind::List, Vec::new()),
            Value::List(items) => (SequenceKind::List, items.as_ref().clone()),
            Value::Vector(items) => (SequenceKind::Vector, items.as_ref().clone()),
            Value::String(value) => (
                SequenceKind::String,
                value.chars().map(Value::Character).collect(),
            ),
            value => {
                return Err(RuntimeError::Type {
                    expected: "SEQUENCE".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(span),
                });
            }
        };
        if matches!(kind, SequenceKind::String) && !matches!(new_item, Value::Character(_)) {
            return Err(RuntimeError::Type {
                expected: "CHARACTER".to_string(),
                actual: new_item.type_name().to_string(),
                span: Some(span),
            });
        }
        let end = end.unwrap_or(items.len());
        if start > end || end > items.len() {
            return Err(self.invalid("sequence substitution bounds are invalid", span));
        }

        let invert_test = test_not.is_some()
            || matches!(operation, "SUBSTITUTE-IF-NOT" | "NSUBSTITUTE-IF-NOT");
        let test_designator = if is_predicate {
            old_or_predicate.clone()
        } else {
            test.or(test_not)
                .unwrap_or_else(|| Value::symbol("EQL"))
        };
        let test_function = Value::Function(self.resolve_function_designator(
            &test_designator,
            span,
            environment,
        )?);
        let key_function = match key {
            Some(value) if value.is_truthy() => Some(self.resolve_function_designator(
                &value,
                span,
                environment,
            )?),
            _ => None,
        };
        let mut candidates = items.clone();
        for index in start..end {
            candidates[index] = match &key_function {
                Some(key_function) => self
                    .apply_in(
                        &Value::Function(key_function.clone()),
                        std::slice::from_ref(&items[index]),
                        span,
                        environment,
                    )?
                    .primary_value(),
                None => items[index].clone(),
            };
        }

        let mut matched = Vec::new();
        for index in start..end {
            let matches = if is_predicate {
                self.apply_in(
                    &test_function,
                    std::slice::from_ref(&candidates[index]),
                    span,
                    environment,
                )?
                .primary_value()
                .is_truthy()
            } else {
                self.apply_in(
                    &test_function,
                    &[old_or_predicate.clone(), candidates[index].clone()],
                    span,
                    environment,
                )?
                .primary_value()
                .is_truthy()
            };
            let matches = if invert_test { !matches } else { matches };
            if matches {
                matched.push(index);
            }
        }

        let limit = count.unwrap_or(matched.len()).min(matched.len());
        let mut replace = vec![false; items.len()];
        if from_end {
            for index in matched.into_iter().rev().take(limit) {
                replace[index] = true;
            }
        } else {
            for index in matched.into_iter().take(limit) {
                replace[index] = true;
            }
        }

        let result = items
            .into_iter()
            .enumerate()
            .map(|(index, value)| {
                if replace[index] {
                    new_item.clone()
                } else {
                    value
                }
            })
            .collect::<Vec<_>>();
        match kind {
            SequenceKind::List => Ok(Value::list(result)),
            SequenceKind::Vector => Ok(Value::vector(result)),
            SequenceKind::String => {
                let mut value = String::new();
                for item in result {
                    let Value::Character(character) = item else {
                        return Err(RuntimeError::Type {
                            expected: "CHARACTER".to_string(),
                            actual: item.type_name().to_string(),
                            span: Some(span),
                        });
                    };
                    value.push(character);
                }
                Ok(Value::string(value))
            }
        }
    }

    fn apply_sequence_map_into(
        &self,
        destination: &Value,
        function: &Value,
        sequences: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let (result_kind, mut result) = match destination {
            Value::Nil => ("NIL", Vec::new()),
            Value::List(items) => ("LIST", items.as_ref().clone()),
            Value::Vector(items) => ("VECTOR", items.as_ref().clone()),
            Value::String(value) => (
                "STRING",
                value.chars().map(Value::Character).collect::<Vec<_>>(),
            ),
            value => {
                return Err(RuntimeError::Type {
                    expected: "SEQUENCE".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(span),
                });
            }
        };
        let function = Value::Function(self.resolve_function_designator(
            function,
            span,
            environment,
        )?);
        let sequences = sequences
            .iter()
            .map(|value| match value {
                Value::Nil => Ok(Vec::new()),
                Value::List(items) | Value::Vector(items) => Ok(items.as_ref().clone()),
                Value::String(value) => Ok(value.chars().map(Value::Character).collect()),
                value => Err(RuntimeError::Type {
                    expected: "SEQUENCE".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(span),
                }),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let length = sequences
            .iter()
            .map(Vec::len)
            .fold(result.len(), |length, sequence_length| {
                length.min(sequence_length)
            });
        for index in 0..length {
            let arguments = sequences
                .iter()
                .map(|items| items[index].clone())
                .collect::<Vec<_>>();
            let value = self
                .apply_in(&function, &arguments, span, environment)?
                .primary_value();
            if result_kind == "STRING" && !matches!(value, Value::Character(_)) {
                return Err(RuntimeError::Type {
                    expected: "CHARACTER".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(span),
                });
            }
            result[index] = value;
        }
        match result_kind {
            "NIL" => Ok(Value::Nil),
            "LIST" => Ok(Value::list(result)),
            "VECTOR" => Ok(Value::vector(result)),
            "STRING" => {
                let mut string = String::new();
                for value in result {
                    let Value::Character(character) = value else {
                        return Err(RuntimeError::Type {
                            expected: "CHARACTER".to_string(),
                            actual: value.type_name().to_string(),
                            span: Some(span),
                        });
                    };
                    string.push(character);
                }
                Ok(Value::string(string))
            }
            _ => unreachable!("validated MAP-INTO destination type"),
        }
    }

    fn apply_list_set_operation(
        &self,
        operation: &str,
        first: &Value,
        second: &Value,
        options: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if !matches!(
            operation,
            "UNION"
                | "NUNION"
                | "INTERSECTION"
                | "NINTERSECTION"
                | "SET-DIFFERENCE"
                | "NSET-DIFFERENCE"
                | "SET-EXCLUSIVE-OR"
                | "NSET-EXCLUSIVE-OR"
                | "SUBSETP"
        ) {
            return Err(self.invalid("unknown list set operation", span));
        }
        if options.len() % 2 != 0 {
            return Err(self.invalid(
                "list set operation keyword arguments must be supplied in pairs",
                span,
            ));
        }

        let mut test = None;
        let mut test_not = None;
        let mut key = None;
        for pair in options.chunks_exact(2) {
            let keyword_name = match &pair[0] {
                Value::Keyword(keyword) | Value::KeywordExact(keyword) => normalize_name(keyword),
                _ => {
                    return Err(self.invalid(
                        "list set operation keyword argument name must be a keyword",
                        span,
                    ));
                }
            };
            match keyword_name.as_str() {
                "TEST" => {
                    if test_not.is_some() {
                        return Err(self.invalid(
                            "list set operation cannot use both :test and :test-not",
                            span,
                        ));
                    }
                    test = Some(pair[1].clone());
                }
                "TEST-NOT" => {
                    if test.is_some() {
                        return Err(self.invalid(
                            "list set operation cannot use both :test and :test-not",
                            span,
                        ));
                    }
                    test_not = Some(pair[1].clone());
                }
                "KEY" => key = Some(pair[1].clone()),
                _ => {
                    return Err(RuntimeError::InvalidForm {
                        message: format!("unknown list set operation keyword :{keyword_name}"),
                        span: Some(span),
                    });
                }
            }
        }

        let first_items = first.list_items().ok_or_else(|| RuntimeError::Type {
            expected: "LIST".to_string(),
            actual: first.type_name().to_string(),
            span: Some(span),
        })?;
        let second_items = second.list_items().ok_or_else(|| RuntimeError::Type {
            expected: "LIST".to_string(),
            actual: second.type_name().to_string(),
            span: Some(span),
        })?;

        let invert_test = test_not.is_some();
        let test_designator = test
            .or(test_not)
            .unwrap_or_else(|| Value::symbol("EQL"));
        let test_function = Value::Function(self.resolve_function_designator(
            &test_designator,
            span,
            environment,
        )?);
        let key_function = match key {
            Some(value) if value.is_truthy() => Some(Value::Function(
                self.resolve_function_designator(&value, span, environment)?,
            )),
            _ => None,
        };

        let mut first_keys = Vec::with_capacity(first_items.len());
        for item in &first_items {
            first_keys.push(match &key_function {
                Some(key_function) => self
                    .apply_in(
                        key_function,
                        std::slice::from_ref(item),
                        span,
                        environment,
                    )?
                    .primary_value(),
                None => item.clone(),
            });
        }
        let mut second_keys = Vec::with_capacity(second_items.len());
        for item in &second_items {
            second_keys.push(match &key_function {
                Some(key_function) => self
                    .apply_in(
                        key_function,
                        std::slice::from_ref(item),
                        span,
                        environment,
                    )?
                    .primary_value(),
                None => item.clone(),
            });
        }

        let contains_key = |key: &Value, candidates: &[Value]| -> Result<bool, RuntimeError> {
            for candidate in candidates {
                let equal = self
                    .apply_in(
                        &test_function,
                        &[key.clone(), candidate.clone()],
                        span,
                        environment,
                    )?
                    .primary_value()
                    .is_truthy();
                if if invert_test { !equal } else { equal } {
                    return Ok(true);
                }
            }
            Ok(false)
        };

        if operation == "SUBSETP" {
            for key in &first_keys {
                if !contains_key(key, &second_keys)? {
                    return Ok(Value::Nil);
                }
            }
            return Ok(Value::boolean(true));
        }

        let mut result = Vec::new();
        let mut result_keys = Vec::new();
        let mut append_unique = |item: &Value, key: &Value| -> Result<(), RuntimeError> {
            if !contains_key(key, &result_keys)? {
                result.push(item.clone());
                result_keys.push(key.clone());
            }
            Ok(())
        };

        match operation {
            "UNION" | "NUNION" => {
                for (item, key) in first_items.iter().zip(&first_keys) {
                    append_unique(item, key)?;
                }
                for (item, key) in second_items.iter().zip(&second_keys) {
                    append_unique(item, key)?;
                }
            }
            "INTERSECTION" | "NINTERSECTION" => {
                for (item, key) in first_items.iter().zip(&first_keys) {
                    if contains_key(key, &second_keys)? {
                        append_unique(item, key)?;
                    }
                }
            }
            "SET-DIFFERENCE" | "NSET-DIFFERENCE" => {
                for (item, key) in first_items.iter().zip(&first_keys) {
                    if !contains_key(key, &second_keys)? {
                        append_unique(item, key)?;
                    }
                }
            }
            "SET-EXCLUSIVE-OR" | "NSET-EXCLUSIVE-OR" => {
                for (item, key) in first_items.iter().zip(&first_keys) {
                    if !contains_key(key, &second_keys)? {
                        append_unique(item, key)?;
                    }
                }
                for (item, key) in second_items.iter().zip(&second_keys) {
                    if !contains_key(key, &first_keys)? {
                        append_unique(item, key)?;
                    }
                }
            }
            _ => return Err(self.invalid("unknown list set operation", span)),
        }

        Ok(Value::list(result))
    }

    fn special_mapcar(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(self.arity("mapcar", "at least two", items.len().saturating_sub(1)));
        }
        let function = self.eval_in(&items[1], environment)?;
        let sequences = items[2..]
            .iter()
            .map(|form| self.eval_in(form, environment))
            .collect::<Result<Vec<_>, _>>()?;
        self.apply_list_mapping(
            "MAPCAR",
            &function,
            &sequences,
            environment,
            items[0].span,
        )
    }

    fn special_map_into(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(self.arity("map-into", "at least two", items.len().saturating_sub(1)));
        }
        let destination_form = &items[1];
        let destination = self.eval_in(destination_form, environment)?;
        let function = self.eval_in(&items[2], environment)?;
        let sequences = items[3..]
            .iter()
            .map(|form| self.eval_in(form, environment))
            .collect::<Result<Vec<_>, _>>()?;
        let result = self.apply_sequence_map_into(
            &destination,
            &function,
            &sequences,
            environment,
            items[0].span,
        )?;
        self.set_map_into_destination(destination_form, result.clone(), environment)?;
        Ok(result)
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
                    *class_value.borrow_mut() = value.clone();
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
            slots.push((slot.name.clone(), value));
        }
        let instance = Value::instance(class.clone(), slots);
        for (initarg, value) in initargs {
            let Some(index) = class.slots.iter().position(|slot| {
                slot.initarg.as_deref() == Some(initarg.as_str())
            }) else {
                return Err(self.invalid("unknown make-instance initarg", span));
            };
            if !instance.set_instance_slot(&class.name, &class.slots[index].name, value) {
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
                crate::vm::run_entry(
                    self,
                    program,
                    0,
                    environment.clone(),
                    expanded.span,
                )?
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
        message: String,
        format_control: Option<String>,
        format_arguments: &[Value],
        warning: bool,
        span: Span,
    ) -> RuntimeError {
        RuntimeError::Signaled {
            condition: normalize_name(condition)
                .trim_start_matches(':')
                .to_owned(),
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
            message,
            format_control,
            &format_arguments,
            warning,
            span,
        ))
    }

    fn make_condition(
        &self,
        arguments: &[Value],
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.is_empty() {
            return Err(self.arity("make-condition", "at least one", arguments.len()));
        }
        let initargs = &arguments[1..];
        if !initargs.len().is_multiple_of(2) {
            return Err(self.invalid(
                "make-condition initargs must be keyword/value pairs",
                span,
            ));
        }

        let actual_type = self.name_designator_from_value(&arguments[0], span)?;
        let mut format_control = None;
        let mut format_arguments = Vec::new();
        for pair in initargs.chunks_exact(2) {
            let initarg = self.name_designator_from_value(&pair[0], span)?;
            match initarg.as_str() {
                "FORMAT-CONTROL" => {
                    let Value::String(control) = &pair[1] else {
                        return Err(RuntimeError::Type {
                            expected: "STRING".to_owned(),
                            actual: pair[1].type_name().to_owned(),
                            span: Some(span),
                        });
                    };
                    format_control = Some(control.to_string());
                }
                "FORMAT-ARGUMENTS" => {
                    format_arguments = pair[1].list_items().ok_or_else(|| {
                        RuntimeError::Type {
                            expected: "PROPER-LIST".to_owned(),
                            actual: pair[1].type_name().to_owned(),
                            span: Some(span),
                        }
                    })?;
                }
                _ => {
                    return Err(self.invalid(
                        &format!("unknown make-condition initarg :{initarg}"),
                        span,
                    ));
                }
            }
        }

        let message = match format_control.as_deref() {
            Some(control) => builtins::format_control(control, &format_arguments)?,
            None => String::new(),
        };
        Ok(Value::condition_from_parts(
            actual_type,
            message,
            format_control,
            format_arguments,
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
            message,
            format_control,
            format_arguments,
            warning,
            span,
        );
        let condition_value = Value::condition(&error);
        self.dispatch_condition(error, &condition_value, environment, span)
    }

    fn restart_invocation_error(
        name: &str,
        arguments: &[Value],
        span: Span,
    ) -> RuntimeError {
        let value = match arguments {
            [] => Value::Nil,
            [value] => value.clone(),
            values => Value::values(values.to_vec()),
        };
        RuntimeError::InvokeRestart {
            name: normalize_name(name),
            value: ReturnValue::new(value),
            arguments: arguments
                .iter()
                .cloned()
                .map(ReturnValue::new)
                .collect(),
            span: Some(span),
        }
    }

    fn restart_binding_for_designator_in(
        &self,
        designator: &Value,
        bindings: &[RestartBinding],
        span: Span,
    ) -> Result<Option<RestartBinding>, RuntimeError> {
        if let Some((name, _)) = designator.symbol_reference() {
            let normalized = normalize_name(name);
            return Ok(bindings
                .iter()
                .rev()
                .find(|binding| normalize_name(&binding.name) == normalized)
                .cloned());
        }
        if designator.restart_name().is_some() {
            return Ok(bindings
                .iter()
                .rev()
                .find(|binding| binding.restart.eq_value(designator))
                .cloned());
        }
        Err(self.invalid("restart designator must be a symbol or restart", span))
    }

    fn restart_binding_for_designator(
        &self,
        designator: &Value,
        span: Span,
    ) -> Result<Option<RestartBinding>, RuntimeError> {
        let bindings = self.restart_bindings();
        self.restart_binding_for_designator_in(designator, &bindings, span)
    }

    fn invoke_restart_binding(
        &self,
        binding: RestartBinding,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let Some(function) = binding.function else {
            return Err(Self::restart_invocation_error(
                &binding.name,
                arguments,
                span,
            ));
        };
        self.apply_in(&function, arguments, span, environment)
    }

    fn invoke_restart_named(
        &self,
        name: &str,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let normalized = normalize_name(name);
        let Some(binding) = self
            .restart_bindings()
            .into_iter()
            .rev()
            .find(|binding| normalize_name(&binding.name) == normalized)
        else {
            return Err(Self::restart_invocation_error(&normalized, arguments, span));
        };
        self.invoke_restart_binding(binding, arguments, environment, span)
    }

    fn apply_primitive(
        &self,
        name: &str,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        match name {
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
            "MAKE-CONDITION" => self.make_condition(arguments, span),
            "EVAL" => {
                if arguments.len() != 1 {
                    return Err(self.arity("eval", "one", arguments.len()));
                }
                let form = self.form_from_value(&arguments[0], span)?;
                self.eval_values_in(&form, environment)
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
                Ok(Value::boolean(arguments[0].instance_slot_exists(&slot_name)))
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
                        return Err(self.invalid("call-next-method is only available in a method", span));
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
                let symbol_name = self.symbol_name_from_value(&arguments[0], span)?;
                let package_name = arguments
                    .get(1)
                    .map(|value| self.package_name_from_value(value, span))
                    .transpose()?
                    .unwrap_or_else(|| self.current_package());
                let status = match self
                    .packages
                    .borrow_mut()
                    .intern_symbol(&package_name, &symbol_name)
                {
                    Some(status) => status,
                    None => {
                        return Err(
                            self.package_error(&format!("unknown package {package_name}"), span)
                        );
                    }
                };
                let symbol = self.package_symbol_value(&package_name, &symbol_name);
                Ok(Value::values(vec![
                    symbol,
                    Self::symbol_status_value(status),
                ]))
            }
            "FIND-SYMBOL" => {
                if !(1..=2).contains(&arguments.len()) {
                    return Err(self.arity("find-symbol", "one or two", arguments.len()));
                }
                let symbol_name = self.symbol_name_from_value(&arguments[0], span)?;
                let package_name = arguments
                    .get(1)
                    .map(|value| self.package_name_from_value(value, span))
                    .transpose()?
                    .unwrap_or_else(|| self.current_package());
                let status = self
                    .packages
                    .borrow()
                    .symbol_status(&package_name, &symbol_name);
                match status {
                    Some(status) => {
                        let symbol = self.package_symbol_value(&package_name, &symbol_name);
                        Ok(Value::values(vec![
                            symbol,
                            Self::symbol_status_value(status),
                        ]))
                    }
                    None => Ok(Value::values(vec![Value::Nil, Value::Nil])),
                }
            }
            "FIND-PACKAGE" => {
                if arguments.len() != 1 {
                    return Err(self.arity("find-package", "one", arguments.len()));
                }
                let package = self.package_designator_name(&arguments[0], span)?;
                let packages = self.packages.borrow();
                if packages.package_exists(&package) {
                    Ok(Value::package(packages.canonical_package_name(&package)))
                } else {
                    Ok(Value::Nil)
                }
            }
            "PACKAGE-NAME" => {
                if arguments.len() != 1 {
                    return Err(self.arity("package-name", "one", arguments.len()));
                }
                match &arguments[0] {
                    Value::Package(package) => Ok(Value::string(package.as_ref())),
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
                        let names = self.packages.borrow().use_packages_for(package);
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
                    Value::Package(package) => Ok(self
                        .packages
                        .borrow()
                        .package_documentation(package)
                        .map_or(Value::Nil, |documentation| {
                            Value::string(documentation.as_str())
                        })),
                    other => Err(RuntimeError::Type {
                        expected: "PACKAGE".to_string(),
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
                let symbols = self.symbol_names_from_value(&arguments[0], span)?;
                let package = arguments
                    .get(1)
                    .map(|value| self.package_name_from_value(value, span))
                    .transpose()?
                    .unwrap_or_else(|| self.current_package());
                self.packages
                    .borrow_mut()
                    .export_symbols(&package, &symbols);
                Ok(Value::boolean(true))
            }
            "UNEXPORT" => {
                if arguments.len() != 1 && arguments.len() != 2 {
                    return Err(self.arity("unexport", "one or two", arguments.len()));
                }
                let symbols = self.symbol_names_from_value(&arguments[0], span)?;
                let package = arguments
                    .get(1)
                    .map(|value| self.package_name_from_value(value, span))
                    .transpose()?
                    .unwrap_or_else(|| self.current_package());
                self.packages
                    .borrow_mut()
                    .unexport_symbols(&package, &symbols);
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
                    for (source_package, source_name) in &imports {
                        if !state.symbol_exists(source_package, source_name) {
                            return Err(self.package_error(
                                &format!(
                                    "unknown symbol {source_package}::{source_name}"
                                ),
                                span,
                            ));
                        }
                    }
                }
                let shadowing = name == "SHADOWING-IMPORT";
                let mut state = self.packages.borrow_mut();
                for (source_package, source_name) in imports {
                    state.import_symbol(
                        &source_package,
                        &source_name,
                        &target,
                        shadowing,
                    );
                }
                Ok(Value::boolean(true))
            }
            "SHADOW" => {
                if arguments.len() != 1 && arguments.len() != 2 {
                    return Err(self.arity("shadow", "one or two", arguments.len()));
                }
                let symbols = self.symbol_names_from_value(&arguments[0], span)?;
                let target = arguments
                    .get(1)
                    .map(|value| self.package_name_from_value(value, span))
                    .transpose()?
                    .unwrap_or_else(|| self.current_package());
                let mut state = self.packages.borrow_mut();
                for symbol in symbols {
                    state.shadow_symbol(&target, &symbol);
                }
                Ok(Value::boolean(true))
            }
            "UNINTERN" => {
                if arguments.len() != 1 && arguments.len() != 2 {
                    return Err(self.arity("unintern", "one or two", arguments.len()));
                }
                let symbols = self.symbol_names_from_value(&arguments[0], span)?;
                let target = arguments
                    .get(1)
                    .map(|value| self.package_name_from_value(value, span))
                    .transpose()?
                    .unwrap_or_else(|| self.current_package());
                let mut removed = false;
                let mut local_names = Vec::new();
                {
                    let mut state = self.packages.borrow_mut();
                    for symbol in symbols {
                        let local_name = package::canonical_symbol_name(&target, &symbol);
                        removed |= state.unintern_symbol(&target, &symbol);
                        local_names.push(local_name);
                    }
                }
                for local_name in local_names {
                    self.remove_global_symbol(&local_name);
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
                let (name, exact) = arguments[0]
                    .symbol_reference()
                    .ok_or_else(|| self.invalid("macro-function argument must be a symbol", span))?;
                let lookup_environment = match arguments.get(1) {
                    None | Some(Value::Nil | Value::Boolean(false)) => &self.global,
                    Some(Value::Environment(environment)) => environment,
                    Some(_) => {
                        return Err(self.invalid(
                            "macro-function environment must be an environment",
                            span,
                        ))
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
                            crate::Function::Macro { .. }
                                | crate::Function::ModifyMacro { .. }
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
                let (name, _) = arguments[0]
                    .symbol_reference()
                    .ok_or_else(|| self.invalid("special-operator-p argument must be a symbol", span))?;
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
                let (name, exact) = arguments[0]
                    .symbol_reference()
                    .ok_or_else(|| self.invalid("symbol-function argument must be a symbol", span))?;
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
                    self.lookup_exact_in(name, environment)
                } else {
                    self.lookup_in(name, environment)
                };
                value
                    .ok_or_else(|| RuntimeError::UnboundVariable {
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
            "REMOVE" | "REMOVE-IF" | "REMOVE-IF-NOT" | "DELETE" | "DELETE-IF"
            | "DELETE-IF-NOT" => {
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
            "UNION"
            | "NUNION"
            | "INTERSECTION"
            | "NINTERSECTION"
            | "SET-DIFFERENCE"
            | "NSET-DIFFERENCE"
            | "SET-EXCLUSIVE-OR"
            | "NSET-EXCLUSIVE-OR"
            | "SUBSETP" => {
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
            "ASSOC" | "ASSOC-IF" | "ASSOC-IF-NOT" | "RASSOC" | "RASSOC-IF"
            | "RASSOC-IF-NOT" => {
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

    fn method_score(&self, method: &MethodDefinition, arguments: &[Value]) -> Option<usize> {
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
        let mut score = 0usize;
        for (specializer, argument) in method
            .specializers
            .iter()
            .zip(arguments.iter().take(required_count))
        {
            if specializer == "T" || specializer == "OBJECT" {
                score = score.saturating_add(1_000_000);
                continue;
            }
            let class = argument.instance_class_definition()?;
            let position = class
                .precedence
                .iter()
                .position(|name| name == specializer)?;
            score = score.saturating_add(position);
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
            } => self.invoke_core(
                &before,
                &primary,
                &after,
                arguments,
                span,
                environment,
            ),
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
        applicable.sort_by_key(|(score, _)| *score);

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
            crate::Function::Builtin { function, .. } => function(arguments),
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
                if object.set_instance_slot(class_name, slot_name, value.clone()) {
                    Ok(value)
                } else {
                    Err(self.invalid("slot is not defined for this class", span))
                }
            }
            crate::Function::StructureConstructor {
                name,
                slots,
                structure_types,
                constructor_lambda_list,
                environment: definition_environment,
            } => {
                if let Some(lambda_list) = constructor_lambda_list {
                    self.apply_structure_boa_constructor(
                        name,
                        slots,
                        structure_types,
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
                        supplied[index] = Some(pair[1].clone());
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
                    Ok(Value::structure_with_types(
                        name,
                        values,
                        structure_types.clone(),
                    ))
                }
            }
            crate::Function::StructurePredicate { name } => {
                if arguments.len() != 1 {
                    return Err(self.arity("structure predicate", "one", arguments.len()));
                }
                Ok(Value::boolean(arguments[0].structure_is_type(name)))
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

                let local = environment.child();
                let _dynamic_guard = self.dynamic_guard();
                for (index, (parameter, argument)) in parameters
                    .iter()
                    .zip(arguments.iter())
                    .enumerate()
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
                        supplied_keywords.insert(keyword_name, pair[1].clone());
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
        let slot_index = |parameter_name: &str| {
            slots
                .iter()
                .position(|slot| slot.name == parameter_name)
        };
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
            if lambda_list.required_escaped.get(index).copied().unwrap_or(false) {
                self.define_exact_in(parameter, argument.clone(), &local);
            } else {
                self.define_in(parameter, argument.clone(), &local);
            }
            if let Some(slot_index) = slot_index(parameter) {
                slot_values[slot_index] = Some(argument.clone());
            }
        }

        for (index, specification) in lambda_list.optional.iter().enumerate() {
            let supplied = (index < optional_supplied_count)
                .then(|| &arguments[required_count + index]);
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
                let keyword_name = match &pair[0] {
                    Value::Keyword(keyword) | Value::KeywordExact(keyword) => keyword.to_string(),
                    _ => return Err(self.invalid("keyword argument name must be a keyword", span)),
                };
                if normalize_name(&keyword_name) == "ALLOW-OTHER-KEYS" && pair[1].is_truthy() {
                    accepts_unknown_keywords = true;
                }
                supplied_keywords.push((keyword_name, pair[1].clone()));
            }
            let keyword_matches = |specification: &LambdaListKeywordParameter,
                                   actual_name: &str| {
                if specification.keyword_name_escaped {
                    specification.keyword_name == actual_name
                } else {
                    normalize_name(&specification.keyword_name) == normalize_name(actual_name)
                }
            };
            if !accepts_unknown_keywords {
                for (keyword_name, _) in &supplied_keywords {
                    if normalize_name(keyword_name) != "ALLOW-OTHER-KEYS"
                        && !lambda_list
                            .keywords
                            .iter()
                            .any(|specification| keyword_matches(specification, keyword_name))
                    {
                        return Err(RuntimeError::InvalidForm {
                            message: format!("unknown keyword :{keyword_name}"),
                            span: Some(span),
                        });
                    }
                }
            }
            for specification in &lambda_list.keywords {
                let supplied = supplied_keywords
                    .iter()
                    .rev()
                    .find(|(keyword_name, _)| keyword_matches(specification, keyword_name));
                let value = match supplied {
                    Some((_, argument)) => argument.clone(),
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
        Ok(Value::structure_with_types(
            name,
            values,
            structure_types.to_vec(),
        ))
    }

    fn parameters(&self, form: &Form) -> Result<OrdinaryLambdaList, RuntimeError> {
        parse_ordinary_lambda_list(form).map_err(|error| {
            let message = error.kind.to_string();
            self.invalid(&message, error.span)
        })
    }

    fn macro_parameters(&self, form: &Form) -> Result<MacroLambdaList, RuntimeError> {
        let FormKind::List(parameters) = &form.kind else {
            return Err(self.invalid("macro parameters must be a list", form.span));
        };

        let mut lambda_list = MacroLambdaList {
            whole: None,
            environment: None,
            required: Vec::new(),
            optional: Vec::new(),
            rest: None,
            keywords: Vec::new(),
            has_keyword_section: false,
            allow_other_keys: false,
            auxiliary: Vec::new(),
        };
        let mut seen = HashSet::new();
        let mut section = MacroLambdaListSection::Required;
        let mut index = 0;

        while index < parameters.len() {
            let parameter = &parameters[index];
            if let Some(name) = atom_name(parameter) {
                let marker = normalize_name(name);
                match marker.as_str() {
                    "&WHOLE" => {
                        if index != 0
                            || lambda_list.whole.is_some()
                            || index + 1 >= parameters.len()
                        {
                            return Err(self.invalid(
                                "&whole must be the first marker followed by one parameter",
                                parameter.span,
                            ));
                        }
                        lambda_list.whole =
                            Some(self.macro_binding_name(&parameters[index + 1], &mut seen)?);
                        index += 2;
                    }
                    "&OPTIONAL" => {
                        if section != MacroLambdaListSection::Required {
                            return Err(self.invalid(
                                "&optional is out of order in macro lambda list",
                                parameter.span,
                            ));
                        }
                        section = MacroLambdaListSection::Optional;
                        index += 1;
                    }
                    "&REST" | "&BODY" => {
                        if lambda_list.rest.is_some()
                            || matches!(
                                section,
                                MacroLambdaListSection::Rest
                                    | MacroLambdaListSection::Keyword
                                    | MacroLambdaListSection::Auxiliary
                            )
                            || index + 1 >= parameters.len()
                        {
                            return Err(self.invalid(
                                "&rest or &body must be followed by one parameter",
                                parameter.span,
                            ));
                        }
                        lambda_list.rest =
                            Some(self.macro_binding_name(&parameters[index + 1], &mut seen)?);
                        section = MacroLambdaListSection::Rest;
                        index += 2;
                    }
                    "&KEY" => {
                        if lambda_list.has_keyword_section
                            || matches!(
                                section,
                                MacroLambdaListSection::Keyword | MacroLambdaListSection::Auxiliary
                            )
                        {
                            return Err(self.invalid(
                                "&key is out of order or repeated in macro lambda list",
                                parameter.span,
                            ));
                        }
                        lambda_list.has_keyword_section = true;
                        section = MacroLambdaListSection::Keyword;
                        index += 1;
                    }
                    "&ALLOW-OTHER-KEYS" => {
                        if section != MacroLambdaListSection::Keyword
                            || lambda_list.allow_other_keys
                        {
                            return Err(self.invalid(
                                "&allow-other-keys requires a keyword section",
                                parameter.span,
                            ));
                        }
                        lambda_list.allow_other_keys = true;
                        index += 1;
                    }
                    "&AUX" => {
                        if section == MacroLambdaListSection::Auxiliary {
                            return Err(self
                                .invalid("&aux is repeated in macro lambda list", parameter.span));
                        }
                        section = MacroLambdaListSection::Auxiliary;
                        index += 1;
                    }
                    "&ENVIRONMENT" => {
                        if lambda_list.environment.is_some() || index + 1 >= parameters.len() {
                            return Err(self.invalid(
                                "&environment must be followed by one parameter",
                                parameter.span,
                            ));
                        }
                        lambda_list.environment =
                            Some(self.macro_binding_name(&parameters[index + 1], &mut seen)?);
                        index += 2;
                    }
                    _ if marker.starts_with('&') => {
                        return Err(
                            self.invalid("unsupported marker in macro lambda list", parameter.span)
                        );
                    }
                    _ => {
                        if section == MacroLambdaListSection::Rest {
                            return Err(self.invalid(
                                "macro rest parameter must be followed by a keyword or auxiliary section",
                                parameter.span,
                            ));
                        }
                        match section {
                            MacroLambdaListSection::Required => {
                                lambda_list
                                    .required
                                    .push(self.macro_pattern(parameter, &mut seen)?);
                            }
                            MacroLambdaListSection::Optional => {
                                lambda_list.optional.push(
                                    self.parse_macro_optional_parameter(parameter, &mut seen)?,
                                );
                            }
                            MacroLambdaListSection::Keyword => {
                                if lambda_list.allow_other_keys {
                                    return Err(self.invalid(
                                        "&allow-other-keys must be the last keyword-list marker",
                                        parameter.span,
                                    ));
                                }
                                let specification =
                                    self.parse_macro_keyword_parameter(parameter, &mut seen)?;
                                if lambda_list
                                    .keywords
                                    .iter()
                                    .any(|item| item.keyword_name == specification.keyword_name)
                                {
                                    return Err(self.invalid(
                                        "macro keyword names must be unique",
                                        parameter.span,
                                    ));
                                }
                                lambda_list.keywords.push(specification);
                            }
                            MacroLambdaListSection::Auxiliary => {
                                lambda_list.auxiliary.push(
                                    self.parse_macro_auxiliary_parameter(parameter, &mut seen)?,
                                );
                            }
                            MacroLambdaListSection::Rest => unreachable!(),
                        }
                        index += 1;
                    }
                }
                continue;
            }

            if section == MacroLambdaListSection::Rest {
                return Err(self.invalid(
                    "macro rest parameter must be followed by a keyword or auxiliary section",
                    parameter.span,
                ));
            }
            match section {
                MacroLambdaListSection::Required => {
                    lambda_list
                        .required
                        .push(self.macro_pattern(parameter, &mut seen)?);
                }
                MacroLambdaListSection::Optional => {
                    lambda_list
                        .optional
                        .push(self.parse_macro_optional_parameter(parameter, &mut seen)?);
                }
                MacroLambdaListSection::Keyword => {
                    if lambda_list.allow_other_keys {
                        return Err(self.invalid(
                            "&allow-other-keys must be the last keyword-list marker",
                            parameter.span,
                        ));
                    }
                    let specification = self.parse_macro_keyword_parameter(parameter, &mut seen)?;
                    if lambda_list
                        .keywords
                        .iter()
                        .any(|item| item.keyword_name == specification.keyword_name)
                    {
                        return Err(
                            self.invalid("macro keyword names must be unique", parameter.span)
                        );
                    }
                    lambda_list.keywords.push(specification);
                }
                MacroLambdaListSection::Auxiliary => {
                    lambda_list
                        .auxiliary
                        .push(self.parse_macro_auxiliary_parameter(parameter, &mut seen)?);
                }
                MacroLambdaListSection::Rest => unreachable!(),
            }
            index += 1;
        }

        Ok(lambda_list)
    }

    fn macro_binding_name(
        &self,
        form: &Form,
        seen: &mut HashSet<String>,
    ) -> Result<String, RuntimeError> {
        let Some(name) = atom_name(form) else {
            return Err(self.invalid("macro parameter must be a symbol", form.span));
        };
        let normalized = normalize_name(name);
        if normalized.is_empty()
            || normalized.starts_with('&')
            || literal_atom(name).is_some()
            || !seen.insert(normalized.clone())
        {
            return Err(self.invalid(
                "macro parameter names must be unique and bindable",
                form.span,
            ));
        }
        Ok(normalized)
    }

    fn macro_pattern(
        &self,
        form: &Form,
        seen: &mut HashSet<String>,
    ) -> Result<MacroPattern, RuntimeError> {
        match &form.kind {
            FormKind::Atom(_) => Ok(MacroPattern::Name(self.macro_binding_name(form, seen)?)),
            FormKind::List(items) => Ok(MacroPattern::List(
                items
                    .iter()
                    .map(|item| self.macro_pattern(item, seen))
                    .collect::<Result<Vec<_>, _>>()?,
            )),
            FormKind::DottedList { items, tail } => Ok(MacroPattern::Dotted {
                items: items
                    .iter()
                    .map(|item| self.macro_pattern(item, seen))
                    .collect::<Result<Vec<_>, _>>()?,
                tail: Box::new(self.macro_pattern(tail, seen)?),
            }),
            _ => Err(self.invalid(
                "macro destructuring pattern must be a symbol or list",
                form.span,
            )),
        }
    }

    fn parse_macro_optional_parameter(
        &self,
        form: &Form,
        seen: &mut HashSet<String>,
    ) -> Result<MacroOptionalParameter, RuntimeError> {
        let nil = || Form::atom("NIL", form.span);
        match &form.kind {
            FormKind::Atom(_) => Ok(MacroOptionalParameter {
                pattern: self.macro_pattern(form, seen)?,
                init_form: nil(),
                supplied_p: None,
            }),
            FormKind::List(items) if (1..=3).contains(&items.len()) => {
                let pattern = self.macro_pattern(&items[0], seen)?;
                let init_form = items.get(1).cloned().unwrap_or_else(nil);
                let supplied_p = items
                    .get(2)
                    .map(|item| self.macro_binding_name(item, seen))
                    .transpose()?;
                Ok(MacroOptionalParameter {
                    pattern,
                    init_form,
                    supplied_p,
                })
            }
            FormKind::List(_) => Err(self.invalid(
                "macro optional parameter must contain one to three items",
                form.span,
            )),
            _ => Err(self.invalid(
                "macro optional parameter must be a symbol or list",
                form.span,
            )),
        }
    }

    fn parse_macro_keyword_parameter(
        &self,
        form: &Form,
        seen: &mut HashSet<String>,
    ) -> Result<MacroKeywordParameter, RuntimeError> {
        let nil = || Form::atom("NIL", form.span);
        let (keyword_name, pattern, trailing_start) = match &form.kind {
            FormKind::Atom(_) => {
                let name = self.macro_binding_name(form, seen)?;
                let keyword_name = normalize_name(&name);
                (keyword_name, MacroPattern::Name(name), 0)
            }
            FormKind::List(items) if !items.is_empty() => {
                if let FormKind::List(key_specification) = &items[0].kind {
                    if key_specification.len() != 2 {
                        return Err(self.invalid(
                            "macro keyword designator must contain a keyword and variable",
                            items[0].span,
                        ));
                    }
                    let Some(keyword_name) = macro_keyword_name(&key_specification[0]) else {
                        return Err(self.invalid(
                            "macro keyword designator must start with a keyword",
                            key_specification[0].span,
                        ));
                    };
                    let pattern = self.macro_pattern(&key_specification[1], seen)?;
                    (keyword_name, pattern, 1)
                } else if atom_name(&items[0]).is_some_and(|name| name.starts_with(':')) {
                    let Some(keyword_name) = macro_keyword_name(&items[0]) else {
                        return Err(self.invalid(
                            "macro keyword designator must be a nonempty keyword",
                            items[0].span,
                        ));
                    };
                    if items.len() < 2 {
                        return Err(
                            self.invalid("macro keyword parameter needs a variable", form.span)
                        );
                    }
                    let pattern = self.macro_pattern(&items[1], seen)?;
                    (keyword_name, pattern, 2)
                } else {
                    let pattern = self.macro_pattern(&items[0], seen)?;
                    let MacroPattern::Name(name) = &pattern else {
                        return Err(self.invalid(
                            "macro keyword parameter must have a variable name",
                            items[0].span,
                        ));
                    };
                    (normalize_name(name), pattern, 1)
                }
            }
            FormKind::List(_) => unreachable!(),
            _ => {
                return Err(self.invalid(
                    "macro keyword parameter must be a symbol or list",
                    form.span,
                ));
            }
        };

        let item_count = match &form.kind {
            FormKind::Atom(_) => 0,
            FormKind::List(items) => items.len(),
            _ => unreachable!(),
        };
        if item_count > trailing_start + 2 {
            return Err(self.invalid("macro keyword parameter contains too many items", form.span));
        }
        let (init_form, supplied_p) = match &form.kind {
            FormKind::Atom(_) => (nil(), None),
            FormKind::List(items) => (
                items.get(trailing_start).cloned().unwrap_or_else(nil),
                items
                    .get(trailing_start + 1)
                    .map(|item| self.macro_binding_name(item, seen))
                    .transpose()?,
            ),
            _ => unreachable!(),
        };
        Ok(MacroKeywordParameter {
            keyword_name,
            pattern,
            init_form,
            supplied_p,
        })
    }

    fn parse_macro_auxiliary_parameter(
        &self,
        form: &Form,
        seen: &mut HashSet<String>,
    ) -> Result<MacroAuxiliaryParameter, RuntimeError> {
        match &form.kind {
            FormKind::Atom(_) => Ok(MacroAuxiliaryParameter {
                name: self.macro_binding_name(form, seen)?,
                init_form: Form::atom("NIL", form.span),
            }),
            FormKind::List(items) if (1..=2).contains(&items.len()) => {
                Ok(MacroAuxiliaryParameter {
                    name: self.macro_binding_name(&items[0], seen)?,
                    init_form: items
                        .get(1)
                        .cloned()
                        .unwrap_or_else(|| Form::atom("NIL", form.span)),
                })
            }
            FormKind::List(_) => Err(self.invalid(
                "macro auxiliary parameter must contain one or two items",
                form.span,
            )),
            _ => Err(self.invalid(
                "macro auxiliary parameter must be a symbol or list",
                form.span,
            )),
        }
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
                        .iter()
                        .map(|value| self.form_from_value(value, span))
                        .collect::<Result<Vec<_>, _>>()?,
                ),
                span,
            )),
            Value::Array { .. }
            | Value::HashTable { .. }
            | Value::Stream(_)
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
            || (!token.escaped
                && (token.name.starts_with('&') || literal_atom(name).is_some()))
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
        self.variable_name_info(form, context)
            .map(|(name, _)| name)
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

fn is_nil_form(form: &Form) -> bool {
    atom_name(form).is_some_and(|name| name.eq_ignore_ascii_case("nil"))
}

fn is_macro_keyword_form(form: &Form) -> bool {
    macro_keyword_name(form).is_some()
}

fn macro_keyword_name(form: &Form) -> Option<String> {
    let name = atom_name(form)?;
    let keyword = name.strip_prefix(':')?;
    (!keyword.is_empty()).then(|| normalize_name(keyword))
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
            | "ECASE"
            | "TYPECASE"
            | "ETYPECASE"
            | "DESTRUCTURING-BIND"
            | "LET"
            | "LET*"
            | "FLET"
            | "LABELS"
            | "MACROLET"
            | "SYMBOL-MACROLET"
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
            if let Some((numerator, denominator)) = token.name.split_once('/') {
                if let (Ok(numerator), Ok(denominator)) =
                    (numerator.parse::<i128>(), denominator.parse::<i128>())
                {
                    return Value::rational(numerator, denominator).ok();
                }
            }
            token.name.parse::<f64>().ok().map(Value::Float)
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
