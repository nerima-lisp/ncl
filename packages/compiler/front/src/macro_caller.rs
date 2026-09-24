//! The trait the front end uses to call macro functions.

use ncl_object::{Runtime, ThreadContext, Word};

use crate::error::FrontError;
use crate::symbols::SymbolRef;

/// Calls macro functions during expansion.
///
/// The front end never reverse-depends on the runtime: `ncl-runtime` (Wave 3)
/// implements this trait and hands the implementation to the expander. A mock
/// implementation is enough to exercise the expander's macro paths.
pub trait MacroCaller {
    /// Expand one macro form.
    ///
    /// `form` is the whole macro call whose head is `name`. The returned word is
    /// the expansion.
    ///
    /// # Errors
    ///
    /// Returns [`FrontError::MacroExpansion`] when the macro signals or the
    /// runtime cannot call it.
    fn call_macro(
        &mut self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        name: &SymbolRef,
        form: Word,
    ) -> Result<Word, FrontError>;
}
