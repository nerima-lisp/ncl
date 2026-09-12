use std::cell::RefCell;
use std::rc::Rc;

use super::Value;

impl Value {
    /// Clones the elements of a finite proper list, retaining nested identities.
    #[must_use]
    pub fn list_items(&self) -> Option<Vec<Self>> {
        let (items, tail) = self.list_parts()?;
        matches!(tail, Self::Nil | Self::Boolean(false)).then_some(items)
    }

    pub(crate) fn list_parts(&self) -> Option<(Vec<Self>, Self)> {
        if !matches!(self, Self::Nil | Self::Boolean(false) | Self::Cons(_)) {
            return None;
        }
        let mut seen = std::collections::HashSet::new();
        let mut items = Vec::new();
        let mut tail = self.clone();
        while let Self::Cons(cell) = tail {
            if !seen.insert(cell.identity()) {
                return None;
            }
            items.push(cell.car());
            tail = cell.cdr();
        }
        Some((items, tail))
    }

    pub(crate) fn nth_tail(&self, index: usize) -> Option<Self> {
        let mut tail = self.clone();
        for _ in 0..index {
            tail = match tail {
                Self::Cons(cell) => cell.cdr(),
                Self::Nil | Self::Boolean(false) => return Some(Self::Nil),
                _ => return None,
            };
        }
        Some(tail)
    }

    pub(crate) fn list_cells(&self) -> Option<Vec<super::SharedCons>> {
        let mut cells = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut tail = self.clone();
        while let Self::Cons(cell) = tail {
            if !seen.insert(cell.identity()) {
                return None;
            }
            tail = cell.cdr();
            cells.push(cell);
        }
        matches!(tail, Self::Nil | Self::Boolean(false)).then_some(cells)
    }

    pub(crate) fn replace_list_range(&self, start: usize, values: &[Self]) -> bool {
        let Some(cells) = self.list_cells() else {
            return false;
        };
        let Some(end) = start.checked_add(values.len()) else {
            return false;
        };
        let Some(range) = cells.get(start..end) else {
            return false;
        };
        for (cell, value) in range.iter().zip(values) {
            cell.set_car(value.clone());
        }
        true
    }

    /// Returns a copied vector payload when this value is a vector.
    #[must_use]
    pub fn vector_items(&self) -> Option<Vec<Self>> {
        match self {
            Self::Vector(items) => Some(items.visible_snapshot()),
            _ => None,
        }
    }

    pub(crate) fn string_contents(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }

    pub(crate) fn sequence_items(&self) -> Option<Vec<Self>> {
        self.list_items().or_else(|| self.vector_items())
    }

    pub(crate) fn set_vector_item(&self, index: usize, value: Self) -> bool {
        match self {
            Self::Vector(elements) => elements.set(index, value),
            _ => false,
        }
    }

    pub(crate) fn set_array_item(&self, index: usize, value: Self) -> bool {
        match self {
            Self::Array { elements, .. } => elements.set(index, value),
            Self::Vector(elements) => elements.set(index, value),
            _ => false,
        }
    }

    pub(crate) fn array_adjustable(&self) -> Option<bool> {
        self.array_storage()
            .map(|elements| elements.is_adjustable())
    }

    pub(crate) fn array_has_fill_pointer(&self) -> Option<bool> {
        self.array_storage()
            .map(|elements| elements.has_fill_pointer())
    }

    pub(crate) fn is_displaced(&self) -> Option<bool> {
        self.array_storage().map(|elements| elements.is_displaced())
    }

    pub(crate) fn array_storage(&self) -> Option<super::SharedElements> {
        match self {
            Self::Vector(items)
            | Self::Array {
                elements: items, ..
            } => Some(items.clone()),
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
            Self::Array { elements, .. } => Some(elements.snapshot()),
            _ => None,
        }
    }

    pub(crate) fn hash_table_test(&self) -> Option<&str> {
        match self {
            Self::HashTable { test, .. } => Some(test),
            _ => None,
        }
    }

    pub(crate) fn hash_table_size(&self) -> Option<usize> {
        match self {
            Self::HashTable { size, .. } => Some(size.get()),
            _ => None,
        }
    }

    pub(crate) fn set_hash_table_size(&self, size: usize) -> bool {
        match self {
            Self::HashTable { size: current, .. } => {
                current.set(size);
                true
            }
            _ => false,
        }
    }

