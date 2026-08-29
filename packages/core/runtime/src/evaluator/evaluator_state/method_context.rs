use crate::Value;
use crate::value::MethodDefinition;

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
