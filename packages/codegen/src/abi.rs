//! Stable physical register and runtime callback contracts.

/// Register identifiers frozen by the native calling convention.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u16)]
pub enum RegisterId {
    /// Return value register.
    Rax = 0,
    /// Multiple-value count register.
    Rdx = 1,
    /// Argument count register.
    Rdi = 2,
    /// First argument register.
    Rsi = 3,
    /// Second argument register.
    Rcx = 4,
    /// Third argument register.
    R8 = 5,
    /// Rest argument register.
    R9 = 6,
    /// First scratch register.
    R10 = 7,
    /// Second scratch register.
    R11 = 8,
    /// Frame pointer.
    Rbp = 9,
    /// Callee-saved register.
    Rbx = 10,
    /// Callee-saved register.
    R12 = 11,
    /// Callee-saved register.
    R13 = 12,
    /// Callee-saved register.
    R14 = 13,
    /// Pinned `ThreadContext` register.
    R15 = 14,
}

/// A runtime-owned field consumed by generated code.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ContextField {
    /// TLAB bump pointer.
    TlabBump,
    /// TLAB limit pointer.
    TlabLimit,
    /// Cooperative safepoint request bits.
    SafepointRequest,
    /// Pending condition flag.
    Pending,
    /// Multiple-value count.
    MultipleValueCount,
    /// Multiple-value word area.
    MultipleValueArea,
    /// Current handler record.
    Handler,
    /// Current cleanup record.
    Cleanup,
    /// Current catch record.
    Catch,
}

impl ContextField {
    /// Returns the legacy identifier used by existing runtime providers.
    #[must_use]
    pub const fn identifier(self) -> &'static str {
        match self {
            Self::TlabBump => "tlab_bump",
            Self::TlabLimit => "tlab_limit",
            Self::SafepointRequest => "safepoint_request",
            Self::Pending => "pending",
            Self::MultipleValueCount => "mv_count",
            Self::MultipleValueArea => "mv_area",
            Self::Handler => "handler",
            Self::Cleanup => "cleanup",
            Self::Catch => "catch",
        }
    }
}

/// A runtime entry point called by generated code.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeFunction {
    /// Allocation slow path.
    AllocateSlow,
    /// Safepoint slow path.
    SafepointSlow,
    /// Non-local exit unwinder.
    Unwind,
    /// Fixed or variadic builtin entry.
    Builtin,
    /// Runtime constant table lookup.
    ConstantTable,
    /// Construct a closure object.
    MakeClosure,
    /// Enter a catch handler.
    EnterCatch,
    /// Enter an unwind-protect handler.
    EnterUnwindProtect,
    /// Enter a progv handler.
    EnterProgv,
    /// Leave a catch handler.
    LeaveCatch,
    /// Leave an unwind-protect handler.
    LeaveUnwindProtect,
    /// Leave a progv handler.
    LeaveProgv,
}

/// A named runtime constant without exposing a raw string as the ABI selector.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ConstantName<'a>(&'a str);

impl<'a> ConstantName<'a> {
    /// Creates a runtime constant name borrowed from the caller.
    #[must_use]
    pub const fn new(name: &'a str) -> Self {
        Self(name)
    }

    /// Returns the underlying legacy identifier.
    #[must_use]
    pub const fn as_str(self) -> &'a str {
        self.0
    }
}

impl RegisterId {
    /// Returns the stable numeric identifier used by stack maps.
    #[must_use]
    pub const fn id(self) -> u16 {
        self as u16
    }
}

/// Runtime values and entry points needed by code generation.
pub trait RuntimeAbi {
    /// Returns the address of a typed builtin or a precise lookup error.
    ///
    /// # Errors
    /// Returns [`AbiError::MissingBuiltin`] when the identifier is not registered.
    fn builtin_address(&self, identifier: ncl_object::BuiltinIdentifier) -> Result<u64, AbiError>;
    /// Returns the `ThreadContext` field offset used by a runtime operation.
    ///
    /// # Errors
    /// Returns [`AbiError::UnsupportedContextField`] when the field is unavailable.
    fn field_offset(&self, field: ContextField) -> Result<i32, AbiError>;
    /// Returns a runtime function address or a precise lookup error.
    ///
    /// # Errors
    /// Returns [`AbiError::UnsupportedRuntimeFunction`] when the function is unavailable.
    fn runtime_address(&self, function: RuntimeFunction) -> Result<u64, AbiError>;
    /// Returns a runtime constant encoded as a machine word.
    fn constant_word(&self, _name: &str) -> Option<i64> {
        None
    }
    /// Returns a runtime constant through a typed selector.
    fn constant_word_named(&self, name: ConstantName<'_>) -> Option<i64> {
        self.constant_word(name.as_str())
    }
}

/// Errors raised while resolving a runtime ABI selector.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AbiError {
    /// The builtin is not registered in the runtime registry.
    MissingBuiltin(ncl_object::BuiltinIdentifier),
    /// The runtime does not expose this context field.
    UnsupportedContextField(ContextField),
    /// The runtime does not expose this entry point.
    UnsupportedRuntimeFunction(RuntimeFunction),
}