    pub(crate) fn hash_table_rehash_size(&self) -> Option<&Self> {
        match self {
            Self::HashTable { rehash_size, .. } => Some(rehash_size),
            _ => None,
        }
    }

    pub(crate) fn hash_table_rehash_threshold(&self) -> Option<&Self> {
        match self {
            Self::HashTable {
                rehash_threshold, ..
            } => Some(rehash_threshold),
            _ => None,
        }
    }

    pub(crate) fn hash_table_weakness(&self) -> Option<Option<&str>> {
        match self {
            Self::HashTable { weakness, .. } => Some(weakness.as_deref()),
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
            Self::InternedSymbol(symbol) => Some(symbol.name()),
            Self::Nil | Self::Boolean(false) => Some("NIL"),
            Self::Boolean(true) => Some("T"),
            _ => None,
        }
    }

    /// Returns a symbol name and whether its spelling is exact.
    #[must_use]
    pub fn symbol_reference(&self) -> Option<(String, bool)> {
        match self {
            Self::Symbol(name) | Self::UninternedSymbol(name) | Self::Keyword(name) => {
                Some((name.to_string(), false))
            }
            Self::InternedSymbol(symbol) => Some((symbol.reference(), symbol.exact())),
            Self::SymbolExact(name) | Self::KeywordExact(name) => Some((name.to_string(), true)),
            Self::Nil | Self::Boolean(false) => Some(("NIL".to_string(), false)),
            Self::Boolean(true) => Some(("T".to_string(), false)),
            _ => None,
        }
    }

    /// Returns the variable-cell reference represented by this value.
    ///
    /// Uninterned symbols include their allocation identity in variable-cell
    /// references, so same-spelled symbols do not alias one another.
    #[must_use]
    pub fn variable_reference(&self) -> Option<(String, bool)> {
        match self {
            Self::UninternedSymbol(name) => Some((
                format!("__NCL_UNINTERNED__{:p}", Rc::as_ptr(name) as *const (),),
                false,
            )),
            Self::Symbol(name) | Self::Keyword(name) => Some((name.to_string(), false)),
            Self::InternedSymbol(symbol) => Some((symbol.reference(), symbol.exact())),
            Self::SymbolExact(name) | Self::KeywordExact(name) => Some((name.to_string(), true)),
            Self::Nil | Self::Boolean(false) => Some(("NIL".to_string(), false)),
            Self::Boolean(true) => Some(("T".to_string(), false)),
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

    #[test]
    fn array_accessors_reject_non_array_values() {
        assert!(Value::Nil.array_dimensions().is_none());
        assert!(Value::Nil.array_items().is_none());
    }

    #[test]
    fn symbol_name_and_reference_cover_every_symbol_like_variant() {
        assert_eq!(
            Value::uninterned_symbol("gensym").symbol_name(),
            Some("gensym")
        );
        assert_eq!(
            Value::Nil.symbol_reference(),
            Some(("NIL".to_string(), false))
        );
        assert_eq!(
            Value::Boolean(true).symbol_reference(),
            Some(("T".to_string(), false))
        );
        assert_eq!(
            Value::keyword("key").symbol_reference(),
            Some(("KEY".to_string(), false))
        );
        assert_eq!(
            Value::uninterned_symbol("gensym").symbol_reference(),
            Some(("gensym".to_string(), false))
        );
        let first_symbol = Value::uninterned_symbol("gensym");
        let second_symbol = Value::uninterned_symbol("gensym");
        let first_uninterned = first_symbol
            .variable_reference()
            .expect("uninterned symbol should have a variable reference");
        let second_uninterned = second_symbol
            .variable_reference()
            .expect("uninterned symbol should have a variable reference");
        assert!(!first_uninterned.1);
        assert!(!second_uninterned.1);
        assert!(first_uninterned.0.starts_with("__NCL_UNINTERNED__"));
        assert!(second_uninterned.0.starts_with("__NCL_UNINTERNED__"));
        assert_ne!(first_uninterned.0, second_uninterned.0);
        assert_eq!(
            Value::symbol_exact("Exact").symbol_reference(),
            Some(("Exact".to_string(), true))
        );
        assert_eq!(
            Value::keyword_exact("Exact").symbol_reference(),
            Some(("Exact".to_string(), true))
        );
    }
}
