use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::rc::Rc;

use ncl_compiler::Compiler;
use ncl_syntax::{
    Form, FormKind, LambdaListAuxiliaryParameter, LambdaListKeywordParameter,
    LambdaListOptionalParameter, OrdinaryLambdaList, Span, SymbolTokenKind,
    parse_ordinary_lambda_list, parse_symbol_token, read,
};

use crate::builtins;
use crate::environment::normalize_name;
use crate::error::{SignaledError, ThrowTag};
use crate::package::{self, PackageState};
use crate::value::{
    ClassDefinition, ClassSlot, MacroAuxiliaryParameter, MacroKeywordParameter, MacroLambdaList,
    MacroOptionalParameter, MacroPattern, MethodDefinition, StructureDefinition, StructureSlot,
};
use crate::{Environment, ReturnValue, RuntimeError, Value};

const MAX_MACRO_EXPANSIONS: usize = 64;

pub mod evaluator_state;
pub use evaluator_state::{ConditionHandlerBinding, RestartBinding};
mod compilation;
mod evaluator_package_primitives;
mod evaluator_primitive_dispatch;
mod evaluator_resolution;
mod packages;
use evaluator_state::{
    ConditionHandlerGuard, ConditionHandlerSuspension, ConditionRestartBinding,
    ConditionRestartGuard, DynamicGuard, DynamicState, MacroLambdaListSection, MethodContext,
    MethodContinuation, RestartGuard, SetfExpansion,
};

#[derive(Clone, Copy)]
pub struct MacroBindingContext<'a> {
    form: &'a Form,
    arguments: &'a [Form],
    macro_name: &'a str,
    lambda_list: &'a MacroLambdaList,
    macro_environment: &'a Environment,
    environment: &'a Environment,
}

pub struct ModifyMacroContext<'a> {
    binding: MacroBindingContext<'a>,
    function: &'a Form,
}

#[derive(Debug)]
/// Stateful evaluator owning the global environment and package context.
pub struct Runtime {
    global: Environment,
    packages: Rc<RefCell<PackageState>>,
    dynamic: Rc<RefCell<DynamicState>>,
    next_block_target: Cell<u64>,
    gensym_counter: Cell<u64>,
    method_context: RefCell<Vec<MethodContext>>,
}

impl Runtime {
    /// Creates an evaluator with the standard NCL primitives installed.
    #[must_use]
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

    /// Returns a clone of the global environment handle.
    pub fn global_environment(&self) -> Environment {
        self.global.clone()
    }

    /// Returns the name of the package used to resolve unqualified symbols.
    pub fn current_package(&self) -> String {
        self.packages.borrow().current().to_string()
    }

    pub(crate) fn fresh_block_target(&self) -> u64 {
        let target = self.next_block_target.get();
        self.next_block_target.set(target.wrapping_add(1));
        target
    }

    /// Evaluates one parsed form using the tree-walking evaluator.
    ///
    /// # Errors
    ///
    /// Returns a [`RuntimeError`] when resolving or evaluating the form fails.
    pub fn eval(&self, form: &Form) -> Result<Value, RuntimeError> {
        let resolved = self.resolve_form(form)?;
        self.eval_in(&resolved, &self.global)
    }

    /// Reads and evaluates every top-level form in source text.
    ///
    /// # Errors
    ///
    /// Returns a [`RuntimeError`] when reading or evaluating any form fails.
    pub fn eval_source(&self, source: &str) -> Result<Vec<Value>, RuntimeError> {
        read(source)?.iter().map(|form| self.eval(form)).collect()
    }

    /// Compiles and evaluates one parsed form with the bytecode VM.
    ///
    /// # Errors
    ///
    /// Returns a [`RuntimeError`] when resolving, compiling, or evaluating the form fails.
    pub fn eval_compiled(&self, form: &Form) -> Result<Value, RuntimeError> {
        let resolved = self.resolve_form(form)?;
        let expanded = self.prepare_compiled_form(&resolved, &self.global)?;
        let program = Rc::new(Compiler::compile_form(&expanded)?);
        crate::vm::run_entry(self, &program, 0, &self.global, expanded.span)
            .map(|value| value.primary_value())
    }

