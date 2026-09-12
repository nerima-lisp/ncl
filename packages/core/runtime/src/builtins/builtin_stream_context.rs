use std::cell::RefCell;
use std::rc::Rc;

use crate::Stream;

thread_local! {
    static ACTIVE_STANDARD_INPUT: RefCell<Option<Rc<RefCell<Stream>>>> = const { RefCell::new(None) };
    static ACTIVE_STANDARD_OUTPUT: RefCell<Option<Rc<RefCell<Stream>>>> = const { RefCell::new(None) };
}

struct StreamContextGuard {
    input: Option<Rc<RefCell<Stream>>>,
    output: Option<Rc<RefCell<Stream>>>,
}

impl Drop for StreamContextGuard {
    fn drop(&mut self) {
        ACTIVE_STANDARD_INPUT.with(|active| {
            *active.borrow_mut() = self.input.take();
        });
        ACTIVE_STANDARD_OUTPUT.with(|active| {
            *active.borrow_mut() = self.output.take();
        });
    }
}

pub(crate) fn with_stream_context<T>(
    input: Option<Rc<RefCell<Stream>>>,
    output: Option<Rc<RefCell<Stream>>>,
    function: impl FnOnce() -> T,
) -> T {
    let previous_input = ACTIVE_STANDARD_INPUT.with(|active| active.replace(input));
    let previous_output = ACTIVE_STANDARD_OUTPUT.with(|active| active.replace(output));
    let _guard = StreamContextGuard {
        input: previous_input,
        output: previous_output,
    };
    function()
}

pub(crate) fn standard_input() -> Option<Rc<RefCell<Stream>>> {
    ACTIVE_STANDARD_INPUT
        .with(|active| active.borrow().clone())
        .or_else(|| match crate::builtins::standard_streams::input() {
            Some(crate::Value::Stream(stream)) => Some(stream),
            _ => None,
        })
}

pub(crate) fn standard_output() -> Option<Rc<RefCell<Stream>>> {
    ACTIVE_STANDARD_OUTPUT
        .with(|active| active.borrow().clone())
        .or_else(|| match crate::builtins::standard_streams::output() {
            Some(crate::Value::Stream(stream)) => Some(stream),
            _ => None,
        })
}
