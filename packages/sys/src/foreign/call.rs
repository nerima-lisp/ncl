use super::{ForeignError, SapAddress, loader::ForeignLibrary, memory};
use core::ffi::c_void;
use core::marker::PhantomData;
use core::ptr::NonNull;

/// A scalar or pointer type in a foreign signature.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ForeignType {
    /// No return value. This type cannot be used for an argument.
    Void,
    /// Signed eight-bit integer.
    I8,
    /// Unsigned eight-bit integer.
    U8,
    /// Signed sixteen-bit integer.
    I16,
    /// Unsigned sixteen-bit integer.
    U16,
    /// Signed thirty-two-bit integer.
    I32,
    /// Unsigned thirty-two-bit integer.
    U32,
    /// Signed sixty-four-bit integer.
    I64,
    /// Unsigned sixty-four-bit integer.
    U64,
    /// Single-precision floating-point value.
    F32,
    /// Double-precision floating-point value.
    F64,
    /// Native pointer type. A system-area pointer (SAP) can be read or written only
    /// when the complete access range, including its width, belongs to a live
    /// `ForeignRegion`. A foreign call may pass an untracked pointer when the caller
    /// meets `call`'s safety requirements.
    Pointer,
}

/// A scalar or pointer value passed to or returned from a foreign function.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ForeignValue {
    /// No return value.
    Void,
    /// Signed eight-bit integer.
    I8(i8),
    /// Unsigned eight-bit integer.
    U8(u8),
    /// Signed sixteen-bit integer.
    I16(i16),
    /// Unsigned sixteen-bit integer.
    U16(u16),
    /// Signed thirty-two-bit integer.
    I32(i32),
    /// Unsigned thirty-two-bit integer.
    U32(u32),
    /// Signed sixty-four-bit integer.
    I64(i64),
    /// Unsigned sixty-four-bit integer.
    U64(u64),
    /// Single-precision floating-point value.
    F32(f32),
    /// Double-precision floating-point value.
    F64(f64),
    /// Native pointer value. Reads and writes through this address require the
    /// complete access range, including its width, to belong to a live
    /// `ForeignRegion`. A foreign call may pass it untracked when the caller meets
    /// `call`'s safety requirements.
    Pointer(SapAddress),
}

impl ForeignValue {
    const fn foreign_type(self) -> ForeignType {
        match self {
            Self::Void => ForeignType::Void,
            Self::I8(_) => ForeignType::I8,
            Self::U8(_) => ForeignType::U8,
            Self::I16(_) => ForeignType::I16,
            Self::U16(_) => ForeignType::U16,
            Self::I32(_) => ForeignType::I32,
            Self::U32(_) => ForeignType::U32,
            Self::I64(_) => ForeignType::I64,
            Self::U64(_) => ForeignType::U64,
            Self::F32(_) => ForeignType::F32,
            Self::F64(_) => ForeignType::F64,
            Self::Pointer(_) => ForeignType::Pointer,
        }
    }

    fn encode(self) -> (u64, Option<SapAddress>) {
        match self {
            Self::Void => (0, None),
            Self::I8(value) => (u64::from(u8::from_ne_bytes(value.to_ne_bytes())), None),
            Self::U8(value) => (u64::from(value), None),
            Self::I16(value) => (u64::from(u16::from_ne_bytes(value.to_ne_bytes())), None),
            Self::U16(value) => (u64::from(value), None),
            Self::I32(value) => (u64::from(u32::from_ne_bytes(value.to_ne_bytes())), None),
            Self::U32(value) => (u64::from(value), None),
            Self::I64(value) => (u64::from_ne_bytes(value.to_ne_bytes()), None),
            Self::U64(value) => (value, None),
            Self::F32(value) => (u64::from(value.to_bits()), None),
            Self::F64(value) => (value.to_bits(), None),
            Self::Pointer(address) => (address.get(), Some(address)),
        }
    }
}

/// A description of a foreign call's result and argument types.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForeignSignature {
    result: ForeignType,
    arguments: Box<[ForeignType]>,
    call_kind: CallKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CallKind {
    Fixed,
    Variadic,
    Aggregate,
}

