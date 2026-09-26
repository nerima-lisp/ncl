//! Signalling and handler records.

use ncl_object::{Runtime, ThreadContext, Word, make_simple_vector, make_string};

use crate::class::{class_named, condition_class_of, superclass_of};
use crate::error::ConditionError;
use crate::records;

/// A token capturing the handler-cluster head before a handler push.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HandlerChain(Word);

/// Push a handler for `class` onto the handler cluster.
///
/// The handler is active until [`pop_handler`] restores the captured head.
/// Phase 1 does not invoke the handler function (there is no generated code
/// yet); a matching handler merely marks the condition as handled.
///
/// # Errors
/// Returns an object-layer error when the record cannot be allocated.
pub fn push_handler(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    class: crate::class::ConditionClass,
    handler: Word,
) -> Result<HandlerChain, ConditionError> {
    let previous = records::cluster_head(ctx);
    let depth = records::cluster_next_depth(ctx, previous)?;
    let record = make_simple_vector(
        ctx,
        runtime,
        &[
            records::HANDLER_TAG,
            class.as_word(),
            handler,
            previous,
            depth,
        ],
    )
    .map_err(ConditionError::from)?;
    records::set_cluster_head(ctx, record);
    Ok(HandlerChain(previous))
}

/// Restore the handler cluster to the head captured by `chain`.
pub const fn pop_handler(ctx: &mut ThreadContext, chain: HandlerChain) {
    records::set_cluster_head(ctx, chain.0);
}

/// Signal a condition, invoking the first matching handler for its class chain.
///
/// An unhandled warning is muffled and returns `Ok`; any other unhandled
/// condition returns [`ConditionError::Unhandled`].
///
/// # Errors
/// Returns an object-layer error when the condition or a record is malformed,
/// or [`ConditionError::Unhandled`] when no handler matched and the condition
/// is not a warning.
pub fn signal(ctx: &mut ThreadContext, condition: Word) -> Result<(), ConditionError> {
    let class = condition_class_of(ctx, condition)?;
    let mut head = records::cluster_head(ctx);
    while head != Word::NIL {
        let record = records::ClusterRecord::from_word(ctx, head).map_err(ConditionError::from)?;
        if let records::ClusterRecord::Handler(handler) = record {
            let handler_class = handler.class(ctx).map_err(ConditionError::from)?;
            if class_matches(ctx, class.as_word(), handler_class)? {
                return Ok(());
            }
        }
        head = records::record_previous(ctx, head)?;
    }
    if class_named(ctx, class.as_word(), "WARNING")? {
        return Ok(());
    }
    Err(ConditionError::Unhandled)
}

/// Signal an error condition, raising a non-local exit when unhandled.
///
/// # Errors
/// Returns [`ConditionError::Unhandled`] after recording a pending exit.
pub fn error(ctx: &mut ThreadContext, condition: Word) -> Result<(), ConditionError> {
    match signal(ctx, condition) {
        Ok(()) => Ok(()),
        Err(ConditionError::Unhandled) => {
            ctx.set_pending(ncl_object::ObjectError::Unsupported);
            ctx.set_non_local_exit(true);
            Err(ConditionError::Unhandled)
        }
        Err(error) => Err(error),
    }
}

/// Signal a warning condition; an unhandled warning is muffled.
///
/// # Errors
/// Returns an object-layer error when the condition is malformed.
pub fn warn(ctx: &mut ThreadContext, condition: Word) -> Result<(), ConditionError> {
    match signal(ctx, condition) {
        Ok(()) | Err(ConditionError::Unhandled) => Ok(()),
        Err(error) => Err(error),
    }
}

/// Signal a condition after installing a `CONTINUE` restart.
///
/// The `continue_control` word is stored as the restart function and
/// `continue_args` as its report. The restart is removed before returning.
///
/// # Errors
/// Returns the same failures as [`signal`].
pub fn cerror(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    continue_control: Word,
    continue_args: Word,
    condition: Word,
) -> Result<(), ConditionError> {
    let name = make_string(ctx, runtime, &"CONTINUE".chars().collect::<Vec<_>>())
        .map_err(ConditionError::from)?;
    let restart = crate::restart::push_restart(
        ctx,
        runtime,
        name,
        continue_control,
        continue_args,
        Word::NIL,
        Word::NIL,
    )?;
    let result = signal(ctx, condition);
    crate::restart::pop_restart(ctx, restart);
    result
}

/// Whether `handler_class` equals `class` or one of its ancestors.
fn class_matches(
    ctx: &ThreadContext,
    class: Word,
    handler_class: Word,
) -> Result<bool, ConditionError> {
    let mut current = class;
    loop {
        if current == handler_class {
            return Ok(true);
        }
        let superclass = superclass_of(ctx, current)?;
        if superclass == Word::NIL {
            return Ok(false);
        }
        current = superclass;
    }
}
