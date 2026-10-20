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
    ///
    /// Modeled in signatures, but rejected by the current register call backend.
    F32,
    /// Double-precision floating-point value.
    ///
    /// Modeled in signatures, but rejected by the current register call backend.
    F64,
    /// Native pointer value; SAP access still requires an active tracked region.
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
    /// Native pointer value; SAP access still requires an active tracked region.
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

/// A scalar calling signature supplied for one call.
///
/// The Lisp foreign-call caller must choose a function export and supply its exact
/// ABI, including pointer bounds and lifetime rules, and must not unwind across the
/// C boundary. The checked call validates value tags and supported register shapes,
/// but cannot verify that contract. The caller must also synchronize C access to
/// tracked memory with concurrent SAP access and ensure the function supports its
/// calling thread. The `ncl-ffi` adapter can use this safe API.
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
}

impl ForeignSignature {
    /// Create a fixed-argument signature.
    /// The C return type represented by this signature.
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
    /// A SAP address or pointer failed tracked-region validation.
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

/// Call a resolved function using a caller-supplied integer or pointer ABI signature.
///
/// Float, variadic, aggregate, and stack-passed signatures return
/// [`ForeignCallError::UnsupportedAbi`].
///
/// # Errors
///
/// Returns an error if the signature is unsupported, the argument count or types do
/// not match, or tracked pointer validation fails.
pub fn call(
    function: &ForeignFunction<'_>,
    signature: &ForeignSignature,
    arguments: &[ForeignValue],
) -> Result<ForeignValue, ForeignCallError> {
    if signature.call_kind == CallKind::Variadic {
        return Err(ForeignCallError::UnsupportedAbi);
    }
    if is_float_type(signature.result) || signature.arguments.iter().copied().any(is_float_type) {
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
    let maximum = maximum_register_arguments();
    if arguments.len() > maximum {
        return Err(ForeignCallError::UnsupportedAbi);
    }
    let mut raw_arguments = [0_u64; 8];
    let mut pointers = [None; 8];
    for ((raw_slot, pointer_slot), argument) in raw_arguments
        .iter_mut()
        .zip(pointers.iter_mut())
        .zip(arguments.iter().copied())
    {
        let (raw, pointer) = argument.encode();
        *raw_slot = raw;
        *pointer_slot = pointer;
    }
    let pointers = pointers
        .get(..arguments.len())
        .ok_or(ForeignCallError::UnsupportedAbi)?;
    let raw_arguments = raw_arguments
        .get(..arguments.len())
        .ok_or(ForeignCallError::UnsupportedAbi)?;
    memory::call_with_tracked_inputs(pointers, || {
        let raw = invoke(function.address, raw_arguments)?;
        decode_result(signature.result, raw)
    })
    .map_err(|error| match error {
        ForeignError::UnsupportedAbi => ForeignCallError::UnsupportedAbi,
        other => ForeignCallError::Memory(other),
    })
}

fn decode_result(result_type: ForeignType, raw: u64) -> Result<ForeignValue, ForeignError> {
    match result_type {
        ForeignType::Void => Ok(ForeignValue::Void),
        ForeignType::I8 => {
            let bits = low_u8(raw)?;
            Ok(ForeignValue::I8(i8::from_ne_bytes([bits])))
        }
        ForeignType::U8 => Ok(ForeignValue::U8(low_u8(raw)?)),
        ForeignType::I16 => {
            let bits = low_u16(raw)?;
            Ok(ForeignValue::I16(i16::from_ne_bytes(bits.to_ne_bytes())))
        }
        ForeignType::U16 => Ok(ForeignValue::U16(low_u16(raw)?)),
        ForeignType::I32 => {
            let bits = low_u32(raw)?;
            Ok(ForeignValue::I32(i32::from_ne_bytes(bits.to_ne_bytes())))
        }
        ForeignType::U32 => Ok(ForeignValue::U32(low_u32(raw)?)),
        ForeignType::I64 => Ok(ForeignValue::I64(i64::from_ne_bytes(raw.to_ne_bytes()))),
        ForeignType::U64 => Ok(ForeignValue::U64(raw)),
        ForeignType::F32 => Ok(ForeignValue::F32(f32::from_bits(low_u32(raw)?))),
        ForeignType::F64 => Ok(ForeignValue::F64(f64::from_bits(raw))),
        ForeignType::Pointer => SapAddress::new(raw).map(ForeignValue::Pointer),
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

const fn is_float_type(foreign_type: ForeignType) -> bool {
    matches!(foreign_type, ForeignType::F32 | ForeignType::F64)
}

const fn maximum_register_arguments() -> usize {
    #[cfg(all(target_arch = "x86_64", target_family = "unix"))]
    {
        6
    }
    #[cfg(all(target_arch = "aarch64", target_family = "unix"))]
    {
        8
    }
    #[cfg(not(any(
        all(target_arch = "x86_64", target_family = "unix"),
        all(target_arch = "aarch64", target_family = "unix")
    )))]
    {
        0
    }
}

fn invoke(function: NonNull<c_void>, arguments: &[u64]) -> Result<u64, ForeignError> {
    #[cfg(all(target_arch = "x86_64", target_family = "unix"))]
    {
        let mut registers = [0_u64; 6];
        if arguments.len() > registers.len() {
            return Err(ForeignError::UnsupportedAbi);
        }
        for (register, argument) in registers.iter_mut().zip(arguments.iter().copied()) {
            *register = argument;
        }
        // SAFETY: the call API limits this to six integer/pointer arguments and the
        // Lisp foreign-call caller supplies the export's matching System V ABI.
        Ok(unsafe { super::call_x86_64::invoke(function.as_ptr(), registers) })
    }
    #[cfg(all(target_arch = "aarch64", target_family = "unix"))]
    {
        let mut registers = [0_u64; 8];
        if arguments.len() > registers.len() {
            return Err(ForeignError::UnsupportedAbi);
        }
        for (register, argument) in registers.iter_mut().zip(arguments.iter().copied()) {
            *register = argument;
        }
        // SAFETY: the call API limits this to eight integer/pointer arguments and the
        // Lisp foreign-call caller supplies the export's matching AAPCS64 ABI.
        Ok(unsafe { super::call_aarch64::invoke(function.as_ptr(), registers) })
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