impl ForeignSignature {
    /// Create a fixed-argument signature with the given result and argument types.
    #[must_use]
    pub fn new(result: ForeignType, arguments: impl Into<Box<[ForeignType]>>) -> Self {
        Self {
            result,
            arguments: arguments.into(),
            call_kind: CallKind::Fixed,
        }
    }

    /// Create a variadic signature, which calls reject with [`ForeignCallError::UnsupportedAbi`].
    /// The fixed positional argument types, in call order.
    #[must_use]
    pub fn variadic(result: ForeignType, arguments: impl Into<Box<[ForeignType]>>) -> Self {
        Self {
            result,
            arguments: arguments.into(),
            call_kind: CallKind::Variadic,
        }
    }

    /// Mark an ABI shape with an aggregate argument or result; calls reject it with
    /// [`ForeignCallError::UnsupportedAbi`].
    ///
    /// This marker carries no aggregate layout.
    #[must_use]
    pub fn aggregate(result: ForeignType, arguments: impl Into<Box<[ForeignType]>>) -> Self {
        Self {
            result,
            arguments: arguments.into(),
            call_kind: CallKind::Aggregate,
        }
    }

    /// Return the C result type represented by this signature.
    #[must_use]
    pub const fn result(&self) -> ForeignType {
        self.result
    }

    /// Return the declared argument types in call order.
    #[must_use]
    pub fn arguments(&self) -> &[ForeignType] {
        &self.arguments
    }
}

/// A resolved function handle that retains its library lifetime.
#[derive(Clone, Debug)]
pub struct ForeignFunction<'library> {
    address: NonNull<c_void>,
    _library: PhantomData<&'library ForeignLibrary>,
}

impl<'library> ForeignFunction<'library> {
    pub(super) const fn from_address(
        address: NonNull<c_void>,
        _library: &'library ForeignLibrary,
    ) -> Self {
        Self {
            address,
            _library: PhantomData,
        }
    }
}

#[cfg(test)]
impl ForeignFunction<'static> {
    pub(super) const fn from_test_address(address: NonNull<c_void>) -> Self {
        Self {
            address,
            _library: PhantomData,
        }
    }
}

/// A typed error from validating or making a foreign call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ForeignCallError {
    /// The requested operation or ABI shape is unsupported.
    UnsupportedAbi,
    /// The argument count differs from the supplied signature.
    ArgumentCountMismatch,
    /// A value's type differs from the supplied signature.
    ArgumentTypeMismatch,
    /// The signature contains `Void` as an argument.
    InvalidSignature,
    /// A returned address is out of range, or a tracked-region registry operation failed.
    Memory(ForeignError),
}

