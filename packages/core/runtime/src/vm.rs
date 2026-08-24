use std::collections::HashMap;
use std::rc::Rc;

use ncl_compiler::{
    Constant, DestructureLambdaList, DestructurePattern, DestructureSpec, FunctionCode, FunctionId,
    Instruction, Program,
};
use ncl_syntax::{Form, FormKind, Span};

use crate::builtins::eql_value;
use crate::environment::normalize_name;
use crate::error::ThrowTag;
use crate::evaluator::{ConditionHandlerBinding, RestartBinding};
use crate::{Environment, ReturnValue, Runtime, RuntimeError, Value};

include!("vm/entry.rs");
include!("vm/execution.rs");
include!("vm/operations.rs");
include!("vm/destructuring.rs");
include!("vm/helpers.rs");
