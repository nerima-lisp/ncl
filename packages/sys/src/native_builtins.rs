use crate::{
    Heap, NativeError, NativeOperation, OverflowSemantics, Thread, Word, collect, read_cons_word,
};

/// Allocate a cons cell through the heap already associated with `thread`.
///
/// This is the native-entry boundary used by generated code. The caller must
/// pass a registered thread pointer that remains valid for the duration of the
/// call; the generated entry owns that condition through its pinned context.
///
/// # Safety
/// The thread pointer must be non-null and point to a registered live thread.
pub unsafe extern "C" fn native_cons(thread: *mut Thread, car: Word, cdr: Word) -> Word {
    if thread.is_null() {
        return Word::UNBOUND;
    }
    // SAFETY: generated code passes the pinned, registered Thread pointer.
    let thread = unsafe { &mut *thread };
    let Some(heap) = thread.heap().map(std::ptr::from_ref::<Heap>) else {
        thread.set_native_error(NativeError::ThreadNotRegistered {
            operation: NativeOperation::Cons,
        });
        return Word::UNBOUND;
    };
    // SAFETY: the heap pointer is owned by the registered thread for this call.
    match unsafe { (&*heap).alloc_cons(thread, car, cdr) } {
        Ok(value) => value,
        Err(condition) => {
            thread.set_native_error(NativeError::Allocation {
                operation: NativeOperation::Cons,
                condition,
            });
            Word::UNBOUND
        }
    }
}

/// Return the car of a cons cell through the native-entry boundary.
///
/// # Safety
/// The thread pointer must be non-null and point to a registered live thread.
#[allow(clippy::option_if_let_else, clippy::single_match_else)]
pub unsafe extern "C" fn native_car(thread: *mut Thread, value: Word) -> Word {
    if thread.is_null() {
        return Word::UNBOUND;
    }
    // SAFETY: generated code passes the pinned, registered Thread pointer.
    let thread = unsafe { &mut *thread };
    if thread.heap().is_none() {
        thread.set_native_error(NativeError::ThreadNotRegistered {
            operation: NativeOperation::Car,
        });
        return Word::UNBOUND;
    }
    if value == Word::NIL {
        return Word::NIL;
    }
    if !value.is_cons() {
        thread.set_native_error(NativeError::TypeMismatch {
            operation: NativeOperation::Car,
            operand: 0,
            value,
        });
        return Word::UNBOUND;
    }
    match read_cons_word(thread, value, 0) {
        Some(value) => value,
        None => {
            thread.set_native_error(NativeError::Invalid {
                operation: NativeOperation::Car,
                value,
            });
            Word::UNBOUND
        }
    }
}

/// Add two fixnums through the native-entry boundary.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn native_add(thread: *mut Thread, left: Word, right: Word) -> Word {
    // SAFETY: the caller contract permits a null pointer and otherwise requires a live Thread.
    let Some(thread) = (unsafe { thread.as_mut() }) else {
        return Word::UNBOUND;
    };
    if thread.heap().is_none() {
        thread.set_native_error(NativeError::ThreadNotRegistered {
            operation: NativeOperation::Add,
        });
        return Word::UNBOUND;
    }
    if let (Some(left), Some(right)) = (left.as_fixnum(), right.as_fixnum()) {
        if let Some(value) = left.checked_add(right) {
            let word = Word::fixnum(value);
            if word.as_fixnum() == Some(value) {
                return word;
            }
        }
        thread.set_native_error(NativeError::Overflow {
            operation: NativeOperation::Add,
            semantics: OverflowSemantics::ArithmeticFixnum,
        });
    } else {
        let (operand, value) = if left.as_fixnum().is_none() {
            (0, left)
        } else {
            (1, right)
        };
        thread.set_native_error(NativeError::TypeMismatch {
            operation: NativeOperation::Add,
            operand,
            value,
        });
    }
    Word::UNBOUND
}

/// Multiply two fixnums through the native-entry boundary.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn native_mul(thread: *mut Thread, left: Word, right: Word) -> Word {
    // SAFETY: the caller contract permits a null pointer and otherwise requires a live Thread.
    let Some(thread) = (unsafe { thread.as_mut() }) else {
        return Word::UNBOUND;
    };
    if thread.heap().is_none() {
        thread.set_native_error(NativeError::ThreadNotRegistered {
            operation: NativeOperation::Mul,
        });
        return Word::UNBOUND;
    }
    if let (Some(left), Some(right)) = (left.as_fixnum(), right.as_fixnum()) {
        if let Some(value) = left.checked_mul(right) {
            let word = Word::fixnum(value);
            if word.as_fixnum() == Some(value) {
                return word;
            }
        }
        thread.set_native_error(NativeError::Overflow {
            operation: NativeOperation::Mul,
            semantics: OverflowSemantics::ArithmeticFixnum,
        });
    } else {
        let (operand, value) = if left.as_fixnum().is_none() {
            (0, left)
        } else {
            (1, right)
        };
        thread.set_native_error(NativeError::TypeMismatch {
            operation: NativeOperation::Mul,
            operand,
            value,
        });
    }
    Word::UNBOUND
}

/// Capture the generated frame and service a cooperative safepoint request.
///
/// # Safety
/// The thread pointer must be non-null and the frame and PC must describe the
/// live generated frame at a registered safepoint map.
pub unsafe extern "C" fn native_safepoint(thread: *mut Thread, frame: *mut u8, pc: usize) {
    if thread.is_null() {
        return;
    }
    // SAFETY: generated code passes the registered thread and its live frame.
    let thread = unsafe { &mut *thread };
    thread.capture_native_frame(frame.addr(), pc);
    thread.enter_native();
    collect(thread, false);
    thread.leave_native();
    thread.clear_safepoint_request();
}
