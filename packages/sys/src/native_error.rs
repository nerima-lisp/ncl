use crate::word::Word;

/// The native operation that reported a failure.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeOperation {
    /// Cons allocation.
    Cons,
    /// Cons car access.
    Car,
    /// Fixnum addition.
    Add,
    /// Fixnum subtraction.
    Sub,
    /// Fixnum multiplication.
    Mul,
    /// Fixnum less-than comparison.
    Less,
    /// Safepoint service.
    Safepoint,
}

/// The overflow semantics of a native arithmetic result.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OverflowSemantics {
    /// The result overflowed the fixnum arithmetic/storage representation.
    ArithmeticFixnum,
}

/// A typed failure produced by a direct native entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeError {
    /// The ABI received a null thread pointer.
    NullThread {
        /// Native operation that received the null pointer.
        operation: NativeOperation,
    },
    /// The thread exists but is not registered with a heap.
    ThreadNotRegistered {
        /// Native operation that required registration.
        operation: NativeOperation,
    },
    /// The value was tagged as an object but is not valid for the operation.
    Invalid {
        /// Native operation that rejected the value.
        operation: NativeOperation,
        /// Rejected value.
        value: Word,
    },
    /// The value has a valid ABI word but the wrong object type.
    TypeMismatch {
        /// Native operation that rejected the value.
        operation: NativeOperation,
        /// Zero-based argument position.
        operand: u8,
        /// Rejected value.
        value: Word,
    },
    /// Heap allocation failed.
    Allocation {
        /// Native operation that allocated.
        operation: NativeOperation,
        /// Underlying storage failure.
        condition: crate::StorageCondition,
    },
    /// Arithmetic overflowed the represented fixnum/storage domain.
    Overflow {
        /// Native operation that overflowed.
        operation: NativeOperation,
        /// Arithmetic/storage interpretation of the overflow.
        semantics: OverflowSemantics,
    },
}
