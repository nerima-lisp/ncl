//! Lambda lists, both ordinary and macro.
//!
//! One type covers both forms. An ordinary lambda list leaves [`LambdaList::whole`],
//! [`LambdaList::environment`], and [`LambdaList::body`] as `None` and never
//! contains a [`ParamName::Pattern`]; the parser rejects those in an ordinary
//! lambda list.

use crate::ast::Expr;
use crate::symbols::SymbolRef;

/// A parameter name: a symbol, or a destructuring pattern.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum ParamName {
    /// A plain symbol.
    Symbol(SymbolRef),
    /// A nested destructuring pattern, permitted only in a macro lambda list.
    Pattern(Box<LambdaList>),
}

impl ParamName {
    /// The symbol this name binds, if it is not a pattern.
    #[must_use]
    pub const fn symbol(&self) -> Option<&SymbolRef> {
        match self {
            Self::Symbol(symbol) => Some(symbol),
            Self::Pattern(_) => None,
        }
    }
}

/// One `&optional` parameter.
#[derive(Clone, Debug, PartialEq)]
pub struct OptionalParam {
    /// The parameter name.
    pub name: ParamName,
    /// The default form, or `None` for `nil`.
    pub default: Option<Expr>,
    /// The supplied-p variable, if named.
    pub supplied_p: Option<ParamName>,
}

/// One `&key` parameter.
#[derive(Clone, Debug, PartialEq)]
pub struct KeyParam {
    /// The keyword the caller passes.
    pub keyword: SymbolRef,
    /// The parameter name.
    pub name: ParamName,
    /// The default form, or `None` for `nil`.
    pub default: Option<Expr>,
    /// The supplied-p variable, if named.
    pub supplied_p: Option<ParamName>,
}

/// One `&aux` parameter.
#[derive(Clone, Debug, PartialEq)]
pub struct AuxParam {
    /// The parameter name.
    pub name: ParamName,
    /// The initial value form, or `None` for `nil`.
    pub default: Option<Expr>,
}

/// A parsed lambda list.
#[derive(Clone, Debug, PartialEq)]
pub struct LambdaList {
    /// The required parameters.
    pub required: Vec<ParamName>,
    /// The `&optional` parameters.
    pub optional: Vec<OptionalParam>,
    /// The `&rest` parameter, if present.
    pub rest: Option<ParamName>,
    /// The `&key` parameters.
    pub keys: Vec<KeyParam>,
    /// Whether `&allow-other-keys` was present.
    pub allow_other_keys: bool,
    /// The `&aux` parameters.
    pub aux: Vec<AuxParam>,
    /// The `&whole` variable, permitted only in a macro lambda list.
    pub whole: Option<SymbolRef>,
    /// The `&environment` variable, permitted only in a macro lambda list.
    pub environment: Option<SymbolRef>,
    /// The `&body` parameter, permitted only in a macro lambda list.
    pub body: Option<ParamName>,
}

impl LambdaList {
    /// Create an empty lambda list.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            required: Vec::new(),
            optional: Vec::new(),
            rest: None,
            keys: Vec::new(),
            allow_other_keys: false,
            aux: Vec::new(),
            whole: None,
            environment: None,
            body: None,
        }
    }

    /// Whether the lambda list accepts no arguments.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.required.is_empty()
            && self.optional.is_empty()
            && self.rest.is_none()
            && self.keys.is_empty()
            && self.body.is_none()
    }

    /// Whether the lambda list names `&key` parameters.
    #[must_use]
    pub const fn accepts_keywords(&self) -> bool {
        !self.keys.is_empty() || self.allow_other_keys
    }

    /// The keyword symbols accepted by `&key`, in declaration order.
    #[must_use]
    pub fn keyword_names(&self) -> Vec<&SymbolRef> {
        self.keys.iter().map(|key| &key.keyword).collect()
    }
}

impl Default for LambdaList {
    fn default() -> Self {
        Self::new()
    }
}
