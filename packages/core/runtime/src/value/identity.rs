impl Value {
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

    pub fn eq_value(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Nil, Self::Nil) => true,
            (Self::Unbound, Self::Unbound) => true,
            (Self::Nil, Self::Boolean(false)) | (Self::Boolean(false), Self::Nil) => true,
            (Self::Boolean(left), Self::Boolean(right)) => left == right,
            (Self::Integer(left), Self::Integer(right)) => left == right,
            (Self::Rational(left), Self::Rational(right)) => left == right,
            (Self::Float(left), Self::Float(right)) => left == right,
            (
                Self::Complex {
                    real: left_real,
                    imag: left_imag,
                },
                Self::Complex {
                    real: right_real,
                    imag: right_imag,
                },
            ) => left_real.eq_value(right_real) && left_imag.eq_value(right_imag),
            (Self::Character(left), Self::Character(right)) => left == right,
            (Self::Stream(left), Self::Stream(right)) => Rc::ptr_eq(left, right),
            (Self::Package(left), Self::Package(right)) => left == right,
            (Self::String(left), Self::String(right)) => Rc::ptr_eq(left, right),
            (Self::Symbol(left), Self::Symbol(right))
            | (Self::Keyword(left), Self::Keyword(right)) => left == right,
            (Self::SymbolExact(left), Self::SymbolExact(right))
            | (Self::KeywordExact(left), Self::KeywordExact(right)) => left == right,
            (Self::UninternedSymbol(left), Self::UninternedSymbol(right)) => {
                Rc::ptr_eq(left, right)
            }
            (Self::List(left), Self::List(right)) => Rc::ptr_eq(left, right),
            (
                Self::Vector {
                    elements: left_elements,
                    length: left_length,
                    fill_pointer: left_fill_pointer,
                    element_type: left_element_type,
                    adjustable: left_adjustable,
                    displaced_to: left_displaced_to,
                    displaced_index_offset: left_displaced_index_offset,
                },
                Self::Vector {
                    elements: right_elements,
                    length: right_length,
                    fill_pointer: right_fill_pointer,
                    element_type: right_element_type,
                    adjustable: right_adjustable,
                    displaced_to: right_displaced_to,
                    displaced_index_offset: right_displaced_index_offset,
                },
            ) => {
                Rc::ptr_eq(left_elements, right_elements)
                    && left_length == right_length
                    && left_fill_pointer == right_fill_pointer
                    && Rc::ptr_eq(left_element_type, right_element_type)
                    && left_adjustable == right_adjustable
                    && left_displaced_index_offset == right_displaced_index_offset
                    && match (left_displaced_to, right_displaced_to) {
                        (Some(left), Some(right)) => Rc::ptr_eq(left, right),
                        (None, None) => true,
                        _ => false,
                    }
            }
            (
                Self::Array {
                    dimensions: left_dimensions,
                    elements: left_elements,
                    element_type: left_element_type,
                    adjustable: left_adjustable,
                    displaced_to: left_displaced_to,
                    displaced_index_offset: left_displaced_index_offset,
                },
                Self::Array {
                    dimensions: right_dimensions,
                    elements: right_elements,
                    element_type: right_element_type,
                    adjustable: right_adjustable,
                    displaced_to: right_displaced_to,
                    displaced_index_offset: right_displaced_index_offset,
                },
            ) => {
                Rc::ptr_eq(left_dimensions, right_dimensions)
                    && Rc::ptr_eq(left_elements, right_elements)
                    && Rc::ptr_eq(left_element_type, right_element_type)
                    && left_adjustable == right_adjustable
                    && left_displaced_index_offset == right_displaced_index_offset
                    && match (left_displaced_to, right_displaced_to) {
                        (Some(left), Some(right)) => Rc::ptr_eq(left, right),
                        (None, None) => true,
                        _ => false,
                    }
            }
            (
                Self::HashTable {
                    entries: left_entries,
                    ..
                },
                Self::HashTable {
                    entries: right_entries,
                    ..
                },
            ) => Rc::ptr_eq(left_entries, right_entries),
            (Self::Values(left), Self::Values(right)) => Rc::ptr_eq(left, right),
            (Self::Condition(left), Self::Condition(right)) => Rc::ptr_eq(left, right),
            (Self::Restart(left), Self::Restart(right)) => Rc::ptr_eq(left, right),
            (
                Self::Structure {
                    name: left_name,
                    slots: left_slots,
                    ..
                },
                Self::Structure {
                    name: right_name,
                    slots: right_slots,
                    ..
                },
            ) => Rc::ptr_eq(left_name, right_name) && Rc::ptr_eq(left_slots, right_slots),
            (Self::Class(left), Self::Class(right)) => Rc::ptr_eq(left, right),
            (Self::Environment(left), Self::Environment(right)) => left.same(right),
            (Self::Instance(left), Self::Instance(right)) => {
                Rc::ptr_eq(&left.class, &right.class) && Rc::ptr_eq(&left.slots, &right.slots)
            }
            (Self::Method(left), Self::Method(right)) => left.id == right.id,
            (
                Self::DottedList {
                    items: left,
                    tail: left_tail,
                },
                Self::DottedList {
                    items: right,
                    tail: right_tail,
                },
            ) => Rc::ptr_eq(left, right) && Rc::ptr_eq(left_tail, right_tail),
            (Self::Function(left), Self::Function(right)) => Rc::ptr_eq(left, right),
            _ => false,
        }
    }

    pub fn equal_value(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Complex {
                    real: left_real,
                    imag: left_imag,
                },
                Self::Complex {
                    real: right_real,
                    imag: right_imag,
                },
            ) => left_real.equal_value(right_real) && left_imag.equal_value(right_imag),
            (Self::String(left), Self::String(right)) => left == right,
            (Self::List(left), Self::List(right)) => {
                left.len() == right.len()
                    && left
                        .iter()
                        .zip(right.iter())
                        .all(|(left, right)| left.equal_value(right))
            }
            (
                Self::Vector {
                    elements: left_elements,
                    length: left_length,
                    fill_pointer: left_fill_pointer,
                    element_type: left_element_type,
                    adjustable: left_adjustable,
                    displaced_to: left_displaced_to,
                    displaced_index_offset: left_displaced_index_offset,
                },
                Self::Vector {
                    elements: right_elements,
                    length: right_length,
                    fill_pointer: right_fill_pointer,
                    element_type: right_element_type,
                    adjustable: right_adjustable,
                    displaced_to: right_displaced_to,
                    displaced_index_offset: right_displaced_index_offset,
                },
            ) => {
                let left_items = {
                    let end = left_displaced_index_offset + left_length;
                    left_elements.borrow()[*left_displaced_index_offset..end].to_vec()
                };
                let right_items = {
                    let end = right_displaced_index_offset + right_length;
                    right_elements.borrow()[*right_displaced_index_offset..end].to_vec()
                };
                left_fill_pointer == right_fill_pointer
                    && left_length == right_length
                    && left_adjustable == right_adjustable
                    && left_displaced_index_offset == right_displaced_index_offset
                    && left_element_type.equal_value(right_element_type)
                    && match (left_displaced_to, right_displaced_to) {
                        (Some(left), Some(right)) => left.equal_value(right),
                        (None, None) => true,
                        _ => false,
                    }
                    && left_items.len() == right_items.len()
                    && left_items
                        .iter()
                        .zip(right_items.iter())
                        .all(|(left, right)| left.equal_value(right))
            }
            (
                Self::Array {
                    dimensions: left_dimensions,
                    element_type: left_element_type,
                    adjustable: left_adjustable,
                    displaced_to: left_displaced_to,
                    displaced_index_offset: left_displaced_index_offset,
                    ..
                },
                Self::Array {
                    dimensions: right_dimensions,
                    element_type: right_element_type,
                    adjustable: right_adjustable,
                    displaced_to: right_displaced_to,
                    displaced_index_offset: right_displaced_index_offset,
                    ..
                },
            ) => {
                let Some(left_items) = self.array_items() else {
                    return false;
                };
                let Some(right_items) = other.array_items() else {
                    return false;
                };
                left_dimensions == right_dimensions
                    && left_adjustable == right_adjustable
                    && left_displaced_index_offset == right_displaced_index_offset
                    && left_element_type.equal_value(right_element_type)
                    && match (left_displaced_to, right_displaced_to) {
                        (Some(left), Some(right)) => left.equal_value(right),
                        (None, None) => true,
                        _ => false,
                    }
                    && left_items.len() == right_items.len()
                    && left_items
                        .iter()
                        .zip(right_items.iter())
                        .all(|(left, right)| left.equal_value(right))
            }
            (Self::Values(left), Self::Values(right)) => {
                left.len() == right.len()
                    && left
                        .iter()
                        .zip(right.iter())
                        .all(|(left, right)| left.equal_value(right))
            }
            (Self::Condition(left), Self::Condition(right)) => left.equal_value(right),
            (Self::Restart(left), Self::Restart(right)) => Rc::ptr_eq(left, right),
            (
                Self::Structure {
                    name: left_name,
                    slots: left_slots,
                    ..
                },
                Self::Structure {
                    name: right_name,
                    slots: right_slots,
                    ..
                },
            ) => {
                if left_name != right_name {
                    return false;
                }
                let left_slots = left_slots.borrow();
                let right_slots = right_slots.borrow();
                left_slots.len() == right_slots.len()
                    && left_slots.iter().zip(right_slots.iter()).all(
                        |((left_name, left_value), (right_name, right_value))| {
                            left_name == right_name && left_value.equal_value(right_value)
                        },
                    )
            }
            (Self::Class(left), Self::Class(right)) => left.name.eq_ignore_ascii_case(&right.name),
            (Self::Instance(left), Self::Instance(right)) => {
                let left_class = left.class.borrow();
                let right_class = right.class.borrow();
                if !left_class.name.eq_ignore_ascii_case(&right_class.name) {
                    return false;
                }
                let left_slots = left.slots.borrow();
                let right_slots = right.slots.borrow();
                left_slots.len() == right_slots.len()
                    && left_slots.iter().zip(right_slots.iter()).all(
                        |((left_name, left_value), (right_name, right_value))| {
                            left_name.eq_ignore_ascii_case(right_name)
                                && left_value.equal_value(right_value)
                        },
                    )
            }
            (
                Self::DottedList {
                    items: left,
                    tail: left_tail,
                },
                Self::DottedList {
                    items: right,
                    tail: right_tail,
                },
            ) => {
                left.len() == right.len()
                    && left
                        .iter()
                        .zip(right.iter())
                        .all(|(left, right)| left.equal_value(right))
                    && left_tail.equal_value(right_tail)
            }
            _ => self.eq_value(other),
        }
    }
}
