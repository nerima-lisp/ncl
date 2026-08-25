use std::cell::RefCell;
use std::rc::Rc;

use crate::value::{MethodDefinition, Value};

#[derive(Default)]
pub(super) struct DynamicState {
    pub(super) special_names: std::collections::HashSet<String>,
    pub(super) exact_special_names: std::collections::HashSet<String>,
    pub(super) constants: std::collections::HashSet<String>,
    pub(super) exact_constants: std::collections::HashSet<String>,
    pub(super) globals: std::collections::HashMap<String, Value>,
    pub(super) exact_globals: std::collections::HashMap<String, Value>,
    pub(super) bindings: Vec<(String, Value)>,
    pub(super) exact_bindings: Vec<(String, Value)>,
    pub(super) condition_handlers: Vec<ConditionHandlerBinding>,
    pub(super) restart_bindings: Vec<RestartBinding>,
    pub(super) condition_restart_bindings: Vec<ConditionRestartBinding>,
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
    pub(crate) restart: Value,
}

impl RestartBinding {
    pub(crate) fn new(name: String, function: Option<Value>) -> Self {
        Self {
            restart: Value::restart(&name),
            name,
            function,
        }
    }
}

#[derive(Clone)]
pub(crate) struct ConditionRestartBinding {
    pub(crate) condition: Value,
    pub(crate) restarts: Vec<Value>,
}

pub(crate) struct DynamicGuard {
    pub(super) state: Rc<RefCell<DynamicState>>,
    pub(super) depth: usize,
    pub(super) exact_depth: usize,
}

impl Drop for DynamicGuard {
    fn drop(&mut self) {
        let mut state = self.state.borrow_mut();
        state.bindings.truncate(self.depth);
        state.exact_bindings.truncate(self.exact_depth);
    }
}

pub(crate) struct ConditionHandlerGuard {
    pub(super) state: Rc<RefCell<DynamicState>>,
    pub(super) depth: usize,
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
    pub(super) state: Rc<RefCell<DynamicState>>,
    pub(super) depth: usize,
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
    pub(super) state: Rc<RefCell<DynamicState>>,
    pub(super) depth: usize,
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
    pub(super) state: Rc<RefCell<DynamicState>>,
    pub(super) index: usize,
    pub(super) binding: Option<ConditionHandlerBinding>,
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
pub(super) enum MethodContinuation {
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

pub(super) struct MethodContext {
    pub(super) arguments: Vec<Value>,
    pub(super) next: Option<MethodContinuation>,
}
