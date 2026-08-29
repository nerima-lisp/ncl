use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use ncl_syntax::Form;

use super::{MethodDefinition, Value};

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum MacroLambdaListSection {
    Required,
    Optional,
    Rest,
    Keyword,
    Auxiliary,
}

#[derive(Debug, Default)]
pub struct DynamicState {
    pub special_names: HashSet<String>,
    pub exact_special_names: HashSet<String>,
    pub constants: HashSet<String>,
    pub exact_constants: HashSet<String>,
    pub globals: HashMap<String, Value>,
    pub exact_globals: HashMap<String, Value>,
    pub bindings: Vec<(String, Value)>,
    pub exact_bindings: Vec<(String, Value)>,
    pub condition_handlers: Vec<ConditionHandlerBinding>,
    pub restart_bindings: Vec<RestartBinding>,
    pub condition_restart_bindings: Vec<ConditionRestartBinding>,
}

#[derive(Clone, Debug)]
pub struct ConditionHandlerBinding {
    pub condition: String,
    pub function: Option<Value>,
    pub catch: bool,
}

#[derive(Clone, Debug)]
pub struct RestartBinding {
    pub name: String,
    pub function: Option<Value>,
    pub restart: Value,
}

impl RestartBinding {
    pub fn new(name: String, function: Option<Value>) -> Self {
        let restart = Value::restart(&name);
        Self {
            name,
            function,
            restart,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ConditionRestartBinding {
    pub condition: Value,
    pub restarts: Vec<Value>,
}

pub struct SetfExpansion {
    pub temporaries: Vec<Form>,
    pub values: Vec<Form>,
    pub store: Form,
    pub store_form: Form,
    pub access_form: Form,
}

pub struct DynamicGuard {
    pub state: Rc<RefCell<DynamicState>>,
    pub depth: usize,
    pub exact_depth: usize,
}

impl Drop for DynamicGuard {
    fn drop(&mut self) {
        let mut state = self.state.borrow_mut();
        state.bindings.truncate(self.depth);
        state.exact_bindings.truncate(self.exact_depth);
    }
}

pub struct ConditionHandlerGuard {
    pub state: Rc<RefCell<DynamicState>>,
    pub depth: usize,
}

impl Drop for ConditionHandlerGuard {
    fn drop(&mut self) {
        self.state
            .borrow_mut()
            .condition_handlers
            .truncate(self.depth);
    }
}

pub struct RestartGuard {
    pub state: Rc<RefCell<DynamicState>>,
    pub depth: usize,
}

impl Drop for RestartGuard {
    fn drop(&mut self) {
        self.state
            .borrow_mut()
            .restart_bindings
            .truncate(self.depth);
    }
}

pub struct ConditionRestartGuard {
    pub state: Rc<RefCell<DynamicState>>,
    pub depth: usize,
}

impl Drop for ConditionRestartGuard {
    fn drop(&mut self) {
        self.state
            .borrow_mut()
            .condition_restart_bindings
            .truncate(self.depth);
    }
}

pub struct ConditionHandlerSuspension {
    pub state: Rc<RefCell<DynamicState>>,
    pub index: usize,
    pub binding: Option<ConditionHandlerBinding>,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn handler(condition: &str) -> ConditionHandlerBinding {
        ConditionHandlerBinding {
            condition: condition.to_string(),
            function: None,
            catch: false,
        }
    }

    #[test]
    fn suspension_drop_restores_binding_once_at_a_valid_index() {
        let state = Rc::new(RefCell::new(DynamicState {
            condition_handlers: vec![handler("FIRST"), handler("LAST")],
            ..DynamicState::default()
        }));
        let binding = state.borrow_mut().condition_handlers.remove(1);

        drop(ConditionHandlerSuspension {
            state: Rc::clone(&state),
            index: 1,
            binding: Some(binding),
        });

        let conditions = state
            .borrow()
            .condition_handlers
            .iter()
            .map(|binding| binding.condition.clone())
            .collect::<Vec<_>>();
        assert_eq!(conditions, ["FIRST".to_string(), "LAST".to_string()]);
    }

    #[test]
    fn suspension_drop_handles_empty_and_out_of_range_cases() {
        for (initial, index, binding, expected) in [
            (vec![], 0, Some("RESTORED"), vec!["RESTORED".to_string()]),
            (
                vec!["FIRST"],
                usize::MAX,
                Some("RESTORED"),
                vec!["FIRST".to_string(), "RESTORED".to_string()],
            ),
            (vec!["FIRST"], 0, None, vec!["FIRST".to_string()]),
        ] {
            let state = Rc::new(RefCell::new(DynamicState {
                condition_handlers: initial.into_iter().map(handler).collect(),
                ..DynamicState::default()
            }));
            let suspension = ConditionHandlerSuspension {
                state: Rc::clone(&state),
                index,
                binding: binding.map(handler),
            };
            drop(suspension);

            let conditions = state
                .borrow()
                .condition_handlers
                .iter()
                .map(|binding| binding.condition.clone())
                .collect::<Vec<_>>();
            assert_eq!(conditions, expected);
        }
    }
}

#[derive(Clone, Debug)]
pub enum MethodContinuation {
    Chain {
        methods: Vec<MethodDefinition>,
        index: usize,
        fallback: Option<Box<Self>>,
    },
    Core {
        before: Vec<MethodDefinition>,
        primary: Vec<MethodDefinition>,
        after: Vec<MethodDefinition>,
    },
}

#[derive(Debug)]
pub struct MethodContext {
    pub arguments: Vec<Value>,
    pub next: Option<MethodContinuation>,
}
