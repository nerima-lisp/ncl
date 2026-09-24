//! The trait the front end uses to call macro functions.

use ncl_object::{Runtime, ThreadContext, Word};

use crate::ast::LocalMacro;
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

    /// Expand a call of a `macrolet` definition.
    ///
    /// A local macro body is a Lisp form that must be evaluated at expansion
    /// time, which the front end cannot do alone. A runtime that compiles the
    /// definition's body overrides this method; the default reports that the
    /// capability is unavailable, so a `macrolet` call without such a runtime
    /// fails loudly instead of silently mis-expanding.
    ///
    /// # Errors
    ///
    /// Returns [`FrontError::MacroExpansion`] by default.
    fn call_local_macro(
        &mut self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        definition: &LocalMacro,
        form: Word,
    ) -> Result<Word, FrontError> {
        let _ = (ctx, runtime, form);
        Err(FrontError::MacroExpansion {
            name: definition.name.clone(),
            detail: "local macro expansion is not available".to_owned(),
        })
    }
}
