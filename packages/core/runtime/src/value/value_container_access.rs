use std::cell::RefCell;
use std::collections::HashSet;

use super::Value;

impl Value {
    pub(crate) fn array_storage(&self) -> Option<(std::rc::Rc<std::cell::RefCell<Vec<Self>>>, usize, usize)> {
        match self {
            Self::Vector(items) => {
                let metadata = items.metadata.borrow();
                let storage = metadata.displaced_to.as_ref().unwrap_or(&items.elements).clone();
                Some((storage, metadata.displaced_index_offset, items.borrow().len()))
            }
            Self::Array { elements, metadata, .. } => {
                let metadata = metadata.borrow();
                let storage = metadata.displaced_to.as_ref().unwrap_or(elements).clone();
                Some((storage, metadata.displaced_index_offset, elements.borrow().len()))
            }
            _ => None,
        }
    }

    /// Returns a copied proper-list payload when this value is a list.
    #[must_use]
    pub fn list_items(&self) -> Option<Vec<Self>> {
        let mut current = self.clone();
        let mut items = Vec::new();
        let mut visited = HashSet::new();
        loop {
            match current {
                Self::Nil => return Some(items),
                Self::List(values) => {
                    items.extend(values.as_ref().iter().cloned());
                    return Some(items);
                }
                Self::MutableCons(cell) => {
                    if !visited.insert(std::rc::Rc::as_ptr(&cell)) {
                        return None;
                    }
                    let (car, cdr) = cell.borrow().clone();
                    items.push(car);
                    current = cdr;
                }
                _ => return None,
            }
        }
    }

    /// Returns a copied vector payload when this value is a vector.
    #[must_use]
    pub fn vector_items(&self) -> Option<Vec<Self>> {
        match self {
            Self::Vector(items) => {
                let metadata = items.metadata.borrow();
                let storage = metadata.displaced_to.as_ref().unwrap_or(&items.elements);
                let start = metadata.displaced_index_offset;
                let len = items.borrow().len();
                Some(storage.borrow()[start..start + len].to_vec())
            }
            _ => None,
        }
    }

    pub(crate) fn string_contents(&self) -> Option<String> {
        match self {
            Self::String(value) => Some(value.to_string()),
            Self::MutableString(value) => Some(value.borrow().clone()),
            _ => None,
        }
    }

    pub(crate) fn vector_sequence_items(&self) -> Option<Vec<Self>> {
        let Self::Vector(items) = self else { return None };
        let end = self.vector_length().unwrap_or_else(|| items.borrow().len());
        self.vector_items().map(|values| values[..end].to_vec())
    }

    pub(crate) fn is_displaced(&self) -> bool {
        match self {
            Self::Vector(items) => items.metadata.borrow().displaced_to.is_some(),
            Self::Array { metadata, .. } => metadata.borrow().displaced_to.is_some(),
            _ => false,
        }
    }

    pub(crate) fn array_displacement(&self) -> Option<Option<(Self, usize)>> {
        match self {
            Self::Vector(items) => {
                let metadata = items.metadata.borrow();
                Some(metadata.displaced_to_value.clone().map(|array| (array, metadata.displaced_index_offset)))
            }
            Self::Array { metadata, .. } => {
                let metadata = metadata.borrow();
                Some(metadata.displaced_to_value.clone().map(|array| (array, metadata.displaced_index_offset)))
            }
            Self::String(_) | Self::MutableString(_) => Some(None),
            _ => None,
        }
    }

    pub(crate) fn vector_fill_pointer(&self) -> Option<Option<usize>> {
        match self {
            Self::Vector(items) => Some(items.metadata.borrow().fill_pointer),
            _ => None,
        }
    }

    pub(crate) fn array_has_fill_pointer(&self) -> Option<bool> {
        match self {
            Self::Vector(items) => Some(items.metadata.borrow().fill_pointer.is_some()),
            Self::Array { metadata, .. } => Some(metadata.borrow().fill_pointer.is_some()),
            Self::String(_) | Self::MutableString(_) => Some(false),
            _ => None,
        }
    }

    pub(crate) fn vector_length(&self) -> Option<usize> {
        match self {
            Self::Vector(items) => Some(self.vector_fill_pointer().flatten().unwrap_or_else(|| items.borrow().len())),
            _ => None,
        }
    }

    pub(crate) fn set_vector_fill_pointer(&self, fill_pointer: Option<usize>) -> Option<()> {
        match self {
            Self::Vector(items) => {
                items.metadata.borrow_mut().fill_pointer = fill_pointer;
                Some(())
            }
            _ => None,
        }
    }

    pub(crate) fn vector_adjustable(&self) -> Option<bool> {
        match self {
            Self::Vector(items) => Some(items.metadata.borrow().adjustable),
            _ => None,
        }
    }

