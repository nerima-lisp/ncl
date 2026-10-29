use ncl_compiler_front::FrontError;
use ncl_compiler_front::ast::LocalMacro;
use ncl_object::{ThreadContext, Word};

use super::{
    EntryCodes, Held, LocalMacroCallContext, ObjectRuntime, destructure, eval_expr, held_get,
    held_push, object_error,
};

pub fn call_local_macro_preserving_pending_condition(
    ctx: &mut ThreadContext,
    runtime: &ObjectRuntime,
    entry_codes: &EntryCodes,
    definition: &LocalMacro,
    form: Word,
    environment: Word,
) -> Result<Word, FrontError> {
    call_local_macro_with_condition_mode(
        ctx,
        runtime,
        entry_codes,
        definition,
        form,
        environment,
        true,
    )
}

pub(super) fn call_local_macro_with_condition_mode(
    ctx: &mut ThreadContext,
    runtime: &ObjectRuntime,
    entry_codes: &EntryCodes,
    definition: &LocalMacro,
    form: Word,
    environment: Word,
    preserve_pending_condition: bool,
) -> Result<Word, FrontError> {
    let mut held: Held = vec![form];
    let mut env = Vec::new();
    destructure(
        ctx,
        runtime,
        entry_codes,
        &definition.lambda_list,
        &mut held,
        &mut env,
        LocalMacroCallContext {
            macro_environment: environment,
            preserve_pending_condition,
        },
    )?;
    let mut result = held_push(&mut held, Word::NIL);
    for expr in &definition.body {
        result = eval_expr(
            ctx,
            runtime,
            entry_codes,
            &mut held,
            &env,
            expr,
            preserve_pending_condition,
        )?;
    }
    held_get(&held, result).map_err(object_error)
}
