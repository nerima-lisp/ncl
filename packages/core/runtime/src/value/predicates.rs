impl Value {
    pub fn is_truthy(&self) -> bool {
        !matches!(self.primary_value(), Self::Nil | Self::Boolean(false))
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Nil => "NIL",
            Self::Unbound => "UNBOUND",
            Self::Boolean(_) => "BOOLEAN",
            Self::Integer(_) => "INTEGER",
            Self::Rational(_) => "RATIO",
            Self::Float(_) => "FLOAT",
            Self::Complex { .. } => "COMPLEX",
            Self::String(_) => "STRING",
            Self::Character(_) => "CHARACTER",
            Self::Stream(_) => "STREAM",
            Self::Package(_) => "PACKAGE",
            Self::Environment(_) => "ENVIRONMENT",
            Self::Symbol(_) | Self::SymbolExact(_) | Self::UninternedSymbol(_) => "SYMBOL",
            Self::Keyword(_) | Self::KeywordExact(_) => "KEYWORD",
            Self::List(_) | Self::DottedList { .. } => "LIST",
            Self::Vector { .. } => "VECTOR",
            Self::Array { .. } => "ARRAY",
            Self::HashTable { .. } => "HASH-TABLE",
            Self::Values(_) => "VALUES",
            Self::Condition(_) => "CONDITION",
            Self::Restart(_) => "RESTART",
            Self::Structure { representation, .. } => match representation {
                StructureRepresentation::Record => "STRUCTURE",
                StructureRepresentation::List { .. } => "LIST",
                StructureRepresentation::Vector { .. } => "VECTOR",
            },
            Self::Class(_) => "CLASS",
            Self::Instance(_) => "STANDARD-OBJECT",
            Self::Method(_) => "METHOD",
            Self::Function(_) => "FUNCTION",
        }
    }

    pub(crate) fn condition_is_type(&self, expected: &str) -> bool {
        let Self::Condition(condition) = self else {
            return false;
        };
        let expected = expected.trim_start_matches(':').to_ascii_uppercase();
        if condition.actual_type.eq_ignore_ascii_case(&expected) {
            return true;
        }
        if condition
            .type_names
            .iter()
            .any(|type_name| type_name.eq_ignore_ascii_case(&expected))
        {
            return true;
        }
        if expected == "CONDITION" {
            return true;
        }
        match condition.actual_type.as_str() {
            "SIMPLE-ERROR" => matches!(
                expected.as_str(),
                "CONDITION" | "ERROR" | "SERIOUS-CONDITION" | "SIMPLE-CONDITION"
            ),
            "SIMPLE-WARNING" => matches!(
                expected.as_str(),
                "CONDITION" | "WARNING" | "SIMPLE-CONDITION"
            ),
            "SIMPLE-CONDITION" => expected == "CONDITION",
            "DIVISION-BY-ZERO" => matches!(
                expected.as_str(),
                "CONDITION" | "ERROR" | "SERIOUS-CONDITION" | "ARITHMETIC-ERROR"
            ),
            "ARITHMETIC-ERROR" => {
                matches!(
                    expected.as_str(),
                    "CONDITION" | "ERROR" | "SERIOUS-CONDITION"
                )
            }
            "TYPE-ERROR" | "PROGRAM-ERROR" | "PACKAGE-ERROR" | "READER-ERROR"
            | "COMPILER-ERROR" | "FILE-ERROR" | "UNBOUND-VARIABLE" => {
                matches!(
                    expected.as_str(),
                    "CONDITION" | "ERROR" | "SERIOUS-CONDITION"
                )
            }
            "CONTROL-ERROR" => matches!(expected.as_str(), "CONDITION"),
            _ => false,
        }
    }

    pub(crate) fn condition_type_names(&self) -> Option<Vec<String>> {
        match self {
            Self::Condition(condition) => Some(condition.type_names.as_ref().clone()),
            _ => None,
        }
    }

    pub(crate) fn condition_slot(&self, condition_name: &str, slot_name: &str) -> Option<Value> {
        let Self::Condition(condition) = self else {
            return None;
        };
        if !self.condition_is_type(condition_name) {
            return None;
        }
        condition
            .slots
            .borrow()
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(slot_name))
            .map(|(_, value)| value.clone())
    }

    pub(crate) fn set_condition_slot(
        &self,
        condition_name: &str,
        slot_name: &str,
        value: Value,
    ) -> bool {
        let Self::Condition(condition) = self else {
            return false;
        };
        if !self.condition_is_type(condition_name) {
            return false;
        }
        let mut slots = condition.slots.borrow_mut();
        if let Some((_, slot_value)) = slots
            .iter_mut()
            .find(|(name, _)| name.eq_ignore_ascii_case(slot_name))
        {
            *slot_value = value;
            true
        } else {
            false
        }
    }

    pub(crate) fn simple_condition_format_control(&self) -> Option<&str> {
        match self {
            Self::Condition(condition) => condition.format_control.as_deref(),
            _ => None,
        }
    }

    pub(crate) fn condition_type_name(&self) -> Option<&str> {
        match self {
            Self::Condition(condition) => Some(condition.actual_type.as_str()),
            _ => None,
        }
    }

    pub(crate) fn condition_message(&self) -> Option<&str> {
        match self {
            Self::Condition(condition) => Some(condition.message.as_ref()),
            _ => None,
        }
    }

    pub(crate) fn restart_name(&self) -> Option<&str> {
        match self {
            Self::Restart(restart) => Some(restart.name()),
            _ => None,
        }
    }

    pub(crate) fn simple_condition_format_arguments(&self) -> Option<Vec<Value>> {
        match self {
            Self::Condition(condition) if condition.format_control.is_some() => {
                Some(condition.format_arguments.clone())
            }
            _ => None,
        }
    }

}
