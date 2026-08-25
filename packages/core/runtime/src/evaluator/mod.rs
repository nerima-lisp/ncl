use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::rc::Rc;

use ncl_compiler::Compiler;
use ncl_syntax::{
    Form, FormKind, LambdaListKeywordParameter, OrdinaryLambdaList, Span, SymbolTokenKind,
    parse_ordinary_lambda_list, parse_symbol_token,
};

use crate::builtins;
use crate::environment::normalize_name;
use crate::error::ThrowTag;
use crate::package::{self, PackageState};
use crate::value::{
    ClassDefinition, ClassSlot, ClosureSpec, MacroAuxiliaryParameter, MacroKeywordParameter,
    MacroLambdaList, MacroOptionalParameter, MacroPattern, MethodDefinition, StructureDefinition,
    StructureSlot,
};
use crate::{Environment, ReturnValue, RuntimeError, Value};

mod api;
mod bindings;
mod data;
mod definitions;
mod evaluation;
mod function;
mod helpers;
mod invocation;
mod invocation_conditions;
mod lambda_lists;
mod macros;
mod package_designator;
mod resolve;
mod sequence_mapping;
mod sequences;
mod setf;
mod special_forms;
mod special_forms_iteration;
mod state;

use self::data::{
    MacroEnvironments, MacroLambdaListSection, SequenceMergeRequest, SequenceSubstituteRequest,
    SetfExpansion, SignalRequest, StructureBoaRequest,
};
pub(crate) use self::helpers::quoted_form_value;
use self::helpers::{
    atom_name, control_tag, escaped_symbol_atom, is_case_default_form, is_macro_keyword_form,
    is_nil_form, is_operator_form, is_special_operator_name, literal_atom, macro_dotted_parts,
    macro_keyword_name, prefix_argument, quasiquote_marker, resolved_symbol, sequence_items,
    unqualified_name,
};
pub(crate) use self::state::{ConditionHandlerBinding, RestartBinding};
use self::state::{
    ConditionHandlerGuard, ConditionHandlerSuspension, ConditionRestartBinding,
    ConditionRestartGuard, DynamicGuard, DynamicState, MethodContext, MethodContinuation,
    RestartGuard,
};

const MAX_MACRO_EXPANSIONS: usize = 64;

pub struct Runtime {
    global: Environment,
    packages: Rc<RefCell<PackageState>>,
    dynamic: Rc<RefCell<DynamicState>>,
    next_block_target: Cell<u64>,
    gensym_counter: Cell<u64>,
    method_context: RefCell<Vec<MethodContext>>,
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}
