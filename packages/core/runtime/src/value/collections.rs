impl Value {
    pub fn list_items(&self) -> Option<Vec<Value>> {
        match self {
            Self::Nil => Some(Vec::new()),
            Self::List(items) => Some(items.as_ref().clone()),
            Self::Structure {
                representation: StructureRepresentation::List { .. },
                ..
            } => self.structure_sequence_items(),
            _ => None,
        }
    }

    pub fn vector_items(&self) -> Option<Vec<Value>> {
        match self {
            Self::Vector {
                elements,
                length,
                displaced_index_offset,
                ..
            } => {
                let elements = elements.borrow();
                let end = displaced_index_offset.checked_add(*length)?;
                Some(elements[*displaced_index_offset..end].to_vec())
            }
            Self::Structure {
                representation: StructureRepresentation::Vector { .. },
                ..
            } => self.structure_sequence_items(),
            _ => None,
        }
    }

    pub fn vector_length(&self) -> Option<usize> {
        match self {
            Self::Vector { length, .. } => Some(*length),
            _ => None,
        }
    }

    pub fn vector_fill_pointer(&self) -> Option<usize> {
        match self {
            Self::Vector { fill_pointer, .. } => *fill_pointer,
            _ => None,
        }
    }

    pub fn array_element_type_value(&self) -> Option<Self> {
        match self {
            Self::Vector { element_type, .. } | Self::Array { element_type, .. } => {
                Some(element_type.as_ref().clone())
            }
            _ => None,
        }
    }

    pub fn is_simple_vector(&self) -> bool {
        matches!(
            self,
            Self::Vector {
                fill_pointer: None,
                adjustable: false,
                displaced_to: None,
                ..
            } | Self::Structure {
                representation: StructureRepresentation::Vector { .. },
                ..
            }
        )
    }

    pub fn is_adjustable_array(&self) -> bool {
        match self {
            Self::Vector { adjustable, .. } | Self::Array { adjustable, .. } => *adjustable,
            _ => false,
        }
    }

    pub fn array_dimensions(&self) -> Option<Vec<usize>> {
        match self {
            Self::Array { dimensions, .. } => Some(dimensions.as_ref().clone()),
            _ => None,
        }
    }

    pub fn array_items(&self) -> Option<Vec<Value>> {
        match self {
            Self::Array {
                dimensions,
                elements,
                displaced_index_offset,
                ..
            } => {
                let total_size = dimensions.iter().copied().product::<usize>();
                let elements = elements.borrow();
                let end = displaced_index_offset.checked_add(total_size)?;
                Some(elements[*displaced_index_offset..end].to_vec())
            }
            _ => None,
        }
    }

    pub fn array_storage(&self) -> Option<Rc<RefCell<Vec<Value>>>> {
        match self {
            Self::Vector { elements, .. } | Self::Array { elements, .. } => Some(elements.clone()),
            _ => None,
        }
    }

    pub fn array_displacement_value(&self) -> Option<(Self, usize)> {
        match self {
            Self::Vector {
                displaced_to,
                displaced_index_offset,
                ..
            }
            | Self::Array {
                displaced_to,
                displaced_index_offset,
                ..
            } => displaced_to
                .as_ref()
                .map(|displaced_to| (displaced_to.as_ref().clone(), *displaced_index_offset)),
            _ => None,
        }
    }

    pub fn with_array_displacement(
        self,
        displaced_to: Option<Self>,
        displaced_index_offset: usize,
    ) -> Self {
        match self {
            Self::Vector {
                elements,
                length,
                fill_pointer,
                element_type,
                adjustable,
                ..
            } => Self::Vector {
                elements,
                length,
                fill_pointer,
                element_type,
                adjustable,
                displaced_to: displaced_to.map(Rc::new),
                displaced_index_offset,
            },
            Self::Array {
                dimensions,
                elements,
                element_type,
                adjustable,
                ..
            } => Self::Array {
                dimensions,
                elements,
                element_type,
                adjustable,
                displaced_to: displaced_to.map(Rc::new),
                displaced_index_offset,
            },
            value => value,
        }
    }

    pub fn with_array_displacement_value(self, displacement: Option<(Self, usize)>) -> Self {
        match displacement {
            Some((displaced_to, displaced_index_offset)) => {
                self.with_array_displacement(Some(displaced_to), displaced_index_offset)
            }
            None => self.with_array_displacement(None, 0),
        }
    }

    pub(crate) fn hash_table_test(&self) -> Option<&str> {
        match self {
            Self::HashTable { test, .. } => Some(test),
            _ => None,
        }
    }

    pub(crate) fn hash_table_entries(&self) -> Option<&RefCell<Vec<(Value, Value)>>> {
        match self {
            Self::HashTable { entries, .. } => Some(entries),
            _ => None,
        }
    }

}
