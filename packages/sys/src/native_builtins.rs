//! Native builtins require a non-null pointer to a registered live thread.
//!
//! A failed operation reports its only error through `Thread`'s
//! `NativeError` side channel and returns `Word::NIL` as its value. `NIL` is
//! therefore not an error payload and must not be replaced with `UNBOUND`.

use crate::{
    Heap, NativeError, NativeOperation, OverflowSemantics, Thread, Word, collect, read_cons_word,
};
use std::ptr::NonNull;

/// Allocate a cons cell through the heap already associated with `thread`.
///
/// This is the native-entry boundary used by generated code. The caller must
/// pass a registered thread pointer that remains valid for the duration of the
/// call; the generated entry owns that condition through its pinned context.
///
/// # Safety
/// `thread` must point to a registered live thread and remain valid for the
/// duration of the call.
pub unsafe extern "C" fn native_cons(mut thread: NonNull<Thread>, car: Word, cdr: Word) -> Word {
    // SAFETY: the caller provides a valid registered thread.
    let thread = unsafe { thread.as_mut() };
    let Some(heap) = thread.heap().map(std::ptr::from_ref::<Heap>) else {
        thread.set_native_error(NativeError::ThreadNotRegistered {
            operation: NativeOperation::Cons,
        });
        return Word::NIL;
    };
    // SAFETY: the heap pointer is owned by the registered thread for this call.
    match unsafe { (&*heap).alloc_cons(thread, car, cdr) } {
        Ok(value) => value,
        Err(condition) => {
            thread.set_native_error(NativeError::Allocation {
                operation: NativeOperation::Cons,
                condition,
            });
            Word::NIL
        }
    }
}

/// Return the car of a cons cell through the native-entry boundary.
///
/// # Safety
/// `thread` must point to a registered live thread and remain valid for the
/// duration of the call.
#[allow(clippy::option_if_let_else, clippy::single_match_else)]
#[must_use]
pub unsafe extern "C" fn native_car(mut thread: NonNull<Thread>, value: Word) -> Word {
    // SAFETY: the caller provides a valid registered thread.
    let thread = unsafe { thread.as_mut() };
    if thread.heap().is_none() {
        thread.set_native_error(NativeError::ThreadNotRegistered {
            operation: NativeOperation::Car,
        });
        return Word::NIL;
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
        return Word::NIL;
    }
    match read_cons_word(thread, value, 0) {
        Some(value) => value,
        None => {
            thread.set_native_error(NativeError::Invalid {
                operation: NativeOperation::Car,
                value,
            });
            Word::NIL
        }
    }
}

/// Add two fixnums through the native-entry boundary.
#[must_use]
pub extern "C" fn native_add(mut thread: NonNull<Thread>, left: Word, right: Word) -> Word {
    // SAFETY: the caller provides a valid registered thread.
    let thread = unsafe { thread.as_mut() };
    if thread.heap().is_none() {
        thread.set_native_error(NativeError::ThreadNotRegistered {
            operation: NativeOperation::Add,
        });
        return Word::NIL;
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
    Word::NIL
}

/// Subtract two fixnums through the native-entry boundary.
#[must_use]
pub extern "C" fn native_sub(mut thread: NonNull<Thread>, left: Word, right: Word) -> Word {
    // SAFETY: the caller provides a valid registered thread.
    let thread = unsafe { thread.as_mut() };
    if thread.heap().is_none() {
        thread.set_native_error(NativeError::ThreadNotRegistered {
            operation: NativeOperation::Sub,
        });
        return Word::NIL;
    }
    if let (Some(left), Some(right)) = (left.as_fixnum(), right.as_fixnum()) {
        if let Some(value) = left.checked_sub(right) {
            let word = Word::fixnum(value);
            if word.as_fixnum() == Some(value) {
                return word;
            }
        }
        thread.set_native_error(NativeError::Overflow {
            operation: NativeOperation::Sub,
            semantics: OverflowSemantics::ArithmeticFixnum,
        });
    } else {
        let (operand, value) = if left.as_fixnum().is_none() {
            (0, left)
        } else {
            (1, right)
        };
        thread.set_native_error(NativeError::TypeMismatch {
            operation: NativeOperation::Sub,
            operand,
            value,
        });
    }
    Word::NIL
}

/// Compare two fixnums through the native-entry boundary.
#[must_use]
pub extern "C" fn native_less(mut thread: NonNull<Thread>, left: Word, right: Word) -> Word {
    // SAFETY: the caller provides a valid registered thread.
    let thread = unsafe { thread.as_mut() };
    if thread.heap().is_none() {
        thread.set_native_error(NativeError::ThreadNotRegistered {
            operation: NativeOperation::Less,
        });
        return Word::NIL;
    }
    if let (Some(left), Some(right)) = (left.as_fixnum(), right.as_fixnum()) {
        return if left < right { Word::TRUE } else { Word::NIL };
    }
    let (operand, value) = if left.as_fixnum().is_none() {
        (0, left)
    } else {
        (1, right)
    };
    thread.set_native_error(NativeError::TypeMismatch {
        operation: NativeOperation::Less,
        operand,
        value,
    });
    Word::NIL
}

/// Multiply two fixnums through the native-entry boundary.
#[must_use]
pub extern "C" fn native_mul(mut thread: NonNull<Thread>, left: Word, right: Word) -> Word {
    // SAFETY: the caller provides a valid registered thread.
    let thread = unsafe { thread.as_mut() };
    if thread.heap().is_none() {
        thread.set_native_error(NativeError::ThreadNotRegistered {
            operation: NativeOperation::Mul,
        });
        return Word::NIL;
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
    Word::NIL
}

/// Capture the generated frame and service a cooperative safepoint request.
///
/// # Safety
/// `thread` must point to a registered live thread. `frame` and `pc` must
/// describe the live generated frame at a registered safepoint map.
pub unsafe extern "C" fn native_safepoint(mut thread: NonNull<Thread>, frame: *mut u8, pc: usize) {
    // SAFETY: the caller provides a valid registered thread.
    let thread = unsafe { thread.as_mut() };
    thread.capture_native_frame(frame.addr(), pc);
    thread.enter_native();
    collect(thread, false);
    thread.leave_native();
    thread.clear_safepoint_request();
}
