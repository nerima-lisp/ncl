use std::cell::RefCell;
use std::rc::Rc;

use crate::evaluator::evaluator_state::bindings::{ConditionHandlerBinding, DynamicState};

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
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::ConditionHandlerSuspension;
    use crate::evaluator::evaluator_state::bindings::{ConditionHandlerBinding, DynamicState};

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
