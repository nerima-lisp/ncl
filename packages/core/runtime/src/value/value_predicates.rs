use super::Value;

impl Value {
    pub(crate) fn primary_value(&self) -> Self {
        match self {
            Self::Values(values) => values.first().cloned().unwrap_or(Self::Nil),
            _ => self.clone(),
        }
    }

    pub(crate) fn multiple_values(&self) -> Vec<Self> {
        match self {
            Self::Values(values) => values.as_ref().clone(),
            _ => vec![self.clone()],
        }
    }

    /// Returns whether this value is true in Lisp conditional contexts.
    #[must_use]
    pub fn is_truthy(&self) -> bool {
        !matches!(self.primary_value(), Self::Nil | Self::Boolean(false))
    }

    /// Returns the implementation's canonical Lisp type name.
    #[must_use]
    pub const fn type_name(&self) -> &'static str {
        match self {
            Self::Nil => "NIL",
            Self::Unbound => "UNBOUND",
            Self::Boolean(_) => "BOOLEAN",
            Self::Integer(_) => "INTEGER",
            Self::Rational(_) => "RATIO",
            Self::Float(_) => "FLOAT",
            Self::String(_) => "STRING",
            Self::Character(_) => "CHARACTER",
            Self::Stream(_) => "STREAM",
            Self::RandomState(_) => "RANDOM-STATE",
            Self::Package(_) => "PACKAGE",
            Self::Environment(_) => "ENVIRONMENT",
            Self::Symbol(_) | Self::SymbolExact(_) | Self::UninternedSymbol(_) => "SYMBOL",
            Self::Keyword(_) | Self::KeywordExact(_) => "KEYWORD",
            Self::List(_) | Self::DottedList { .. } => "LIST",
            Self::Vector(_) => "VECTOR",
            Self::Array { .. } => "ARRAY",
            Self::HashTable { .. } => "HASH-TABLE",
            Self::Values(_) => "VALUES",
            Self::Condition(_) => "CONDITION",
            Self::Restart(_) => "RESTART",
            Self::Structure { .. } => "STRUCTURE",
            Self::Class(_) => "CLASS",
            Self::Instance(_) => "STANDARD-OBJECT",
            Self::Function(_) => "FUNCTION",
        }
    }
}