    /// Compiles and evaluates every top-level form in source text.
    ///
    /// # Errors
    ///
    /// Returns a [`RuntimeError`] when reading, compiling, or evaluating any form fails.
    pub fn eval_compiled_source(&self, source: &str) -> Result<Vec<Value>, RuntimeError> {
        read(source)?
            .iter()
            .map(|form| self.eval_compiled(form))
            .collect()
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

    pub(crate) fn lookup_exact_in(&self, name: &str, environment: &Environment) -> Option<Value> {
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

    pub(crate) fn define_exact_in(&self, name: &str, value: Value, environment: &Environment) {
        if self.dynamic.borrow().exact_special_names.contains(name) {
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

    pub(crate) fn define_special_value(&self, name: &str, value: Value, force: bool) -> Value {
        let name = normalize_name(name);
        let mut dynamic = self.dynamic.borrow_mut();
        dynamic.special_names.insert(name.clone());
        if !force && let Some(existing) = dynamic.globals.get(&name) {
            return existing.clone();
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
        if !force && let Some(existing) = dynamic.exact_globals.get(name) {
            return existing.clone();
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
            _ => false,
        }
    }

    pub(crate) fn constant_modification_error(name: &str, span: Span) -> RuntimeError {
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
            return Err(Self::constant_modification_error(name, span));
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
            return Err(Self::constant_modification_error(name, span));
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
                if let Some(expanded) = Self::expand_symbol_macro_form(form, environment)? {
                    return self.eval_values_in(&expanded, environment);
                }
                self.eval_atom(atom, form.span, environment)
            }
            FormKind::String(value) => Ok(Value::string(value.clone())),
            FormKind::Character(value) => Ok(Value::Character(*value)),
            FormKind::Vector(items) => Ok(Value::vector(
                items
                    .iter()
                    .map(Self::quoted_value)
                    .collect::<Result<Vec<_>, _>>()?,
            )),
            FormKind::DottedList { .. } => {
                Err(Self::invalid("cannot evaluate a dotted list", form.span))
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
            let escaped = parse_symbol_token(name).is_ok_and(|token| token.escaped);
            if !escaped {
                if let Some(value) = self.eval_special_form_core(form, items, name, environment)? {
                    return Ok(value);
                }
                if let Some(value) =
                    self.eval_special_form_conditionals(items, name, environment)?
                {
                    return Ok(value);
                }
                if let Some(value) = self.eval_special_form_bindings(items, name, environment)? {
                    return Ok(value);
                }
                if let Some(value) = self.eval_special_form_iteration(items, name, environment)? {
                    return Ok(value);
                }
                if let Some(value) = self.eval_special_form_functions(items, name, environment)? {
                    return Ok(value);
                }
                if let Some(value) = self.eval_special_form_macros(items, name, environment)? {
                    return Ok(value);
                }
                if let Some(value) = self.eval_special_form_expansion(items, name, environment)? {
                    return Ok(value);
                }
                if let Some(value) = self.eval_special_form_mutation(items, name, environment)? {
                    return Ok(value);
                }
            }
        }

        self.eval_function_form(operator, &items[1..], form.span, environment)
    }

    fn eval_function_form(
        &self,
        operator: &Form,
        argument_forms: &[Form],
        span: Span,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        let function = if let Some(name) = atom_name(operator) {
            let (resolved_name, escaped) = resolved_symbol(name);
            let function = if escaped {
                self.lookup_function_exact_in(&resolved_name, environment)
            } else {
                self.lookup_function_in(&resolved_name, environment)
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
        let arguments = argument_forms
            .iter()
            .map(|item| self.eval_in(item, environment))
            .collect::<Result<Vec<_>, _>>()?;
        self.apply_in(&function, &arguments, span, environment)
    }

    fn eval_special_form_mutation(
        &self,
        items: &[Form],
        name: &str,
        environment: &Environment,
    ) -> Result<Option<Value>, RuntimeError> {
        let value = match normalize_name(name).as_str() {
            "MACROLET" => Some(self.special_macrolet(items, environment)?),
            "SYMBOL-MACROLET" => Some(self.special_symbol_macrolet(items, environment)?),
            "DEFPACKAGE" => Some(self.special_defpackage(items)?),
            "IN-PACKAGE" => Some(self.special_in_package(items)?),
            "DEFINE" => Some(self.special_define(items, environment)?),
            "DEFINE-SYMBOL-MACRO" => Some(Self::special_define_symbol_macro(items, environment)?),
            "SETQ" => Some(self.special_setq(items, environment)?),
            "PSETQ" => Some(self.special_psetq(items, environment)?),
            "MULTIPLE-VALUE-SETQ" => Some(self.special_multiple_value_setq(items, environment)?),
            "SETF" => Some(self.special_setf(items, environment)?),
            "PSETF" => Some(self.special_psetf(items, environment)?),
            "PUSH" => Some(self.special_push(items, environment)?),
            "POP" => Some(self.special_pop(items, environment)?),
            "PUSHNEW" => Some(self.special_pushnew(items, environment)?),
            "ROTATEF" => Some(self.special_rotatef(items, environment)?),
            "SHIFTF" => Some(self.special_shiftf(items, environment)?),
            "INCF" => Some(self.special_modify_symbol(items, environment, "INCF", "+")?),
            "DECF" => Some(self.special_modify_symbol(items, environment, "DECF", "-")?),
            "DEFSTRUCT" => Some(self.special_defstruct(items, environment)?),
            "DEFCLASS" => Some(Self::special_defclass(items, environment)?),
            "DEFGENERIC" => Some(Self::special_defgeneric(items, environment)?),
            "DEFMETHOD" => Some(Self::special_defmethod(items, environment)?),
            "DEFSETF" => Some(self.special_defsetf(items, environment)?),
            "DEFINE-SETF-EXPANDER" => Some(Self::special_define_setf_expander(items, environment)?),
            "GET-SETF-EXPANSION" => Some(self.special_get_setf_expansion(items, environment)?),
            "DEFVAR" => Some(self.special_defvar(items, environment, false)?),
            "DEFPARAMETER" => Some(self.special_defvar(items, environment, true)?),
            "DEFCONSTANT" => Some(self.special_defconstant(items, environment)?),
            "EVAL" => Some(self.special_eval(items, environment)?),
            "FUNCALL" => Some(self.special_funcall(items, environment)?),
            "APPLY" => Some(self.special_apply(items, environment)?),
            "MAP-INTO" => Some(self.special_map_into(items, environment)?),
            "MAPCAR" => Some(self.special_mapcar(items, environment)?),
            _ => None,
        };
        Ok(value)
    }

    fn eval_special_form_expansion(
        &self,
        items: &[Form],
        name: &str,
        environment: &Environment,
    ) -> Result<Option<Value>, RuntimeError> {
        let value = match normalize_name(name).as_str() {
            "MACROEXPAND-1" => Some(self.special_macroexpand_1(items, environment)?),
            "MACROEXPAND" => Some(self.special_macroexpand(items, environment)?),
            _ => None,
        };
        Ok(value)
    }

    fn eval_special_form_macros(
        &self,
        items: &[Form],
        name: &str,
        environment: &Environment,
    ) -> Result<Option<Value>, RuntimeError> {
        let value = match normalize_name(name).as_str() {
            "DEFMACRO" => Some(self.special_defmacro(items, environment)?),
            "DEFINE-MODIFY-MACRO" => Some(self.special_define_modify_macro(items, environment)?),
            _ => None,
        };
        Ok(value)
    }

    fn eval_special_form_functions(
        &self,
        items: &[Form],
        name: &str,
        environment: &Environment,
    ) -> Result<Option<Value>, RuntimeError> {
        let value = match normalize_name(name).as_str() {
            "LAMBDA" => Some(Self::special_lambda(items, environment)?),
            "FUNCTION" => Some(self.special_function(items, environment)?),
            "DEFUN" => Some(self.special_defun(items, environment)?),
            _ => None,
        };
        Ok(value)
    }

    fn eval_special_form_iteration(
        &self,
        items: &[Form],
        name: &str,
        environment: &Environment,
    ) -> Result<Option<Value>, RuntimeError> {
        let value = match normalize_name(name).as_str() {
            "DOTIMES" => Some(self.special_dotimes(items, environment)?),
            "DOLIST" => Some(self.special_dolist(items, environment)?),
            "DO" => Some(self.special_do(items, environment, false)?),
            "DO*" => Some(self.special_do(items, environment, true)?),
            _ => None,
        };
        Ok(value)
    }

    fn eval_special_form_bindings(
        &self,
        items: &[Form],
        name: &str,
        environment: &Environment,
    ) -> Result<Option<Value>, RuntimeError> {
        let value = match normalize_name(name).as_str() {
            "DESTRUCTURING-BIND" => Some(self.special_destructuring_bind(items, environment)?),
            "LET" => Some(self.special_let(items, environment, false)?),
            "LET*" => Some(self.special_let(items, environment, true)?),
            "FLET" => Some(self.special_flet(items, environment, false)?),
            "LABELS" => Some(self.special_flet(items, environment, true)?),
            _ => None,
        };
        Ok(value)
    }

    fn eval_special_form_conditionals(
        &self,
        items: &[Form],
        name: &str,
        environment: &Environment,
    ) -> Result<Option<Value>, RuntimeError> {
        let value = match normalize_name(name).as_str() {
            "AND" => Some(self.special_and(&items[1..], environment)?),
            "OR" => Some(self.special_or(&items[1..], environment)?),
            "WHEN" => Some(self.special_when(items, environment, true)?),
            "UNLESS" => Some(self.special_when(items, environment, false)?),
            "COND" => Some(self.special_cond(&items[1..], environment)?),
            "CASE" => Some(self.special_case(items, environment, false)?),
            "ECASE" => Some(self.special_case(items, environment, true)?),
            "TYPECASE" => Some(self.special_typecase(items, environment, false)?),
            "ETYPECASE" => Some(self.special_typecase(items, environment, true)?),
            _ => None,
        };
        Ok(value)
    }

    fn eval_special_form_core(
        &self,
        form: &Form,
        items: &[Form],
        name: &str,
        environment: &Environment,
    ) -> Result<Option<Value>, RuntimeError> {
        let value = match normalize_name(name).as_str() {
            "QUOTE" => Some(Self::special_quote(items, form.span)?),
            "QUASIQUOTE" => Some(self.special_quasiquote(items, environment)?),
            "DECLARE" | "DECLAIM" | "PROCLAIM" => Some(Value::Nil),
            "LOCALLY" => Some(self.special_locally(items, environment)?),
            "EVAL-WHEN" => Some(self.special_eval_when(items, environment)?),
            "THE" => Some(self.special_the(items, environment)?),
            "LOAD-TIME-VALUE" => Some(self.special_load_time_value(items, environment)?),
            "NTH-VALUE" => Some(self.special_nth_value(items, environment)?),
            "IF" => Some(self.special_if(items, environment)?),
            "PROGN" => Some(self.special_progn(&items[1..], environment)?),
            "PROG1" => Some(self.special_prog1(items, environment)?),
            "PROG2" => Some(self.special_prog2(items, environment)?),
            "PROG" => Some(self.special_prog(items, environment, false)?),
            "PROG*" => Some(self.special_prog(items, environment, true)?),
            "VALUES" => Some(self.special_values(items, environment)?),
            "IGNORE-ERRORS" => Some(self.special_ignore_errors(items, environment)?),
            "HANDLER-CASE" => Some(self.special_handler_case(items, environment)?),
            "HANDLER-BIND" => Some(self.special_handler_bind(items, environment)?),
            "RESTART-BIND" => Some(self.special_restart_bind(items, environment)?),
            "CATCH" => Some(self.special_catch(items, environment)?),
            "PROGV" => Some(self.special_progv(items, environment)?),
            "THROW" => Some(self.special_throw(items, environment)?),
            "WITH-CONDITION-RESTARTS" => {
                Some(self.special_with_condition_restarts(items, environment)?)
            }
            "WITH-SIMPLE-RESTART" => Some(self.special_with_simple_restart(items, environment)?),
            "WITH-OPEN-FILE" => {
                let expanded = Self::expand_with_open_file(form)?;
                Some(self.eval_expanded_values(&expanded, environment)?)
            }
            "RESTART-CASE" => Some(self.special_restart_case(items, environment)?),
            "UNWIND-PROTECT" => Some(self.special_unwind_protect(items, environment)?),
            "BLOCK" => Some(self.special_block(items, environment)?),
            "RETURN" => Some(self.special_return(items, environment)?),
            "RETURN-FROM" => Some(self.special_return_from(items, environment)?),
            "TAGBODY" => Some(self.special_tagbody(items, environment)?),
            "GO" => Some(Self::special_go(items, environment)?),
            "MULTIPLE-VALUE-BIND" => Some(self.special_multiple_value_bind(items, environment)?),
            "MULTIPLE-VALUE-CALL" => Some(self.special_multiple_value_call(items, environment)?),
            "MULTIPLE-VALUE-LIST" => Some(self.special_multiple_value_list(items, environment)?),
            "MULTIPLE-VALUE-PROG1" => Some(self.special_multiple_value_prog1(items, environment)?),
            _ => None,
        };
        Ok(value)
    }
}

mod evaluator_special_forms;
mod macros;
mod validation;

impl Runtime {
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
            Err(Self::constant_modification_error(name, span))
        } else {
            Ok(())
        }
    }

    fn invalid(message: &str, span: Span) -> RuntimeError {
        RuntimeError::InvalidForm {
            message: message.to_string(),
            span: Some(span),
        }
    }
}

mod collection_primitives;
mod evaluator_condition_methods;
mod evaluator_primitives;

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

mod helpers;
use helpers::{
    atom_name, control_tag, is_case_default_form, is_macro_keyword_form, is_nil_form,
    is_operator_form, is_special_form, is_special_operator_name, macro_dotted_parts,
    macro_keyword_name, prefix_argument, quasiquote_marker, unqualified_name,
};
mod evaluator_literals;
pub use evaluator_literals::{
    escaped_symbol_atom, literal_atom, quoted_form_value, resolved_symbol,
};
