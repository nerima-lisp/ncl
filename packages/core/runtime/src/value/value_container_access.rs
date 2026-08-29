use std::cell::RefCell;

use super::Value;

impl Value {
    /// Returns a copied proper-list payload when this value is a list.
    #[must_use]
    pub fn list_items(&self) -> Option<Vec<Self>> {
        match self {
            Self::Nil => Some(Vec::new()),
            Self::List(items) => Some(items.as_ref().clone()),
            _ => None,
        }
    }

    /// Returns a copied vector payload when this value is a vector.
    #[must_use]
    pub fn vector_items(&self) -> Option<Vec<Self>> {
        match self {
            Self::Vector(items) => Some(items.as_ref().clone()),
            _ => None,
        }
    }

    /// Returns copied array dimensions when this value is an array.
    #[must_use]
    pub fn array_dimensions(&self) -> Option<Vec<usize>> {
        match self {
            Self::Array { dimensions, .. } => Some(dimensions.as_ref().clone()),
            _ => None,
        }
    }

    /// Returns copied row-major array elements when this value is an array.
    #[must_use]
    pub fn array_items(&self) -> Option<Vec<Self>> {
        match self {
            Self::Array { elements, .. } => Some(elements.as_ref().clone()),
            _ => None,
        }
    }

    pub(crate) fn hash_table_test(&self) -> Option<&str> {
        match self {
            Self::HashTable { test, .. } => Some(test),
            _ => None,
        }
    }

    pub(crate) fn hash_table_entries(&self) -> Option<&RefCell<Vec<(Self, Self)>>> {
        match self {
            Self::HashTable { entries, .. } => Some(entries),
            _ => None,
        }
    }

    /// Returns the symbol-like name represented by this value, if any.
    #[must_use]
    pub fn symbol_name(&self) -> Option<&str> {
        match self {
            Self::Symbol(name)
            | Self::SymbolExact(name)
            | Self::UninternedSymbol(name)
            | Self::Keyword(name)
            | Self::KeywordExact(name) => Some(name),
            Self::Nil | Self::Boolean(false) => Some("NIL"),
            Self::Boolean(true) => Some("T"),
            _ => None,
        }
    }

    /// Returns a symbol name and whether its spelling is exact.
    #[must_use]
    pub fn symbol_reference(&self) -> Option<(&str, bool)> {
        match self {
            Self::Symbol(name) | Self::UninternedSymbol(name) | Self::Keyword(name) => {
                Some((name, false))
            }
            Self::SymbolExact(name) | Self::KeywordExact(name) => Some((name, true)),
            Self::Nil | Self::Boolean(false) => Some(("NIL", false)),
            Self::Boolean(true) => Some(("T", false)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::Value;

    #[test]
    fn containers_and_primary_value_have_stable_boundaries() {
        let list = Value::list(vec![Value::Integer(1), Value::symbol("x")]);
        assert_eq!(list.type_name(), "LIST");
        let Some(list_items) = list.list_items() else {
            panic!("expected list value");
        };
        assert_eq!(list_items.len(), 2);
        assert!(Value::list(Vec::new()).equal_value(&Value::Nil));
        let Some(vector_items) = Value::vector(vec![Value::Nil]).vector_items() else {
            panic!("expected vector value");
        };
        assert_eq!(vector_items.len(), 1);
        let array = Value::array(vec![2], vec![Value::Integer(1), Value::Integer(2)]);
        assert_eq!(array.array_dimensions(), Some(vec![2]));
        let Some(array_items) = array.array_items() else {
            panic!("expected array value");
        };
        assert_eq!(array_items.len(), 2);
        assert!(!Value::Nil.is_truthy());
        assert!(Value::values(vec![Value::Integer(7)]).is_truthy());
        assert!(
            Value::values(vec![])
                .primary_value()
                .equal_value(&Value::Nil)
        );
    }
}
