use std::cell::RefCell;
use std::rc::Rc;

use super::Value;

#[derive(Clone)]
pub struct SharedElements {
    data: Rc<RefCell<Vec<Value>>>,
    descriptor: Rc<RefCell<Descriptor>>,
}

#[derive(Clone)]
struct Descriptor {
    offset: usize,
    length: usize,
    fill_pointer: Option<usize>,
    adjustable: bool,
    element_type: Value,
    displaced_from: Option<Rc<Value>>,
    displaced_index_offset: usize,
}

impl std::fmt::Debug for SharedElements {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&Value::Vector(self.clone()), formatter)
    }
}

impl SharedElements {
    pub(crate) fn new(values: Vec<Value>) -> Self {
        Self::new_with_options(values, None, false, Value::symbol("T"))
    }

    pub(crate) fn new_with_options(
        values: Vec<Value>,
        fill_pointer: Option<usize>,
        adjustable: bool,
        element_type: Value,
    ) -> Self {
        let length = values.len();
        let fill_pointer = fill_pointer.filter(|pointer| *pointer <= length);
        Self {
            data: Rc::new(RefCell::new(values)),
            descriptor: Rc::new(RefCell::new(Descriptor {
                offset: 0,
                length,
                fill_pointer,
                adjustable,
                element_type,
                displaced_from: None,
                displaced_index_offset: 0,
            })),
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.descriptor.borrow().length
    }

    #[must_use]
    pub fn sequence_len(&self) -> usize {
        let descriptor = self.descriptor.borrow();
        descriptor.fill_pointer.unwrap_or(descriptor.length)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sequence_len() == 0
    }

    #[must_use]
    pub fn has_fill_pointer(&self) -> bool {
        self.descriptor.borrow().fill_pointer.is_some()
    }

    #[must_use]
    pub fn fill_pointer(&self) -> Option<usize> {
        self.descriptor.borrow().fill_pointer
    }

    #[must_use]
    pub fn is_adjustable(&self) -> bool {
        self.descriptor.borrow().adjustable
    }

    #[must_use]
    pub fn is_displaced(&self) -> bool {
        self.descriptor.borrow().displaced_from.is_some()
    }

    #[must_use]
    pub(crate) fn displacement(&self) -> Option<(Value, usize)> {
        let descriptor = self.descriptor.borrow();
        descriptor
            .displaced_from
            .as_ref()
            .map(|source| ((**source).clone(), descriptor.displaced_index_offset))
    }

    #[must_use]
    pub fn element_type(&self) -> Value {
        self.descriptor.borrow().element_type.clone()
    }

    #[must_use]
    pub fn get(&self, index: usize) -> Option<Value> {
        let descriptor = self.descriptor.borrow();
        if index >= descriptor.length {
            return None;
        }
        self.data.borrow().get(descriptor.offset + index).cloned()
    }

    #[must_use]
    pub fn snapshot(&self) -> Vec<Value> {
        let descriptor = self.descriptor.borrow();
        let data = self.data.borrow();
        data[descriptor.offset..descriptor.offset + descriptor.length].to_vec()
    }

    #[must_use]
    pub fn visible_snapshot(&self) -> Vec<Value> {
        let descriptor = self.descriptor.borrow();
        let data = self.data.borrow();
        let length = descriptor.fill_pointer.unwrap_or(descriptor.length);
        data[descriptor.offset..descriptor.offset + length].to_vec()
    }

    pub(crate) fn set(&self, index: usize, value: Value) -> bool {
        let descriptor = self.descriptor.borrow();
        if index >= descriptor.length {
            return false;
        }
        let mut elements = self.data.borrow_mut();
        let Some(slot) = elements.get_mut(descriptor.offset + index) else {
            return false;
        };
        *slot = value;
        true
    }

    pub(crate) fn replace_range(&self, start: usize, values: &[Value]) -> bool {
        let Some(end) = start.checked_add(values.len()) else {
            return false;
        };
        let descriptor = self.descriptor.borrow();
        if end > descriptor.length {
            return false;
        }
        let mut elements = self.data.borrow_mut();
        let Some(destination) =
            elements.get_mut(descriptor.offset + start..descriptor.offset + end)
        else {
            return false;
        };
        destination.clone_from_slice(values);
        true
    }

    pub(crate) fn set_fill_pointer(&self, value: usize) -> bool {
        let mut descriptor = self.descriptor.borrow_mut();
        if descriptor.fill_pointer.is_none() || value > descriptor.length {
            return false;
        }
        descriptor.fill_pointer = Some(value);
        true
    }

    pub(crate) fn push_value(&self, value: Value) -> Option<usize> {
        let mut descriptor = self.descriptor.borrow_mut();
        let pointer = descriptor.fill_pointer?;
        if pointer >= descriptor.length {
            return None;
        }
        let index = pointer;
        let mut data = self.data.borrow_mut();
        data[descriptor.offset + index] = value;
        descriptor.fill_pointer = Some(pointer + 1);
        Some(index)
    }

    pub(crate) fn pop_value(&self) -> Option<Value> {
        let mut descriptor = self.descriptor.borrow_mut();
        let pointer = descriptor.fill_pointer?;
        if pointer == 0 {
            return None;
        }
        let index = pointer - 1;
        descriptor.fill_pointer = Some(index);
        self.data.borrow().get(descriptor.offset + index).cloned()
    }

    pub(crate) fn grow(&self, extension: usize) -> bool {
        let mut descriptor = self.descriptor.borrow_mut();
        if !descriptor.adjustable
            || descriptor.offset != 0
            || descriptor.length != self.data.borrow().len()
        {
            return false;
        }
        let Some(new_length) = descriptor.length.checked_add(extension) else {
            return false;
        };
        self.data.borrow_mut().resize(new_length, Value::Nil);
        descriptor.length = new_length;
        true
    }

    pub(crate) fn displaced_view(
        &self,
        source: Value,
        offset: usize,
        length: usize,
        element_type: Value,
        fill_pointer: Option<usize>,
        adjustable: bool,
    ) -> Option<Self> {
        let data_length = self.data.borrow().len();
        let end = offset.checked_add(length)?;
        if end > data_length {
            return None;
        }
        Some(Self {
            data: Rc::clone(&self.data),
            descriptor: Rc::new(RefCell::new(Descriptor {
                offset,
                length,
                fill_pointer: fill_pointer.filter(|pointer| *pointer <= length),
                adjustable,
                element_type,
                displaced_from: Some(Rc::new(source)),
                displaced_index_offset: offset,
            })),
        })
    }

    pub(crate) fn ptr_eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.descriptor, &other.descriptor)
    }

    pub(crate) fn identity(&self) -> usize {
        Rc::as_ptr(&self.descriptor) as usize
    }
}
