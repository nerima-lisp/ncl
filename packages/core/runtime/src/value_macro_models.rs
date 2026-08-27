use super::Form;

#[derive(Clone, Debug)]
pub enum MacroPattern {
    Name(String),
    List(Vec<Self>),
    Dotted { items: Vec<Self>, tail: Box<Self> },
}

#[derive(Clone, Debug)]
pub struct MacroOptionalParameter {
    pub(crate) pattern: MacroPattern,
    pub(crate) init_form: Form,
    pub(crate) supplied_p: Option<String>,
}

#[derive(Clone, Debug)]
pub struct MacroKeywordParameter {
    pub(crate) keyword_name: String,
    pub(crate) pattern: MacroPattern,
    pub(crate) init_form: Form,
    pub(crate) supplied_p: Option<String>,
}

#[derive(Clone, Debug)]
pub struct MacroAuxiliaryParameter {
    pub(crate) name: String,
    pub(crate) init_form: Form,
}

#[derive(Clone, Debug)]
pub struct MacroLambdaList {
    pub(crate) whole: Option<String>,
    pub(crate) environment: Option<String>,
    pub(crate) required: Vec<MacroPattern>,
    pub(crate) optional: Vec<MacroOptionalParameter>,
    pub(crate) rest: Option<String>,
    pub(crate) keywords: Vec<MacroKeywordParameter>,
    pub(crate) has_keyword_section: bool,
    pub(crate) allow_other_keys: bool,
    pub(crate) auxiliary: Vec<MacroAuxiliaryParameter>,
}
