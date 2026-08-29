use std::collections::{HashMap, HashSet};

use crate::Value;

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
