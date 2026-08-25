use ncl_syntax::{Form, OrdinaryLambdaList, Span};

use crate::value::StructureSlot;
use crate::{Environment, Value};

pub(super) struct SequenceMergeRequest<'a> {
    pub(super) result_type: &'a Value,
    pub(super) sequence1: &'a Value,
    pub(super) sequence2: &'a Value,
    pub(super) predicate: &'a Value,
    pub(super) options: &'a [Value],
    pub(super) environment: &'a Environment,
    pub(super) span: Span,
}

pub(super) struct SequenceSubstituteRequest<'a> {
    pub(super) operation: &'a str,
    pub(super) new_item: &'a Value,
    pub(super) old_or_predicate: &'a Value,
    pub(super) sequence: &'a Value,
    pub(super) options: &'a [Value],
    pub(super) environment: &'a Environment,
    pub(super) span: Span,
}

pub(super) struct StructureBoaRequest<'a> {
    pub(super) name: &'a str,
    pub(super) slots: &'a [StructureSlot],
    pub(super) structure_types: &'a [String],
    pub(super) lambda_list: &'a OrdinaryLambdaList,
    pub(super) definition_environment: &'a Environment,
    pub(super) arguments: &'a [Value],
    pub(super) span: Span,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum MacroLambdaListSection {
    Required,
    Optional,
    Rest,
    Keyword,
    Auxiliary,
}

pub(crate) struct SetfExpansion {
    pub(super) temporaries: Vec<Form>,
    pub(super) values: Vec<Form>,
    pub(super) store: Form,
    pub(super) store_form: Form,
    pub(super) access_form: Form,
}

#[derive(Clone, Copy)]
pub(super) struct MacroEnvironments<'a> {
    pub(super) macro_environment: &'a Environment,
    pub(super) environment: &'a Environment,
}

pub(super) struct SignalRequest<'a> {
    pub(super) condition: &'a str,
    pub(super) message: String,
    pub(super) format_control: Option<String>,
    pub(super) format_arguments: &'a [Value],
    pub(super) warning: bool,
    pub(super) environment: &'a Environment,
    pub(super) span: Span,
}
