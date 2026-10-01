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

#[cfg(test)]
mod tests {
    use super::SymbolKind;

    #[test]
    fn registration_kinds_report_their_required_bindings() {
        let cases = [
            (SymbolKind::Class, false, true, false, false),
            (SymbolKind::ClassAndFunction, true, true, false, false),
            (SymbolKind::Function, true, false, false, false),
            (SymbolKind::Macro, false, false, true, false),
            (SymbolKind::MacroAndClass, false, true, true, false),
            (
                SymbolKind::SpecialOperatorAndClass,
                false,
                true,
                false,
                false,
            ),
            (SymbolKind::Type, false, false, false, false),
            (SymbolKind::Variable, false, false, false, true),
            (SymbolKind::VariableAndFunction, true, false, false, true),
            (SymbolKind::Other, false, false, false, false),
            (SymbolKind::Constant, false, false, false, false),
        ];
        for (kind, function, class, macro_kind, variable) in cases {
            assert_eq!(kind.defines_function(), function, "{kind:?}");
            assert_eq!(kind.defines_class(), class, "{kind:?}");
            assert_eq!(kind.is_macro(), macro_kind, "{kind:?}");
            assert_eq!(kind.is_variable(), variable, "{kind:?}");
        }
    }
}
