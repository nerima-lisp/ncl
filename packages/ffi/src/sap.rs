//! System area pointers and the arithmetic over them.
//!
//! Phase 1 represents a SAP as a Lisp fixnum holding the machine address, so
//! `sap-int` and `int-sap` are exact inverses. A distinct SAP object kind is a
//! follow-up `ncl-object` requirement; until then a SAP and an integer address
//! are interchangeable until the runtime grows a distinct pointer object.

use ncl_object::Word;

use crate::FfiError;

/// A raw system area pointer.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct SystemAreaPointer(usize);

impl SystemAreaPointer {
    /// Wrap a machine address.
    #[must_use]
    pub const fn new(address: usize) -> Self {
        Self(address)
    }

    /// The null pointer.
    #[must_use]
    pub const fn null() -> Self {
        Self(0)
    }

    /// The wrapped machine address.
    #[must_use]
    pub const fn address(self) -> usize {
        self.0
    }

    /// Whether this is the null pointer.
    #[must_use]
    pub const fn is_null(self) -> bool {
        self.0 == 0
    }

    /// Add a byte offset, wrapping like `sap+`.
    #[must_use]
    pub const fn sap_plus(self, offset: isize) -> Self {
        Self(self.0.wrapping_add_signed(offset))
    }

    /// Subtract a byte offset, wrapping like `sap-`.
    #[must_use]
    pub const fn sap_minus(self, offset: isize) -> Self {
        Self(self.0.wrapping_add_signed(offset.wrapping_neg()))
    }

    /// The signed byte distance to `other`, like `sap-` on two pointers.
    #[must_use]
    pub const fn sap_difference(self, other: Self) -> isize {
        self.0.wrapping_sub(other.0).cast_signed()
    }

    /// Encode as a Lisp fixnum holding the address, like `sap-int`.
    #[must_use]
    #[allow(
        clippy::cast_possible_wrap,
        reason = "user-space addresses fit in a signed 63-bit fixnum"
    )]
    pub const fn sap_int(self) -> Word {
        Word::fixnum(self.0 as i64)
    }

    /// Encode as a Lisp value; the inverse of [`SystemAreaPointer::from_word`].
    #[must_use]
    pub const fn as_word(self) -> Word {
        self.sap_int()
    }

    /// Decode a Lisp integer address, like `int-sap`.
    ///
    /// # Errors
    /// Returns [`FfiError::TypeMismatch`] when `value` is not an integer and
    /// [`FfiError::ValueOutOfRange`] when it is negative.
    pub fn from_word(value: Word) -> Result<Self, FfiError> {
        value.as_fixnum().map_or(
            Err(FfiError::TypeMismatch {
                type_name: "system-area-pointer",
            }),
            |fixnum| {
                usize::try_from(fixnum)
                    .map(Self)
                    .map_err(|_| FfiError::ValueOutOfRange {
                        type_name: "system-area-pointer",
                    })
            },
        )
    }
}

/// Whether two pointers are equal, like `sap=`.
#[must_use]
pub fn sap_eq(left: SystemAreaPointer, right: SystemAreaPointer) -> bool {
    left == right
}

/// Whether `left` addresses a lower byte than `right`, like `sap<`.
#[must_use]
pub const fn sap_lt(left: SystemAreaPointer, right: SystemAreaPointer) -> bool {
    left.address() < right.address()
}

/// Whether `left` addresses a lower or equal byte, like `sap<=`.
#[must_use]
pub const fn sap_le(left: SystemAreaPointer, right: SystemAreaPointer) -> bool {
    left.address() <= right.address()
}

/// Whether `left` addresses a higher byte than `right`, like `sap>`.
#[must_use]
pub const fn sap_gt(left: SystemAreaPointer, right: SystemAreaPointer) -> bool {
    left.address() > right.address()
}

/// Whether `left` addresses a higher or equal byte, like `sap>=`.
#[must_use]
pub const fn sap_ge(left: SystemAreaPointer, right: SystemAreaPointer) -> bool {
    left.address() >= right.address()
}