/// Converts an IR builtin spelling into the object-layer registry key.
#[must_use]
pub fn common_lisp_builtin(name: &str) -> ncl_object::BuiltinIdentifier {
    ncl_object::BuiltinIdentifier::new(
        ncl_object::BuiltinPackage::CommonLisp,
        ncl_object::BuiltinName::new(Box::leak(name.to_owned().into_boxed_str())),
    )
}

impl core::fmt::Display for AbiError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::MissingBuiltin(identifier) => write!(
                f,
                "builtin address is unavailable: {}::{}",
                identifier.package.as_str(),
                identifier.name.as_str()
            ),
            Self::UnsupportedContextField(field) => {
                write!(f, "context offset is unavailable: {field:?}")
            }
            Self::UnsupportedRuntimeFunction(function) => {
                write!(f, "runtime address is unavailable: {function:?}")
            }
        }
    }
}

impl std::error::Error for AbiError {}

/// Supplies stable addresses for generated builtin calls without exposing target details.
#[allow(dead_code)]
pub trait BuiltinAddressProvider {
    /// Return the process address for a typed builtin identifier.
    fn builtin_address(&self, identifier: ncl_object::BuiltinIdentifier) -> Option<u64>;
}

/// Small deterministic address table suitable for embedders and tests.
#[allow(dead_code)]
#[derive(Clone, Debug, Default)]
pub struct BuiltinAddressTable(Vec<(ncl_object::BuiltinIdentifier, u64)>);
#[allow(dead_code)]
impl BuiltinAddressTable {
    /// Create an empty address table.
    #[must_use]
    pub const fn new() -> Self {
        Self(Vec::new())
    }
    /// Insert or replace a builtin address.
    pub fn insert(&mut self, identifier: ncl_object::BuiltinIdentifier, address: u64) {
        if let Some(entry) = self.0.iter_mut().find(|entry| entry.0 == identifier) {
            entry.1 = address;
        } else {
            self.0.push((identifier, address));
        }
    }
}
impl BuiltinAddressProvider for BuiltinAddressTable {
    fn builtin_address(&self, identifier: ncl_object::BuiltinIdentifier) -> Option<u64> {
        self.0
            .iter()
            .find(|entry| entry.0 == identifier)
            .map(|entry| entry.1)
    }
}

#[cfg(test)]
mod builtin_address_tests {
    use super::{BuiltinAddressProvider, BuiltinAddressTable, ContextField};
    use ncl_object::{BuiltinIdentifier, BuiltinName, BuiltinPackage};

    #[test]
    fn address_table_replaces_and_reads_entries() {
        let mut table = BuiltinAddressTable::new();
        let identifier = BuiltinIdentifier::new(BuiltinPackage::NclTest, BuiltinName::new("ADD"));
        table.insert(identifier, 0x10);
        assert_eq!(table.builtin_address(identifier), Some(0x10));
        table.insert(identifier, 0x20);
        assert_eq!(table.builtin_address(identifier), Some(0x20));
    }

    #[test]
    fn context_fields_keep_the_stable_legacy_identifiers() {
        assert_eq!(ContextField::TlabBump.identifier(), "tlab_bump");
        assert_eq!(ContextField::MultipleValueCount.identifier(), "mv_count");
    }
}

/// Default x86-64 ABI policy used by tests and embedders.
#[derive(Clone, Copy, Debug, Default)]
pub struct X86_64Abi;

impl RuntimeAbi for X86_64Abi {
    fn builtin_address(&self, identifier: ncl_object::BuiltinIdentifier) -> Result<u64, AbiError> {
        Err(AbiError::MissingBuiltin(identifier))
    }
    fn field_offset(&self, field: ContextField) -> Result<i32, AbiError> {
        Err(AbiError::UnsupportedContextField(field))
    }
    fn runtime_address(&self, function: RuntimeFunction) -> Result<u64, AbiError> {
        Err(AbiError::UnsupportedRuntimeFunction(function))
    }

    fn field_offset(&self, field: ContextField) -> Option<i32> {
        let layout = ncl_sys::thread_layout();
        let offset = match field {
            ContextField::MultipleValueArea => layout.mv,
            _ => return None,
        };
        i32::try_from(offset).ok()
    }
}

/// `AArch64` ABI policy used by native execution tests and embedders.
#[derive(Clone, Copy, Debug, Default)]
pub struct Aarch64Abi;

impl RuntimeAbi for Aarch64Abi {
    fn builtin_address(&self, identifier: ncl_object::BuiltinIdentifier) -> Result<u64, AbiError> {
        Err(AbiError::MissingBuiltin(identifier))
    }
    fn field_offset(&self, field: ContextField) -> Result<i32, AbiError> {
        Err(AbiError::UnsupportedContextField(field))
    }
    fn runtime_address(&self, function: RuntimeFunction) -> Result<u64, AbiError> {
        Err(AbiError::UnsupportedRuntimeFunction(function))
    }

    fn field_offset(&self, field: ContextField) -> Option<i32> {
        let layout = ncl_sys::thread_layout();
        let offset = match field {
            ContextField::MultipleValueArea => layout.mv,
            _ => return None,
        };
        i32::try_from(offset).ok()
    }
}
