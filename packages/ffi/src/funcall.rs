//! Foreign routine declaration and invocation.

use ncl_object::{Runtime, ThreadContext, Word};

use crate::FfiError;
use crate::alien::{AlienRoutine, AlienType, marshal_argument, size_of};
use crate::sap::SystemAreaPointer;
use crate::sys_requirements::CALL_FOREIGN_FUNCTION;

/// Declare a foreign routine, like `define-alien-routine`.
#[must_use]
pub fn alien_routine(
    name: &str,
    arguments: Vec<AlienType>,
    result: AlienType,
    callable: bool,
) -> AlienRoutine {
    AlienRoutine {
        name: name.to_owned(),
        arguments,
        result,
        callable,
    }
}

/// The size in bytes of `ty`, like `alien-size`.
#[must_use]
pub fn alien_size(ty: &AlienType) -> usize {
    size_of(ty)
}

/// The null alien value, like `null-alien`.
#[must_use]
pub const fn null_alien() -> Word {
    Word::NIL
}

/// The SAP of an alien value, like `alien-sap`.
///
/// # Errors
/// Returns [`FfiError::TypeMismatch`] when `value` is not a SAP or an integer
/// address.
pub fn alien_sap(value: Word) -> Result<SystemAreaPointer, FfiError> {
    SystemAreaPointer::from_word(value)
}

/// Reinterpret an alien address as another type, like `cast`.
#[must_use]
pub const fn cast(_ty: &AlienType, sap: SystemAreaPointer) -> SystemAreaPointer {
    sap
}

/// Call a foreign routine, marshalling the arguments and the result.
///
/// Every argument is marshalled before the call, so an arity or type error is
/// reported without touching foreign code. The call itself returns
/// [`FfiError::MissingSysPrimitive`] because `ncl-sys` has no C
/// function-pointer call primitive: `invoke_entry` enters NCL generated code
/// with the NCL register convention, not a C function with the C ABI.
///
/// # Errors
/// Returns [`FfiError::ArityMismatch`] when the argument count differs from the
/// declaration, any marshalling error from the arguments, and
/// [`FfiError::MissingSysPrimitive`] for the call itself.
pub fn alien_funcall(
    ctx: &ThreadContext,
    _runtime: &Runtime,
    routine: &AlienRoutine,
    arguments: &[Word],
) -> Result<Word, FfiError> {
    if arguments.len() != routine.arguments.len() {
        return Err(FfiError::ArityMismatch {
            expected: routine.arguments.len(),
            got: arguments.len(),
        });
    }
    let mut buffer = Vec::new();
    for (ty, value) in routine.arguments.iter().zip(arguments) {
        buffer.extend(marshal_argument(ctx, ty, *value)?);
    }
    let _ = buffer.len();
    Err(FfiError::MissingSysPrimitive(CALL_FOREIGN_FUNCTION))
}
