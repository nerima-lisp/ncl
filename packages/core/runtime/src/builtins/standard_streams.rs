use std::cell::RefCell;

use crate::Value;

thread_local! {
    static CONTEXT: RefCell<Option<(Value, Value)>> = const { RefCell::new(None) };
}

pub(crate) struct Guard(Option<(Value, Value)>);

impl Drop for Guard {
    fn drop(&mut self) {
        let previous = self.0.take();
        CONTEXT.with(|context| *context.borrow_mut() = previous);
    }
}

pub(crate) fn bind(input: Value, output: Value) -> Guard {
    let previous = CONTEXT.with(|context| context.replace(Some((input, output))));
    Guard(previous)
}

pub(crate) fn input() -> Option<Value> {
    CONTEXT.with(|context| context.borrow().as_ref().map(|streams| streams.0.clone()))
}

pub(crate) fn output() -> Option<Value> {
    CONTEXT.with(|context| context.borrow().as_ref().map(|streams| streams.1.clone()))
}
