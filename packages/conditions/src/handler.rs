//! Signalling and handler records.

use ncl_object::{Runtime, ThreadContext, Word, make_simple_vector, make_string};

use crate::class::{class_named, condition_class_of, direct_parents, superclass_of};
use crate::error::ConditionError;
use crate::records;

/// A token capturing the handler-cluster head before a handler push.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HandlerChain(Word);

impl HandlerChain {
    /// Return the handler-cluster head captured before the push.
    #[must_use]
    pub const fn as_word(self) -> Word {
        self.0
    }

    /// Recreate a chain token from a previously captured head.
    #[must_use]
    pub const fn from_word(word: Word) -> Self {
        Self(word)
    }
}

/// Push a handler for `class` onto the handler cluster.
///
/// The handler is active until [`pop_handler`] restores the captured head.
///
/// # Errors
/// Returns an object-layer error when the record cannot be allocated.
pub fn push_handler(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    class: crate::class::ConditionClass,
    handler: Word,
) -> Result<HandlerChain, ConditionError> {
    let added_root = ctx.root_condition_handler_head();
    let result = (|| {
        let previous = records::cluster_head(ctx);
        let depth = records::cluster_next_depth(ctx, previous)?;
        let record =
            ncl_object::with_roots(ctx, &[class.as_word(), handler, previous], |ctx, roots| {
                let class = roots.first().ok_or(ncl_object::ObjectError::Layout)?;
                let handler = roots.get(1).ok_or(ncl_object::ObjectError::Layout)?;
                let previous = roots.get(2).ok_or(ncl_object::ObjectError::Layout)?;
                make_simple_vector(
                    ctx,
                    runtime,
                    &[records::HANDLER_TAG, **class, **handler, **previous, depth],
                )
            })
            .map_err(ConditionError::from)?;
        records::set_cluster_head(ctx, record);
        Ok(HandlerChain(previous))
    })();
    if result.is_err() && added_root {
        let _ = ctx.unroot_condition_handler_head(runtime);
    }
    result
}

/// Restore the handler cluster to the head captured by `chain`.
pub fn pop_handler(ctx: &mut ThreadContext, runtime: &Runtime, chain: HandlerChain) {
    records::set_cluster_head(ctx, chain.0);
    if chain.0 == Word::NIL {
        let _ = ctx.unroot_condition_handler_head(runtime);
    }
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
    if signal_matched(ctx, condition)? {
        return Ok(());
    }
    let class = condition_class_of(ctx, condition)?;
    if class_named(ctx, class.as_word(), "WARNING")? {
        return Ok(());
    }
    Err(ConditionError::Unhandled)
}

/// Signal a condition, reporting whether a handler actually matched.
///
/// Unlike [`signal`], this never muffles an unhandled warning on its own:
/// `Ok(false)` means no handler matched, regardless of the condition's
/// class. `WARN`'s caller uses this to tell "a handler ran" apart from "no
/// handler matched, so the default report should print" for every
/// condition, not just non-warnings.
///
/// # Errors
/// Returns an object-layer error when the condition or a record is
/// malformed, or the failure of an invoked handler.
pub fn signal_matched(ctx: &mut ThreadContext, condition: Word) -> Result<bool, ConditionError> {
    let class = condition_class_of(ctx, condition)?;
    let mut head = records::cluster_head(ctx);
    while head != Word::NIL {
        let record = records::ClusterRecord::from_word(ctx, head).map_err(ConditionError::from)?;
        if let records::ClusterRecord::Handler(handler) = record {
            let handler_class = handler.class(ctx).map_err(ConditionError::from)?;
            if class_matches(ctx, class.as_word(), handler_class)? {
                let previous = handler.previous(ctx).map_err(ConditionError::from)?;
                let cluster_head = records::cluster_head(ctx);
                records::set_cluster_head(ctx, previous);
                let result = ncl_object::with_roots(ctx, &[head, cluster_head], |ctx, roots| {
                    records::set_cluster_head(
                        ctx,
                        **roots.get(1).ok_or(ncl_object::ObjectError::Layout)?,
                    );
                    let result = handler.function(ctx).and_then(|function| {
                        ncl_object::with_roots(ctx, &[function, condition], |ctx, roots| {
                            let function =
                                **roots.first().ok_or(ncl_object::ObjectError::Layout)?;
                            let condition =
                                **roots.get(1).ok_or(ncl_object::ObjectError::Layout)?;
                            ctx.invoke_condition_handler(function, &[condition])
                                .map(|_value| ())
                        })
                    });
                    records::set_cluster_head(
                        ctx,
                        **roots.get(1).ok_or(ncl_object::ObjectError::Layout)?,
                    );
                    result
                });
                match result {
                    Ok(()) | Err(ncl_object::ObjectError::NonLocalExit) => return Ok(true),
                    Err(error) => return Err(ConditionError::from(error)),
                }
            }
        }
        head = records::record_previous(ctx, head)?;
    }
    Ok(false)
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
    let matched = ncl_object::with_roots(
        ctx,
        &[continue_control, continue_args, condition],
        |ctx, roots| {
            let continue_control = **roots.first().ok_or(ncl_object::ObjectError::Layout)?;
            let continue_args = **roots.get(1).ok_or(ncl_object::ObjectError::Layout)?;
            let condition = **roots.get(2).ok_or(ncl_object::ObjectError::Layout)?;
            let name = make_string(ctx, runtime, &"CONTINUE".chars().collect::<Vec<_>>())?;
            let restart = crate::restart::push_restart(
                ctx,
                runtime,
                name,
                continue_control,
                continue_args,
                Word::NIL,
                Word::NIL,
            )
            .map_err(condition_object_error)?;
            // `signal_matched`, not `signal`: the caller (`CERROR`'s builtin)
            // needs to distinguish "a handler ran" from "unhandled" to know
            // whether to print the default report, and `ConditionError`
            // does not survive this closure's `ObjectError` boundary.
            let result = signal_matched(ctx, condition);
            crate::restart::pop_restart(ctx, restart);
            result.map_err(condition_object_error)
        },
    )
    .map_err(ConditionError::from)?;
    if matched {
        Ok(())
    } else {
        Err(ConditionError::Unhandled)
    }
}

