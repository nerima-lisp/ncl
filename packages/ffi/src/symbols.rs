//! Static table of the symbols owned by `ncl-ffi`.
//!
//! Every row mirrors `conformance/ownership/symbols.tsv` for crate `ncl-ffi`,
//! phase 1. The rows are split by package across the submodules below; the
//! `symbols` accessor yields them in ownership-table order.

mod sb_alien;
mod sb_ext;
mod sb_sys;

/// Registration kind of an owned symbol.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SymbolKind {
    /// A CLOS class, registered with `define_class`.
    Class,
    /// A CLOS class that is also a function.
    ClassAndFunction,
    /// A function, registered with `define_function`.
    Function,
    /// A macro, interned with its macro bit set.
    Macro,
    /// A macro that is also a class.
    MacroAndClass,
    /// A symbol with no dedicated registry, which only needs to be interned.
    Other,
    /// A special operator that is also a class.
    SpecialOperatorAndClass,
    /// A type specifier name, interned only.
    Type,
    /// A variable, interned with its special bit set.
    Variable,
    /// A variable that is also a function.
    VariableAndFunction,
}

impl SymbolKind {
    /// Whether the kind requires a registered function object.
    #[must_use]
    pub const fn defines_function(self) -> bool {
        matches!(
            self,
            Self::Function | Self::ClassAndFunction | Self::VariableAndFunction
        )
    }

    /// Whether the kind requires a registered class object.
    #[must_use]
    pub const fn defines_class(self) -> bool {
        matches!(
            self,
            Self::Class
                | Self::ClassAndFunction
                | Self::MacroAndClass
                | Self::SpecialOperatorAndClass
        )
    }

    /// Whether the kind requires the symbol's macro bit.
    #[must_use]
    pub const fn is_macro(self) -> bool {
        matches!(self, Self::Macro | Self::MacroAndClass)
    }

    /// Whether the kind requires the symbol's special bit.
    #[must_use]
    pub const fn is_variable(self) -> bool {
        matches!(self, Self::Variable | Self::VariableAndFunction)
    }
}

/// One owned symbol.
pub struct SymbolRow {
    /// Owning package name.
    pub package: &'static str,
    /// Symbol name.
    pub name: &'static str,
    /// Registration kind.
    pub kind: SymbolKind,
}

pub use sb_alien::SB_ALIEN;
pub use sb_ext::SB_EXT;
pub use sb_sys::SB_SYS;

/// Every Phase-1 symbol owned by this crate, in ownership-table order.
pub fn symbols() -> impl Iterator<Item = &'static SymbolRow> {
    SB_ALIEN.iter().chain(SB_EXT).chain(SB_SYS)
}
