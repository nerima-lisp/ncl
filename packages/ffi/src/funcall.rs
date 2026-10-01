//! Foreign routine declaration and invocation.

#![allow(clippy::indexing_slicing, clippy::missing_const_for_fn)]

use ncl_object::{Runtime, ThreadContext, Word};

use crate::FfiError;
use crate::alien::{AlienRoutine, AlienType, marshal_argument, size_of};
use crate::sap::SystemAreaPointer;

/// Declare a foreign routine, like `define-alien-routine`.
#[must_use]
pub fn alien_routine(
    name: &str,
    arguments: Vec<AlienType>,
    result: AlienType,
    callable: bool,
) -> AlienRoutine {
    AlienRoutine::new(name, arguments, result, callable)
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
/// The Phase-1 call path supports up to eight integer, pointer, or enumeration
/// arguments and a scalar integer or pointer result. Floating-point arguments,
/// aggregate values, and callbacks remain explicit unsupported cases.
///
/// # Errors
/// Returns [`FfiError::ArityMismatch`] when the argument count differs from the
/// declaration, any marshalling error from the arguments, and an explicit
/// unsupported-type error for an ABI outside this Phase-1 subset.
pub fn alien_funcall(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    routine: &AlienRoutine,
    arguments: &[Word],
) -> Result<Word, FfiError> {
    if arguments.len() != routine.arguments().len() {
        return Err(FfiError::ArityMismatch {
            expected: routine.arguments().len(),
            got: arguments.len(),
        });
    }
    if routine.arguments().iter().any(|ty| !scalar_call_type(ty))
        || !scalar_call_type(routine.result())
    {
        return Err(FfiError::UnsupportedType("foreign call ABI"));
    }
    let mut buffer = Vec::with_capacity(routine.arguments().len() * 8);
    for (ty, value) in routine.arguments().iter().zip(arguments) {
        let bytes = marshal_argument(ctx, ty, *value)?;
        let mut slot = [0_u8; 8];
        let sign_extend = matches!(
            ty,
            AlienType::Char
                | AlienType::Short
                | AlienType::Int
                | AlienType::Long
                | AlienType::LongLong
                | AlienType::SSizeT
        );
        if sign_extend && bytes.last().is_some_and(|byte| byte & 0x80 != 0) {
            slot.fill(0xff);
        }
        slot[..bytes.len()].copy_from_slice(&bytes);
        buffer.extend(slot);
    }
    if routine.address() == 0 {
        return Err(FfiError::NullPointer);
    }
    let mut result = vec![0_u8; size_of(routine.result())];
    ncl_sys::ffi::call_foreign_function(routine.address(), &buffer, &mut result)
        .map_err(FfiError::ForeignCall)?;
    crate::alien::unmarshal_result(ctx, runtime, routine.result(), &result)
}

fn scalar_call_type(ty: &AlienType) -> bool {
    matches!(
        ty,
        AlienType::Boolean
            | AlienType::Char
            | AlienType::UnsignedChar
            | AlienType::Short
            | AlienType::UnsignedShort
            | AlienType::Int
            | AlienType::UnsignedInt
            | AlienType::Long
            | AlienType::UnsignedLong
            | AlienType::LongLong
            | AlienType::UnsignedLongLong
            | AlienType::SizeT
            | AlienType::SSizeT
            | AlienType::Pointer(_)
            | AlienType::CString
            | AlienType::Utf8String
            | AlienType::SystemAreaPointer
            | AlienType::Function(_)
            | AlienType::Enumeration(_)
            | AlienType::Void
    )
}