impl core::fmt::Display for ForeignCallError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnsupportedAbi => formatter.write_str("foreign call ABI is unsupported"),
            Self::ArgumentCountMismatch => {
                formatter.write_str("foreign argument count does not match the supplied signature")
            }
            Self::ArgumentTypeMismatch => {
                formatter.write_str("foreign argument type does not match the supplied signature")
            }
            Self::InvalidSignature => {
                formatter.write_str("foreign signature contains an invalid argument type")
            }
            Self::Memory(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ForeignCallError {}

/// Call a resolved function using a caller-supplied scalar or pointer ABI signature.
///
/// Fixed scalar and pointer calls are supported on Unix targets with the `x86_64`
/// or `AArch64` architecture.
/// Other targets, variadic signatures, and aggregate signatures return
/// [`ForeignCallError::UnsupportedAbi`].
///
/// # Safety
///
/// If this function invokes `function`, its actual C ABI and parameter and return
/// types must match `signature`. Every pointer argument must be valid for the
/// accesses the target performs, and must remain valid for as long as the target
/// uses it. The target must not unwind across the call boundary. The caller must
/// ensure the target is safe to invoke on the current thread and synchronize its
/// memory access with Rust and foreign code so the call creates no data races or
/// aliasing violations. For each non-null pointer inside a registered region, the
/// call retains that region's allocation until the call returns. This does not
/// validate an untracked pointer or keep untracked storage alive.
///
/// # Errors
///
/// Returns [`ForeignCallError::UnsupportedAbi`] for an unsupported signature or
/// target, [`ForeignCallError::InvalidSignature`] if the signature uses `Void` as
/// an argument, [`ForeignCallError::ArgumentCountMismatch`] or
/// [`ForeignCallError::ArgumentTypeMismatch`] if arguments do not match, or
/// [`ForeignCallError::Memory`] if a returned address is out of range or a
/// tracked-region registry operation fails.
pub unsafe fn call(
    function: &ForeignFunction<'_>,
    signature: &ForeignSignature,
    arguments: &[ForeignValue],
) -> Result<ForeignValue, ForeignCallError> {
    if matches!(
        signature.call_kind,
        CallKind::Variadic | CallKind::Aggregate
    ) {
        return Err(ForeignCallError::UnsupportedAbi);
    }
    if signature.arguments.len() != arguments.len() {
        return Err(ForeignCallError::ArgumentCountMismatch);
    }
    if signature.arguments.contains(&ForeignType::Void) {
        return Err(ForeignCallError::InvalidSignature);
    }
    if arguments
        .iter()
        .zip(signature.arguments.iter())
        .any(|(value, expected)| value.foreign_type() != *expected)
    {
        return Err(ForeignCallError::ArgumentTypeMismatch);
    }
    let (marshalled, pointers) = marshal_arguments(arguments)?;
    memory::call_with_tracked_inputs(&pointers, || {
        let (integer_result, float_result) = invoke(function.address, &marshalled)?;
        decode_result(signature.result, integer_result, float_result)
    })
    .map_err(|error| match error {
        ForeignError::UnsupportedAbi => ForeignCallError::UnsupportedAbi,
        other => ForeignCallError::Memory(other),
    })
}

#[derive(Debug)]
struct MarshalledArguments {
    general: [u64; 8],
    floating: [u64; 8],
    stack: Vec<u64>,
    stack_area_bytes: usize,
}

fn marshal_arguments(
    arguments: &[ForeignValue],
) -> Result<(MarshalledArguments, Vec<Option<SapAddress>>), ForeignCallError> {
    let (general_limit, floating_limit) = argument_register_limits();
    let mut general = [0_u64; 8];
    let mut floating = [0_u64; 8];
    let mut general_count = 0;
    let mut floating_count = 0;
    let mut stack = Vec::new();
    let mut pointers = Vec::new();
    stack
        .try_reserve(arguments.len())
        .map_err(|_| ForeignCallError::UnsupportedAbi)?;
    pointers
        .try_reserve(arguments.len())
        .map_err(|_| ForeignCallError::UnsupportedAbi)?;

    for argument in arguments.iter().copied() {
        let (encoded, pointer) = argument.encode();
        if let Some(address) = pointer {
            pointers.push(Some(address));
        }
        match argument.foreign_type() {
            ForeignType::F32 | ForeignType::F64 => {
                if floating_count < floating_limit {
                    let slot = floating
                        .get_mut(floating_count)
                        .ok_or(ForeignCallError::UnsupportedAbi)?;
                    *slot = encoded;
                    floating_count += 1;
                } else {
                    stack.push(encoded);
                }
            }
            ForeignType::Void => return Err(ForeignCallError::UnsupportedAbi),
            ForeignType::I8
            | ForeignType::U8
            | ForeignType::I16
            | ForeignType::U16
            | ForeignType::I32
            | ForeignType::U32
            | ForeignType::I64
            | ForeignType::U64
            | ForeignType::Pointer => {
                if general_count < general_limit {
                    let slot = general
                        .get_mut(general_count)
                        .ok_or(ForeignCallError::UnsupportedAbi)?;
                    *slot = encoded;
                    general_count += 1;
                } else {
                    stack.push(encoded);
                }
            }
        }
    }

    let stack_bytes = stack
        .len()
        .checked_mul(core::mem::size_of::<u64>())
        .ok_or(ForeignCallError::UnsupportedAbi)?;
    let stack_area_bytes = stack_bytes
        .checked_add(15)
        .map(|bytes| bytes & !15)
        .ok_or(ForeignCallError::UnsupportedAbi)?;
    Ok((
        MarshalledArguments {
            general,
            floating,
            stack,
            stack_area_bytes,
        },
        pointers,
    ))
}

fn decode_result(
    result_type: ForeignType,
    integer_raw: u64,
    float_raw: u64,
) -> Result<ForeignValue, ForeignError> {
    match result_type {
        ForeignType::Void => Ok(ForeignValue::Void),
        ForeignType::I8 => {
            let bits = low_u8(integer_raw)?;
            Ok(ForeignValue::I8(i8::from_ne_bytes([bits])))
        }
        ForeignType::U8 => Ok(ForeignValue::U8(low_u8(integer_raw)?)),
        ForeignType::I16 => {
            let bits = low_u16(integer_raw)?;
            Ok(ForeignValue::I16(i16::from_ne_bytes(bits.to_ne_bytes())))
        }
        ForeignType::U16 => Ok(ForeignValue::U16(low_u16(integer_raw)?)),
        ForeignType::I32 => {
            let bits = low_u32(integer_raw)?;
            Ok(ForeignValue::I32(i32::from_ne_bytes(bits.to_ne_bytes())))
        }
        ForeignType::U32 => Ok(ForeignValue::U32(low_u32(integer_raw)?)),
        ForeignType::I64 => Ok(ForeignValue::I64(i64::from_ne_bytes(
            integer_raw.to_ne_bytes(),
        ))),
        ForeignType::U64 => Ok(ForeignValue::U64(integer_raw)),
        ForeignType::F32 => Ok(ForeignValue::F32(f32::from_bits(low_u32(float_raw)?))),
        ForeignType::F64 => Ok(ForeignValue::F64(f64::from_bits(float_raw))),
        ForeignType::Pointer => SapAddress::new(integer_raw).map(ForeignValue::Pointer),
    }
}

fn low_u8(raw: u64) -> Result<u8, ForeignError> {
    u8::try_from(raw & u64::from(u8::MAX)).map_err(|_| ForeignError::ValueOutOfRange)
}

fn low_u16(raw: u64) -> Result<u16, ForeignError> {
    u16::try_from(raw & u64::from(u16::MAX)).map_err(|_| ForeignError::ValueOutOfRange)
}

fn low_u32(raw: u64) -> Result<u32, ForeignError> {
    u32::try_from(raw & u64::from(u32::MAX)).map_err(|_| ForeignError::ValueOutOfRange)
}

const fn argument_register_limits() -> (usize, usize) {
    #[cfg(all(target_arch = "x86_64", target_family = "unix"))]
    {
        (6, 8)
    }
    #[cfg(all(target_arch = "aarch64", target_family = "unix"))]
    {
        (8, 8)
    }
    #[cfg(not(any(
        all(target_arch = "x86_64", target_family = "unix"),
        all(target_arch = "aarch64", target_family = "unix")
    )))]
    {
        (0, 0)
    }
}

