use ncl_compiler_front::env::LexicalEnv;
use ncl_object::{ThreadContext, Word};

use super::{ObjectRuntime, RuntimeMacroCaller};

impl RuntimeMacroCaller {
    pub(crate) fn call_local_macro_in_environment_preserving_condition(
        &self,
        ctx: &mut ThreadContext,
        runtime: &ObjectRuntime,
        definition: &ncl_compiler_front::ast::LocalMacro,
        form: Word,
        environment: &LexicalEnv,
    ) -> Result<Word, ncl_compiler_front::FrontError> {
        crate::local_macro::with_macro_environment(environment, |environment| {
            crate::local_macro::call_local_macro_preserving_pending_condition(
                ctx,
                runtime,
                &self.entry_codes,
                definition,
                form,
                environment,
            )
        })
    }
}
