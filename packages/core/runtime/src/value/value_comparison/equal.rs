use std::collections::HashSet;
use std::rc::Rc;

use crate::value::{SlotValues, Value};

fn queue_equal_slots(
    left: &SlotValues,
    right: &SlotValues,
    pending: &mut Vec<(Value, Value)>,
) -> bool {
    let left = left.borrow().clone();
    let right = right.borrow().clone();
    if left.len() != right.len() {
        return false;
    }
    for ((left_name, left_value), (right_name, right_value)) in left.into_iter().zip(right) {
        if left_name != right_name {
            return false;
        }
        pending.push((left_value, right_value));
    }
    true
}

impl Value {
    /// Performs recursive Lisp `EQUAL` comparison.
    #[must_use]
    pub fn equal_value(&self, other: &Self) -> bool {
        let mut pending = vec![(self.clone(), other.clone())];
        let mut visited = HashSet::new();
        while let Some((left, right)) = pending.pop() {
            match (&left, &right) {
                (Self::Cons(l), Self::Cons(r)) => {
                    if visited.insert((0, l.identity(), r.identity())) {
                        pending.push((l.cdr(), r.cdr()));
                        pending.push((l.car(), r.car()));
                    }
                }
                (Self::String(l), Self::String(r)) if l == r => {}
                (Self::Complex(l), Self::Complex(r)) => {
                    pending.push((l.real().clone(), r.real().clone()));
                    pending.push((l.imaginary().clone(), r.imaginary().clone()));
                }
                (Self::Values(l), Self::Values(r)) => {
                    if l.len() != r.len() {
                        return false;
                    }
                    pending.extend(l.iter().cloned().zip(r.iter().cloned()));
                }
                (Self::Condition(l), Self::Condition(r)) => {
                    if l.actual_type != r.actual_type
                        || l.type_names != r.type_names
                        || l.message != r.message
                        || l.format_control != r.format_control
                        || l.format_arguments.len() != r.format_arguments.len()
                    {
                        return false;
                    }
                    if visited.insert((3, Rc::as_ptr(l) as usize, Rc::as_ptr(r) as usize)) {
                        pending.extend(
                            l.format_arguments
                                .iter()
                                .cloned()
                                .zip(r.format_arguments.iter().cloned()),
                        );
                        if !queue_equal_slots(&l.slots, &r.slots, &mut pending) {
                            return false;
                        }
                    }
                }
                (Self::Class(l), Self::Class(r)) if l.name.eq_ignore_ascii_case(&r.name) => {}
                (
                    Self::Structure {
                        name: ln,
                        slots: ls,
                        ..
                    },
                    Self::Structure {
                        name: rn,
                        slots: rs,
                        ..
                    },
                ) => {
                    if ln != rn {
                        return false;
                    }
                    if visited.insert((1, Rc::as_ptr(ls) as usize, Rc::as_ptr(rs) as usize))
                        && !queue_equal_slots(ls, rs, &mut pending)
                    {
                        return false;
                    }
                }
                (Self::Instance(l), Self::Instance(r)) => {
                    let l_class = l.class.borrow();
                    let r_class = r.class.borrow();
                    if !l_class.name.eq_ignore_ascii_case(&r_class.name) {
                        return false;
                    }
                    if visited.insert((
                        2,
                        Rc::as_ptr(&l.slots) as usize,
                        Rc::as_ptr(&r.slots) as usize,
                    )) {
                        let ls = l.slots.borrow().clone();
                        let rs = r.slots.borrow().clone();
                        if ls.len() != rs.len() {
                            return false;
                        }
                        for ((ln, lv), (rn, rv)) in ls.into_iter().zip(rs) {
                            if !ln.eq_ignore_ascii_case(&rn) {
                                return false;
                            }
                            pending.push((lv, rv));
                        }
                    }
                }
                _ if left.eq_value(&right) => {}
                _ => return false,
            }
        }
        true
    }
}
