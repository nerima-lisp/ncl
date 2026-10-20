//! Tracked memory and typed access to foreign libraries.

mod call;
#[cfg(all(target_arch = "aarch64", target_family = "unix"))]
#[path = "foreign/call_aarch64.rs"]
mod call_aarch64;
#[cfg(all(target_arch = "x86_64", target_family = "unix"))]
#[path = "foreign/call_x86_64.rs"]
mod call_x86_64;
mod loader;
mod memory;

#[cfg(test)]
#[cfg(all(
    target_family = "unix",
    any(target_arch = "x86_64", target_arch = "aarch64"),
    any(target_os = "linux", target_os = "macos")
))]
#[path = "foreign/tests_call.rs"]
mod call_tests;
#[cfg(test)]
mod memory_tests;
#[cfg(test)]
mod tests;

pub use call::{
    ForeignCallError, ForeignFunction, ForeignSignature, ForeignType, ForeignValue, call,
};
pub use loader::{ForeignLibrary, take_loader_error};
pub use memory::{ForeignRegion, move_sap};

/// A checked address represented independently of a Rust pointer.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SapAddress(u64);

impl SapAddress {
    /// Construct an address if it fits the current platform's pointer width.
    ///
    /// A null address is representable, but cannot be dereferenced.
    ///
    /// # Errors
    ///
    /// Returns [`ForeignError::AddressOutOfRange`] if the value does not fit the
    /// platform pointer width.
    pub fn new(address: u64) -> Result<Self, ForeignError> {
        usize::try_from(address).map_err(|_| ForeignError::AddressOutOfRange)?;
        Ok(Self(address))
    }

    /// Return the integer representation of this address.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Add a checked byte offset to this address.
    ///
    /// # Errors
    ///
    /// Returns [`ForeignError::AddressOverflow`] if the addition overflows or the
    /// resulting address does not fit the platform pointer width.
    pub fn checked_add(self, offset: ByteOffset) -> Result<Self, ForeignError> {
        let address = self
            .0
            .checked_add(offset.0)
            .ok_or(ForeignError::AddressOverflow)?;
        Self::new(address)
    }
}

/// A byte count used for foreign memory regions.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ByteLength(u64);

impl ByteLength {
    /// Create a byte count for an allocation or access.
    #[must_use]
    pub const fn new(bytes: u64) -> Self {
        Self(bytes)
    }

    /// Return the count in bytes.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// A byte offset used for SAP arithmetic.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ByteOffset(u64);

impl ByteOffset {
    /// Create a nonnegative offset from an address base.
    #[must_use]
    pub const fn new(bytes: u64) -> Self {
        Self(bytes)
    }

    /// Return the offset in bytes.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// A valid power-of-two allocation alignment.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Alignment(u64);

impl Alignment {
    /// The minimum byte alignment.
    pub const ONE: Self = Self(1);

    /// Create a nonzero, power-of-two alignment supported by this platform.
    ///
    /// # Errors
    ///
    /// Returns [`ForeignError::InvalidAlignment`] if the value is zero, is not a
    /// power of two, or does not fit the platform's `usize`.
    pub fn new(bytes: u64) -> Result<Self, ForeignError> {
        let native = usize::try_from(bytes).map_err(|_| ForeignError::InvalidAlignment)?;
        if native == 0 || !native.is_power_of_two() {
            return Err(ForeignError::InvalidAlignment);
        }
        Ok(Self(bytes))
    }

    /// Return the alignment in bytes.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    pub(super) fn as_usize(self) -> Result<usize, ForeignError> {
        usize::try_from(self.0).map_err(|_| ForeignError::InvalidAlignment)
    }
}

/// The supported width of one foreign memory access.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum AccessWidth {
    /// One byte.
    U8,
    /// Two bytes.
    U16,
    /// Four bytes.
    U32,
    /// Eight bytes.
    U64,
}

impl AccessWidth {
    /// Return the number of bytes accessed.
    #[must_use]
    pub const fn byte_length(self) -> ByteLength {
        ByteLength(self.bytes())
    }

    pub(super) const fn bytes(self) -> u64 {
        match self {
            Self::U8 => 1,
            Self::U16 => 2,
            Self::U32 => 4,
            Self::U64 => 8,
        }
    }
}

