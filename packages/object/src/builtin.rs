//! Calling convention metadata and builtin expansion support.

use ncl_sys::Word;

/// Calling convention status.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum NclStatus {
    Ok = 0,
    Error = 1,
}

/// Registered builtin function descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Builtin {
    pub arity: u8,
    pub direct: bool,
    pub lambda_list: &'static str,
}

/// A multiple-value result area passed to a variadic builtin.
#[repr(C)]
#[derive(Debug, Default)]
pub struct MultipleValues {
    values: Vec<Word>,
}

impl MultipleValues {
    /// Create an empty result area.
    #[must_use]
    pub const fn new() -> Self {
        Self { values: Vec::new() }
    }
    /// Replace the result values.
    pub fn set(&mut self, values: &[Word]) {
        self.values = values.to_vec();
    }
    /// Borrow the result values.
    #[must_use]
    pub fn as_slice(&self) -> &[Word] {
        &self.values
    }
}

/// A function object identity used by the registration API.
/// A function object identity used by the registration API.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FunctionObject(Word);
impl From<Word> for FunctionObject {
    fn from(value: Word) -> Self {
        Self(value)
    }
}
impl From<FunctionObject> for Word {
    fn from(value: FunctionObject) -> Self {
        value.0
    }
}
impl FunctionObject {
    #[must_use]
    pub const fn as_word(self) -> Word {
        self.0
    }
}
/// Function registration callback.
pub type RegisterFn = fn(&super::Runtime);

/// Declare fixed-arity builtin metadata.
#[macro_export]
macro_rules! builtin {
    ($name:ident, $arity:expr) => {
        pub const $name: $crate::Builtin = $crate::Builtin { arity: $arity, direct: true, lambda_list: "" };
    };
    ($name:ident, $arity:expr, $lambda_list:expr) => {
        pub const $name: $crate::Builtin = $crate::Builtin { arity: $arity, direct: true, lambda_list: $lambda_list };
    };
    ($name:ident, 0, $lambda_list:expr, $direct:ident, $variadic:ident) => {
        $crate::builtin!(@descriptor $name, 0, $lambda_list);
        pub extern "C" fn $direct(_ctx: *mut $crate::ThreadContext) -> $crate::Word { $crate::Word::UNBOUND }
        $crate::builtin!(@variadic $variadic);
    };
    ($name:ident, 1, $lambda_list:expr, $direct:ident, $variadic:ident) => {
        $crate::builtin!(@descriptor $name, 1, $lambda_list);
        pub extern "C" fn $direct(_ctx: *mut $crate::ThreadContext, _a0: $crate::Word) -> $crate::Word { $crate::Word::UNBOUND }
        $crate::builtin!(@variadic $variadic);
    };
    ($name:ident, 2, $lambda_list:expr, $direct:ident, $variadic:ident) => {
        $crate::builtin!(@descriptor $name, 2, $lambda_list);
        pub extern "C" fn $direct(_ctx: *mut $crate::ThreadContext, _a0: $crate::Word, _a1: $crate::Word) -> $crate::Word { $crate::Word::UNBOUND }
        $crate::builtin!(@variadic $variadic);
    };
    ($name:ident, 3, $lambda_list:expr, $direct:ident, $variadic:ident) => {
        $crate::builtin!(@descriptor $name, 3, $lambda_list);
        pub extern "C" fn $direct(_ctx: *mut $crate::ThreadContext, _a0: $crate::Word, _a1: $crate::Word, _a2: $crate::Word) -> $crate::Word { $crate::Word::UNBOUND }
        $crate::builtin!(@variadic $variadic);
    };
    ($name:ident, 4, $lambda_list:expr, $direct:ident, $variadic:ident) => {
        $crate::builtin!(@descriptor $name, 4, $lambda_list);
        pub extern "C" fn $direct(_ctx: *mut $crate::ThreadContext, _a0: $crate::Word, _a1: $crate::Word, _a2: $crate::Word, _a3: $crate::Word) -> $crate::Word { $crate::Word::UNBOUND }
        $crate::builtin!(@variadic $variadic);
    };
    (@descriptor $name:ident, $arity:expr, $lambda_list:expr) => {
        pub const $name: $crate::Builtin = $crate::Builtin { arity: $arity, direct: true, lambda_list: $lambda_list };
    };
    (@variadic $name:ident) => {
        pub extern "C" fn $name(_ctx: *mut $crate::ThreadContext, _argc: usize, _args: *const $crate::Word, _values: *mut $crate::MultipleValues) -> $crate::NclStatus { $crate::NclStatus::Error }
    };
}
