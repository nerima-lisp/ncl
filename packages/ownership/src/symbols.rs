//! Canonical symbol metadata used by registration tables.

/// Registration kind for a symbol owned by a crate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SymbolKind {
    /// A class.
    Class,
    /// A class and function.
    ClassAndFunction,
    /// A function.
    Function,
    /// A macro.
    Macro,
    /// A macro and class.
    MacroAndClass,
    /// A special operator and class.
    SpecialOperatorAndClass,
    /// A type name.
    Type,
    /// A variable.
    Variable,
    /// A variable and function.
    VariableAndFunction,
    /// A symbol that only needs interning.
    Other,
    /// A constant.
    Constant,
}

impl SymbolKind {
    /// Whether this kind requires a function registration.
    #[must_use]
    pub const fn defines_function(self) -> bool {
        matches!(
            self,
            Self::Function | Self::ClassAndFunction | Self::VariableAndFunction
        )
    }

    /// Whether this kind requires a class registration.
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

    /// Whether this kind requires the macro bit.
    #[must_use]
    pub const fn is_macro(self) -> bool {
        matches!(self, Self::Macro | Self::MacroAndClass)
    }

    /// Whether this kind requires the special bit.
    #[must_use]
    pub const fn is_variable(self) -> bool {
        matches!(self, Self::Variable | Self::VariableAndFunction)
    }
}

/// One static registration-table row.
#[derive(Debug)]
pub struct SymbolRow {
    /// Package containing the symbol.
    pub package: &'static str,
    /// Symbol name.
    pub name: &'static str,
    /// Registration kind.
    pub kind: SymbolKind,
}