#[allow(
    clippy::unnecessary_wraps,
    reason = "the unsupported-target configuration returns an ABI error"
)]
fn invoke(
    function: NonNull<c_void>,
    arguments: &MarshalledArguments,
) -> Result<(u64, u64), ForeignError> {
    #[cfg(all(target_arch = "x86_64", target_family = "unix"))]
    {
        // SAFETY: the caller contract guarantees this function's System V ABI, and
        // the marshalled registers and overflow stack arguments follow that ABI.
        Ok(unsafe {
            super::call_x86_64::invoke(
                function.as_ptr(),
                &arguments.general,
                &arguments.floating,
                &arguments.stack,
                arguments.stack_area_bytes,
            )
        })
    }
    #[cfg(all(target_arch = "aarch64", target_family = "unix"))]
    {
        // SAFETY: the caller contract guarantees this function's AAPCS64 ABI, and
        // the marshalled registers and overflow stack arguments follow that ABI.
        Ok(unsafe {
            super::call_aarch64::invoke(
                function.as_ptr(),
                &arguments.general,
                &arguments.floating,
                &arguments.stack,
                arguments.stack_area_bytes,
            )
        })
    }
    #[cfg(not(any(
        all(target_arch = "x86_64", target_family = "unix"),
        all(target_arch = "aarch64", target_family = "unix")
    )))]
    {
        let _ = (function, arguments);
        Err(ForeignError::UnsupportedAbi)
    }
}
