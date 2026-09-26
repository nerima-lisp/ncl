use super::{LOCAL_NICKNAMES, LOCK, Package};
use crate::object_access::{get, put};
use crate::widetag;
use crate::{ObjectError, ThreadContext};
use ncl_sys::Word;

impl Package {
    /// Validate a tagged word as a package reference.
    ///
    /// # Errors
    /// Returns a type error when the word is not a package object.
    pub fn try_from_word(ctx: &ThreadContext, word: Word) -> Result<Self, ObjectError> {
        if word == Word::NIL
            || word.lowtag() != ncl_sys::LowTag::OtherPointer as u8
            || ncl_sys::object_widetag(&ctx.thread, word) != Some(widetag::PACKAGE)
        {
            return Err(ObjectError::TypeError);
        }
        Ok(Self::from_word(word))
    }

    /// Return whether namespace mutation is locked for this package.
    ///
    /// # Errors
    /// Returns a layout error when the package object is malformed.
    pub fn is_locked(self, ctx: &ThreadContext) -> Result<bool, ObjectError> {
        Ok(get(ctx, self.as_word(), widetag::PACKAGE, LOCK)? != Word::fixnum(0))
    }

    /// Set the package namespace lock state.
    ///
    /// # Errors
    /// Returns a layout error when the package object is malformed.
    pub fn set_locked(self, ctx: &mut ThreadContext, locked: bool) -> Result<(), ObjectError> {
        put(
            ctx,
            self.as_word(),
            LOCK,
            if locked {
                Word::fixnum(1)
            } else {
                Word::fixnum(0)
            },
        )
    }

    /// Return the package-local nickname association list.
    ///
    /// # Errors
    /// Returns a layout error when the package object is malformed.
    pub fn local_nicknames(self, ctx: &ThreadContext) -> Result<Word, ObjectError> {
        get(ctx, self.as_word(), widetag::PACKAGE, LOCAL_NICKNAMES)
    }

    /// Replace the package-local nickname association list.
    ///
    /// # Errors
    /// Returns a layout error when the package object is malformed.
    pub fn set_local_nicknames(
        self,
        ctx: &mut ThreadContext,
        nicknames: Word,
    ) -> Result<(), ObjectError> {
        put(ctx, self.as_word(), LOCAL_NICKNAMES, nicknames)
    }
}