const fn condition_object_error(error: ConditionError) -> ncl_object::ObjectError {
    match error {
        ConditionError::Object(error) => error,
        ConditionError::Unhandled
        | ConditionError::NotACondition
        | ConditionError::RestartNotFound
        | ConditionError::ChainCorrupt => ncl_object::ObjectError::Layout,
    }
}

/// Whether `handler_class` equals `class` or one of its ancestors, walking
/// every direct superclass when a class has more than one (the full
/// precedence list, matching `TYPEP`).
fn class_matches(
    ctx: &ThreadContext,
    class: Word,
    handler_class: Word,
) -> Result<bool, ConditionError> {
    if class == handler_class {
        return Ok(true);
    }
    for parent in direct_parents(ctx, superclass_of(ctx, class)?)? {
        if class_matches(ctx, parent, handler_class)? {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "coverage tests assert on helper results")]

    use super::*;
    use crate::class::{condition_class, make_condition};

    fn setup() -> (Runtime, ThreadContext) {
        let runtime = Runtime::new().unwrap();
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime).unwrap();
        crate::register::register(&runtime).unwrap();
        (runtime, ctx)
    }

    #[test]
    fn class_matching_walks_to_a_parent_and_rejects_unrelated_classes() {
        let (runtime, mut ctx) = setup();
        let child = condition_class(&mut ctx, &runtime, "SIMPLE-ERROR").unwrap();
        let parent = condition_class(&mut ctx, &runtime, "ERROR").unwrap();
        let unrelated = condition_class(&mut ctx, &runtime, "WARNING").unwrap();

        assert_eq!(class_matches(&ctx, child.as_word(), parent.as_word()), Ok(true));
        assert_eq!(class_matches(&ctx, child.as_word(), unrelated.as_word()), Ok(false));
    }

    #[test]
    fn condition_object_error_maps_non_object_failures_to_layout() {
        assert_eq!(condition_object_error(ConditionError::Unhandled), ncl_object::ObjectError::Layout);
        assert_eq!(condition_object_error(ConditionError::NotACondition), ncl_object::ObjectError::Layout);
        assert_eq!(condition_object_error(ConditionError::RestartNotFound), ncl_object::ObjectError::Layout);
        assert_eq!(condition_object_error(ConditionError::ChainCorrupt), ncl_object::ObjectError::Layout);
        assert_eq!(condition_object_error(ConditionError::Object(ncl_object::ObjectError::TypeError)), ncl_object::ObjectError::TypeError);
    }

    #[test]
    fn signal_matched_reports_false_for_a_valid_unhandled_condition() {
        let (runtime, mut ctx) = setup();
        let class = condition_class(&mut ctx, &runtime, "PROGRAM-ERROR").unwrap();
        let condition = make_condition(&mut ctx, &runtime, class, &[]).unwrap();

        assert_eq!(signal_matched(&mut ctx, condition), Ok(false));
    }
}
