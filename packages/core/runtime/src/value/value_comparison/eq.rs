use std::rc::Rc;

use crate::value::{SymbolObject, Value};

fn interned_symbol_matches_legacy(interned: &SymbolObject, legacy: &Value) -> bool {
    match legacy {
        Value::Symbol(name) | Value::SymbolExact(name) => interned.reference() == name.as_ref(),
        Value::Keyword(name) | Value::KeywordExact(name) => {
            interned.keyword() && interned.name() == name.as_ref()
        }
        _ => false,
    }
}

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
            (Self::BigRational(left), Self::BigRational(right)) => left == right,
            (Self::Float(left), Self::Float(right)) => left == right,
            (Self::Complex(left), Self::Complex(right)) => Rc::ptr_eq(left, right),
            (Self::Character(left), Self::Character(right)) => left == right,
            (Self::Stream(left), Self::Stream(right)) => Rc::ptr_eq(left, right),
            (Self::Package(left), Self::Package(right))
            | (Self::Symbol(left), Self::Symbol(right))
            | (Self::Keyword(left), Self::Keyword(right))
            | (Self::SymbolExact(left), Self::SymbolExact(right))
            | (Self::KeywordExact(left), Self::KeywordExact(right)) => left == right,
            (Self::PackageObject(left), Self::PackageObject(right)) => left.ptr_eq(right),
            (Self::InternedSymbol(left), Self::InternedSymbol(right)) => left.ptr_eq(right),
            (Self::InternedSymbol(left), right) if interned_symbol_matches_legacy(left, right) => {
                true
            }
            (left, Self::InternedSymbol(right)) if interned_symbol_matches_legacy(right, left) => {
                true
            }
            (Self::String(left), Self::String(right)) => Rc::ptr_eq(left, right),
            (Self::UninternedSymbol(left), Self::UninternedSymbol(right)) => {
                Rc::ptr_eq(left, right)
            }
            (Self::Cons(left), Self::Cons(right)) => left.ptr_eq(right),
            (Self::Vector(left), Self::Vector(right)) => left.ptr_eq(right),
            (Self::Values(left), Self::Values(right)) => Rc::ptr_eq(left, right),
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
                    && left_elements.ptr_eq(right_elements)
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
            (Self::Function(left), Self::Function(right)) => Rc::ptr_eq(left, right),
            _ => false,
        }
    }
}
