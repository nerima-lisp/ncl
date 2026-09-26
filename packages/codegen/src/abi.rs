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
    /// Encodes a fixnum as a machine word.
    fn encode_fixnum(&self, value: i64) -> i64;
    /// Encodes a character as a machine word.
    fn encode_character(&self, value: u32) -> i64;
    /// Returns the address of a runtime symbol.
    fn builtin_address(&self, name: &str) -> Option<u64>;
    /// Returns the `ThreadContext` field offset used by a runtime operation.
    fn context_offset(&self, field: &str) -> Option<i32>;
    /// Returns a typed `ThreadContext` field offset in bytes.
    fn field_offset(&self, field: ContextField) -> Option<i32> {
        self.context_offset(match field {
            ContextField::TlabBump => "tlab_bump",
            ContextField::TlabLimit => "tlab_limit",
            ContextField::SafepointRequest => "safepoint_request",
            ContextField::Pending => "pending",
            ContextField::MultipleValueCount => "mv_count",
            ContextField::MultipleValueArea => "mv_area",
            ContextField::Handler => "handler",
            ContextField::Cleanup => "cleanup",
            ContextField::Catch => "catch",
        })
    }
    /// Returns a runtime function address.
    fn runtime_address(&self, _function: RuntimeFunction, _name: Option<&str>) -> Option<u64> {
        None
    }
    /// Returns a runtime constant encoded as a machine word.
    fn constant_word(&self, _name: &str) -> Option<i64> {
        None
    }
}

/// Supplies stable addresses for generated builtin calls without exposing target details.
#[allow(dead_code)]
pub trait BuiltinAddressProvider {
    /// Return the process address for a named builtin.
    fn builtin_address(&self, name: &str) -> Option<u64>;
}

/// Small deterministic address table suitable for embedders and tests.
#[allow(dead_code)]
#[derive(Clone, Debug, Default)]
pub struct BuiltinAddressTable(Vec<(String, u64)>);
#[allow(dead_code)]
impl BuiltinAddressTable {
    /// Create an empty address table.
    #[must_use]
    pub const fn new() -> Self {
        Self(Vec::new())
    }
    /// Insert or replace a builtin address.
    pub fn insert(&mut self, name: impl Into<String>, address: u64) {
        let name = name.into();
        if let Some(entry) = self.0.iter_mut().find(|entry| entry.0 == name) {
            entry.1 = address;
        } else {
            self.0.push((name, address));
        }
    }
}
impl BuiltinAddressProvider for BuiltinAddressTable {
    fn builtin_address(&self, name: &str) -> Option<u64> {
        self.0
            .iter()
            .find(|entry| entry.0 == name)
            .map(|entry| entry.1)
    }
}

#[cfg(test)]
mod builtin_address_tests {
    use super::{BuiltinAddressProvider, BuiltinAddressTable};

    #[test]
    fn address_table_replaces_and_reads_entries() {
        let mut table = BuiltinAddressTable::new();
        table.insert("NCL-TEST::ADD", 0x10);
        assert_eq!(table.builtin_address("NCL-TEST::ADD"), Some(0x10));
        table.insert("NCL-TEST::ADD", 0x20);
        assert_eq!(table.builtin_address("NCL-TEST::ADD"), Some(0x20));
    }
}

/// Default x86-64 ABI policy used by tests and embedders.
#[derive(Clone, Copy, Debug, Default)]
pub struct X86_64Abi;

impl RuntimeAbi for X86_64Abi {
    fn encode_fixnum(&self, value: i64) -> i64 {
        value << 3
    }
    fn encode_character(&self, value: u32) -> i64 {
        i64::from(value) << 8 | 0x0f
    }
    fn builtin_address(&self, _name: &str) -> Option<u64> {
        None
    }
    fn context_offset(&self, _field: &str) -> Option<i32> {
        None
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
    fn encode_fixnum(&self, value: i64) -> i64 {
        value << 3
    }

    fn encode_character(&self, value: u32) -> i64 {
        i64::from(value) << 8 | 0x0f
    }

    fn builtin_address(&self, _name: &str) -> Option<u64> {
        None
    }

    fn context_offset(&self, _field: &str) -> Option<i32> {
        None
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
