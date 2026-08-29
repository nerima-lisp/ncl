use std::rc::Rc;

use super::Value;

impl Value {
    /// Performs Lisp `EQ` identity/equivalence comparison.
    #[must_use]
    pub fn eq_value(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Nil | Self::Boolean(false), Self::Nil)
            | (Self::Nil, Self::Boolean(false))
            | (Self::Unbound, Self::Unbound) => true,
            (Self::Boolean(left), Self::Boolean(right)) => left == right,
            (Self::Integer(left), Self::Integer(right)) => left == right,
            (Self::Rational(left), Self::Rational(right)) => left == right,
            (Self::Float(left), Self::Float(right)) => left == right,
            (Self::Character(left), Self::Character(right)) => left == right,
            (Self::Stream(left), Self::Stream(right)) => Rc::ptr_eq(left, right),
            (Self::Package(left), Self::Package(right))
            | (Self::Symbol(left), Self::Symbol(right))
            | (Self::Keyword(left), Self::Keyword(right))
            | (Self::SymbolExact(left), Self::SymbolExact(right))
            | (Self::KeywordExact(left), Self::KeywordExact(right)) => left == right,
            (Self::String(left), Self::String(right)) => Rc::ptr_eq(left, right),
            (Self::UninternedSymbol(left), Self::UninternedSymbol(right)) => {
                Rc::ptr_eq(left, right)
            }
            (Self::List(left), Self::List(right)) | (Self::Vector(left), Self::Vector(right)) => {
                Rc::ptr_eq(left, right)
            }
            (
                Self::Array {
                    dimensions: left_dimensions,
                    elements: left_elements,
                },
                Self::Array {
                    dimensions: right_dimensions,
                    elements: right_elements,
                },
            ) => {
                Rc::ptr_eq(left_dimensions, right_dimensions)
                    && Rc::ptr_eq(left_elements, right_elements)
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

    /// Performs recursive Lisp `EQUAL` comparison.
    #[must_use]
    pub fn equal_value(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::String(left), Self::String(right)) => left == right,
            (Self::List(left), Self::List(right)) | (Self::Vector(left), Self::Vector(right)) => {
                left.len() == right.len()
                    && left
                        .iter()
                        .zip(right.iter())
                        .all(|(left, right)| left.equal_value(right))
            }
            (
                Self::Array {
                    dimensions: left_dimensions,
                    elements: left_elements,
                },
                Self::Array {
                    dimensions: right_dimensions,
                    elements: right_elements,
                },
            ) => {
                left_dimensions == right_dimensions
                    && left_elements.len() == right_elements.len()
                    && left_elements
                        .iter()
                        .zip(right_elements.iter())
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
                if !left.class.name.eq_ignore_ascii_case(&right.class.name) {
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

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use crate::environment::Environment;

    use super::super::{ClassDefinition, Value};

    #[test]
    fn eq_and_equal_cover_scalar_and_shared_container_semantics() {
        let rational = match Value::rational(3, 2) {
            Ok(value) => value,
            Err(error) => panic!("unexpected rational construction error: {error}"),
        };
        let scalar_cases = [
            (Value::Nil, Value::Boolean(false), true),
            (Value::Boolean(true), Value::Boolean(true), true),
            (Value::Integer(1), Value::Integer(1), true),
            (rational.clone(), rational, true),
            (Value::Float(1.5), Value::Float(1.5), true),
            (Value::Character('x'), Value::Character('x'), true),
            (Value::symbol("name"), Value::symbol("name"), true),
            (Value::keyword("name"), Value::keyword("name"), true),
            (
                Value::symbol_exact("name"),
                Value::symbol_exact("name"),
                true,
            ),
            (
                Value::keyword_exact("name"),
                Value::keyword_exact("name"),
                true,
            ),
            (Value::package("CL"), Value::package("CL"), true),
            (Value::Integer(1), Value::Integer(2), false),
            (Value::Integer(1), Value::Boolean(true), false),
        ];

        for (left, right, expected) in scalar_cases {
            assert_eq!(left.eq_value(&right), expected);
            assert_eq!(left.equal_value(&right), expected);
        }

        let list = Value::list(vec![Value::Integer(1)]);
        let same_list = list.clone();
        let equivalent_list = Value::list(vec![Value::Integer(1)]);
        assert!(list.eq_value(&same_list));
        assert!(!list.eq_value(&equivalent_list));
        assert!(list.equal_value(&equivalent_list));

        let vector = Value::vector(vec![Value::Integer(1)]);
        assert!(vector.equal_value(&Value::vector(vec![Value::Integer(1)])));
        let array = Value::array(vec![1], vec![Value::Integer(1)]);
        assert!(array.eq_value(&array));
        assert!(!array.eq_value(&Value::array(vec![1], vec![Value::Integer(1)])));
        assert!(array.equal_value(&Value::array(vec![1], vec![Value::Integer(1)])));
        let values = Value::values(vec![Value::Integer(1)]);
        assert!(values.eq_value(&values));
        assert!(values.equal_value(&Value::values(vec![Value::Integer(1)])));

        let hash_table = Value::hash_table("eq");
        assert!(hash_table.eq_value(&hash_table));
        assert!(!hash_table.eq_value(&Value::hash_table("eq")));

        let dotted = Value::dotted_list(vec![Value::Integer(1)], Value::Nil);
        assert!(dotted.equal_value(&Value::dotted_list(vec![Value::Integer(1)], Value::Nil)));
        assert!(!dotted.equal_value(&Value::dotted_list(
            vec![Value::Integer(1)],
            Value::Integer(2)
        )));
    }

    #[test]
    fn comparisons_cover_identity_sensitive_and_fallback_pairs() {
        let string = Value::string("text");
        assert!(string.eq_value(&string));
        assert!(!string.eq_value(&Value::string("text")));
        assert!(string.equal_value(&Value::string("text")));

        let uninterned = Value::uninterned_symbol("name");
        assert!(uninterned.eq_value(&uninterned));
        assert!(!uninterned.eq_value(&Value::uninterned_symbol("name")));

        let values = [
            Value::Nil,
            Value::Boolean(true),
            Value::Integer(1),
            Value::Float(1.0),
            Value::Character('x'),
            Value::string("x"),
            Value::list(vec![Value::Integer(2)]),
        ];
        for (index, left) in values.iter().enumerate() {
            for (other_index, right) in values.iter().enumerate() {
                if index != other_index {
                    assert!(!left.eq_value(right));
                }
            }
        }
    }

    #[test]
    fn comparisons_cover_runtime_identity_and_structural_variants() {
        let condition = Value::condition_from_parts_with_types(
            "ERROR".to_owned(),
            vec!["ERROR".to_owned()],
            vec![("DETAIL".to_owned(), Value::Integer(1))],
            "failed".to_owned(),
            None,
            Vec::new(),
        );
        let equivalent_condition = Value::condition_from_parts_with_types(
            "ERROR".to_owned(),
            vec!["ERROR".to_owned()],
            vec![("DETAIL".to_owned(), Value::Integer(1))],
            "failed".to_owned(),
            None,
            Vec::new(),
        );
        let structure = Value::structure_with_types(
            "POINT",
            vec![("X".to_owned(), Value::Integer(1))],
            Vec::new(),
        );
        let equivalent_structure = Value::structure_with_types(
            "POINT",
            vec![("X".to_owned(), Value::Integer(1))],
            Vec::new(),
        );
        let class = Rc::new(ClassDefinition {
            name: "POINT".to_owned(),
            precedence: vec!["POINT".to_owned()],
            slots: Vec::new(),
            default_initargs: Vec::new(),
        });
        let same_class = Value::class_object(Rc::clone(&class));
        let equivalent_class = Value::class_object(Rc::new(ClassDefinition {
            name: "point".to_owned(),
            precedence: vec!["point".to_owned()],
            slots: Vec::new(),
            default_initargs: Vec::new(),
        }));
        let instance =
            Value::instance(Rc::clone(&class), vec![("X".to_owned(), Value::Integer(1))]);
        let equivalent_instance =
            Value::instance(Rc::clone(&class), vec![("x".to_owned(), Value::Integer(1))]);
        let environment = Value::environment(Environment::new());
        let equivalent_environment = Value::environment(Environment::new());
        let cases = [
            (condition.clone(), condition.clone(), true, true),
            (condition.clone(), equivalent_condition.clone(), false, true),
            (structure.clone(), structure.clone(), true, true),
            (structure.clone(), equivalent_structure.clone(), false, true),
            (same_class.clone(), same_class.clone(), true, true),
            (same_class, equivalent_class, false, true),
            (instance.clone(), instance.clone(), true, true),
            (instance.clone(), equivalent_instance.clone(), false, true),
            (environment.clone(), environment.clone(), true, true),
            (environment, equivalent_environment, false, false),
            (
                Value::restart("retry"),
                Value::restart("retry"),
                false,
                false,
            ),
            (
                Value::builtin("A", |_| Ok(Value::Nil)),
                Value::builtin("A", |_| Ok(Value::Nil)),
                false,
                false,
            ),
        ];

        for (index, (left, right, eq_expected, equal_expected)) in cases.into_iter().enumerate() {
            assert_eq!(left.eq_value(&right), eq_expected, "case {index}");
            assert_eq!(left.equal_value(&right), equal_expected, "case {index}");
        }
        assert!(condition.equal_value(&equivalent_condition));
        assert!(structure.equal_value(&equivalent_structure));
        assert!(instance.equal_value(&equivalent_instance));
    }
}
