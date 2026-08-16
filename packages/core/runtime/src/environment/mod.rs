mod bindings;
mod control;
mod definitions;
mod documentation;
mod frame;
mod functions;

use std::cell::RefCell;
use std::rc::Rc;

use frame::Frame;

#[derive(Clone)]
pub struct Environment(Rc<RefCell<Frame>>);

impl Environment {
    pub fn new() -> Self {
        Self(Rc::new(RefCell::new(Frame::new(None))))
    }

    pub fn child(&self) -> Self {
        Self(Rc::new(RefCell::new(Frame::new(Some(self.clone())))))
    }

    pub(crate) fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Default for Environment {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) fn normalize_name(name: &str) -> String {
    name.to_ascii_uppercase()
}
