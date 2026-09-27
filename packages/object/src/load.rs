use crate::{BuiltinArgs, MultipleValues, ObjectError, Runtime, ThreadContext, Word};

/// Evaluation service used by evaluator-dependent builtins.
pub trait LoadPort: Send + Sync + std::fmt::Debug {
    /// Load the pathname represented by the builtin arguments.
    ///
    /// # Errors
    ///
    /// Returns an object-layer error when the pathname or load options are invalid.
    fn load(
        &self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        args: &BuiltinArgs<'_>,
        values: &mut MultipleValues,
    ) -> Result<Word, ObjectError>;
}
