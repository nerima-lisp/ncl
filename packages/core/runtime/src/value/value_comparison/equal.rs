use std::rc::Rc;

use crate::value::Value;

impl Value {
    /// Performs recursive Lisp `EQUAL` comparison.
    #[must_use]
    pub fn equal_value(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::String(left), Self::String(right)) => left == right,
            (Self::List(left), Self::List(right)) => left.len() == right.len()
                && left.iter().zip(right.iter()).all(|(left, right)| left.equal_value(right)),
            (Self::Vector(left), Self::Vector(right)) => {
                let left = left.borrow();
                let right = right.borrow();
                left.len() == right.len() && left.iter().zip(right.iter()).all(|(left, right)| left.equal_value(right))
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
                    && left_elements.borrow().len() == right_elements.borrow().len()
                    && left_elements.borrow()
                        .iter()
                        .zip(right_elements.borrow().iter())
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
