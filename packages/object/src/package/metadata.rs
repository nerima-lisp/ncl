use super::{LOCAL_NICKNAMES, LOCK, Package};
use crate::object_access::{get, put};
use crate::widetag;
use crate::{
    LispError, ObjectError, PackageError, StringObject, ThreadContext, string_length, string_ref,
};
use ncl_sys::Word;

impl Package {
    pub(crate) fn ensure_unlocked(self, ctx: &mut ThreadContext) -> Result<(), ObjectError> {
        if self.is_locked(ctx)? {
            ctx.set_pending_lisp_error(LispError::PackageError(PackageError::Locked));
            return Err(ObjectError::TypeError);
        }
        Ok(())
    }

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

    /// Resolve a package-local nickname from this package's association list.
    ///
    /// # Errors
    /// Returns a layout or type error when the association list is malformed.
    pub fn resolve_local_nickname(
        self,
        ctx: &ThreadContext,
        nickname: StringObject,
    ) -> Result<Option<Self>, ObjectError> {
        let wanted = nickname.as_word();
        let wanted_length = string_length(ctx, wanted)?;
        let mut entries = self.local_nicknames(ctx)?;
        while entries != Word::NIL {
            let entry =
                ncl_sys::read_cons_word(&ctx.thread, entries, 0).ok_or(ObjectError::Layout)?;
            let next =
                ncl_sys::read_cons_word(&ctx.thread, entries, 1).ok_or(ObjectError::Layout)?;
            let entry_name =
                ncl_sys::read_cons_word(&ctx.thread, entry, 0).ok_or(ObjectError::Layout)?;
            if string_length(ctx, entry_name)? == wanted_length
                && (0..wanted_length).all(|index| {
                    string_ref(ctx, entry_name, index) == string_ref(ctx, wanted, index)
                })
            {
                let package =
                    ncl_sys::read_cons_word(&ctx.thread, entry, 1).ok_or(ObjectError::Layout)?;
                return Self::try_from_word(ctx, package).map(Some);
            }
            entries = next;
        }
        Ok(None)
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
