//! Typed identities for compilation units, forms, and HIR arena entries.

use std::error::Error;
use std::fmt::{Display, Formatter};

/// An error returned when an identity index cannot fit in `u32`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdError {
    /// The requested index exceeds the `u32` identity index space.
    IndexOverflow {
        /// The original index that did not fit.
        index: usize,
    },
}

impl Display for IdError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IndexOverflow { index } => {
                write!(formatter, "arena index {index} exceeds the u32 range")
            }
        }
    }
}

impl Error for IdError {}

/// The identity of one compilation unit.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CompilationUnitId(u32);

impl CompilationUnitId {
    /// Creates an identity from an index already represented as `u32`.
    #[must_use]
    pub const fn new(index: u32) -> Self {
        Self(index)
    }

    /// Creates an identity after checking the index width.
    ///
    /// # Errors
    ///
    /// Returns `IdError::IndexOverflow` when `index` does not fit in `u32`.
    pub fn try_from_index(index: usize) -> Result<Self, IdError> {
        checked_index(index).map(Self)
    }

    /// Returns this compilation unit's stable index.
    #[must_use]
    pub const fn index(self) -> u32 {
        self.0
    }
}

/// The identity of a top-level form within a compilation unit.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FormId {
    compilation_unit: CompilationUnitId,
    index: u32,
}

impl FormId {
    /// Creates a form identity from an index already represented as `u32`.
    #[must_use]
    pub const fn new(compilation_unit: CompilationUnitId, index: u32) -> Self {
        Self {
            compilation_unit,
            index,
        }
    }

    /// Creates a form identity after checking the index width.
    ///
    /// # Errors
    ///
    /// Returns `IdError::IndexOverflow` when `index` does not fit in `u32`.
    pub fn try_from_index(
        compilation_unit: CompilationUnitId,
        index: usize,
    ) -> Result<Self, IdError> {
        checked_index(index).map(|index| Self::new(compilation_unit, index))
    }

    /// Returns the owning compilation unit.
    #[must_use]
    pub const fn compilation_unit(self) -> CompilationUnitId {
        self.compilation_unit
    }

    /// Returns this form's index within its compilation unit.
    #[must_use]
    pub const fn index(self) -> u32 {
        self.index
    }
}

/// A typed index that carries the identity of its owning arena.
pub trait ArenaIndex: Copy + std::fmt::Debug + Eq {
    /// The identity scope shared by this index and its arena.
    type Owner: Copy + std::fmt::Debug + Eq;

    /// Returns the arena identity carried by this index.
    #[must_use]
    fn owner(self) -> Self::Owner;

    /// Returns this index's position in the arena.
    #[must_use]
    fn index(self) -> u32;

    /// Creates an index after checking that its position fits in `u32`.
    ///
    /// # Errors
    ///
    /// Returns `IdError::IndexOverflow` when `index` does not fit in `u32`.
    fn try_from_index(owner: Self::Owner, index: usize) -> Result<Self, IdError>;
}

impl ArenaIndex for FormId {
    type Owner = CompilationUnitId;

    fn owner(self) -> Self::Owner {
        self.compilation_unit
    }

    fn index(self) -> u32 {
        self.index
    }

    fn try_from_index(owner: Self::Owner, index: usize) -> Result<Self, IdError> {
        Self::try_from_index(owner, index)
    }
}

macro_rules! define_form_id {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        pub struct $name {
            form: FormId,
            index: u32,
        }

        impl $name {
            /// Creates this identity after checking the index width.
            ///
            /// # Errors
            ///
            /// Returns [`IdError::IndexOverflow`] when `index` does not fit in `u32`.
            pub fn try_from_index(form: FormId, index: usize) -> Result<Self, IdError> {
                checked_index(index).map(|index| Self { form, index })
            }

            /// Returns the owning top-level form.
            #[must_use]
            pub const fn form(self) -> FormId {
                self.form
            }

            /// Returns this identity's position in its form arena.
            #[must_use]
            pub const fn index(self) -> u32 {
                self.index
            }
        }

        impl ArenaIndex for $name {
            type Owner = FormId;

            fn owner(self) -> Self::Owner {
                self.form
            }

            fn index(self) -> u32 {
                self.index
            }

            fn try_from_index(owner: Self::Owner, index: usize) -> Result<Self, IdError> {
                Self::try_from_index(owner, index)
            }
        }
    };
}

macro_rules! define_unit_id {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        pub struct $name {
            compilation_unit: CompilationUnitId,
            index: u32,
        }

        impl $name {
            /// Creates this identity after checking the index width.
            ///
            /// # Errors
            ///
            /// Returns [`IdError::IndexOverflow`] when `index` does not fit in `u32`.
            pub fn try_from_index(
                compilation_unit: CompilationUnitId,
                index: usize,
            ) -> Result<Self, IdError> {
                checked_index(index).map(|index| Self {
                    compilation_unit,
                    index,
                })
            }

            /// Returns the owning compilation unit.
            #[must_use]
            pub const fn compilation_unit(self) -> CompilationUnitId {
                self.compilation_unit
            }

            /// Returns this identity's position in its compilation-unit arena.
            #[must_use]
            pub const fn index(self) -> u32 {
                self.index
            }
        }

        impl ArenaIndex for $name {
            type Owner = CompilationUnitId;

            fn owner(self) -> Self::Owner {
                self.compilation_unit
            }

            fn index(self) -> u32 {
                self.index
            }

            fn try_from_index(owner: Self::Owner, index: usize) -> Result<Self, IdError> {
                Self::try_from_index(owner, index)
            }
        }
    };
}

define_form_id!(NodeId, "Identity of a HIR node within one top-level form.");

define_form_id!(
    TypeId,
    "Identity of a HIR type node within one top-level form."
);

define_form_id!(
    BindingId,
    "Identity of a lexical binding within one top-level form."
);

define_form_id!(
    BlockId,
    "Identity of a basic block within one top-level form."
);

define_form_id!(
    ValueId,
    "Identity of an SSA value within one top-level form."
);

define_form_id!(
    SourceOriginId,
    "Identity of a source origin within one top-level form."
);

define_form_id!(
    TypeSpecConstId,
    "Identity of a type-specifier constant within one top-level form."
);

define_form_id!(
    CleanupId,
    "Identity of cleanup metadata within one top-level form."
);

define_unit_id!(
    SymbolId,
    "Identity of a source symbol in one compilation unit, independent of spelling."
);

fn checked_index(index: usize) -> Result<u32, IdError> {
    u32::try_from(index).map_err(|_| IdError::IndexOverflow { index })
}
