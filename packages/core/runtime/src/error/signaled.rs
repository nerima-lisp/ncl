use std::rc::Rc;

use ncl_syntax::Span;

use crate::error::ReturnValue;

pub type ConditionName = Rc<str>;

#[derive(Clone, Debug, Eq, PartialEq)]
/// Payload for a signaled condition.
pub struct SignaledError {
    pub(crate) condition: ConditionName,
    pub(crate) condition_types: Box<[ConditionName]>,
    pub(crate) message: String,
    pub(crate) format_control: Option<String>,
    pub(crate) format_arguments: Box<[ReturnValue]>,
    pub(crate) warning: bool,
    pub(crate) span: Option<Span>,
}
