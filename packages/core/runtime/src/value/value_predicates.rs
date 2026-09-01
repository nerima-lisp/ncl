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

    pub(crate) fn is_number(&self) -> bool {
        matches!(
            self,
            Self::Integer(_)
                | Self::BigInteger(_)
                | Self::Rational(_)
                | Self::Float(_)
                | Self::Complex(_)
        )
    }

    pub(crate) const fn is_complex(&self) -> bool {
        matches!(self, Self::Complex(_))
    }

    /// Returns the implementation's canonical Lisp type name.
    #[must_use]
    pub const fn type_name(&self) -> &'static str {
        match self {
            Self::Nil => "NIL",
            Self::Unbound => "UNBOUND",
            Self::Boolean(_) => "BOOLEAN",
            Self::Integer(_) | Self::BigInteger(_) => "INTEGER",
            Self::Rational(_) => "RATIO",
            Self::Float(_) => "FLOAT",
            Self::Complex(_) => "COMPLEX",
            Self::String(_) => "STRING",
            Self::Character(_) => "CHARACTER",
            Self::Stream(_) => "STREAM",
            Self::RandomState(_) => "RANDOM-STATE",
            Self::Package(_) => "PACKAGE",
            Self::Environment(_) => "ENVIRONMENT",
            Self::Symbol(_) | Self::SymbolExact(_) | Self::UninternedSymbol(_) => "SYMBOL",
            Self::Keyword(_) | Self::KeywordExact(_) => "KEYWORD",
            Self::List(_) | Self::MutableCons(_) | Self::DottedList { .. } => "LIST",
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

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use super::super::{ClassDefinition, Environment, RandomState, Value};

    #[test]
    fn type_name_covers_every_value_variant() {
        let class_definition = Rc::new(ClassDefinition {
            name: "POINT".to_owned(),
            precedence: Vec::new(),
            slots: Vec::new(),
            default_initargs: Vec::new(),
        });
        let cases = [
            (Value::Unbound, "UNBOUND"),
            (Value::Boolean(true), "BOOLEAN"),
            (Value::string_output_stream(), "STREAM"),
            (Value::random_state(RandomState::seeded()), "RANDOM-STATE"),
            (Value::package("USER"), "PACKAGE"),
            (Value::Environment(Environment::new()), "ENVIRONMENT"),
            (Value::keyword("key"), "KEYWORD"),
            (Value::keyword_exact("key"), "KEYWORD"),
            (Value::array(vec![1], vec![Value::Nil]), "ARRAY"),
            (Value::hash_table("EQ"), "HASH-TABLE"),
            (Value::values(vec![Value::Integer(1)]), "VALUES"),
            (Value::restart("retry"), "RESTART"),
            (
                Value::structure_with_types("point", Vec::new(), Vec::new()),
                "STRUCTURE",
            ),
            (Value::class_object(class_definition.clone()), "CLASS"),
            (
                Value::instance(class_definition, Vec::new()),
                "STANDARD-OBJECT",
            ),
            (Value::primitive("PRIM"), "FUNCTION"),
        ];
        for (value, expected) in cases {
            assert_eq!(value.type_name(), expected);
        }
    }
}
