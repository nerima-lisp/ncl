use ncl_object::{
    ArithmeticError, HeapRootedWord, Instance, LispError, ObjectError, ObjectType,
    Runtime as ObjectRuntime, ThreadContext, Word, make_cons, make_string, slot_ref, symbol_name,
};
use ncl_sys::{NativeError, NativeOperation, OverflowSemantics, StorageCondition};

use crate::RuntimeError;

/// The typed condition category corresponding to a direct native failure.
#[derive(Debug)]
pub enum NativeCondition {
    /// An object-layer failure, such as storage exhaustion or an invalid object.
    Object(ObjectError),
    /// A Lisp condition category, such as type-error or arithmetic-error.
    Lisp(LispError),
}

pub const fn native_failure(error: NativeError) -> RuntimeError {
    let condition = match error {
        NativeError::TypeMismatch {
            operation: NativeOperation::Car,
            value,
            ..
        } => NativeCondition::Lisp(LispError::TypeError {
            datum: value,
            expected: ObjectType::Cons,
        }),
        NativeError::TypeMismatch {
            operation: NativeOperation::Add | NativeOperation::Mul,
            value,
            ..
        } => NativeCondition::Lisp(LispError::TypeError {
            datum: value,
            expected: ObjectType::Fixnum,
        }),
        NativeError::Overflow {
            semantics: OverflowSemantics::ArithmeticFixnum,
            ..
        } => NativeCondition::Lisp(LispError::ArithmeticError(
            ArithmeticError::InvalidOperation,
        )),
        NativeError::Allocation { condition, .. } => {
            NativeCondition::Object(ObjectError::Storage(condition))
        }
        NativeError::NullThread { .. } | NativeError::ThreadNotRegistered { .. } => {
            NativeCondition::Object(ObjectError::Storage(StorageCondition::ThreadNotRegistered))
        }
        NativeError::Invalid { .. } => NativeCondition::Object(ObjectError::Layout),
        NativeError::TypeMismatch { .. } => NativeCondition::Object(ObjectError::TypeError),
    };
    RuntimeError::NativeFailure { error, condition }
}

/// Map an unhandled object error to its user-facing runtime error.
pub fn pending_object_error(
    context: &mut ThreadContext,
    object: &ObjectRuntime,
    error: ObjectError,
    pending_undefined_function_condition: &mut Option<HeapRootedWord>,
) -> Result<RuntimeError, ObjectError> {
    if matches!(error, ObjectError::UndefinedFunction)
        && let Some(condition) = context.take_pending_condition()
    {
        let condition = HeapRootedWord::new(context, condition)?;
        #[allow(clippy::option_if_let_else)]
        let name = match slot_ref(context, Instance::from_word(condition.get()), 0)
            .and_then(|name| symbol_name(context, name))
            .and_then(|name| {
                ncl_compiler_front::form::word_string(context, name)
                    .map_err(|_| ObjectError::Layout)
            }) {
            Ok(name) => name,
            Err(_) => "<unknown>".to_owned(),
        };
        *pending_undefined_function_condition = Some(condition);
        return Ok(RuntimeError::UndefinedFunction { name });
    }
    if matches!(error, ObjectError::TypeError)
        && let Some(condition) = context.take_pending_condition()
    {
        let condition = HeapRootedWord::new(context, condition)?;
        let simple_error = ncl_conditions::ConditionIdentifier::SimpleError.class(context, object);
        if ncl_conditions::condition_class_of(context, condition.get()).ok() == simple_error
            && let Some(report) = ncl_conditions::condition_report(context, condition.get())
        {
            return Ok(RuntimeError::Native(report));
        }
    }
    Ok(error.into())
}

/// Create a Lisp condition that preserves a native LOAD failure message.
pub fn signal_native_load_error(
    ctx: &mut ThreadContext,
    object: &ObjectRuntime,
    message: &str,
) -> Result<(), ObjectError> {
    let format_control = make_string(ctx, object, &['~', 'A'])?;
    let format_control = HeapRootedWord::new(ctx, format_control)?;
    let message = make_string(ctx, object, &message.chars().collect::<Vec<_>>())?;
    let message = HeapRootedWord::new(ctx, message)?;
    let format_arguments = make_cons(ctx, object, message.get(), Word::NIL)?;
    let format_arguments = HeapRootedWord::new(ctx, format_arguments)?;
    ctx.set_pending_lisp_error(LispError::SimpleError {
        format_control: format_control.get(),
        format_arguments: format_arguments.get(),
    });
    Ok(())
}
#[cfg(test)]
mod tests {
    use ncl_object::{LispError, ObjectError, ObjectType};
    use ncl_sys::{NativeError, NativeOperation, OverflowSemantics, Word};

    use super::{NativeCondition, native_failure, pending_object_error, signal_native_load_error};
    use crate::{Runtime, RuntimeError};

