use std::cell::RefCell;
use std::rc::Rc;

use super::Value;

struct Cons {
    car: Value,
    cdr: Value,
}

/// A cons identity with independently mutable CAR and CDR.
#[derive(Clone)]
pub struct SharedCons(Rc<RefCell<Cons>>);

impl SharedCons {
    pub(crate) fn new(car: Value, cdr: Value) -> Self {
        Self(Rc::new(RefCell::new(Cons { car, cdr })))
    }

    /// Clones the CAR, retaining any nested container identity.
    pub fn car(&self) -> Value {
        self.0.borrow().car.clone()
    }

    /// Clones the CDR, retaining the actual tail identity.
    pub fn cdr(&self) -> Value {
        self.0.borrow().cdr.clone()
    }

    /// Replaces the CAR without exposing a borrow to callbacks.
    pub fn set_car(&self, value: Value) {
        self.0.borrow_mut().car = value;
    }

    /// Replaces the CDR with an arbitrary object.
    pub fn set_cdr(&self, value: Value) {
        self.0.borrow_mut().cdr = value;
    }

    pub(crate) fn printed_with(&self, print: impl Fn(&Value) -> String) -> String {
        let mut current = Value::Cons(self.clone());
        let mut guards = Vec::new();
        let mut output = String::from("(");
        loop {
            match current {
                Value::Cons(cell) => {
                    let Some(guard) =
                        super::PrintGuard::enter(super::PrintKind::Cons, cell.identity())
                    else {
                        if guards.is_empty() {
                            return String::from("#<CIRCULAR>");
                        }
                        output.push_str(" . #<CIRCULAR>");
                        break;
                    };
                    if !guards.is_empty() {
                        output.push(' ');
                    }
                    let car = cell.car();
                    current = cell.cdr();
                    guards.push((cell, guard));
                    output.push_str(&print(&car));
                }
                Value::Nil | Value::Boolean(false) => break,
                tail => {
                    output.push_str(" . ");
                    output.push_str(&print(&tail));
                    break;
                }
            }
        }
        output.push(')');
        output
    }

    pub(crate) fn identity(&self) -> usize {
        // Address keys are valid only while the owning allocation stays alive.
        Rc::as_ptr(&self.0) as usize
    }

    pub(crate) fn ptr_eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl std::fmt::Debug for SharedCons {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.printed_with(ToString::to_string))
    }
}
