use ncl_object::{ArithmeticError, LispError, ObjectError, ObjectType};
use ncl_sys::{NativeError, NativeOperation, OverflowSemantics, StorageCondition};

use crate::RuntimeError;

/// The typed condition category corresponding to a direct native failure.
#[derive(Debug)]
pub enum NativeCondition {
    /// An object-layer failure, such as storage exhaustion or an invalid object.
    Object(ObjectError),
    /// A Lisp condition category, such as type-error or arithmetic-error.
    Lisp(LispError),
}

pub const fn native_failure(error: NativeError) -> RuntimeError {
    let condition = match error {
        NativeError::TypeMismatch {
            operation: NativeOperation::Car,
            value,
            ..
        } => NativeCondition::Lisp(LispError::TypeError {
            datum: value,
            expected: ObjectType::Cons,
        }),
        NativeError::TypeMismatch {
            operation: NativeOperation::Add | NativeOperation::Mul,
            value,
            ..
        } => NativeCondition::Lisp(LispError::TypeError {
            datum: value,
            expected: ObjectType::Fixnum,
        }),
        NativeError::Overflow {
            semantics: OverflowSemantics::ArithmeticFixnum,
            ..
        } => NativeCondition::Lisp(LispError::ArithmeticError(
            ArithmeticError::InvalidOperation,
        )),
        NativeError::Allocation { condition, .. } => {
            NativeCondition::Object(ObjectError::Storage(condition))
        }
        NativeError::NullThread { .. } | NativeError::ThreadNotRegistered { .. } => {
            NativeCondition::Object(ObjectError::Storage(StorageCondition::ThreadNotRegistered))
        }
        NativeError::Invalid { .. } => NativeCondition::Object(ObjectError::Layout),
        NativeError::TypeMismatch { .. } => NativeCondition::Object(ObjectError::TypeError),
    };
    RuntimeError::NativeFailure { error, condition }
}

#[cfg(test)]
mod tests {
    use ncl_object::{LispError, ObjectType};
    use ncl_sys::{NativeError, NativeOperation, OverflowSemantics, Word};

    use super::{NativeCondition, native_failure};
    use crate::{Runtime, RuntimeError};

    #[test]
    fn native_failures_are_typed_with_stress_modes_enabled() {
        let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("{error}"));
        runtime.context.set_gc_stress(true);
        runtime.context.set_strict_forwarding(true);

        let thread = std::ptr::NonNull::from(runtime.context.thread_mut());
        let returned = ncl_sys::native_add(
            thread,
            Word::fixnum(i64::MAX / 2),
            Word::fixnum(i64::MAX / 2),
        );
        assert_eq!(returned, Word::NIL);
        assert!(matches!(
            runtime.context.thread_mut().take_native_error(),
            Some(NativeError::Overflow {
                operation: NativeOperation::Add,
                semantics: OverflowSemantics::ArithmeticFixnum,
            })
        ));
        let error = NativeError::TypeMismatch {
            operation: NativeOperation::Car,
            operand: 0,
            value: Word::fixnum(7),
        };
        let failure = native_failure(error);
        assert!(matches!(
            failure,
            RuntimeError::NativeFailure {
                error: NativeError::TypeMismatch { .. },
                condition: NativeCondition::Lisp(LispError::TypeError {
                    expected: ObjectType::Cons,
                    ..
                }),
            }
        ));

        let failure = native_failure(NativeError::Overflow {
            operation: NativeOperation::Mul,
            semantics: OverflowSemantics::ArithmeticFixnum,
        });
        assert!(matches!(
            failure,
            RuntimeError::NativeFailure {
                condition: NativeCondition::Lisp(LispError::ArithmeticError(_)),
                ..
            }
        ));
    }
}
