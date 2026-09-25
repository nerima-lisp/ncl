//! Numeric typed views over the tagged-word ABI.

use ncl_sys::Word;

use super::error::{ObjectType, TypeError};

/// A fixnum view validated at the builtin boundary.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Fixnum(i64);
impl Fixnum {
    /// Validate and wrap a tagged fixnum.
    ///
    /// # Errors
    ///
    /// Returns a type error when the word is not a fixnum.
    pub fn try_from_word(word: Word) -> Result<Self, TypeError> {
        word.as_fixnum()
            .filter(|_| word.bits() < 256 || !word.is_character())
            .map(Self)
            .ok_or(TypeError {
                datum: word,
                expected: ObjectType::Fixnum,
            })
    }
    #[must_use]
    pub const fn value(self) -> i64 {
        self.0
    }
    #[must_use]
    pub const fn as_word(self) -> Word {
        Word::fixnum(self.0)
    }
}
/// The ANSI integer type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Integer {
    Fixnum(Fixnum),
    Bignum(crate::Bignum),
}
/// The ANSI rational type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Rational {
    Integer(Integer),
    Ratio(crate::Ratio),
}
/// The ANSI real type.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Real {
    Rational(Rational),
    DoubleFloat(crate::DoubleFloat),
}
/// The ANSI number type.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Number {
    Real(Real),
    Complex(crate::Complex),
}