/// A typed failure from a foreign-memory, loader, or call operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ForeignError {
    /// The requested address cannot fit in the platform pointer width.
    AddressOutOfRange,
    /// Address arithmetic overflowed.
    AddressOverflow,
    /// A null address was used for a memory access.
    NullAddress,
    /// No currently tracked region contains the address.
    UnknownRegion,
    /// The requested access extends past its tracked region.
    OutOfBounds,
    /// The address does not meet the requested access alignment.
    Misaligned,
    /// A value does not fit the requested access width.
    ValueOutOfRange,
    /// An allocation cannot have zero bytes.
    EmptyRegion,
    /// The requested alignment is invalid for this platform.
    InvalidAlignment,
    /// The native allocator could not allocate the requested region.
    AllocationFailed,
    /// The requested byte length cannot fit the platform allocation size.
    LengthOutOfRange,
    /// The size and alignment cannot form a valid platform allocation layout.
    InvalidLayout,
    /// The region registry could not reserve an entry.
    RegistryFull,
    /// The region registry was poisoned by a prior panic.
    RegistryPoisoned,
    /// The region registry identifier space is exhausted.
    RegionIdExhausted,
    /// A shared-object path must be absolute and contain no NUL byte.
    InvalidLibraryPath,
    /// The exact shared-object path could not be loaded.
    LibraryLoadFailed,
    /// The requested symbol could not be resolved.
    SymbolNotFound,
    /// The requested symbol resolved without a loader error but has a null address.
    NullFunctionAddress,
    /// This platform does not support the requested operation.
    UnsupportedPlatform,
    /// The signature requires an unsupported calling convention or ABI shape.
    UnsupportedAbi,
    /// The argument count differs from the signature.
    ArgumentCountMismatch,
    /// An argument value does not match its declared type.
    ArgumentTypeMismatch,
    /// The signature contains a void parameter or another invalid shape.
    InvalidSignature,
}

impl core::fmt::Display for ForeignError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let message = match self {
            Self::AddressOutOfRange => "foreign address exceeds platform pointer width",
            Self::AddressOverflow => "foreign address arithmetic overflowed",
            Self::NullAddress => "null address cannot be dereferenced",
            Self::UnknownRegion => "foreign address is not in a tracked region",
            Self::OutOfBounds => "foreign access exceeds its tracked region",
            Self::Misaligned => "foreign address does not meet access alignment",
            Self::ValueOutOfRange => "value does not fit the foreign access width",
            Self::EmptyRegion => "foreign memory region cannot be empty",
            Self::InvalidAlignment => "foreign memory alignment is invalid",
            Self::AllocationFailed => "foreign memory allocation failed",
            Self::LengthOutOfRange => "foreign memory length exceeds platform limits",
            Self::InvalidLayout => "foreign size and alignment do not form a valid layout",
            Self::RegistryFull => "foreign memory registry allocation failed",
            Self::RegistryPoisoned => "foreign memory registry is poisoned",
            Self::RegionIdExhausted => "foreign memory registry identifiers are exhausted",
            Self::InvalidLibraryPath => "foreign library path must be absolute and contain no NUL",
            Self::LibraryLoadFailed => "foreign library could not be loaded from the exact path",
            Self::SymbolNotFound => "foreign symbol could not be resolved",
            Self::NullFunctionAddress => "foreign symbol resolved to a null function address",
            Self::UnsupportedPlatform => "foreign operation is unsupported on this platform",
            Self::UnsupportedAbi => "foreign signature is unsupported by this ABI",
            Self::ArgumentCountMismatch => "foreign argument count does not match the signature",
            Self::ArgumentTypeMismatch => "foreign argument type does not match the signature",
            Self::InvalidSignature => "foreign signature contains an invalid type shape",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for ForeignError {}

/// Read a value only when its address belongs to a live tracked region.
///
/// # Errors
///
/// Returns an error if the address is null, untracked, out of bounds, or misaligned,
/// or if the region registry is unavailable.
pub fn read_sap(address: SapAddress, width: AccessWidth) -> Result<u64, ForeignError> {
    memory::read_sap(address, width)
}

/// Write a value only when its address belongs to a live tracked region.
///
/// # Errors
///
/// Returns an error if the address is null, untracked, out of bounds, or misaligned,
/// the value does not fit the requested width, or the region registry is unavailable.
pub fn write_sap(address: SapAddress, width: AccessWidth, value: u64) -> Result<(), ForeignError> {
    memory::write_sap(address, width, value)
}
