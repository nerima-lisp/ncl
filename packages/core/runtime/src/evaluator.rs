use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::rc::Rc;

use ncl_compiler::Compiler;
use ncl_syntax::{
    Form, FormKind, LambdaListAuxiliaryParameter, LambdaListKeywordParameter,
    LambdaListOptionalParameter, OrdinaryLambdaList, Span, SymbolTokenKind, parse_symbol_token,
    read,
};

use crate::builtins;
use crate::environment::normalize_name;
use crate::package::{self, PackageState};
use crate::value::{
    ClassDefinition, ClassSlot, MacroLambdaList, MacroPattern, MethodDefinition,
    StructureDefinition, StructureSlot,
};
use crate::{Environment, RuntimeError, Value};

const MAX_MACRO_EXPANSIONS: usize = 64;

pub mod evaluator_state;
pub use evaluator_state::{ConditionHandlerBinding, RestartBinding};
mod compilation;
mod dispatch;
mod dynamic_guards;
mod entry_points;
mod evaluator_package_primitives;
mod evaluator_primitive_dispatch;
mod evaluator_resolution;
mod packages;
mod special_form_dispatch_control;
mod special_form_dispatch_definitions;
mod special_variables;
mod variable_assignment;
mod variable_binding;
mod variable_lookup;
use evaluator_state::{
    ConditionHandlerGuard, ConditionHandlerSuspension, ConditionRestartBinding,
    ConditionRestartGuard, DynamicGuard, DynamicState, MethodContext, MethodContinuation,
    RestartGuard, SetfExpansion,
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
}

mod evaluator_special_forms;
mod macros;
mod validation;

impl Runtime {
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
    atom_name, is_nil_form, is_operator_form, is_special_form, is_special_operator_name,
    macro_keyword_name, prefix_argument, quasiquote_marker, unqualified_name,
};
mod evaluator_literals;
pub use evaluator_literals::{literal_atom, quoted_form_value, resolved_symbol};