    #[test]
    fn native_failures_are_typed_with_stress_modes_enabled() {
        let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("{error}"));
        runtime.context.set_gc_stress(true);
        runtime.context.set_strict_forwarding(true);

        let thread = runtime.context.thread_access().as_non_null();
        let returned = ncl_sys::native_add(
            thread,
            Word::fixnum(i64::MAX / 2),
            Word::fixnum(i64::MAX / 2),
        );
        assert_eq!(returned, Word::NIL);
        assert!(matches!(
            runtime.context.take_pending_runtime_errors().native,
            Some(NativeError::Overflow {
                operation: NativeOperation::Add,
                semantics: OverflowSemantics::ArithmeticFixnum,
            })
        ));
        let error = NativeError::TypeMismatch {
            operation: NativeOperation::Car,
            operand: 0,
            value: Word::fixnum(7),
        };
        let failure = native_failure(error);
        assert!(matches!(
            failure,
            RuntimeError::NativeFailure {
                error: NativeError::TypeMismatch { .. },
                condition: NativeCondition::Lisp(LispError::TypeError {
                    expected: ObjectType::Cons,
                    ..
                }),
            }
        ));

        let failure = native_failure(NativeError::Overflow {
            operation: NativeOperation::Mul,
            semantics: OverflowSemantics::ArithmeticFixnum,
        });
        assert!(matches!(
            failure,
            RuntimeError::NativeFailure {
                condition: NativeCondition::Lisp(LispError::ArithmeticError(_)),
                ..
            }
        ));
    }

    #[test]
    fn native_load_failure_preserves_its_verifier_report() {
        let mut runtime =
            Runtime::new().unwrap_or_else(|error| panic!("runtime initialization failed: {error}"));
        let message = "IR verification before optimization failed: function lambda-FunctionId(1): \
             [UndefinedValue(ValueId(4)), SuccessorType(BlockId(2))]";
        let (ctx, object) = (&mut runtime.context, &runtime.object);

        signal_native_load_error(ctx, object, message)
            .unwrap_or_else(|error| panic!("native load condition failed: {error}"));
        let error = ctx
            .take_pending_lisp_error()
            .unwrap_or_else(|| panic!("pending Lisp error missing"));
        assert!(matches!(error, LispError::SimpleError { .. }));
        let converter = object
            .lisp_error_converter()
            .unwrap_or_else(|| panic!("Lisp error converter missing"));
        let condition = converter(ctx, object, error)
            .unwrap_or_else(|error| panic!("converted condition failed: {error}"));

        assert_eq!(
            ncl_conditions::condition_report(ctx, condition).as_deref(),
            Some(message)
        );
        ctx.set_pending_condition(condition);
        let runtime_error = pending_object_error(
            ctx,
            object,
            ObjectError::TypeError,
            &mut runtime.pending_undefined_function_condition,
        )
        .unwrap_or_else(|error| panic!("mapped runtime error failed: {error}"));
        assert!(matches!(
            runtime_error,
            RuntimeError::Native(report) if report == message
        ));
    }

    #[test]
    fn unrelated_object_errors_leave_pending_conditions_untouched()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut runtime = Runtime::new()?;
        let condition = ncl_conditions::condition_from_lisp_error(
            &mut runtime.context,
            &runtime.object,
            LispError::TypeError {
                datum: Word::fixnum(7),
                expected: ObjectType::Cons,
            },
        )?;
        runtime.context.set_pending_condition(condition);

        let error = pending_object_error(
            &mut runtime.context,
            &runtime.object,
            ObjectError::Layout,
            &mut runtime.pending_undefined_function_condition,
        )?;

        assert!(matches!(error, RuntimeError::Object(ObjectError::Layout)));
        assert_eq!(runtime.context.take_pending_condition(), Some(condition));
        Ok(())
    }

    #[test]
    fn native_load_failure_reaches_the_runtime_error_report() {
        let path = std::env::temp_dir().join(format!(
            "ncl-native-load-diagnostic-{}.lisp",
            std::process::id()
        ));
        std::fs::write(&path, [0xff])
            .unwrap_or_else(|error| panic!("invalid UTF-8 source creation failed: {error}"));
        let source = format!("(load \"{}\")", path.display());
        let mut runtime =
            Runtime::new().unwrap_or_else(|error| panic!("runtime initialization failed: {error}"));

        let result = runtime.eval(&source);
        assert!(
            matches!(
                &result,
                Err(RuntimeError::Native(message))
                    if message == "source file is not valid UTF-8"
            ),
            "unexpected LOAD error: {result:?}"
        );

        std::fs::remove_file(path)
            .unwrap_or_else(|error| panic!("invalid UTF-8 source cleanup failed: {error}"));
    }
}