    pub(crate) fn set_vector_adjustable(&self, adjustable: bool) -> Option<()> {
        match self {
            Self::Vector(items) => {
                items.metadata.borrow_mut().adjustable = adjustable;
                Some(())
            }
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
            Self::Array { elements, metadata, .. } => {
                let metadata = metadata.borrow();
                let storage = metadata.displaced_to.as_ref().unwrap_or(elements);
                let start = metadata.displaced_index_offset;
                let len = elements.borrow().len();
                Some(storage.borrow()[start..start + len].to_vec())
            }
            _ => None,
        }
    }

    pub(crate) fn array_adjustable(&self) -> Option<bool> {
        match self {
            Self::Vector(items) => Some(items.metadata.borrow().adjustable),
            Self::Array { metadata, .. } => Some(metadata.borrow().adjustable),
            Self::String(_) | Self::MutableString(_) => Some(false),
            _ => None,
        }
    }

    pub(crate) fn array_element_type(&self) -> Option<Self> {
        match self {
            Self::Vector(items) => Some(items.metadata.borrow().element_type.clone()),
            Self::Array { metadata, .. } => Some(metadata.borrow().element_type.clone()),
            Self::String(_) | Self::MutableString(_) => Some(Self::symbol("CHARACTER")),
            _ => None,
        }
    }

    pub(crate) fn array_element_type_accepts(&self, value: &Self) -> Result<bool, crate::error::RuntimeError> {
        let Some(element_type) = self.array_element_type() else {
            return Ok(true);
        };
        crate::builtins::typep_value(value, &element_type)
    }

    pub(crate) fn set_array_element_type(&self, element_type: Self) -> Option<()> {
        match self {
            Self::Vector(items) => {
                items.metadata.borrow_mut().element_type = element_type;
                Some(())
            }
            Self::Array { metadata, .. } => {
                metadata.borrow_mut().element_type = element_type;
                Some(())
            }
            _ => None,
        }
    }

    pub(crate) fn set_array_adjustable(&self, adjustable: bool) -> Option<()> {
        match self {
            Self::Array { metadata, .. } => {
                metadata.borrow_mut().adjustable = adjustable;
                Some(())
            }
            _ => None,
        }
    }

    pub(crate) fn set_vector_item(&self, index: usize, value: Self) -> Option<()> {
        match self {
            Self::Vector(items) => {
                let metadata = items.metadata.borrow();
                let storage = metadata.displaced_to.as_ref().unwrap_or(&items.elements);
                storage.borrow_mut().get_mut(metadata.displaced_index_offset + index).map(|slot| *slot = value)
            }
            Self::MutableString(items) => {
                let Self::Character(character) = value else { return None };
                let mut text = items.borrow_mut();
                let mut characters: Vec<char> = text.chars().collect();
                let slot = characters.get_mut(index)?;
                *slot = character;
                *text = characters.into_iter().collect();
                Some(())
            }
            _ => None,
        }
    }

    pub(crate) fn set_array_item(&self, index: usize, value: Self) -> Option<()> {
        match self {
            Self::Array { elements, metadata, .. } => {
                let metadata = metadata.borrow();
                let storage = metadata.displaced_to.as_ref().unwrap_or(elements);
                storage.borrow_mut().get_mut(metadata.displaced_index_offset + index).map(|slot| *slot = value)
            }
            _ => None,
        }
    }

    /// Returns copied list or vector elements when this value is a sequence.
    #[must_use]
    pub fn sequence_items(&self) -> Option<Vec<Self>> {
        self.list_items()
            .or_else(|| self.vector_sequence_items())
            .or_else(|| self.string_contents().map(|text| text.chars().map(Self::Character).collect()))
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

    pub(crate) fn hash_table_rehash_size(&self) -> Option<f64> {
        match self {
            Self::HashTable { rehash_size, .. } => Some(*rehash_size),
            _ => None,
        }
    }

    pub(crate) fn hash_table_rehash_threshold(&self) -> Option<f64> {
        match self {
            Self::HashTable { rehash_threshold, .. } => Some(*rehash_threshold),
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

    #[test]
    fn cyclic_mutable_cons_is_not_a_proper_list() {
        let cell = Value::cons_cell(Value::Integer(1), Value::Nil);
        let Value::MutableCons(storage) = &cell else {
            panic!("expected mutable cons");
        };
        storage.borrow_mut().1 = cell.clone();
        assert!(cell.list_items().is_none());
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
        assert_eq!(Value::Nil.symbol_reference(), Some(("NIL", false)));
        assert_eq!(Value::Boolean(true).symbol_reference(), Some(("T", false)));
        assert_eq!(
            Value::keyword("key").symbol_reference(),
            Some(("KEY", false))
        );
        assert_eq!(
            Value::uninterned_symbol("gensym").symbol_reference(),
            Some(("gensym", false))
        );
        assert_eq!(
            Value::symbol_exact("Exact").symbol_reference(),
            Some(("Exact", true))
        );
        assert_eq!(
            Value::keyword_exact("Exact").symbol_reference(),
            Some(("Exact", true))
        );
    }
}
